//! Unix-socket API for the agent/dashboard, and the udev event loop.
use crate::broker::Broker;
use crate::udev::{Event, Parser};
use anyhow::{Context, Result};
use axum::extract::connect_info::{ConnectInfo, Connected};
use axum::extract::{Path as AxPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::serve::IncomingStream;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::Mutex;

#[derive(Clone, Copy, Debug)]
pub struct Peer {
    pub uid: u32,
}
impl Connected<IncomingStream<'_, UnixListener>> for Peer {
    fn connect_info(stream: IncomingStream<'_, UnixListener>) -> Self {
        Peer {
            uid: stream.io().peer_cred().map(|c| c.uid()).unwrap_or(u32::MAX),
        }
    }
}

pub struct AppState {
    pub broker: Mutex<Broker>,
    pub admin_uid: u32,
}
type Shared = Arc<AppState>;

fn err(status: StatusCode, msg: impl Into<String>) -> Response {
    (status, Json(json!({"error": msg.into()}))).into_response()
}

fn admin(s: &AppState, p: &Peer) -> Result<(), Response> {
    if p.uid == s.admin_uid || p.uid == 0 {
        Ok(())
    } else {
        Err(err(
            StatusCode::FORBIDDEN,
            format!("uid {} is not the usb admin", p.uid),
        ))
    }
}

pub fn router(state: Shared) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/devices", get(devices))
        .route("/v1/devices/{sysname}/approve", post(approve))
        .route("/v1/devices/{sysname}/deny", post(deny))
        .route("/v1/policy", get(policy))
        .with_state(state)
}

async fn health(State(s): State<Shared>) -> Json<serde_json::Value> {
    let b = s.broker.lock().await;
    let pending = b
        .devices
        .values()
        .filter(|r| r.verdict == crate::policy::Verdict::Pending)
        .count();
    Json(
        json!({"ok": true, "devices": b.devices.len(), "pending": pending, "mount_enabled": b.mount_enabled}),
    )
}

async fn devices(State(s): State<Shared>) -> Json<serde_json::Value> {
    Json(json!({"devices": s.broker.lock().await.devices}))
}

async fn policy(State(s): State<Shared>, ConnectInfo(p): ConnectInfo<Peer>) -> Response {
    if let Err(r) = admin(&s, &p) {
        return r;
    }
    Json(json!({"policy": s.broker.lock().await.policy})).into_response()
}

#[derive(Deserialize, Default)]
struct Decision {
    #[serde(default)]
    remember: bool,
}

async fn approve(
    State(s): State<Shared>,
    ConnectInfo(p): ConnectInfo<Peer>,
    AxPath(sysname): AxPath<String>,
    body: Option<Json<Decision>>,
) -> Response {
    if let Err(r) = admin(&s, &p) {
        return r;
    }
    let remember = body.map(|b| b.remember).unwrap_or(false);
    match s.broker.lock().await.approve(&sysname, remember) {
        Ok(true) => Json(json!({"approved": sysname, "remembered": remember})).into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, "no such device"),
        Err(e) => err(StatusCode::FORBIDDEN, e.to_string()),
    }
}

