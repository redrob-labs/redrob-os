//! Redrob OS display module -- kiosk mode.
//!
//! The dashboard is drawn into a plain XRGB8888 pixel buffer with tiny-skia; `main.rs`
//! puts that buffer on the screen through KMS dumb buffers. Everything here is pure so
//! the frame can be rendered to a file in tests and on a host without a GPU.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use fontdue::Font;
use serde::Deserialize;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Rect, Transform};

/// `/run/redrob-pairing/pairing.json`, written by `redrob-pairing`.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Pairing {
    pub device: String,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub addresses: Vec<String>,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub code: Option<String>,
}

impl Pairing {
    pub fn load(path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Same URI `redrob-pairing` encodes in its QR, so both screens agree.
    pub fn uri(&self) -> Option<String> {
        let code = self.code.as_deref()?;
        let host = self.addresses.first().cloned().unwrap_or_default();
        Some(format!(
            "redrob://pair?device={}&host={}:{}&code={}",
            self.device, host, self.port, code
        ))
    }
}

/// One line of the unit table.
#[derive(Debug, Clone, PartialEq)]
pub struct UnitState {
    pub unit: String,
    pub state: String,
}

/// Everything the dashboard shows. Built by `Status::collect` on the device, by hand in
/// tests.
#[derive(Debug, Clone, Default)]
pub struct Status {
    pub os_name: String,
    pub os_version: String,
    pub pairing: Option<Pairing>,
    pub units: Vec<UnitState>,
    pub input_devices: usize,
    pub uptime_secs: u64,
}

pub const WATCHED_UNITS: &[&str] = &[
    "redrob-agent.service",
    "redrob-broker.service",
    "redrob-usb-broker.service",
    "redrob-pairing.timer",
    "rauc.service",
];

impl Status {
    pub fn collect(pairing_path: &Path, input_devices: usize) -> Self {
        let (os_name, os_version) = os_release(Path::new("/etc/os-release"));
        Status {
            os_name,
            os_version,
            pairing: Pairing::load(pairing_path),
            units: WATCHED_UNITS
                .iter()
                .map(|u| UnitState {
                    unit: u.to_string(),
                    state: unit_state(u),
                })
                .collect(),
            input_devices,
            uptime_secs: std::fs::read_to_string("/proc/uptime")
                .ok()
                .and_then(|s| s.split('.').next()?.parse().ok())
                .unwrap_or(0),
        }
    }
}

pub fn os_release(path: &Path) -> (String, String) {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let get = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
            .map(|v| v.trim().trim_matches('"').to_string())
            .unwrap_or_default()
    };
    (get("NAME"), get("VERSION"))
}

