//! redrob-pairing: render the first-boot pairing banner.
//!
//! Reads the device id, the host's addresses and the agent's current one-time pairing
//! code (GET /admin/paircode on the loopback gateway, admin token from the agent data
//! dir), and writes a banner with a QR code to `/run/issue.d/50-redrob-pairing.issue`
//! so every login prompt (serial, VT) shows it, plus a machine-readable
//! `/run/redrob-pairing/pairing.json` for the display module. Headless devices expose the
//! same code over the agent's channels.
//!
//! The QR encodes `redrob://pair?device=<id>&host=<ip>:<port>&code=<code>[&fp=<sha256>]`.
//! `fp` is the hex SHA-256 of the gateway TLS certificate's public key (SPKI; present
//! when `[gateway.tls]` is enabled); the phone connects over `https://` and pins it. The
//! agent renews the certificate with the same key, so the pin does not change.
//!
//! `--notify` is the desktop half (Redrob OS bootc image, KDE): run from the user's
//! session autostart, it reads the `pairing.json` the root service keeps fresh and
//! shows a desktop notification (`notify-send`) while a pairing code exists. It
//! exits by itself once the device is paired, so nothing lingers in the tray.
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
    notify: bool,
}

fn usage() -> ! {
    eprintln!(
        "usage: redrob-pairing [--data-dir D] [--identity-dir D] [--gateway host:port] [--issue PATH] [--json PATH] [--stdout]\n       redrob-pairing --notify [--json PATH]   (desktop session: notification while unpaired)"
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

/// QR as an 8-bit grayscale PNG, `px` pixels per module, quiet zone of 4.
/// Written by hand (stored deflate blocks) so no image crate is needed; the
/// desktop notification shows it full size.
pub fn qr_png(data: &str, px: usize) -> Option<Vec<u8>> {
    let qr = QrCode::encode_text(data, QrCodeEcc::Medium).ok()?;
    let q = 4i32;
    let side = (qr.size() + 2 * q) as usize * px;
    let mut raw = Vec::with_capacity((side + 1) * side);
    for y in 0..side {
        raw.push(0u8); // filter: none
        for x in 0..side {
            let (mx, my) = ((x / px) as i32 - q, (y / px) as i32 - q);
            raw.push(if qr.get_module(mx, my) { 0 } else { 255 });
        }
    }
    let mut z = vec![0x78, 0x01];
    let mut chunks = raw.chunks(65_535).peekable();
    while let Some(c) = chunks.next() {
        z.push(if chunks.peek().is_none() { 1 } else { 0 });
        let n = c.len() as u16;
        z.extend_from_slice(&n.to_le_bytes());
        z.extend_from_slice(&(!n).to_le_bytes());
        z.extend_from_slice(c);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &v in &raw {
        a = (a + v as u32) % 65_521;
        b = (b + a) % 65_521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());

    fn crc(bytes: &[u8]) -> u32 {
        let mut c = 0xffff_ffffu32;
        for &v in bytes {
            c ^= v as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        let mut k = kind.to_vec();
        k.extend_from_slice(body);
        out.extend_from_slice(&k);
        out.extend_from_slice(&crc(&k).to_be_bytes());
    }
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(side as u32).to_be_bytes());
    ihdr.extend_from_slice(&(side as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 0, 0, 0, 0]); // 8-bit grayscale
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"IDAT", &z);
    chunk(&mut png, b"IEND", &[]);
    Some(png)
}

pub fn pairing_uri(device: &str, host: &str, code: &str, fp: Option<&str>) -> String {
    let mut uri = format!("redrob://pair?device={device}&host={host}&code={code}");
    if let Some(fp) = fp {
        uri.push_str(&format!("&fp={fp}"));
    }
    uri
}

/// What `pairing.json` says right now, as far as the desktop notification cares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingState {
    pub device: String,
    pub host: String,
    pub code: Option<String>,
    pub fp: Option<String>,
}

/// First address from the flat `"addresses":["a","b"]` array, if any.
fn json_first_addr(body: &str) -> Option<String> {
    let needle = "\"addresses\":[";
    let i = body.find(needle)? + needle.len();
    let rest = body[i..].trim_start();
    let rest = rest.strip_prefix('"')?;
    Some(rest.split('"').next()?.to_string()).filter(|s| !s.is_empty())
}

/// Bare integer after `"port":`.
fn json_port(body: &str) -> Option<String> {
    let needle = "\"port\":";
    let i = body.find(needle)? + needle.len();
    let digits: String = body[i..]
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    (!digits.is_empty()).then_some(digits)
}

/// Parse the `pairing.json` this tool writes in service mode.
pub fn parse_pairing_json(body: &str) -> Option<PairingState> {
    let device = json_str(body, "device")?;
    let port = json_port(body).unwrap_or_else(|| "42617".into());
    let host = json_first_addr(body)
        .map(|a| format!("{a}:{port}"))
        .unwrap_or_else(|| format!("<no address yet>:{port}"));
    Some(PairingState {
        device,
        host,
        code: json_str(body, "code"),
        fp: json_str(body, "tls_fingerprint"),
    })
}

/// True when the desktop session speaks Korean (same rule as redrob-comp).
fn session_is_ko() -> bool {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() {
                return v.starts_with("ko");
            }
        }
    }
    true
}

