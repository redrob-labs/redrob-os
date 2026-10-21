//! Login screen: `redrob-comp --greeter`, run by greetd as the greeter.
//!
//! It is the lock screen with a user to pick. The password goes to greetd
//! over its socket ($GREETD_SOCK, length-prefixed JSON), and on success
//! greetd starts `redrob-session` for that user once this process exits.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::OnceLock;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static CURRENT: AtomicUsize = AtomicUsize::new(0);
static USERS: OnceLock<Vec<(String, String)>> = OnceLock::new();

pub fn start() {
    ACTIVE.store(true, Ordering::Relaxed);
    let users = USERS.get_or_init(read_users);
    // Start on the last user who logged in, when we know them.
    if let Ok(last) = std::fs::read_to_string(last_file()) {
        if let Some(i) = users.iter().position(|(n, _)| *n == last.trim()) {
            CURRENT.store(i, Ordering::Relaxed);
        }
    }
}

pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// (login name, name to show) of the selected user.
pub fn current() -> (String, String) {
    let users = USERS.get_or_init(read_users);
    users
        .get(CURRENT.load(Ordering::Relaxed))
        .cloned()
        .unwrap_or_else(|| ("user".into(), "user".into()))
}

pub fn user_count() -> usize {
    USERS.get_or_init(read_users).len()
}

/// Select the next (+1) or previous (-1) user.
pub fn step(d: isize) {
    let n = user_count();
    if n > 1 {
        let i = CURRENT.load(Ordering::Relaxed) as isize;
        CURRENT.store((i + d).rem_euclid(n as isize) as usize, Ordering::Relaxed);
    }
}

fn last_file() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/var/lib/greetd".into());
    std::path::Path::new(&home).join(".redrob-last-user")
}

/// People with a login shell (uid 1000-59999), from /etc/passwd.
fn read_users() -> Vec<(String, String)> {
    let text = std::fs::read_to_string("/etc/passwd").unwrap_or_default();
    let mut v = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split(':').collect();
        if f.len() < 7 {
            continue;
        }
        let Ok(uid) = f[2].parse::<u32>() else { continue };
        if !(1000..60000).contains(&uid) || f[6].ends_with("nologin") || f[6].ends_with("false") {
            continue;
        }
        let shown = f[4].split(',').next().filter(|s| !s.trim().is_empty()).unwrap_or(f[0]);
        v.push((f[0].to_string(), shown.to_string()));
    }
    v
}

fn send(s: &mut UnixStream, msg: &str) -> std::io::Result<()> {
    s.write_all(&(msg.len() as u32).to_ne_bytes())?;
    s.write_all(msg.as_bytes())
}

fn recv(s: &mut UnixStream) -> std::io::Result<String> {
    let mut n = [0u8; 4];
    s.read_exact(&mut n)?;
    let mut buf = vec![0u8; u32::from_ne_bytes(n) as usize];
    s.read_exact(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn kind(reply: &str) -> &str {
    reply
        .split("\"type\"")
        .nth(1)
        .and_then(|r| r.split('"').nth(1))
        .unwrap_or("")
}

/// Log `user` in with `password`; true when greetd will start the session.
pub fn login(user: &str, password: &str) -> bool {
    match try_login(user, password) {
        Ok(ok) => ok,
        Err(e) => {
            tracing::warn!(?e, "greeter: greetd");
            false
        }
    }
}

fn try_login(user: &str, password: &str) -> std::io::Result<bool> {
    let path = std::env::var("GREETD_SOCK").map_err(|_| std::io::Error::other("GREETD_SOCK unset"))?;
    let mut s = UnixStream::connect(path)?;
    send(&mut s, &format!("{{\"type\":\"create_session\",\"username\":{}}}", json_str(user)))?;
    let mut reply = recv(&mut s)?;
    // Answer every prompt with the password (PAM asks once for it; info
    // messages get an empty answer).
    let mut rounds = 0;
    while kind(&reply) == "auth_message" && rounds < 4 {
        let secret = reply.contains("\"secret\"") || reply.contains("\"visible\"");
        let answer = if secret { json_str(password) } else { "null".into() };
        send(&mut s, &format!("{{\"type\":\"post_auth_message_response\",\"response\":{answer}}}"))?;
        reply = recv(&mut s)?;
        rounds += 1;
    }
    if kind(&reply) != "success" {
        tracing::info!(reply = %reply.replace(password, "***"), "greeter: login refused");
        send(&mut s, "{\"type\":\"cancel_session\"}")?;
        let _ = recv(&mut s);
        return Ok(false);
    }
    send(
        &mut s,
        "{\"type\":\"start_session\",\"cmd\":[\"/usr/bin/redrob-session\"],\
         \"env\":[\"XDG_SESSION_TYPE=wayland\",\"XDG_CURRENT_DESKTOP=Redrob\",\"XDG_SESSION_DESKTOP=redrob\"]}",
    )?;
    let reply = recv(&mut s)?;
    if kind(&reply) != "success" {
        tracing::warn!(%reply, "greeter: start_session");
        return Ok(false);
    }
    let _ = std::fs::write(last_file(), user);
    Ok(true)
}