fn unit_state(unit: &str) -> String {
    Command::new("systemctl")
        .args(["is-active", unit])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

// Design-system-ish palette; the frame is black-on-dark so it also reads on a TV.
#[derive(Clone, Copy)]
struct Rgb(u8, u8, u8);
const BG: Rgb = Rgb(0x0b, 0x0d, 0x12);
const PANEL: Rgb = Rgb(0x15, 0x19, 0x22);
const FG: Rgb = Rgb(0xe6, 0xe8, 0xee);
const DIM: Rgb = Rgb(0x8a, 0x91, 0xa0);
const ACCENT: Rgb = Rgb(0xc1, 0x62, 0xf4);
const OK: Rgb = Rgb(0x3d, 0xd6, 0x8c);
const BAD: Rgb = Rgb(0xff, 0x5c, 0x5c);
const WHITE: Rgb = Rgb(0xff, 0xff, 0xff);
const BLACK: Rgb = Rgb(0, 0, 0);

impl From<Rgb> for Color {
    fn from(c: Rgb) -> Color {
        Color::from_rgba8(c.0, c.1, c.2, 0xff)
    }
}

pub struct Renderer {
    font: Font,
}

impl Renderer {
    pub fn new(font_bytes: &[u8]) -> Result<Self> {
        let font = Font::from_bytes(font_bytes, fontdue::FontSettings::default())
            .map_err(|e| anyhow::anyhow!("font: {e}"))?;
        Ok(Renderer { font })
    }

    pub fn from_file(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).with_context(|| format!("read font {}", path.display()))?;
        Self::new(&bytes)
    }

    /// Render one frame. `w`/`h` are the mode's size.
    pub fn render(&self, st: &Status, w: u32, h: u32) -> Pixmap {
        let mut pm = Pixmap::new(w.max(1), h.max(1)).expect("pixmap");
        pm.fill(BG.into());
        let margin = (w.min(h) as f32 * 0.05).max(16.0);
        let base = (h as f32 / 36.0).clamp(12.0, 40.0); // body text size

        // Header
        let title = if st.os_name.is_empty() {
            "Redrob OS"
        } else {
            &st.os_name
        };
        self.text(&mut pm, title, margin, margin + base * 1.6, base * 1.8, FG);
        self.text(
            &mut pm,
            &st.os_version,
            margin,
            margin + base * 3.0,
            base * 0.9,
            DIM,
        );
        fill_rect(
            &mut pm,
            margin,
            margin + base * 3.6,
            w as f32 - 2.0 * margin,
            3.0,
            ACCENT,
        );

        // Left panel: units
        let top = margin + base * 4.6;
        let col_w = (w as f32 - 3.0 * margin) / 2.0;
        fill_rect(&mut pm, margin, top, col_w, h as f32 - top - margin, PANEL);
        self.text(
            &mut pm,
            "Services",
            margin + base,
            top + base * 1.6,
            base * 1.1,
            DIM,
        );
        let mut y = top + base * 3.4;
        for u in &st.units {
            let good = u.state == "active";
            circle(
                &mut pm,
                margin + base * 1.5,
                y - base * 0.35,
                base * 0.3,
                if good { OK } else { BAD },
            );
            self.text(&mut pm, &u.unit, margin + base * 2.4, y, base, FG);
            self.text(
                &mut pm,
                &u.state,
                margin + col_w - base * 6.0,
                y,
                base,
                if good { OK } else { BAD },
            );
            y += base * 1.6;
        }
        y += base * 0.8;
        self.text(
            &mut pm,
            &format!("input devices held: {}", st.input_devices),
            margin + base,
            y,
            base * 0.9,
            DIM,
        );
        y += base * 1.4;
        self.text(
            &mut pm,
            &format!("up {}", human_uptime(st.uptime_secs)),
            margin + base,
            y,
            base * 0.9,
            DIM,
        );

        // Right panel: pairing
        let rx = margin * 2.0 + col_w;
        fill_rect(&mut pm, rx, top, col_w, h as f32 - top - margin, PANEL);
        match &st.pairing {
            Some(p) => {
                let paired = p.code.is_none();
                self.text(
                    &mut pm,
                    if paired { "Paired" } else { "Pair this device" },
                    rx + base,
                    top + base * 1.6,
                    base * 1.1,
                    DIM,
                );
                let mut py = top + base * 3.4;
                for (k, v) in [
                    ("device", p.device.clone()),
                    ("host", p.hostname.clone()),
                    (
                        "address",
                        format!(
                            "{}:{}",
                            p.addresses.first().cloned().unwrap_or_else(|| "-".into()),
                            p.port
                        ),
                    ),
                ] {
                    self.text(&mut pm, k, rx + base, py, base * 0.9, DIM);
                    self.text(&mut pm, &v, rx + base * 5.0, py, base * 0.9, FG);
                    py += base * 1.4;
                }
                if let Some(code) = &p.code {
                    py += base * 0.4;
                    self.text(&mut pm, "code", rx + base, py, base * 0.9, DIM);
                    self.text(&mut pm, code, rx + base * 5.0, py, base * 1.0, ACCENT);
                    py += base * 1.8;
                    if let Some(uri) = p.uri() {
                        let avail = (h as f32 - margin - py - base).min(col_w - 2.0 * base);
                        qr(&mut pm, &uri, rx + base, py, avail.max(base * 4.0));
                    }
                }
            }
            None => {
                self.text(
                    &mut pm,
                    "Pairing",
                    rx + base,
                    top + base * 1.6,
                    base * 1.1,
                    DIM,
                );
                self.text(
                    &mut pm,
                    "waiting for the agent...",
                    rx + base,
                    top + base * 3.4,
                    base,
                    FG,
                );
            }
        }
        pm
    }

    fn text(&self, pm: &mut Pixmap, s: &str, x: f32, baseline: f32, px: f32, color: Rgb) {
        let mut cx = x;
        let (r, g, b) = (
            color.0 as f32 / 255.0,
            color.1 as f32 / 255.0,
            color.2 as f32 / 255.0,
        );
        let w = pm.width() as i32;
        let h = pm.height() as i32;
        for ch in s.chars() {
            let (metrics, bitmap) = self.font.rasterize(ch, px);
            let ox = cx.round() as i32 + metrics.xmin;
            let oy = baseline.round() as i32 - metrics.ymin - metrics.height as i32;
            let data = pm.pixels_mut();
            for row in 0..metrics.height {
                let yy = oy + row as i32;
                if yy < 0 || yy >= h {
                    continue;
                }
                for col in 0..metrics.width {
                    let xx = ox + col as i32;
                    if xx < 0 || xx >= w {
                        continue;
                    }
                    let a = bitmap[row * metrics.width + col] as f32 / 255.0;
                    if a <= 0.0 {
                        continue;
                    }
                    let idx = (yy * w + xx) as usize;
                    let dst = data[idx];
                    let mix =
                        |d: u8, s: f32| ((d as f32 * (1.0 - a)) + s * 255.0 * a).round() as u8;
                    data[idx] = tiny_skia::PremultipliedColorU8::from_rgba(
                        mix(dst.red(), r),
                        mix(dst.green(), g),
                        mix(dst.blue(), b),
                        255,
                    )
                    .unwrap_or(dst);
                }
            }
            cx += metrics.advance_width;
        }
    }
}

