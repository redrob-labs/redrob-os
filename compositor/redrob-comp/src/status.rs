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
