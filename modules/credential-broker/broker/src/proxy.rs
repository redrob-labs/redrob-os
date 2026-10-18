//! Egress proxy for the agent: an HTTP forward proxy on loopback that only lets
//! allow-listed hosts through, never injects credentials, and logs every request.
//! HTTPS goes through `CONNECT host:443` (opaque tunnel, no MITM); plain HTTP is
//! relayed with the request line rewritten to origin-form.
use crate::audit::{Audit, Entry, now_iso};
use crate::config::EgressConfig;
use crate::policy::host_allowed;
use anyhow::{Context, Result, bail};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

pub async fn serve(cfg: EgressConfig, audit: Arc<Audit>) -> Result<()> {
    let listener = TcpListener::bind(&cfg.listen)
        .await
        .with_context(|| format!("bind {}", cfg.listen))?;
    tracing::info!(listen = %cfg.listen, allow = ?cfg.allow, "egress proxy up");
    let cfg = Arc::new(cfg);
    loop {
        let (stream, peer) = listener.accept().await?;
        let (cfg, audit) = (cfg.clone(), audit.clone());
        tokio::spawn(async move {
            if let Err(e) = handle(stream, &cfg, &audit).await {
                tracing::debug!(%peer, %e, "egress connection ended");
            }
        });
    }
}

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

fn log(audit: &Audit, method: &str, host: &str, path: &str, status: u16, decision: &str) {
    audit.write(&Entry {
        ts: now_iso(),
        task: "",
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

async fn handle(stream: TcpStream, cfg: &EgressConfig, audit: &Audit) -> Result<()> {
    let mut reader = BufReader::new(stream);
    let head = read_head(&mut reader).await?;

    if head.method.eq_ignore_ascii_case("CONNECT") {
        let (host, port) = head
            .target
            .rsplit_once(':')
            .unwrap_or((&head.target, "443"));
        let port: u16 = port.parse().unwrap_or(443);
        if !host_allowed(&cfg.allow, host) {
            log(audit, "CONNECT", host, "", 403, "refused:host-not-allowed");
            let mut s = reader.into_inner();
            s.write_all(
                b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .await?;
            return Ok(());
        }
        let upstream = match TcpStream::connect((host, port)).await {
            Ok(u) => u,
            Err(e) => {
                log(audit, "CONNECT", host, "", 502, "auto");
                let mut s = reader.into_inner();
                s.write_all(
                    b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await?;
                bail!("connect {host}:{port}: {e}");
            }
        };
        log(audit, "CONNECT", host, "", 200, "auto");
        let mut client = reader.into_inner();
        client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;
        let (mut cr, mut cw) = client.into_split();
        let (mut ur, mut uw) = upstream.into_split();
        let a = tokio::io::copy(&mut cr, &mut uw);
        let b = tokio::io::copy(&mut ur, &mut cw);
        let _ = tokio::try_join!(a, b);
        return Ok(());
    }

    // Plain HTTP: absolute-form target `http://host[:port]/path`.
    let Some(rest) = head.target.strip_prefix("http://") else {
        let mut s = reader.into_inner();
        s.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await?;
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
    if !host_allowed(&cfg.allow, host) {
        log(
            audit,
            &head.method,
            host,
            path,
            403,
            "refused:host-not-allowed",
        );
        let mut s = reader.into_inner();
        s.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await?;
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
        reader.read_exact(&mut body).await?;
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
    log(audit, &head.method, host, path, status, "auto");
    let mut client = reader.into_inner();
    client.write_all(status_line.as_bytes()).await?;
    tokio::io::copy(&mut up, &mut client).await?;
    Ok(())
}