fn fill_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: Rgb) {
    if let Some(r) = Rect::from_xywh(x, y, w.max(0.0), h.max(0.0)) {
        let mut paint = Paint::default();
        paint.set_color(color.into());
        pm.fill_rect(r, &paint, Transform::identity(), None);
    }
}

fn circle(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, color: Rgb) {
    if let Some(path) = PathBuilder::from_circle(cx, cy, r) {
        let mut paint = Paint::default();
        paint.set_color(color.into());
        paint.anti_alias = true;
        pm.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

/// QR with a white quiet zone, scaled to `size` px.
fn qr(pm: &mut Pixmap, data: &str, x: f32, y: f32, size: f32) {
    let Ok(code) = qrcodegen::QrCode::encode_text(data, qrcodegen::QrCodeEcc::Medium) else {
        return;
    };
    let n = code.size();
    let quiet = 2;
    let (x, y) = (x.round(), y.round());
    let cell = (size / (n + 2 * quiet) as f32).floor().max(1.0);
    let total = cell * (n + 2 * quiet) as f32;
    fill_rect(pm, x, y, total, total, WHITE);
    for r in 0..n {
        for c in 0..n {
            if code.get_module(c, r) {
                fill_rect(
                    pm,
                    x + (c + quiet) as f32 * cell,
                    y + (r + quiet) as f32 * cell,
                    cell,
                    cell,
                    BLACK,
                );
            }
        }
    }
}

pub fn human_uptime(secs: u64) -> String {
    let (d, h, m) = (secs / 86400, (secs % 86400) / 3600, (secs % 3600) / 60);
    if d > 0 {
        format!("{d}d {h}h {m}m")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}

/// Copy a premultiplied RGBA pixmap into an XRGB8888 scanout buffer with `pitch` bytes per row.
pub fn blit_xrgb(pm: &Pixmap, dst: &mut [u8], pitch: usize) {
    let w = pm.width() as usize;
    for (row, px_row) in pm.pixels().chunks(w).enumerate() {
        let start = row * pitch;
        let Some(line) = dst.get_mut(start..start + w * 4) else {
            break;
        };
        for (i, p) in px_row.iter().enumerate() {
            line[i * 4] = p.blue();
            line[i * 4 + 1] = p.green();
            line[i * 4 + 2] = p.red();
            line[i * 4 + 3] = 0;
        }
    }
}

/// Binary PPM (P6), for the harness screendump comparison and for host-side tests.
pub fn write_ppm(pm: &Pixmap, path: &Path) -> Result<()> {
    let mut out = format!("P6\n{} {}\n255\n", pm.width(), pm.height()).into_bytes();
    for p in pm.pixels() {
        out.extend_from_slice(&[p.red(), p.green(), p.blue()]);
    }
    std::fs::write(path, out).with_context(|| format!("write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FONT: &str = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";

    fn sample() -> Status {
        Status {
            os_name: "Redrob OS".into(),
            os_version: "0.1.dev3 (Generic x86-64)".into(),
            pairing: Some(Pairing {
                device: "17af20c4-895b-4b74-baf5-3832c6396f78".into(),
                hostname: "redrob-17af20".into(),
                addresses: vec!["10.0.2.15".into()],
                port: 42617,
                code: Some("uH61jhvOSeKDLpKm7agj8JQwI7vJaVal".into()),
            }),
            units: vec![
                UnitState {
                    unit: "redrob-agent.service".into(),
                    state: "active".into(),
                },
                UnitState {
                    unit: "redrob-broker.service".into(),
                    state: "failed".into(),
                },
            ],
            input_devices: 2,
            uptime_secs: 3725,
        }
    }

    #[test]
    fn pairing_json_parses_and_uri_matches_pairing_tool() {
        let p: Pairing = serde_json::from_str(
            r#"{"device":"d","hostname":"h","addresses":["10.0.2.15"],"port":42617,"code":"abc"}"#,
        )
        .unwrap();
        assert_eq!(
            p.uri().unwrap(),
            "redrob://pair?device=d&host=10.0.2.15:42617&code=abc"
        );
        let paired: Pairing =
            serde_json::from_str(r#"{"device":"d","addresses":[],"port":1,"code":null}"#).unwrap();
        assert!(paired.uri().is_none());
    }

    #[test]
    fn frame_has_header_bar_panels_and_qr() {
        let Ok(r) = Renderer::from_file(Path::new(FONT)) else {
            eprintln!("font missing, skipping");
            return;
        };
        let pm = r.render(&sample(), 1280, 800);
        // Accent header rule is drawn full width at y ~= margin + 3.6*base.
        let base: f32 = (800.0 / 36.0f32).clamp(12.0, 40.0);
        let margin = 40.0;
        let y = (margin + base * 3.6 + 1.0) as u32;
        let px = pm.pixel(640, y).unwrap();
        assert_eq!((px.red(), px.green(), px.blue()), (0xc1, 0x62, 0xf4));
        // The QR has a white quiet zone: there is pure white in the right panel.
        let white = pm
            .pixels()
            .iter()
            .filter(|p| p.red() == 255 && p.green() == 255 && p.blue() == 255)
            .count();
        assert!(white > 500, "quiet zone missing: {white} white px");
        // And the frame is not mostly background.
        let bg = pm
            .pixels()
            .iter()
            .filter(|p| p.red() == 0x0b && p.blue() == 0x12)
            .count();
        assert!(bg < (1280 * 800) as usize * 9 / 10);
    }

    #[test]
    fn blit_swaps_to_bgrx_and_respects_pitch() {
        let mut pm = Pixmap::new(2, 1).unwrap();
        pm.fill(Color::from_rgba8(0x11, 0x22, 0x33, 0xff));
        let mut dst = vec![0xaau8; 16];
        blit_xrgb(&pm, &mut dst, 16);
        assert_eq!(&dst[..8], &[0x33, 0x22, 0x11, 0, 0x33, 0x22, 0x11, 0]);
        assert_eq!(&dst[8..], &[0xaa; 8]);
    }

    #[test]
    fn ppm_roundtrip_header() {
        let pm = Pixmap::new(3, 2).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f.ppm");
        write_ppm(&pm, &p).unwrap();
        let bytes = std::fs::read(&p).unwrap();
        assert!(bytes.starts_with(b"P6\n3 2\n255\n"));
        assert_eq!(bytes.len(), "P6\n3 2\n255\n".len() + 3 * 2 * 3);
    }

    #[test]
    fn uptime_text() {
        assert_eq!(human_uptime(59), "0m");
        assert_eq!(human_uptime(3725), "1h 2m");
        assert_eq!(human_uptime(90061), "1d 1h 1m");
    }
}
