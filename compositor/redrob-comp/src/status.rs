//! System status for the bar / tray: network, volume, battery. Polled on a
//! background thread every few seconds; the shell reads the latest copy.

use std::{
    process::Command,
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    /// Some interface other than loopback is up.
    pub online: bool,
    /// Volume percent of the default sink, if PipeWire answers.
    pub volume: Option<u32>,
    pub muted: bool,
    /// Battery percent and whether it is charging, on machines with one.
    pub battery: Option<(u32, bool)>,
    /// Audio outputs and inputs (PipeWire nodes): id, name, is default.
    pub sinks: Vec<(u32, String, bool)>,
    pub sources: Vec<(u32, String, bool)>,
    /// Default input volume percent and mute.
    pub in_volume: Option<u32>,
    pub in_muted: bool,
}

pub type Shared = Arc<Mutex<Status>>;

pub fn start() -> Shared {
    let shared: Shared = Arc::default();
    let out = shared.clone();
    std::thread::spawn(move || {
        loop {
            let s = read();
            if let Ok(mut g) = out.lock() {
                *g = s;
            }
            std::thread::sleep(Duration::from_secs(3));
        }
    });
    shared
}

fn read() -> Status {
    let mut s = Status::default();
    if let Ok(rd) = std::fs::read_dir("/sys/class/net") {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name == "lo" {
                continue;
            }
            let state = std::fs::read_to_string(e.path().join("operstate")).unwrap_or_default();
            if state.trim() == "up" {
                s.online = true;
            }
        }
    }
    // "Volume: 0.40 [MUTED]"
    if let Ok(o) = Command::new("wpctl").args(["get-volume", "@DEFAULT_AUDIO_SINK@"]).output() {
        let t = String::from_utf8_lossy(&o.stdout);
        if let Some(v) = t.split_whitespace().nth(1).and_then(|v| v.parse::<f32>().ok()) {
            s.volume = Some((v * 100.0).round() as u32);
            s.muted = t.contains("MUTED");
        }
    }
    if let Ok(o) = Command::new("wpctl").args(["get-volume", "@DEFAULT_AUDIO_SOURCE@"]).output() {
        let t = String::from_utf8_lossy(&o.stdout);
        if let Some(v) = t.split_whitespace().nth(1).and_then(|v| v.parse::<f32>().ok()) {
            s.in_volume = Some((v * 100.0).round() as u32);
            s.in_muted = t.contains("MUTED");
        }
    }
    if let Ok(o) = Command::new("wpctl").arg("status").output() {
        let (sinks, sources) = parse_wpctl(&String::from_utf8_lossy(&o.stdout));
        s.sinks = sinks;
        s.sources = sources;
    }
    if let Ok(rd) = std::fs::read_dir("/sys/class/power_supply") {
        for e in rd.flatten() {
            let p = e.path();
            if std::fs::read_to_string(p.join("type")).unwrap_or_default().trim() != "Battery" {
                continue;
            }
            let cap = std::fs::read_to_string(p.join("capacity")).ok().and_then(|c| c.trim().parse().ok());
            let charging = std::fs::read_to_string(p.join("status")).unwrap_or_default().trim() == "Charging";
            if let Some(c) = cap {
                s.battery = Some((c, charging));
            }
        }
    }
    s
}

pub fn toggle_mute() {
    crate::shell::spawn(&["wpctl", "set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"]);
}

type Nodes = Vec<(u32, String, bool)>;

/// Sinks and sources from `wpctl status` (the Audio section):
/// ` │  *   47. Built-in Audio Analog Stereo   [vol: 0.40]`
fn parse_wpctl(t: &str) -> (Nodes, Nodes) {
    let (mut sinks, mut sources) = (Vec::new(), Vec::new());
    let mut in_audio = false;
    let mut list: Option<bool> = None; // Some(true) = sinks, Some(false) = sources
    for line in t.lines() {
        let trimmed = line.trim();
        if !line.starts_with(' ') && !line.starts_with('│') && !line.starts_with('├') && !line.starts_with('└') && !trimmed.is_empty() {
            in_audio = trimmed == "Audio";
            list = None;
            continue;
        }
        if !in_audio {
            continue;
        }
        let body = trimmed.trim_start_matches(['│', '├', '└', '─', ' ']);
        if body.starts_with("Sinks:") {
            list = Some(true);
            continue;
        }
        if body.starts_with("Sources:") {
            list = Some(false);
            continue;
        }
        if body.ends_with(':') {
            list = None;
            continue;
        }
        let Some(which) = list else { continue };
        let default = body.starts_with('*');
        let rest = body.trim_start_matches(['*', ' ']);
        let Some((id, name)) = rest.split_once('.') else { continue };
        let Ok(id) = id.trim().parse::<u32>() else { continue };
        let name = name.split('[').next().unwrap_or("").trim().to_string();
        if which { sinks.push((id, name, default)) } else { sources.push((id, name, default)) }
    }
    (sinks, sources)
}

pub fn set_volume(source: bool, pct: u32) {
    let target = if source { "@DEFAULT_AUDIO_SOURCE@" } else { "@DEFAULT_AUDIO_SINK@" };
    let v = format!("{:.2}", pct.min(150) as f32 / 100.0);
    crate::shell::spawn(&["wpctl", "set-volume", target, &v]);
}

pub fn toggle_mute_of(source: bool) {
    let target = if source { "@DEFAULT_AUDIO_SOURCE@" } else { "@DEFAULT_AUDIO_SINK@" };
    crate::shell::spawn(&["wpctl", "set-mute", target, "toggle"]);
}

pub fn set_default(id: u32) {
    crate::shell::spawn(&["wpctl", "set-default", &id.to_string()]);
}
