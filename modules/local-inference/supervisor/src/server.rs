//! The llama.cpp server command the supervisor runs. `build_argv` is pure so
//! the loopback-only invariant and the alias/model wiring are unit-testable
//! without spawning anything.
use crate::config::Config;
use std::path::Path;

/// Build the argument vector for the llama.cpp OpenAI-compatible server.
///
/// Invariants enforced by `Config::validate` and reflected here:
///   * `--host` is always the configured loopback address, never a routable one.
///   * `--alias` matches the agent's provider `model` so the fallback resolves.
pub fn build_argv(cfg: &Config, model: &Path) -> Vec<String> {
    let mut args = vec![
        "--model".into(),
        model.display().to_string(),
        "--host".into(),
        cfg.listen_host.clone(),
        "--port".into(),
        cfg.listen_port.to_string(),
        "--ctx-size".into(),
        cfg.ctx_size.to_string(),
        "--alias".into(),
        cfg.model_alias.clone(),
    ];
    if cfg.threads > 0 {
        args.push("--threads".into());
        args.push(cfg.threads.to_string());
    }
    args.extend(cfg.extra_args.iter().cloned());
    args
}

/// Health endpoint the supervisor (and probes) poll once the server is up.
pub fn health_url(cfg: &Config) -> String {
    format!("http://{}:{}/health", cfg.listen_host, cfg.listen_port)
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn arg_after<'a>(argv: &'a [String], flag: &str) -> Option<&'a str> {
        argv.iter()
            .position(|a| a == flag)
            .and_then(|i| argv.get(i + 1))
            .map(|s| s.as_str())
    }

    #[test]
    fn argv_binds_loopback_and_names_model_alias() {
        let cfg = Config::default();
        let m = PathBuf::from("/data/models/router.gguf");
        let argv = build_argv(&cfg, &m);
        assert_eq!(arg_after(&argv, "--host"), Some("127.0.0.1"));
        assert_eq!(arg_after(&argv, "--port"), Some("8081"));
        assert_eq!(
            arg_after(&argv, "--model"),
            Some("/data/models/router.gguf")
        );
        assert_eq!(arg_after(&argv, "--alias"), Some("router"));
        assert_eq!(arg_after(&argv, "--ctx-size"), Some("8192"));
        assert_eq!(arg_after(&argv, "--threads"), Some("4"));
    }

    #[test]
    fn argv_never_contains_a_routable_host() {
        // Whatever the (validated) config, the host flag is the configured
        // loopback value and nothing in the vector opens a wildcard bind.
        let cfg = Config::default();
        let argv = build_argv(&cfg, Path::new("/m.gguf"));
        assert!(!argv.iter().any(|a| a == "0.0.0.0" || a == "::"));
        let host = arg_after(&argv, "--host").unwrap();
        assert!(host.parse::<std::net::IpAddr>().unwrap().is_loopback());
    }

    #[test]
    fn zero_threads_lets_server_decide() {
        let mut cfg = Config::default();
        cfg.threads = 0;
        let argv = build_argv(&cfg, Path::new("/m.gguf"));
        assert_eq!(arg_after(&argv, "--threads"), None);
    }

    #[test]
    fn extra_args_appended() {
        let mut cfg = Config::default();
        cfg.extra_args = vec!["--no-webui".into(), "--parallel".into(), "2".into()];
        let argv = build_argv(&cfg, Path::new("/m.gguf"));
        let tail = &argv[argv.len() - 3..];
        assert_eq!(tail, ["--no-webui", "--parallel", "2"]);
    }

    #[test]
    fn health_url_tracks_config() {
        let mut cfg = Config::default();
        cfg.listen_port = 9099;
        assert_eq!(health_url(&cfg), "http://127.0.0.1:9099/health");
    }
}