/// Body of the desktop notification shown while unpaired.
pub fn notify_body(st: &PairingState, code: &str, ko: bool) -> String {
    let uri = pairing_uri(&st.device, &st.host, code, st.fp.as_deref());
    if ko {
        format!(
            "같은 Wi-Fi에서 휴대폰 레드롭 앱의 'QR로 연결'로 찍거나 코드를 입력하세요.\n\
             코드  {code}\n\
             기기 주소  {host}\n\
             또는 열기  {uri}",
            host = st.host
        )
    } else {
        format!(
            "On the same Wi-Fi, scan this with \"Connect with QR\" in the Redrob app, or enter\n\
             code  {code}\n\
             device at  {host}\n\
             or open  {uri}",
            host = st.host
        )
    }
}

/// Send (or replace) a desktop notification via `notify-send`; returns the id
/// `notify-send -p` printed, so the next call can replace this one in place.
fn notify_send(summary: &str, body: &str, urgency: &str, replace: Option<u32>, image: Option<&Path>) -> Option<u32> {
    let mut cmd = std::process::Command::new("notify-send");
    cmd.args(["--app-name=Redrob OS", "--icon=redrob-os", "--print-id"])
        .arg(format!("--urgency={urgency}"));
    if let Some(img) = image {
        cmd.arg(format!("--hint=string:image-path:{}", img.display()));
    }
    if let Some(id) = replace {
        cmd.arg(format!("--replace-id={id}"));
    }
    if urgency == "low" {
        cmd.arg("--expire-time=8000");
    }
    cmd.arg(summary).arg(body);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<u32>()
        .ok()
}

/// Desktop-session loop: poll `pairing.json` every 30 s, keep ONE notification
/// up to date while a code exists, replace it with a short "paired" note and exit
/// once the code is gone. Exits quietly if the agent never produces a code
/// (already paired before login, pairing disabled, or the service is not running).
fn run_notify(json_path: &Path) -> ! {
    const POLL: Duration = Duration::from_secs(30);
    const MAX_WAIT_FOR_FILE: u32 = 40; // 20 min: service not up / not a Redrob OS box
    const MAX_QUIET: u32 = 4; // 2 min of "no code" before shown -> paired or disabled

    let mut shown: Option<(String, Option<u32>)> = None; // (code, notification id)
    let mut missing = 0u32;
    let mut quiet = 0u32;
    loop {
        match fs::read_to_string(json_path)
            .ok()
            .and_then(|b| parse_pairing_json(&b))
        {
            None => {
                missing += 1;
                if missing >= MAX_WAIT_FOR_FILE {
                    std::process::exit(0);
                }
            }
            Some(st) => {
                let prev_code = shown.as_ref().map(|(c, _)| c.clone());
                let prev_id = shown.as_ref().and_then(|(_, id)| *id);
                match st.code.as_deref() {
                    Some(code) if prev_code.as_deref() == Some(code) => {}
                    Some(code) => {
                        let ko = session_is_ko();
                        // The QR the phone app's "QR로 연결" scans, next to the code.
                        let uri = pairing_uri(&st.device, &st.host, code, st.fp.as_deref());
                        let qr_path = std::env::var_os("XDG_RUNTIME_DIR")
                            .map(std::path::PathBuf::from)
                            .unwrap_or_else(std::env::temp_dir)
                            .join("redrob-pairing-qr.png");
                        let qr = qr_png(&uri, 4).and_then(|b| fs::write(&qr_path, b).ok()).map(|_| qr_path.as_path());
                        let id = notify_send(
                            if ko { "Redrob OS: 이 기기 페어링" } else { "Redrob OS: pair this device" },
                            &notify_body(&st, code, ko),
                            "critical",
                            prev_id,
                            qr,
                        );
                        shown = Some((code.to_string(), id));
                    }
                    None if shown.is_some() => {
                        let ko = session_is_ko();
                        notify_send(
                            if ko { "Redrob OS: 페어링됨" } else { "Redrob OS: paired" },
                            if ko { "이 기기가 휴대폰과 페어링됐어요." } else { "This device is now paired with your phone." },
                            "low",
                            prev_id,
                            None,
                        );
                        std::process::exit(0);
                    }
                    None => {
                        quiet += 1;
                        if quiet >= MAX_QUIET {
                            std::process::exit(0);
                        }
                    }
                }
            }
        }
        std::thread::sleep(POLL);
    }
}

