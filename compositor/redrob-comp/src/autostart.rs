//! XDG autostart for the Redrob session: run the entries in
//! `~/.config/autostart` and `/etc/xdg/autostart` that are meant for this
//! desktop (`OnlyShowIn` names Redrob or is absent, `NotShowIn` does not,
//! not `Hidden`, and `TryExec` resolves). A user entry hides the system entry
//! of the same file name, as the spec says.

use std::{collections::HashSet, path::PathBuf};

const DESKTOP: &str = "Redrob";

pub fn run() {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".config/autostart"));
    }
    dirs.push(PathBuf::from("/etc/xdg/autostart"));

    let mut seen = HashSet::new();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut files: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
        files.sort();
        for path in files {
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let Some(name) = path.file_name().map(|n| n.to_os_string()) else { continue };
            if !seen.insert(name) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            if let Some(cmd) = exec_for_us(&text) {
                tracing::info!(entry = %path.display(), cmd = %cmd.join(" "), "autostart");
                let args: Vec<&str> = cmd.iter().map(String::as_str).collect();
                crate::shell::spawn(&args);
            }
        }
    }
}

/// The command line of a `[Desktop Entry]` that should start here, if any.
fn exec_for_us(text: &str) -> Option<Vec<String>> {
    let mut in_entry = false;
    let (mut exec, mut only, mut not, mut hidden, mut try_exec) = (None, None, None, false, None);
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        match k.trim() {
            "Exec" => exec = Some(v.trim().to_string()),
            "OnlyShowIn" => only = Some(v.to_string()),
            "NotShowIn" => not = Some(v.to_string()),
            "Hidden" => hidden = v.trim().eq_ignore_ascii_case("true"),
            "TryExec" => try_exec = Some(v.trim().to_string()),
            _ => {}
        }
    }
    let names = |list: &Option<String>| list.as_deref().map(|l| l.split(';').any(|d| d.trim() == DESKTOP));
    if hidden || names(&not) == Some(true) || names(&only) == Some(false) {
        return None;
    }
    if let Some(t) = try_exec {
        if !on_path(&t) {
            return None;
        }
    }
    // Field codes (%u, %f, ...) have nothing to expand at login: drop them.
    let words: Vec<String> = split_exec(&exec?).into_iter().filter(|w| !(w.len() == 2 && w.starts_with('%'))).collect();
    (!words.is_empty()).then_some(words)
}

fn on_path(prog: &str) -> bool {
    if prog.contains('/') {
        return std::path::Path::new(prog).exists();
    }
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(prog).exists()))
        .unwrap_or(false)
}

/// Split an Exec value into words, honouring double quotes.
fn split_exec(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut quoted) = (Vec::new(), String::new(), false);
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => quoted = !quoted,
            '\\' if quoted => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            ' ' if !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}
