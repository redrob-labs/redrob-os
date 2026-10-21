//! The output's resolution and scale, as the settings window shows and
//! changes them. The DRM backend fills in the modes and applies requests;
//! everything that places elements reads `scale()`.

use std::sync::atomic::{AtomicU64, Ordering};

static SCALE: AtomicU64 = AtomicU64::new(0x3FF0_0000_0000_0000); // 1.0

/// Output scale (logical -> physical), 1.0 by default.
pub fn scale() -> f64 {
    f64::from_bits(SCALE.load(Ordering::Relaxed))
}

pub fn set_scale(s: f64) {
    SCALE.store(s.clamp(0.5, 3.0).to_bits(), Ordering::Relaxed);
}

pub const SCALES: [f64; 5] = [1.0, 1.25, 1.5, 1.75, 2.0];

pub enum Request {
    /// Switch the main screen to mode i (asks to keep it).
    Mode(usize),
    /// Go back to mode i (not kept, or the prompt timed out).
    Revert(usize),
    Scale(f64),
}

#[derive(Default)]
pub struct Display {
    /// Connector name ("Virtual-1", "eDP-1").
    pub name: String,
    /// Distinct resolutions, largest first: (width, height, refresh mHz).
    pub modes: Vec<(i32, i32, i32)>,
    pub current: usize,
    pub request: Option<Request>,
    /// Every connected screen, main first ("Virtual-1 1920×1080").
    pub screens: Vec<String>,
}
