//! Desktop settings: style (mac-like or Windows-like layout and shortcuts),
//! light/dark theme, and shortcut overrides.
//!
//! One plain file, read at start and re-read whenever it changes on disk, so a
//! settings app (or a text editor) changes the running desktop:
//!
//! ```text
//! # ~/.config/redrob/desktop.conf   (falls back to /etc/redrob/desktop.conf)
//! style = mac          # mac | windows
//! theme = dark         # dark | light
//!
//! [shortcuts]          # added on top of the style's own set
//! Super+Return = terminal
//! Alt+F4 = none        # "none" removes a binding
//! ```
//!
//! Shortcut keys: modifiers `Super` (Command / Windows key), `Ctrl`, `Alt`,
//! `Shift`, joined with `+`, then one key name (`Q`, `Tab`, `Space`, `F4`,
//! `Left`, `Comma`, ...). A bare `Super` fires when the key is tapped alone.

use std::{path::PathBuf, time::SystemTime};

use crate::theme::Mode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// Top bar + floating dock; Command-key shortcuts.
    Mac,
    /// Bottom taskbar with a start button; Windows-key shortcuts.
    Windows,
}

/// What a shortcut does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Close,
    Cycle,
    Minimize,
    Expose,
    Lock,
    Screenshot,
    ScreenshotRegion,
    Launcher,
    ShowDesktop,
    Maximize,
    Restore,
    SnapLeft,
    SnapRight,
    Terminal,
    Files,
    Browser,
    Settings,
    ToggleTheme,
}

impl Act {
    fn parse(s: &str) -> Option<Option<Act>> {
        Some(Some(match s.trim().to_ascii_lowercase().as_str() {
            "none" => return Some(None),
            "close" => Act::Close,
            "cycle" | "switch" => Act::Cycle,
            "minimize" => Act::Minimize,
            "expose" | "mission-control" | "task-view" => Act::Expose,
            "lock" => Act::Lock,
            "screenshot" => Act::Screenshot,
            "screenshot-region" => Act::ScreenshotRegion,
            "launcher" | "search" | "start" => Act::Launcher,
            "show-desktop" => Act::ShowDesktop,
            "maximize" => Act::Maximize,
            "restore" => Act::Restore,
            "snap-left" => Act::SnapLeft,
            "snap-right" => Act::SnapRight,
            "terminal" => Act::Terminal,
            "files" => Act::Files,
            "browser" => Act::Browser,
            "settings" => Act::Settings,
            "toggle-theme" => Act::ToggleTheme,
            _ => return None,
        }))
    }
}

pub const SUPER: u8 = 1;
pub const CTRL: u8 = 2;
pub const ALT: u8 = 4;
pub const SHIFT: u8 = 8;

/// A key combination: modifier bits plus a lower-cased xkb key name
/// (`"q"`, `"tab"`, `"f4"`). `key == ""` is a bare modifier tap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Combo {
    pub mods: u8,
    pub key: String,
}

impl Combo {
    pub fn parse(s: &str) -> Option<Combo> {
        let mut mods = 0;
        let mut key = String::new();
        for part in s.split('+').map(str::trim).filter(|p| !p.is_empty()) {
            match part.to_ascii_lowercase().as_str() {
                "super" | "cmd" | "command" | "win" | "meta" | "logo" => mods |= SUPER,
                "ctrl" | "control" => mods |= CTRL,
                "alt" | "option" | "opt" => mods |= ALT,
                "shift" => mods |= SHIFT,
                k => key = key_alias(k).to_string(),
            }
        }
        (mods != 0 || !key.is_empty()).then_some(Combo { mods, key })
    }
}

