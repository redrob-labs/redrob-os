//! redrob-broker: credential broker daemon (design: docs/design/credential-broker.md).
use redrob_broker::{api, audit, config, proxy, store};

use anyhow::{Context, Result};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::UnixListener;

fn usage() -> ! {
    eprintln!(
        "usage: redrob-broker [--config /etc/redrob/broker.toml] [--socket PATH] [--state-dir DIR] [--audit-dir DIR] [--admin-uid N] [--no-egress]"
    );
    std::process::exit(2)
}

/// Resolve a user name to a uid from /etc/passwd (no libc dependency).
fn uid_of(user: &str) -> Option<u32> {
    let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
    passwd.lines().find_map(|l| {
        let mut f = l.split(':');
        (f.next()? == user).then(|| f.nth(1)?.parse().ok())?
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive("info".parse()?),
        )
        .with_ansi(false)
        .init();

    let mut args = std::env::args().skip(1);
    let mut config_path = PathBuf::from("/etc/redrob/broker.toml");
    let (mut socket, mut state_dir, mut audit_dir, mut admin_uid, mut no_egress) =
        (None, None, None, None, false);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--config" => config_path = args.next().map(PathBuf::from).unwrap_or_else(|| usage()),
            "--socket" => socket = args.next().map(PathBuf::from),
            "--state-dir" => state_dir = args.next().map(PathBuf::from),
            "--audit-dir" => audit_dir = args.next().map(PathBuf::from),
            "--admin-uid" => admin_uid = args.next().and_then(|v| v.parse().ok()),
            "--no-egress" => no_egress = true,
            _ => usage(),
        }
    }
    let mut cfg = if config_path.exists() {
        config::BrokerConfig::load(&config_path)?
    } else {
        tracing::warn!(path = %config_path.display(), "no config file; running with defaults and no scopes");
        toml::from_str("")?
    };
    if let Some(p) = socket {
        cfg.socket = p;
    }
    if let Some(p) = state_dir {
        cfg.state_dir = p;
    }
    if let Some(p) = audit_dir {
        cfg.audit_dir = p;
    }
    if no_egress {
        cfg.egress = None;
    }
    let admin_uid = admin_uid.or_else(|| uid_of(&cfg.admin_user)).unwrap_or(0);

    let store = store::Store::open(&cfg.state_dir)?;
    let audit = Arc::new(audit::Audit::new(cfg.audit_dir.clone())?);
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(cfg.timeout_secs))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    if let Some(parent) = cfg.socket.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _ = std::fs::remove_file(&cfg.socket);
    let listener = UnixListener::bind(&cfg.socket)
        .with_context(|| format!("bind {}", cfg.socket.display()))?;
    // group redrob-broker (the agent is a member) may connect; others may not.
    std::fs::set_permissions(&cfg.socket, std::fs::Permissions::from_mode(0o660))?;
    tracing::info!(socket = %cfg.socket.display(), scopes = cfg.scopes.len(), admin_uid, "broker up");

    let egress = cfg.egress.clone();
    let state = Arc::new(api::AppState {
        cfg,
        store: tokio::sync::Mutex::new(store),
        audit: Arc::clone(&audit),
        http,
        admin_uid,
    });

    let api_task = tokio::spawn(api::serve(state, listener));
    let proxy_task = match egress {
        Some(e) => tokio::spawn(proxy::serve(e, audit)),
        None => tokio::spawn(async { std::future::pending::<Result<()>>().await }),
    };
    tokio::select! {
        r = api_task => r??,
        r = proxy_task => r??,
        _ = tokio::signal::ctrl_c() => tracing::info!("shutting down"),
    }
    Ok(())
}
