//! Broker configuration (`/etc/redrob/broker.toml`, overridable per field).
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Risk class of a scope. Drives the approval gate (design: credential-broker.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RiskClass {
    /// Auto within a granted scope.
    Read,
    /// Auto within a granted scope, logged.
    Mutate,
    /// Leaves the device as a message/mail/push: per-action approval.
    ExternalSend,
    /// Destroys data: per-action approval.
    Delete,
    /// Metered: per-action approval.
    Spend,
}

impl RiskClass {
    pub fn needs_approval(self) -> bool {
        matches!(self, Self::ExternalSend | Self::Delete | Self::Spend)
    }
}

/// One named scope: what vendor it reaches, with which credential, on which paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scope {
    /// Vendor base URL, e.g. `https://gmail.googleapis.com`.
    pub base_url: String,
    /// Name of the credential in the store, e.g. `google.user`.
    pub credential: String,
    /// How the credential is sent. `bearer` (default) or `header:<Name>`.
    #[serde(default = "default_auth")]
    pub auth: String,
    /// Allowed HTTP methods (uppercase). Empty = GET only.
    #[serde(default)]
    pub methods: Vec<String>,
    /// Allowed path prefixes; a request path must start with one of them.
    pub paths: Vec<String>,
    pub risk: RiskClass,
}

fn default_auth() -> String {
    "bearer".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EgressConfig {
    /// Loopback TCP listener for the forward proxy the agent is pointed at.
    #[serde(default = "default_proxy_listen")]
    pub listen: String,
    /// Host allow-list. `*.example.com` matches any subdomain, not the apex.
    #[serde(default)]
    pub allow: Vec<String>,
}

fn default_proxy_listen() -> String {
    "127.0.0.1:3128".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerConfig {
    /// Unix socket the agent (Z1) talks to.
    #[serde(default = "default_socket")]
    pub socket: PathBuf,
    /// Encrypted credential store, key file and grants live here.
    #[serde(default = "default_state_dir")]
    pub state_dir: PathBuf,
    /// Append-only audit log directory (`broker-YYYY-MM-DD.jsonl`).
    #[serde(default = "default_audit_dir")]
    pub audit_dir: PathBuf,
    /// Unix user allowed to manage credentials, grants and approvals over the socket.
    #[serde(default = "default_admin_user")]
    pub admin_user: String,
    /// Upstream call timeout.
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    #[serde(default)]
    pub egress: Option<EgressConfig>,
    #[serde(default)]
    pub scopes: BTreeMap<String, Scope>,
}

fn default_socket() -> PathBuf {
    "/run/redrob/broker.sock".into()
}
fn default_state_dir() -> PathBuf {
    "/mnt/data/redrob/broker".into()
}
fn default_audit_dir() -> PathBuf {
    "/mnt/data/redrob/audit".into()
}
fn default_admin_user() -> String {
    "redrob-agent".into()
}
fn default_timeout() -> u64 {
    60
}

impl BrokerConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read broker config {}", path.display()))?;
        let cfg: Self = toml::from_str(&text).context("parse broker config")?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&self) -> Result<()> {
        for (name, scope) in &self.scopes {
            anyhow::ensure!(
                scope.base_url.starts_with("https://"),
                "scope {name}: base_url must be https://"
            );
            anyhow::ensure!(
                !scope.paths.is_empty(),
                "scope {name}: paths must not be empty"
            );
            for p in &scope.paths {
                anyhow::ensure!(
                    p.starts_with('/'),
                    "scope {name}: path {p} must start with /"
                );
            }
            anyhow::ensure!(
                scope.auth == "bearer" || scope.auth.starts_with("header:"),
                "scope {name}: auth must be bearer or header:<Name>"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_plain_http_scope() {
        let cfg: BrokerConfig = toml::from_str(
            r#"
            [scopes."x.read"]
            base_url = "http://example.com"
            credential = "x"
            paths = ["/v1/"]
            risk = "read"
            "#,
        )
        .unwrap();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn approval_classes() {
        assert!(RiskClass::ExternalSend.needs_approval());
        assert!(RiskClass::Spend.needs_approval());
        assert!(!RiskClass::Read.needs_approval());
        assert!(!RiskClass::Mutate.needs_approval());
    }
}
