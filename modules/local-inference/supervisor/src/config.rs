//! Supervisor configuration (`/etc/redrob/local-inference.toml`, every field
//! overridable). Defaults match the device layout; L0 host runs override
//! `models_dir`, `server_bin` and the listen address on the command line.
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Directory holding GGUF router models. Provisioned SEPARATELY from OTA
    /// (see module.yaml `models_dir`): the model can appear after boot, so the
    /// supervisor polls for it rather than failing when it is absent.
    pub models_dir: PathBuf,
    /// Explicit model file. When set and present it wins over discovery.
    pub model: Option<PathBuf>,
    /// Loopback listen host. The router is NEVER exposed off the device, so a
    /// non-loopback address is rejected at load time.
    pub listen_host: String,
    /// Loopback listen port the agent's `llamacpp.router` provider points at.
    pub listen_port: u16,
    /// llama.cpp OpenAI-compatible server binary.
    pub server_bin: PathBuf,
    /// Context window served, in tokens. Kept modest for a ~1B model on CPU.
    pub ctx_size: u32,
    /// Generation threads. 0 lets the server pick (nproc).
    pub threads: u32,
    /// Model alias advertised on the API; MUST match agent.toml provider
    /// `model` (`router`) so the agent resolves the fallback.
    pub model_alias: String,
    /// Seconds between model-presence polls while no model is installed.
    pub poll_secs: u64,
    /// Seconds to back off before relaunching after the server exits.
    pub restart_secs: u64,
    /// Extra raw arguments appended to the server command line.
    pub extra_args: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            models_dir: PathBuf::from("/mnt/data/redrob/models"),
            model: None,
            listen_host: "127.0.0.1".into(),
            listen_port: 8081,
            server_bin: PathBuf::from("/usr/bin/llama-server"),
            ctx_size: 8192,
            threads: 4,
            model_alias: "router".into(),
            poll_secs: 15,
            restart_secs: 5,
            extra_args: Vec::new(),
        }
    }
}

impl Config {
    pub fn load(path: &std::path::Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("read {}: {e}", path.display()))?;
        let cfg: Self = toml::from_str(&text).map_err(|e| anyhow::anyhow!("parse config: {e}"))?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Reject anything that would publish the router off the device, or that
    /// cannot name a port/alias. Loopback-only is a security invariant, not a
    /// preference (the router answers raw prompts with no auth).
    pub fn validate(&self) -> Result<()> {
        match self.listen_host.parse::<IpAddr>() {
            Ok(ip) if ip.is_loopback() => {}
            Ok(ip) => bail!(
                "listen_host {ip} is not loopback; the router must never bind a routable address"
            ),
            Err(_) => bail!("listen_host {} is not an IP address", self.listen_host),
        }
        if self.listen_port == 0 {
            bail!("listen_port must be non-zero");
        }
        if self.model_alias.trim().is_empty() {
            bail!("model_alias must not be empty");
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    #[test]
    fn default_is_loopback_and_valid() {
        let c = Config::default();
        assert!(c.validate().is_ok());
        assert_eq!(c.listen_host, "127.0.0.1");
        assert_eq!(c.listen_port, 8081);
        assert_eq!(c.model_alias, "router");
    }

    #[test]
    fn rejects_non_loopback_bind() {
        let mut c = Config::default();
        c.listen_host = "0.0.0.0".into();
        assert!(c.validate().is_err());
        c.listen_host = "192.168.1.10".into();
        assert!(c.validate().is_err());
    }

    #[test]
    fn accepts_ipv6_loopback() {
        let mut c = Config::default();
        c.listen_host = "::1".into();
        assert!(c.validate().is_ok());
    }

    #[test]
    fn rejects_garbage_host_and_empty_alias() {
        let mut c = Config::default();
        c.listen_host = "localhost".into(); // not an IP literal
        assert!(c.validate().is_err());
        let mut c = Config::default();
        c.model_alias = "   ".into();
        assert!(c.validate().is_err());
    }

    #[test]
    fn unknown_field_is_rejected() {
        // deny_unknown_fields guards against a typo'd key silently doing nothing.
        assert!(toml::from_str::<Config>("listen_prt = 9000\n").is_err());
    }

    #[test]
    fn parses_partial_override() {
        let c: Config = toml::from_str("listen_port = 9099\nthreads = 2\n").unwrap();
        assert_eq!(c.listen_port, 9099);
        assert_eq!(c.threads, 2);
        assert_eq!(c.listen_host, "127.0.0.1"); // default kept
    }
}
