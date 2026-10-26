//! Egress proxies: HTTP forward proxies that only let allow-listed hosts through,
//! never inject credentials, and log every request.
//!
//! - The agent's own proxy listens on loopback TCP (`[egress] listen`).
//! - Each task sandbox gets its own proxy on a Unix socket (`<task_dir>/<task>.sock`)
//!   with its own, narrower allow-list; audit lines carry the task name. The
//!   socket is bind-mounted into the task container, which has no network.
//!
//! HTTPS goes through `CONNECT host:443` (opaque tunnel, no MITM); plain HTTP is
//! relayed with the request line rewritten to origin-form.
use crate::audit::{Audit, Entry, now_iso};
use crate::config::EgressConfig;
use crate::policy::host_allowed;
use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream, UnixListener};
use tokio::task::JoinHandle;

/// The agent's proxy on loopback TCP.
pub async fn serve(cfg: EgressConfig, audit: Arc<Audit>) -> Result<()> {
    let listener = TcpListener::bind(&cfg.listen)
        .await
        .with_context(|| format!("bind {}", cfg.listen))?;
    tracing::info!(listen = %cfg.listen, allow = ?cfg.allow, "egress proxy up");
    let allow = Arc::new(cfg.allow);
    loop {
        let (stream, peer) = listener.accept().await?;
        let (allow, audit) = (allow.clone(), audit.clone());
        tokio::spawn(async move {
            if let Err(e) = handle(stream, &allow, "", &audit).await {
                tracing::debug!(%peer, %e, "egress connection ended");
            }
        });
    }
}

/// Per-task proxies on Unix sockets, opened from config at start-up or by the
/// admin over the API, closed on revoke or when their time runs out.
pub struct TaskProxies {
    dir: PathBuf,
    audit: Arc<Audit>,
    running: Mutex<HashMap<String, JoinHandle<()>>>,
}

impl TaskProxies {
    pub fn new(dir: PathBuf, audit: Arc<Audit>) -> Self {
        Self {
            dir,
            audit,
            running: Mutex::new(HashMap::new()),
        }
    }

    pub fn socket_path(&self, task: &str) -> PathBuf {
        self.dir.join(format!("{task}.sock"))
    }

    /// Start (or replace) the proxy for `task`. `ttl_secs == 0` keeps it until
    /// `stop`. Returns the socket path the sandbox should mount.
    pub fn start(&self, task: &str, allow: Vec<String>, ttl_secs: u64) -> Result<PathBuf> {
        self.stop(task);
        std::fs::create_dir_all(&self.dir)?;
        // The agent (group redrob-broker) may see the socket to mount it; only the
        // broker and root -- the podman service that mounts it -- may connect.
        std::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o750))?;
        let path = self.socket_path(task);
        let _ = std::fs::remove_file(&path);
        let listener =
            UnixListener::bind(&path).with_context(|| format!("bind {}", path.display()))?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        tracing::info!(
            task,
            socket = %path.display(),
            allow = ?allow,
            ttl_secs,
            "task egress proxy up"
        );

        let (allow, audit, name) = (Arc::new(allow), self.audit.clone(), task.to_string());
        let accept = async move {
            let mut failures = 0u32;
            while failures < 100 {
                let stream = match listener.accept().await {
                    Ok((s, _)) => {
                        failures = 0;
                        s
                    }
                    Err(e) => {
                        failures += 1;
                        tracing::warn!(task = %name, %e, "task egress accept");
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        continue;
                    }
                };
                let (allow, audit, name) = (allow.clone(), audit.clone(), name.clone());
                tokio::spawn(async move {
                    if let Err(e) = handle(stream, &allow, &name, &audit).await {
                        tracing::debug!(task = %name, %e, "task egress connection ended");
                    }
                });
            }
        };
        let cleanup = path.clone();
        let handle = tokio::spawn(async move {
            if ttl_secs == 0 {
                accept.await;
            } else {
                let ttl = std::time::Duration::from_secs(ttl_secs);
                let _ = tokio::time::timeout(ttl, accept).await;
                let _ = std::fs::remove_file(&cleanup);
            }
        });
        self.lock().insert(task.to_string(), handle);
        Ok(path)
    }

    /// Close the task's proxy. Open tunnels end with the task's container.
    pub fn stop(&self, task: &str) -> bool {
        let Some(h) = self.lock().remove(task) else {
            return false;
        };
        h.abort();
        let _ = std::fs::remove_file(self.socket_path(task));
        true
    }

    pub fn tasks(&self) -> Vec<String> {
        let mut v: Vec<String> = self.lock().keys().cloned().collect();
        v.sort();
        v
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, JoinHandle<()>>> {
        self.running.lock().unwrap_or_else(|p| p.into_inner())
    }
}