async fn deny(
    State(s): State<Shared>,
    ConnectInfo(p): ConnectInfo<Peer>,
    AxPath(sysname): AxPath<String>,
    body: Option<Json<Decision>>,
) -> Response {
    if let Err(r) = admin(&s, &p) {
        return r;
    }
    let remember = body.map(|b| b.remember).unwrap_or(false);
    match s.broker.lock().await.deny(&sysname, remember) {
        Ok(true) => Json(json!({"denied": sysname, "remembered": remember})).into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, "no such device"),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

pub async fn serve(state: Shared, listener: UnixListener) -> Result<()> {
    axum::serve(
        listener,
        router(state).into_make_service_with_connect_info::<Peer>(),
    )
    .await?;
    Ok(())
}

/// Map a udev DEVPATH to the usb_device sysfs dir and its sysname (`.../usb1/1-2` -> `1-2`).
pub fn usb_sysname(devpath: &str) -> Option<&str> {
    devpath
        .rsplit('/')
        .next()
        .filter(|n| n.contains('-') && !n.contains(':'))
}

/// For a block device, find the usb_device ancestor name in its DEVPATH
/// (`/devices/pci.../usb1/1-2/1-2:1.0/host6/target6:0:0/6:0:0:0/block/sda/sda1` -> `1-2`).
pub fn usb_ancestor(devpath: &str) -> Option<&str> {
    let mut found = None;
    for seg in devpath.split('/') {
        if seg.contains('-')
            && !seg.contains(':')
            && seg.chars().next().is_some_and(|c| c.is_ascii_digit())
        {
            found = Some(seg);
        }
    }
    found
}

pub async fn handle_event(state: &Shared, sysfs: &Path, ev: &Event) {
    let mut b = state.broker.lock().await;
    match (ev.subsystem.as_str(), ev.devtype(), ev.action.as_str()) {
        ("usb", Some("usb_device"), "add") => {
            if let Some(name) = usb_sysname(&ev.devpath) {
                let dir = sysfs.join("bus/usb/devices").join(name);
                if let Err(e) = b.handle_add(&dir, false) {
                    tracing::warn!(%e, name, "handle_add");
                }
            }
        }
        ("usb", Some("usb_device"), "remove") => {
            if let Some(name) = usb_sysname(&ev.devpath) {
                b.handle_remove(name);
            }
        }
        ("block", Some(dt), "add") if dt == "partition" || dt == "disk" => {
            if ev.get("ID_BUS") != Some("usb") && !ev.devpath.contains("/usb") {
                return;
            }
            // whole disks that carry partitions are mounted via their partitions
            if dt == "disk" && ev.get("ID_PART_TABLE_TYPE").is_some() {
                return;
            }
            let Some(usb) = usb_ancestor(&ev.devpath) else {
                return;
            };
            let Some(devnode) = ev.get("DEVNAME") else {
                return;
            };
            if let Err(e) = b.handle_block_add(usb, devnode, ev.get("ID_FS_LABEL")) {
                tracing::warn!(%e, devnode, "mount");
            }
        }
        _ => {}
    }
}

/// Run `udevadm monitor` and feed events until it exits.
pub async fn monitor_loop(state: Shared, sysfs: PathBuf) -> Result<()> {
    let mut child = tokio::process::Command::new("udevadm")
        .args([
            "monitor",
            "--udev",
            "--property",
            "--subsystem-match=usb",
            "--subsystem-match=block",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("spawn udevadm monitor")?;
    let stdout = child.stdout.take().context("udevadm stdout")?;
    let mut lines = BufReader::new(stdout).lines();
    let mut parser = Parser::default();
    while let Some(line) = lines.next_line().await? {
        if let Some(ev) = parser.feed(&line) {
            handle_event(&state, &sysfs, &ev).await;
        }
    }
    anyhow::bail!("udevadm monitor exited")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn devpath_mapping() {
        assert_eq!(
            usb_sysname("/devices/pci0000:00/0000:00:1d.0/usb1/1-2"),
            Some("1-2")
        );
        assert_eq!(
            usb_sysname("/devices/pci0000:00/0000:00:1d.0/usb1/1-2/1-2:1.0"),
            None
        );
        assert_eq!(usb_sysname("/devices/pci0000:00/0000:00:1d.0/usb1"), None);
        assert_eq!(
            usb_ancestor(
                "/devices/pci0000:00/0000:00:1d.0/usb1/1-2/1-2:1.0/host6/target6:0:0/6:0:0:0/block/sda/sda1"
            ),
            Some("1-2")
        );
        assert_eq!(
            usb_ancestor(
                "/devices/pci0000:00/0000:00:1f.2/ata1/host0/target0:0:0/0:0:0:0/block/vda"
            ),
            None
        );
    }
}