pub fn banner(
    device: &str,
    hostname: &str,
    addrs: &[String],
    port: &str,
    code: Option<&str>,
    fp: Option<&str>,
) -> String {
    let mut b = String::new();
    b.push_str("\n  Redrob OS: pair this device\n");
    b.push_str(&format!("  device   {device}\n  hostname {hostname}\n"));
    let host = addrs
        .first()
        .map(|a| format!("{a}:{port}"))
        .unwrap_or_else(|| format!("<no address yet>:{port}"));
    b.push_str(&format!("  address  {host}\n"));
    match code {
        Some(c) => {
            b.push_str(&format!("  code     {c}\n\n"));
            let uri = pairing_uri(device, &host, c, fp);
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
        notify: false,
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
            "--notify" => o.notify = true,
            _ => usage(),
        }
    }
    if o.notify {
        // Unprivileged desktop session: only the world-readable JSON is touched.
        run_notify(&o.json_path);
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
    let mut fp = None;
    for attempt in 0..8 {
        let body = read_trim(&o.data_dir.join("gateway-admin.token")).and_then(|t| {
            http_get(
                &o.gateway,
                "/admin/paircode",
                &[("x-zeroclaw-admin-token", &t)],
            )
        });
        code = body.as_deref().and_then(|b| json_str(b, "pairing_code"));
        fp = body.as_deref().and_then(|b| json_str(b, "tls_fingerprint"));
        if code.is_some() || attempt == 7 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    let text = banner(
        &device,
        &hostname,
        &addrs,
        &port,
        code.as_deref(),
        fp.as_deref(),
    );
    if o.stdout {
        print!("{text}");
    }
    for (p, content) in [
        (&o.issue_path, text.clone()),
        (
            &o.json_path,
            format!(
                "{{\"device\":\"{device}\",\"hostname\":\"{hostname}\",\"addresses\":[{}],\"port\":{port},\"code\":{},\"tls_fingerprint\":{}}}\n",
                addrs
                    .iter()
                    .map(|a| format!("\"{a}\""))
                    .collect::<Vec<_>>()
                    .join(","),
                code.as_deref()
                    .map(|c| format!("\"{c}\""))
                    .unwrap_or_else(|| "null".into()),
                fp.as_deref()
                    .map(|f| format!("\"{f}\""))
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
        assert!(pairing_uri("abc", "h:1", "C", Some("ab12")).ends_with("&code=C&fp=ab12"));
        let uri = pairing_uri("abc", "10.0.0.5:42617", "CODE1234", None);
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
            None,
        );
        assert!(
            b.contains("code     XYZ")
                && b.contains("redrob://pair?device=dev-1&host=192.168.1.9:42617&code=XYZ")
        );
        let b2 = banner("dev-1", "redrob-abc123", &[], "42617", None, None);
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

    #[test]
    fn pairing_json_roundtrip_for_notify() {
        // Exactly the shape the service mode writes.
        let body = r#"{"device":"dev-1","hostname":"redrob-abc123","addresses":["192.168.1.9","10.0.0.2"],"port":42617,"code":"XYZ"}"#;
        let st = parse_pairing_json(body).unwrap();
        assert_eq!(st.device, "dev-1");
        assert_eq!(st.host, "192.168.1.9:42617");
        assert_eq!(st.code.as_deref(), Some("XYZ"));
        let text = notify_body(&st, "XYZ", false);
        assert!(text.contains("code  XYZ"));
        assert!(text.contains("redrob://pair?device=dev-1&host=192.168.1.9:42617&code=XYZ"));

        let paired = r#"{"device":"dev-1","hostname":"h","addresses":[],"port":42617,"code":null}"#;
        let st = parse_pairing_json(paired).unwrap();
        assert_eq!(st.code, None);
        assert_eq!(st.host, "<no address yet>:42617");
        assert!(parse_pairing_json("not json").is_none());
    }
}