// The body names the proxy, so a client can tell this refusal apart from a
// 403 sent by the upstream service itself (the agent matches on it).
const FORBIDDEN: &[u8] = b"HTTP/1.1 403 Forbidden\r\nContent-Type: text/plain\r\nContent-Length: 54\r\nConnection: close\r\n\r\nredrob-egress: refused, host is not on the allow-list\n";
const BAD_GATEWAY: &[u8] =
    b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
const BAD_REQUEST: &[u8] =
    b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

struct Head {
    method: String,
    target: String,
    version: String,
    headers: Vec<(String, String)>,
}

async fn read_head<R: AsyncBufReadExt + Unpin>(r: &mut R) -> Result<Head> {
    let mut line = String::new();
    if r.read_line(&mut line).await? == 0 {
        bail!("eof before request line");
    }
    let mut it = line.split_whitespace();
    let (Some(m), Some(t), Some(v)) = (it.next(), it.next(), it.next()) else {
        bail!("bad request line");
    };
    let mut headers = Vec::new();
    loop {
        let mut h = String::new();
        if r.read_line(&mut h).await? == 0 {
            bail!("eof in headers");
        }
        let h = h.trim_end_matches(['\r', '\n']);
        if h.is_empty() {
            break;
        }
        if let Some((k, val)) = h.split_once(':') {
            headers.push((k.trim().to_string(), val.trim().to_string()));
        }
        if headers.len() > 100 {
            bail!("too many headers");
        }
    }
    Ok(Head {
        method: m.to_string(),
        target: t.to_string(),
        version: v.to_string(),
        headers,
    })
}

fn log(
    audit: &Audit,
    task: &str,
    method: &str,
    host: &str,
    path: &str,
    status: u16,
    decision: &str,
) {
    audit.write(&Entry {
        ts: now_iso(),
        task,
        scope: "egress",
        method,
        host,
        path,
        status,
        bytes_in: 0,
        bytes_out: 0,
        decision,
        peer_uid: 0,
    });
}

async fn handle<S>(stream: S, allow: &[String], task: &str, audit: &Audit) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    // BufReader passes writes through, so `client` is used for both directions and
    // bytes the client sent right behind the head are not lost.
    let mut client = BufReader::new(stream);
    let head = read_head(&mut client).await?;

    if head.method.eq_ignore_ascii_case("CONNECT") {
        let (host, port) = head
            .target
            .rsplit_once(':')
            .unwrap_or((&head.target, "443"));
        let port: u16 = port.parse().unwrap_or(443);
        if !host_allowed(allow, host) {
            log(audit, task, "CONNECT", host, "", 403, "refused:host-not-allowed");
            client.write_all(FORBIDDEN).await?;
            return Ok(());
        }
        let mut upstream = match TcpStream::connect((host, port)).await {
            Ok(u) => u,
            Err(e) => {
                log(audit, task, "CONNECT", host, "", 502, "auto");
                client.write_all(BAD_GATEWAY).await?;
                bail!("connect {host}:{port}: {e}");
            }
        };
        log(audit, task, "CONNECT", host, "", 200, "auto");
        client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;
        let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
        return Ok(());
    }

    // Plain HTTP: absolute-form target `http://host[:port]/path`.
    let Some(rest) = head.target.strip_prefix("http://") else {
        client.write_all(BAD_REQUEST).await?;
        bail!("non-proxy request {}", head.target);
    };
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = hostport
        .rsplit_once(':')
        .map(|(h, p)| (h, p.parse().unwrap_or(80)))
        .unwrap_or((hostport, 80u16));
    if !host_allowed(allow, host) {
        log(audit, task, &head.method, host, path, 403, "refused:host-not-allowed");
        client.write_all(FORBIDDEN).await?;
        return Ok(());
    }
    let mut upstream = TcpStream::connect((host, port))
        .await
        .with_context(|| format!("connect {host}:{port}"))?;
    let mut out = format!("{} {} {}\r\n", head.method, path, head.version);
    let mut content_length = 0usize;
    for (k, v) in &head.headers {
        if k.eq_ignore_ascii_case("proxy-connection")
            || k.eq_ignore_ascii_case("proxy-authorization")
        {
            continue;
        }
        if k.eq_ignore_ascii_case("content-length") {
            content_length = v.parse().unwrap_or(0);
        }
        out.push_str(&format!("{k}: {v}\r\n"));
    }
    out.push_str("Connection: close\r\n\r\n");
    upstream.write_all(out.as_bytes()).await?;
    if content_length > 0 {
        let mut body = vec![0u8; content_length.min(16 << 20)];
        client.read_exact(&mut body).await?;
        upstream.write_all(&body).await?;
    }
    // Relay the response; peek at the status line for the audit entry.
    let mut up = BufReader::new(upstream);
    let mut status_line = String::new();
    up.read_line(&mut status_line).await?;
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    log(audit, task, &head.method, host, path, status, "auto");
    client.write_all(status_line.as_bytes()).await?;
    tokio::io::copy(&mut up, &mut client).await?;
    client.flush().await?;
    Ok(())
}
