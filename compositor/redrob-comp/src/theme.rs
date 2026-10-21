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

pub const fn hexa(v: u32) -> Rgba {
    hex(v)
}

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
    pub ink_muted: Rgba,
    pub surface_sunken: Rgba,
    pub border_strong: Rgba,
    /// --action-primary: Redrob Blue.
    pub action_primary: Rgba,
    /// Drop shadow colour of shadow-md / shadow-lg (alpha is the token's).
    pub shadow_md: Rgba,
    pub shadow_lg: Rgba,
    /// Hover wash on chrome controls (ink at low alpha).
    pub hover: Rgba,
    /// status-danger / -warning / -success base colours (window dots).
    pub danger: Rgba,
    pub warning: Rgba,
    pub success: Rgba,
    /// The desktop ground gradient for this theme (130deg stops).
    pub ground: &'static [(f32, u32)],
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
            ink_muted: hex(0x686e78),
            surface_sunken: hex(0xeff1f4),
            border_strong: hex(0x7c8390),
            action_primary: hex(0x2b52ff),
            shadow_md: Rgba(10, 11, 12, 26),
            shadow_lg: Rgba(10, 11, 12, 36),
            hover: Rgba(10, 11, 12, 15),
            danger: hex(0xff4b4b),
            warning: hex(0xff9c1b),
            success: hex(0x29e474),
            // --gradient-light: brand blue cooling into tint.
            ground: &[(0.0, 0x8aafff), (0.55, 0xd9e6ff), (1.0, 0xeef4ff)],
        },
        Mode::Dark => Theme {
            surface_base: hex(0x0a0b0c),
            surface_raised: hex(0x141719),
            surface_material: Rgba(20, 23, 25, (0.86 * 255.0) as u8),
            border_subtle: hex(0x292e37),
            ink_primary: hex(0xf8f9fb),
            ink_secondary: hex(0xaab0bb),
            ink_muted: hex(0x838a97),
            surface_sunken: hex(0x1c1f26),
            border_strong: hex(0x7c8390),
            action_primary: hex(0x2b52ff),
            shadow_md: Rgba(0, 0, 0, 178),
            shadow_lg: Rgba(0, 0, 0, 204),
            hover: Rgba(248, 249, 251, 20),
            danger: hex(0xff4b4b),
            warning: hex(0xff9c1b),
            success: hex(0x29e474),
            // Fallback when the Redrob wallpaper is missing: the deep ground.
            ground: &[(0.0, 0x0a0b0c), (0.7, 0x0a0b0c), (1.0, 0x061460)],
        },
    }
}

// Lengths (px), identical in both themes.
/// --control-height-sm: the top bar is one small control tall.
pub const BAR_H: i32 = 32;
/// --radius-md: dock slots (a control).
pub const RADIUS_SM: f32 = 6.0;
pub const RADIUS_MD: f32 = 8.0;
pub const RADIUS_LG: f32 = 12.0;
pub const RADIUS_XL_F: f32 = 16.0;
/// --radius-2xl: the dock material (a large floating surface).
pub const RADIUS_2XL: f32 = 24.0;
/// --radius-full: pills (running indicator).
pub const RADIUS_FULL: f32 = 9999.0;
/// --space-2 / --space-3 / --space-4.
/// --control-height-lg: one dock slot.
/// Server-side title bar height.
pub const TITLE_H: i32 = 36;
/// Windows-style taskbar height and its launcher slot.
pub const TASKBAR_H: i32 = 48;
pub const TASK_SLOT: i32 = 40;
