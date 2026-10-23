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
    ScreenshotWindow,
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
            "screenshot-window" => Act::ScreenshotWindow,
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
    /// `resolution = 1920x1080` and `scale = 1.25`, when set.
    pub resolution: Option<(i32, i32)>,
    pub scale: Option<f64>,
    /// `language = ko|en`; empty follows the system language.
    pub language: String,
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
        let mut resolution = None;
        let mut scale = None;
        let mut language = String::new();
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
            } else if k.eq_ignore_ascii_case("resolution") {
                resolution = v.split_once('x').and_then(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?)));
            } else if k.eq_ignore_ascii_case("language") {
                language = v.to_ascii_lowercase();
            } else if k.eq_ignore_ascii_case("scale") {
                scale = v.parse::<f64>().ok().filter(|s| (0.5..=3.0).contains(s));
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
        Config { style, theme, shortcuts, resolution, scale, language }
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
            ("Ctrl+Up", Act::Expose),
            ("F3", Act::Expose),
            ("Ctrl+Super+Q", Act::Lock),
            ("Super+Shift+3", Act::Screenshot),
            ("Super+Shift+4", Act::ScreenshotRegion),
            ("Super+Shift+5", Act::ScreenshotRegion),
            ("Super+Shift+6", Act::ScreenshotWindow),
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
            ("Alt+Print", Act::ScreenshotWindow),
            ("Super+Left", Act::SnapLeft),
            ("Super+Right", Act::SnapRight),
            ("Super+I", Act::Settings),
            ("Ctrl+Alt+T", Act::Terminal),
        ],
    };
    set.iter().filter_map(|(c, a)| Some((Combo::parse(c)?, *a))).collect()
}

impl Act {
    /// The settings-file name of the action (inverse of `parse`).
    pub fn name(self) -> &'static str {
        match self {
            Act::Close => "close",
            Act::Cycle => "cycle",
            Act::Minimize => "minimize",
            Act::Expose => "expose",
            Act::Lock => "lock",
            Act::Screenshot => "screenshot",
            Act::ScreenshotRegion => "screenshot-region",
            Act::ScreenshotWindow => "screenshot-window",
            Act::Launcher => "launcher",
            Act::ShowDesktop => "show-desktop",
            Act::Maximize => "maximize",
            Act::Restore => "restore",
            Act::SnapLeft => "snap-left",
            Act::SnapRight => "snap-right",
            Act::Terminal => "terminal",
            Act::Files => "files",
            Act::Browser => "browser",
            Act::Settings => "settings",
            Act::ToggleTheme => "toggle-theme",
        }
    }

    /// What the settings screen calls it.
    pub fn label(self) -> &'static str {
        crate::i18n::tr(match self {
            Act::Close => "창 닫기",
            Act::Cycle => "창 전환",
            Act::Minimize => "창 최소화",
            Act::Expose => "모든 창 보기",
            Act::Lock => "화면 잠금",
            Act::Screenshot => "스크린샷 (전체)",
            Act::ScreenshotRegion => "스크린샷 (영역)",
            Act::ScreenshotWindow => "스크린샷 (창)",
            Act::Launcher => "앱 찾기",
            Act::ShowDesktop => "바탕화면 보기",
            Act::Maximize => "최대화",
            Act::Restore => "원래 크기",
            Act::SnapLeft => "왼쪽 반에 붙이기",
            Act::SnapRight => "오른쪽 반에 붙이기",
            Act::Terminal => "터미널 열기",
            Act::Files => "파일 열기",
            Act::Browser => "브라우저 열기",
            Act::Settings => "설정 열기",
            Act::ToggleTheme => "라이트/다크 전환",
        })
    }

    pub const ALL: [Act; 19] = [
        Act::Launcher, Act::Cycle, Act::Expose, Act::Close, Act::Minimize, Act::Maximize, Act::Restore,
        Act::SnapLeft, Act::SnapRight, Act::ShowDesktop, Act::Lock, Act::Screenshot, Act::ScreenshotRegion,
        Act::ScreenshotWindow,
        Act::Terminal, Act::Files, Act::Browser, Act::Settings, Act::ToggleTheme,
    ];
}

impl Combo {
    /// Settings-file spelling ("Super+Shift+4").
    pub fn spec(&self) -> String {
        let mut v: Vec<String> = Vec::new();
        if self.mods & CTRL != 0 {
            v.push("Ctrl".into());
        }
        if self.mods & ALT != 0 {
            v.push("Alt".into());
        }
        if self.mods & SHIFT != 0 {
            v.push("Shift".into());
        }
        if self.mods & SUPER != 0 {
            v.push("Super".into());
        }
        if !self.key.is_empty() {
            v.push(pretty_key(&self.key));
        }
        v.join("+")
    }

