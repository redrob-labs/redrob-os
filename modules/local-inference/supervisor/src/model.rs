//! Router-model discovery. The device holds its GGUF models under
//! `models_dir`, provisioned separately from OTA, so the file may be absent at
//! boot and appear later. Selection is a pure function of the directory
//! contents and the config, which keeps it unit-testable without a device.
use crate::config::Config;
use std::path::{Path, PathBuf};

/// Pick the router model to serve, or `None` when none is installed yet.
///
/// Precedence:
///   1. `config.model`, when set and the file exists.
///   2. `<models_dir>/router.gguf`, the canonical name/symlink.
///   3. The most recently modified `*.gguf` in `models_dir`, preferring a file
///      whose name contains `router` over a plain generation model.
pub fn select_model(cfg: &Config) -> Option<PathBuf> {
    // Configured file wins when present; otherwise fall through to discovery
    // (the file may not be provisioned yet) rather than serving nothing.
    if let Some(m) = &cfg.model
        && m.is_file()
    {
        return Some(m.clone());
    }
    let canonical = cfg.models_dir.join("router.gguf");
    if canonical.is_file() {
        return Some(canonical);
    }
    newest_gguf(&cfg.models_dir)
}

fn newest_gguf(dir: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<(bool, std::time::SystemTime, PathBuf)> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let p = e.path();
            if !p.is_file() {
                return None;
            }
            if p.extension().and_then(|x| x.to_str()) != Some("gguf") {
                return None;
            }
            let name = p.file_name()?.to_string_lossy().to_lowercase();
            let is_router = name.contains("router");
            let mtime = e.metadata().ok()?.modified().ok()?;
            Some((is_router, mtime, p))
        })
        .collect();
    // Router-named files win; within a class, newest mtime wins.
    candidates.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    candidates.pop().map(|(_, _, p)| p)
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Duration;

    fn touch(p: &Path) {
        fs::write(p, b"gguf").unwrap();
    }

    #[test]
    fn none_when_dir_absent_or_empty() {
        let mut c = Config::default();
        c.models_dir = PathBuf::from("/no/such/dir/xyz");
        assert_eq!(select_model(&c), None);
        let d = tempfile::tempdir().unwrap();
        c.models_dir = d.path().into();
        assert_eq!(select_model(&c), None);
    }

    #[test]
    fn explicit_model_wins_when_present() {
        let d = tempfile::tempdir().unwrap();
        let m = d.path().join("explicit.gguf");
        touch(&m);
        touch(&d.path().join("router.gguf"));
        let mut c = Config::default();
        c.models_dir = d.path().into();
        c.model = Some(m.clone());
        assert_eq!(select_model(&c), Some(m));
    }

    #[test]
    fn explicit_model_absent_falls_through() {
        let d = tempfile::tempdir().unwrap();
        touch(&d.path().join("router.gguf"));
        let mut c = Config::default();
        c.models_dir = d.path().into();
        c.model = Some(d.path().join("gone.gguf"));
        assert_eq!(select_model(&c), Some(d.path().join("router.gguf")));
    }

    #[test]
    fn canonical_router_gguf_preferred_over_other() {
        let d = tempfile::tempdir().unwrap();
        touch(&d.path().join("qwen2.5-0.5b.gguf"));
        touch(&d.path().join("router.gguf"));
        let mut c = Config::default();
        c.models_dir = d.path().into();
        assert_eq!(select_model(&c), Some(d.path().join("router.gguf")));
    }

    #[test]
    fn router_named_beats_newer_plain_model() {
        let d = tempfile::tempdir().unwrap();
        let router = d.path().join("my-router-1b.gguf");
        touch(&router);
        std::thread::sleep(Duration::from_millis(20));
        touch(&d.path().join("plain-generation.gguf")); // newer, but not a router
        let mut c = Config::default();
        c.models_dir = d.path().into();
        assert_eq!(select_model(&c), Some(router));
    }

    #[test]
    fn newest_plain_wins_when_no_router_named() {
        let d = tempfile::tempdir().unwrap();
        touch(&d.path().join("old.gguf"));
        std::thread::sleep(Duration::from_millis(20));
        let newer = d.path().join("new.gguf");
        touch(&newer);
        let mut c = Config::default();
        c.models_dir = d.path().into();
        assert_eq!(select_model(&c), Some(newer));
    }

    #[test]
    fn non_gguf_ignored() {
        let d = tempfile::tempdir().unwrap();
        touch(&d.path().join("notes.txt"));
        touch(&d.path().join("model.bin"));
        let mut c = Config::default();
        c.models_dir = d.path().into();
        assert_eq!(select_model(&c), None);
    }
}
