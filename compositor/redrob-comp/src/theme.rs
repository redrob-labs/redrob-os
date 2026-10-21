//! Redrob design tokens for the shell, light and dark.
//!
//! Values are copied from redrob-ui `src/styles/tokens.json` (generated from
//! `tokens.css`, redrob-ui commit 89fc3c9) -- the same numbers the web, the
//! browser chrome (`dist/native/redrob_tokens.h`) and Android get. Change them
//! there and re-copy; do not invent shell-only colours.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Light,
    Dark,
}

impl Mode {
    pub fn toggled(self) -> Self {
        match self {
            Mode::Light => Mode::Dark,
            Mode::Dark => Mode::Light,
        }
    }
}

/// RGBA, 0..=255, straight (not premultiplied) alpha.
#[derive(Clone, Copy, Debug)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

const fn hex(v: u32) -> Rgba {
    Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 0xFF)
}

pub struct Theme {
    /// --surface-base: the desktop ground behind every window.
    pub surface_base: Rgba,
    /// --surface-raised: tiles and hovered slots.
    pub surface_raised: Rgba,
    /// --surface-material: the translucent material of the bar and the dock.
    pub surface_material: Rgba,
    /// --border-subtle: hairline around the bar/dock material.
    pub border_subtle: Rgba,
    /// --ink-primary / --ink-secondary: text and icon strokes.
    pub ink_primary: Rgba,
    pub ink_secondary: Rgba,
    /// --action-primary: Redrob Blue; the running-app indicator and focus.
    pub action_primary: Rgba,
}

pub fn theme(mode: Mode) -> Theme {
    match mode {
        Mode::Light => Theme {
            surface_base: hex(0xffffff),
            surface_raised: hex(0xf8f9fb),
            surface_material: Rgba(255, 255, 255, (0.82 * 255.0) as u8),
            border_subtle: hex(0xdfe2e8),
            ink_primary: hex(0x0a0b0c),
            ink_secondary: hex(0x576071),
            action_primary: hex(0x2b52ff),
        },
        Mode::Dark => Theme {
            surface_base: hex(0x0a0b0c),
            surface_raised: hex(0x141719),
            surface_material: Rgba(20, 23, 25, (0.86 * 255.0) as u8),
            border_subtle: hex(0x292e37),
            ink_primary: hex(0xf8f9fb),
            ink_secondary: hex(0xaab0bb),
            action_primary: hex(0x2b52ff),
        },
    }
}

// Lengths (px), identical in both themes.
/// --control-height-sm: the top bar is one small control tall.
pub const BAR_H: i32 = 32;
/// --radius-md: dock slots (a control).
pub const RADIUS_MD: f32 = 8.0;
/// --radius-2xl: the dock material (a large floating surface).
pub const RADIUS_2XL: f32 = 24.0;
/// --radius-full: pills (running indicator).
pub const RADIUS_FULL: f32 = 9999.0;
/// --space-2 / --space-3 / --space-4.
pub const SPACE_2: i32 = 8;
pub const SPACE_3: i32 = 12;
pub const SPACE_4: i32 = 16;
/// --control-height-lg: one dock slot.
pub const SLOT: i32 = 48;
/// Windows-style taskbar height and its launcher slot.
pub const TASKBAR_H: i32 = 48;
pub const TASK_SLOT: i32 = 40;
pub const SPACE_1: i32 = 4;