/// Friendly names to xkb keysym names (lower-cased).
fn key_alias(k: &str) -> &str {
    match k {
        "enter" => "return",
        "esc" => "escape",
        "," => "comma",
        "." => "period",
        "del" => "delete",
        k => k,
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub style: Style,
    pub theme: Mode,
    pub shortcuts: Vec<(Combo, Act)>,
}

impl Config {
    pub fn user_path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("redrob").join("desktop.conf"))
    }

    fn paths() -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = Self::user_path().into_iter().collect();
        v.push(PathBuf::from("/etc/redrob/desktop.conf"));
        v
    }

    /// The first settings file that exists, or the built-in defaults.
    /// `REDROB_STYLE` / `REDROB_THEME` override the file (development).
    pub fn load() -> Config {
        let text = Self::paths().iter().find_map(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
        let mut style = Style::Mac;
        let mut theme = Mode::Dark;
        let mut extra: Vec<(Combo, Option<Act>)> = Vec::new();
        let mut in_shortcuts = false;
        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('[') {
                in_shortcuts = line.eq_ignore_ascii_case("[shortcuts]");
                continue;
            }
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            if in_shortcuts {
                match (Combo::parse(k), Act::parse(v)) {
                    (Some(c), Some(a)) => extra.push((c, a)),
                    _ => tracing::warn!(line = raw, "skipping shortcut"),
                }
            } else if k.eq_ignore_ascii_case("style") {
                style = parse_style(v).unwrap_or(style);
            } else if k.eq_ignore_ascii_case("theme") {
                theme = parse_theme(v).unwrap_or(theme);
            }
        }
        if let Some(s) = std::env::var("REDROB_STYLE").ok().and_then(|v| parse_style(&v)) {
            style = s;
        }
        if let Some(t) = std::env::var("REDROB_THEME").ok().and_then(|v| parse_theme(&v)) {
            theme = t;
        }
        let mut shortcuts = defaults(style);
        for (combo, act) in extra {
            shortcuts.retain(|(c, _)| *c != combo);
            if let Some(a) = act {
                shortcuts.push((combo, a));
            }
        }
        Config { style, theme, shortcuts }
    }

    /// Modification time of the file `load` would read, to notice edits.
    pub fn stamp() -> Option<(PathBuf, SystemTime)> {
        Self::paths().into_iter().find_map(|p| {
            let t = std::fs::metadata(&p).and_then(|m| m.modified()).ok()?;
            Some((p, t))
        })
    }

    /// Remember the theme the user switched to (bar button / shortcut) in the
    /// user's file, keeping everything else in it as it was.
    pub fn save_theme(mode: Mode) {
        let Some(path) = Self::user_path() else { return };
        let word = if mode == Mode::Light { "light" } else { "dark" };
        let old = std::fs::read_to_string(&path)
            .or_else(|_| std::fs::read_to_string("/etc/redrob/desktop.conf"))
            .unwrap_or_default();
        let mut done = false;
        let mut out: Vec<String> = Vec::new();
        for line in old.lines() {
            let key = line.split('=').next().unwrap_or("").trim();
            if !done && key.eq_ignore_ascii_case("theme") && !line.trim_start().starts_with('#') {
                out.push(format!("theme = {word}"));
                done = true;
            } else {
                out.push(line.to_string());
            }
        }
        if !done {
            // Before any [section], so it stays a top-level key.
            let at = out.iter().position(|l| l.trim_start().starts_with('[')).unwrap_or(out.len());
            out.insert(at, format!("theme = {word}"));
        }
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, out.join("\n") + "\n");
    }
}

fn parse_style(v: &str) -> Option<Style> {
    match v.to_ascii_lowercase().as_str() {
        "mac" | "macos" => Some(Style::Mac),
        "windows" | "win" | "win11" => Some(Style::Windows),
        _ => None,
    }
}

fn parse_theme(v: &str) -> Option<Mode> {
    match v.to_ascii_lowercase().as_str() {
        "light" => Some(Mode::Light),
        "dark" => Some(Mode::Dark),
        _ => None,
    }
}

/// Each style's shortcuts, as close to the original system as the shell has
/// features for. `Ctrl+Alt+T` (terminal) is the Linux habit, kept in both.
fn defaults(style: Style) -> Vec<(Combo, Act)> {
    let set: &[(&str, Act)] = match style {
        Style::Mac => &[
            ("Super+Q", Act::Close),
            ("Super+W", Act::Close),
            ("Super+Tab", Act::Cycle),
            ("Super+M", Act::Minimize),
            ("Super+Space", Act::Launcher),
            ("Ctrl+Super+F", Act::Maximize),
            ("Ctrl+Super+Left", Act::SnapLeft),
            ("Ctrl+Super+Right", Act::SnapRight),
            ("F11", Act::ShowDesktop),
            ("Super+F3", Act::ShowDesktop),
            ("Super+Alt+Space", Act::Files),
            ("Super+Comma", Act::Settings),
            ("Ctrl+Alt+T", Act::Terminal),
        ],
        Style::Windows => &[
            ("Alt+F4", Act::Close),
            ("Alt+Tab", Act::Cycle),
            ("Super", Act::Launcher),
            ("Super+S", Act::Launcher),
            ("Super+E", Act::Files),
            ("Super+D", Act::ShowDesktop),
            ("Super+Up", Act::Maximize),
            ("Super+Down", Act::Restore),
            ("Super+M", Act::Minimize),
            ("Super+Tab", Act::Expose),
            ("Super+L", Act::Lock),
            ("Print", Act::Screenshot),
            ("Super+Print", Act::Screenshot),
            ("Super+Shift+S", Act::ScreenshotRegion),
            ("Super+Left", Act::SnapLeft),
            ("Super+Right", Act::SnapRight),
            ("Super+I", Act::Settings),
            ("Ctrl+Alt+T", Act::Terminal),
        ],
    };
    set.iter().filter_map(|(c, a)| Some((Combo::parse(c)?, *a))).collect()
}
