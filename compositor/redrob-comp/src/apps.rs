//! Installed applications: XDG desktop entries and their icons, for the app
//! finder, the window switcher and notification toasts.

use std::path::{Path, PathBuf};

use resvg::usvg;
use tiny_skia::Pixmap;

#[derive(Clone)]
pub struct App {
    pub id: String,
    pub name: String,
    /// Lower-cased text the finder matches against.
    pub haystack: String,
    pub exec: Vec<String>,
    pub icon: Option<String>,
    /// StartupWMClass, matched against a window's app id.
    pub wm_class: Option<String>,
}

fn data_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if let Some(h) = &home {
        v.push(h.join(".local/share"));
        v.push(h.join(".local/share/flatpak/exports/share"));
    }
    v.push(PathBuf::from("/var/lib/flatpak/exports/share"));
    let sys = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    v.extend(sys.split(':').filter(|s| !s.is_empty()).map(PathBuf::from));
    v
}

/// Every app that should be shown, sorted by name. A user entry hides the
/// system entry with the same file name.
pub fn load() -> Vec<App> {
    let mut seen = std::collections::HashSet::new();
    let mut apps = Vec::new();
    for dir in data_dirs() {
        let Ok(rd) = std::fs::read_dir(dir.join("applications")) else { continue };
        let mut files: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
        files.sort();
        for path in files {
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let id = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
            if !seen.insert(id.clone()) {
                continue;
            }
            if let Some(app) = parse(&path, id) {
                apps.push(app);
            }
        }
    }
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

fn parse(path: &Path, id: String) -> Option<App> {
    let text = std::fs::read_to_string(path).ok()?;
    let lang = std::env::var("LANG").unwrap_or_default();
    let ko = lang.starts_with("ko");
    let (mut name, mut name_ko, mut generic, mut exec, mut icon, mut kw, mut wm) = (None, None, None, None, None, String::new(), None);
    let mut in_entry = false;
    let mut hidden = false;
    let mut only: Option<String> = None;
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
        let v = v.trim().to_string();
        match k.trim() {
            "Name" => name = Some(v),
            "Name[ko]" => name_ko = Some(v),
            "GenericName" | "GenericName[ko]" | "Comment[ko]" => generic = Some(v),
            "Keywords" | "Keywords[ko]" => {
                kw.push(' ');
                kw.push_str(&v.replace(';', " "));
            }
            "Exec" => exec = Some(v),
            "Icon" => icon = Some(v),
            "StartupWMClass" => wm = Some(v),
            "Type" if v != "Application" => return None,
            "NoDisplay" | "Hidden" if v.eq_ignore_ascii_case("true") => hidden = true,
            "OnlyShowIn" => only = Some(v),
            _ => {}
        }
    }
    if hidden {
        return None;
    }
    if let Some(o) = only {
        if !o.split(';').any(|d| d == "Redrob" || d == "KDE") {
            return None;
        }
    }
    let base = name?;
    let shown = if ko { name_ko.clone().unwrap_or_else(|| base.clone()) } else { base.clone() };
    let exec: Vec<String> = split_exec(&exec?).into_iter().filter(|w| !(w.len() == 2 && w.starts_with('%'))).collect();
    if exec.is_empty() {
        return None;
    }
    let haystack = format!(
        "{} {} {} {} {} {}",
        shown,
        base,
        name_ko.unwrap_or_default(),
        generic.unwrap_or_default(),
        kw,
        exec.first().map(|e| e.rsplit('/').next().unwrap_or(e).to_string()).unwrap_or_default()
    )
    .to_lowercase();
    Some(App { id, name: shown, haystack, exec, icon, wm_class: wm })
}

/// Split an Exec value into words, honouring double quotes.
pub fn split_exec(s: &str) -> Vec<String> {
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

/// Finder ranking: name prefix, word prefix, then anywhere. `None` = no match.
pub fn score(app: &App, q: &str) -> Option<u32> {
    if q.is_empty() {
        return Some(3);
    }
    let name = app.name.to_lowercase();
    if name.starts_with(q) {
        Some(0)
    } else if name.split_whitespace().any(|w| w.starts_with(q)) {
        Some(1)
    } else if app.haystack.contains(q) {
        Some(2)
    } else {
        None
    }
}

/// The desktop entry for a window's app id (`org.kde.konsole`, `firefox`).
pub fn for_app_id<'a>(apps: &'a [App], app_id: &str) -> Option<&'a App> {
    let low = app_id.to_lowercase();
    apps.iter()
        .find(|a| a.id.to_lowercase() == low)
        .or_else(|| apps.iter().find(|a| a.wm_class.as_deref().is_some_and(|w| w.to_lowercase() == low)))
        .or_else(|| apps.iter().find(|a| a.id.to_lowercase().ends_with(&format!(".{low}"))))
        .or_else(|| apps.iter().find(|a| a.exec.first().is_some_and(|e| e.rsplit('/').next() == Some(&low))))
}

/// An icon name or path rendered to `px` square, from the hicolor and Breeze
/// themes or /usr/share/pixmaps. None when nothing usable is found.
pub fn icon(name: &str, px: u32) -> Option<Pixmap> {
    let path = if name.starts_with('/') { Some(PathBuf::from(name)) } else { find_icon(name, px) }?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default();
    let img = match ext {
        "svg" | "svgz" => {
            let data = std::fs::read(&path).ok()?;
            let tree = usvg::Tree::from_data(&data, &usvg::Options::default()).ok()?;
            let s = tree.size();
            let scale = px as f32 / s.width().max(s.height());
            let mut out = Pixmap::new(px, px)?;
            let dx = (px as f32 - s.width() * scale) / 2.0;
            let dy = (px as f32 - s.height() * scale) / 2.0;
            resvg::render(&tree, tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, dx, dy), &mut out.as_mut());
            out
        }
        "png" => {
            let src = Pixmap::load_png(&path).ok()?;
            let mut out = Pixmap::new(px, px)?;
            let s = px as f32 / src.width().max(src.height()) as f32;
            let pp = tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() };
            out.draw_pixmap(0, 0, src.as_ref(), &pp, tiny_skia::Transform::from_scale(s, s), None);
            out
        }
        _ => return None,
    };
    Some(img)
}

fn find_icon(name: &str, px: u32) -> Option<PathBuf> {
    let mut sizes: Vec<&str> = vec!["scalable", "256x256", "128x128", "96x96", "64x64", "48x48", "32x32"];
    if px <= 32 {
        sizes.rotate_left(5);
    }
    for dir in data_dirs() {
        let base = dir.join("icons");
        for theme in ["hicolor", "breeze"] {
            for size in &sizes {
                for ext in ["svg", "png"] {
                    let p = base.join(theme).join(size).join("apps").join(format!("{name}.{ext}"));
                    if p.exists() {
                        return Some(p);
                    }
                }
            }
            for size in ["48", "64", "32"] {
                let p = base.join(theme).join("apps").join(size).join(format!("{name}.svg"));
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }
    for ext in ["svg", "png"] {
        let p = PathBuf::from(format!("/usr/share/pixmaps/{name}.{ext}"));
        if p.exists() {
            return Some(p);
        }
    }
    None
}
