//! redrob-usb-broker: default-deny USB admission, approval API, read-only automount.
use anyhow::{Context, Result};
use redrob_usb_broker::{api, broker::Broker};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::UnixListener;

fn usage() -> ! {
    eprintln!(
        "usage: redrob-usb-broker [--socket PATH] [--sysfs DIR] [--policy FILE] [--audit-dir DIR] [--media-dir DIR] [--admin-uid N] [--no-mount] [--no-monitor]"
    );
    std::process::exit(2)
}

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
    let mut socket = PathBuf::from("/run/redrob/usb.sock");
    let mut sysfs = PathBuf::from("/sys");
    let mut policy = PathBuf::from("/mnt/data/redrob/usb/policy.json");
    let mut audit_dir = PathBuf::from("/mnt/data/redrob/audit");
    let mut media_dir = PathBuf::from("/run/media/redrob");
    let mut admin_uid: Option<u32> = None;
    let (mut mount, mut monitor) = (true, true);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--socket" => socket = args.next().map(PathBuf::from).unwrap_or_else(|| usage()),
            "--sysfs" => sysfs = args.next().map(PathBuf::from).unwrap_or_else(|| usage()),
            "--policy" => policy = args.next().map(PathBuf::from).unwrap_or_else(|| usage()),
            "--audit-dir" => audit_dir = args.next().map(PathBuf::from).unwrap_or_else(|| usage()),
            "--media-dir" => media_dir = args.next().map(PathBuf::from).unwrap_or_else(|| usage()),
            "--admin-uid" => admin_uid = args.next().and_then(|v| v.parse().ok()),
            "--no-mount" => mount = false,
            "--no-monitor" => monitor = false,
            _ => usage(),
        }
    }
    let admin_uid = admin_uid.or_else(|| uid_of("redrob-agent")).unwrap_or(0);
    let mut broker = Broker::new(sysfs.clone(), policy, audit_dir, media_dir, mount)?;
    match broker.enforce_default_deny() {
        Ok(n) => tracing::info!(controllers = n, "default-deny armed"),
        Err(e) => tracing::warn!(%e, "could not arm default-deny (not root?)"),
    }
    broker.scan()?;
    tracing::info!(devices = broker.devices.len(), "scanned");

    if let Some(p) = socket.parent() {
        std::fs::create_dir_all(p)?;
    }
    let _ = std::fs::remove_file(&socket);
    let listener =
        UnixListener::bind(&socket).with_context(|| format!("bind {}", socket.display()))?;
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o660))?;
    let state = Arc::new(api::AppState {
        broker: tokio::sync::Mutex::new(broker),
        admin_uid,
    });
    tracing::info!(socket = %socket.display(), admin_uid, "usb broker up");

    let api_task = tokio::spawn(api::serve(state.clone(), listener));
    let mon_task = if monitor {
        tokio::spawn(api::monitor_loop(state, sysfs))
    } else {
        tokio::spawn(async { std::future::pending::<Result<()>>().await })
    };
    tokio::select! {
        r = api_task => r??,
        r = mon_task => r??,
        _ = tokio::signal::ctrl_c() => tracing::info!("shutting down"),
    }
    Ok(())
}