    /// How the settings screen shows it: Mac glyphs, or Windows words.
    pub fn display(&self, style: Style) -> String {
        match style {
            Style::Mac => {
                let mut s = String::new();
                if self.mods & CTRL != 0 {
                    s.push('⌃');
                }
                if self.mods & ALT != 0 {
                    s.push('⌥');
                }
                if self.mods & SHIFT != 0 {
                    s.push('⇧');
                }
                if self.mods & SUPER != 0 {
                    s.push('⌘');
                }
                s + &pretty_key(&self.key)
            }
            Style::Windows => self.spec().replace("Super", "Win"),
        }
    }
}

fn pretty_key(k: &str) -> String {
    match k {
        "" => String::new(),
        "space" => "Space".into(),
        "return" => "Enter".into(),
        "tab" => "Tab".into(),
        "comma" => ",".into(),
        "period" => ".".into(),
        "up" => "↑".into(),
        "down" => "↓".into(),
        "left" => "←".into(),
        "right" => "→".into(),
        "print" => "PrtSc".into(),
        "escape" => "Esc".into(),
        k if k.len() == 1 => k.to_uppercase(),
        k => {
            let mut c = k.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        }
    }
}

/// Rewrite one top-level key (`style`, `theme`) in the user's settings file.
pub fn set_value(key: &str, value: &str) {
    edit(|lines| {
        let mut done = false;
        for l in lines.iter_mut() {
            if l.trim_start().starts_with('[') {
                break;
            }
            let k = l.split('=').next().unwrap_or("").trim();
            if !l.trim_start().starts_with('#') && k.eq_ignore_ascii_case(key) {
                *l = format!("{key} = {value}");
                done = true;
            }
        }
        if !done {
            let at = lines.iter().position(|l| l.trim_start().starts_with('[')).unwrap_or(lines.len());
            lines.insert(at, format!("{key} = {value}"));
        }
    });
}

/// Bind `combo` to `act` in [shortcuts], dropping any other line for that
/// action or that combo. `act = None` writes "none" (unbinds the combo).
pub fn set_shortcut(combo: &Combo, act: Option<Act>, replaces: &[Combo]) {
    let spec = combo.spec();
    edit(|lines| {
        let start = lines.iter().position(|l| l.trim().eq_ignore_ascii_case("[shortcuts]"));
        let start = match start {
            Some(i) => i,
            None => {
                lines.push(String::new());
                lines.push("[shortcuts]".into());
                lines.len() - 1
            }
        };
        let end = lines.iter().skip(start + 1).position(|l| l.trim_start().starts_with('[')).map(|i| i + start + 1).unwrap_or(lines.len());
        let mut keep: Vec<String> = Vec::new();
        for l in &lines[start + 1..end] {
            let k = l.split('#').next().unwrap_or("").split('=').next().unwrap_or("").trim();
            let same = Combo::parse(k).is_some_and(|c| c == *combo);
            if !same {
                keep.push(l.clone());
            }
        }
        // The action's old default combos stop doing it.
        for r in replaces {
            if r != combo {
                keep.push(format!("{} = none", r.spec()));
            }
        }
        keep.push(format!("{spec} = {}", act.map(|a| a.name()).unwrap_or("none")));
        lines.splice(start + 1..end, keep);
    });
}

/// Back to the style's own shortcuts: empty the [shortcuts] section.
pub fn reset_shortcuts() {
    edit(|lines| {
        if let Some(start) = lines.iter().position(|l| l.trim().eq_ignore_ascii_case("[shortcuts]")) {
            let end = lines.iter().skip(start + 1).position(|l| l.trim_start().starts_with('[')).map(|i| i + start + 1).unwrap_or(lines.len());
            lines.drain(start + 1..end);
        }
    });
}

fn edit(f: impl FnOnce(&mut Vec<String>)) {
    let Some(path) = Config::user_path() else { return };
    let old = std::fs::read_to_string(&path)
        .or_else(|_| std::fs::read_to_string("/etc/redrob/desktop.conf"))
        .unwrap_or_default();
    let mut lines: Vec<String> = old.lines().map(String::from).collect();
    f(&mut lines);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&path, lines.join("\n") + "\n");
}

/// The style's built-in combos for `act` (shown, and replaced on rebind).
pub fn default_combos(style: Style, act: Act) -> Vec<Combo> {
    defaults(style).into_iter().filter(|(_, a)| *a == act).map(|(c, _)| c).collect()
}
