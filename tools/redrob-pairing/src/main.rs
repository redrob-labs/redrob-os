//! redrob-pairing: render the first-boot pairing banner.
//!
//! Reads the device id, the host's addresses and the agent's current one-time pairing
//! code (GET /admin/paircode on the loopback gateway, admin token from the agent data
//! dir), and writes a banner with a QR code to `/run/issue.d/50-redrob-pairing.issue`
//! so every login prompt (serial, VT) shows it, plus a machine-readable
//! `/run/redrob-pairing/pairing.json` for the display module. Headless devices expose the
//! same code over the agent's channels.
//!
//! The QR encodes `redrob://pair?device=<id>&host=<ip>:<port>&code=<code>`.
use qrcodegen::{QrCode, QrCodeEcc};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

struct Opts {
    data_dir: PathBuf,
    identity_dir: PathBuf,
    gateway: String,
    issue_path: PathBuf,
    json_path: PathBuf,
    stdout: bool,
}

fn usage() -> ! {
    eprintln!(
        "usage: redrob-pairing [--data-dir D] [--identity-dir D] [--gateway host:port] [--issue PATH] [--json PATH] [--stdout]"
    );
    std::process::exit(2)
}

fn read_trim(p: &Path) -> Option<String> {
    fs::read_to_string(p)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Minimal HTTP/1.1 GET on loopback; returns the body. No TLS, no deps.
fn http_get(addr: &str, path: &str, headers: &[(&str, &str)]) -> Option<String> {
    let mut s = TcpStream::connect(addr).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut req = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n");
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    s.write_all(req.as_bytes()).ok()?;
    let mut buf = String::new();
    s.read_to_string(&mut buf).ok()?;
    let body = buf.split_once("\r\n\r\n")?.1;
    // chunked bodies: take the longest chunk payload line that looks like JSON
    Some(
        body.lines()
            .filter(|l| l.starts_with('{'))
            .max_by_key(|l| l.len())
            .unwrap_or(body)
            .to_string(),
    )
}

/// Pull `"key":"value"` or `"key":null` out of a flat JSON object without a JSON crate.
fn json_str(body: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\":");
    let i = body.find(&needle)? + needle.len();
    let rest = body[i..].trim_start();
    if rest.starts_with("null") {
        return None;
    }
    let rest = rest.strip_prefix('"')?;
    Some(rest.split('"').next()?.to_string())
}

/// Global IPv4 addresses from /proc/net/fib_trie (no `ip` dependency): the entries marked
/// `/32 host LOCAL` under a non-loopback prefix.
fn local_ipv4() -> Vec<String> {
    let Ok(text) = fs::read_to_string("/proc/net/fib_trie") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    for (i, l) in lines.iter().enumerate() {
        if l.contains("/32 host LOCAL") && i >= 1 {
            let ip = lines[i - 1]
                .trim()
                .trim_start_matches(['|', '-', '+', ' '])
                .trim();
            if ip.contains('.') && !ip.starts_with("127.") && !out.iter().any(|x| x == ip) {
                out.push(ip.to_string());
            }
        }
    }
    out
}

/// QR as UTF-8 half-block art (two module rows per text line), quiet zone of 2.
pub fn qr_text(data: &str) -> Option<String> {
    let qr = QrCode::encode_text(data, QrCodeEcc::Medium).ok()?;
    let n = qr.size();
    let q = 2;
    let mut s = String::new();
    let mut y = -q;
    while y < n + q {
        for x in -q..n + q {
            let top = qr.get_module(x, y);
            let bottom = qr.get_module(x, y + 1);
            s.push(match (top, bottom) {
                (true, true) => '\u{2588}',
                (true, false) => '\u{2580}',
                (false, true) => '\u{2584}',
                (false, false) => ' ',
            });
        }
        s.push('\n');
        y += 2;
    }
    Some(s)
}

pub fn pairing_uri(device: &str, host: &str, code: &str) -> String {
    format!("redrob://pair?device={device}&host={host}&code={code}")
}

pub fn banner(
    device: &str,
    hostname: &str,
    addrs: &[String],
    port: &str,
    code: Option<&str>,
) -> String {
    let mut b = String::new();
    b.push_str("\n  Redrob OS -- pair this device\n");
    b.push_str(&format!("  device   {device}\n  hostname {hostname}\n"));
    let host = addrs
        .first()
        .map(|a| format!("{a}:{port}"))
        .unwrap_or_else(|| format!("<no address yet>:{port}"));
    b.push_str(&format!("  address  {host}\n"));
    match code {
        Some(c) => {
            b.push_str(&format!("  code     {c}\n\n"));
            let uri = pairing_uri(device, &host, c);
            if let Some(qr) = qr_text(&uri) {
                for line in qr.lines() {
                    b.push_str("  ");
                    b.push_str(line);
                    b.push('\n');
                }
            }
            b.push_str(&format!("  {uri}\n"));
        }
        None => b.push_str("  code     (already paired, or the agent is still starting)\n"),
    }
    b.push('\n');
    b
}

fn main() {
    let mut o = Opts {
        data_dir: PathBuf::from("/mnt/data/redrob/agent/data"),
        identity_dir: PathBuf::from("/mnt/data/redrob/identity"),
        gateway: "127.0.0.1:42617".into(),
        issue_path: PathBuf::from("/run/issue.d/50-redrob-pairing.issue"),
        json_path: PathBuf::from("/run/redrob-pairing/pairing.json"),
        stdout: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--data-dir" => o.data_dir = args.next().map(PathBuf::from).unwrap_or_else(|| usage()),
            "--identity-dir" => {
                o.identity_dir = args.next().map(PathBuf::from).unwrap_or_else(|| usage())
            }
            "--gateway" => o.gateway = args.next().unwrap_or_else(|| usage()),
            "--issue" => o.issue_path = args.next().map(PathBuf::from).unwrap_or_else(|| usage()),
            "--json" => o.json_path = args.next().map(PathBuf::from).unwrap_or_else(|| usage()),
            "--stdout" => o.stdout = true,
            _ => usage(),
        }
    }
    let device = read_trim(&o.identity_dir.join("device-id")).unwrap_or_else(|| "unknown".into());
    let hostname = read_trim(Path::new("/etc/hostname"))
        .or_else(|| read_trim(Path::new("/proc/sys/kernel/hostname")))
        .unwrap_or_default();
    let addrs = local_ipv4();
    let port = o.gateway.rsplit(':').next().unwrap_or("42617").to_string();

    // The timer's first run lands while the agent may still be starting; a few short
    // retries avoid printing an ambiguous banner for a whole timer period.
    let mut code = None;
    for attempt in 0..8 {
        code = read_trim(&o.data_dir.join("gateway-admin.token"))
            .and_then(|t| {
                http_get(
                    &o.gateway,
                    "/admin/paircode",
                    &[("x-zeroclaw-admin-token", &t)],
                )
            })
            .and_then(|b| json_str(&b, "pairing_code"));
        if code.is_some() || attempt == 7 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    let text = banner(&device, &hostname, &addrs, &port, code.as_deref());
    if o.stdout {
        print!("{text}");
    }
    for (p, content) in [
        (&o.issue_path, text.clone()),
        (
            &o.json_path,
            format!(
                "{{\"device\":\"{device}\",\"hostname\":\"{hostname}\",\"addresses\":[{}],\"port\":{port},\"code\":{}}}\n",
                addrs
                    .iter()
                    .map(|a| format!("\"{a}\""))
                    .collect::<Vec<_>>()
                    .join(","),
                code.as_deref()
                    .map(|c| format!("\"{c}\""))
                    .unwrap_or_else(|| "null".into())
            ),
        ),
    ] {
        if let Some(parent) = p.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(e) = fs::write(p, content) {
            eprintln!("redrob-pairing: write {}: {e}", p.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_renders_and_uri_is_stable() {
        let uri = pairing_uri("abc", "10.0.0.5:42617", "CODE1234");
        assert_eq!(
            uri,
            "redrob://pair?device=abc&host=10.0.0.5:42617&code=CODE1234"
        );
        let qr = qr_text(&uri).unwrap();
        let lines: Vec<&str> = qr.lines().collect();
        assert!(lines.len() >= 15 && lines.len() <= 30, "{}", lines.len());
        assert!(
            lines
                .iter()
                .all(|l| l.chars().count() == lines[0].chars().count())
        );
        assert!(qr.contains('\u{2588}'));
    }

    #[test]
    fn banner_with_and_without_code() {
        let b = banner(
            "dev-1",
            "redrob-abc123",
            &["192.168.1.9".into()],
            "42617",
            Some("XYZ"),
        );
        assert!(
            b.contains("code     XYZ")
                && b.contains("redrob://pair?device=dev-1&host=192.168.1.9:42617&code=XYZ")
        );
        let b2 = banner("dev-1", "redrob-abc123", &[], "42617", None);
        assert!(b2.contains("already paired") && b2.contains("<no address yet>:42617"));
    }

    #[test]
    fn json_extraction() {
        let body =
            r#"{"success":true,"pairing_required":true,"pairing_code":"AbC123","message":"x"}"#;
        assert_eq!(json_str(body, "pairing_code").as_deref(), Some("AbC123"));
        assert_eq!(json_str(r#"{"pairing_code":null}"#, "pairing_code"), None);
        assert_eq!(json_str(r#"{"other":"v"}"#, "pairing_code"), None);
    }
}
