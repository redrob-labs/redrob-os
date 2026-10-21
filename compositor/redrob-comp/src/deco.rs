//! Server-side window frames (xdg-decoration), in the shell's style:
//! a soft shadow, a 1px border-subtle outline, and a title bar on the
//! surface-raised tone with rounded top corners (radius-lg).
//!
//! - Mac: close / minimize / zoom dots at the left (status danger / warning /
//!   success; neutral when the window is inactive), title centred.
//! - Windows: title at the left, minimize / maximize / close at the right;
//!   close turns status-danger under the pointer.
//!
//! Only windows that asked for server-side decorations get a frame, so GTK
//! apps that draw their own (CSD) are left alone.

use smithay::{
    backend::renderer::{
        element::{
            Kind,
            memory::{MemoryRenderBuffer, MemoryRenderBufferRenderElement},
        },
        gles::GlesRenderer,
    },
    desktop::Window,
    reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode as DecoMode,
    utils::{IsAlive, Logical, Point, Rectangle},
    wayland::{compositor::with_states, shell::xdg::XdgToplevelSurfaceData},
};
use tiny_skia::Pixmap;

use crate::{
    config::Style,
    draw::{self, fill_circle, fill_rect, line, upload, with_alpha},
    state::Smallvil,
    theme::{self, Mode, TITLE_H},
};

/// Shadow room around a frame, in logical pixels.
const SHADOW: i32 = 28;
const MAC_DOTS: [i32; 3] = [20, 40, 60]; // dot centres from the frame's left
const WIN_BTN: i32 = 46;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecoHit {
    Close,
    Minimize,
    Maximize,
    Title,
}

#[derive(Clone, PartialEq, Eq)]
struct Key {
    size: (i32, i32),
    focused: bool,
    mode: Mode,
    style: Style,
    title: String,
    hover: Option<DecoHit>,
}

#[derive(Default)]
pub struct Decorations {
    cache: Vec<(Window, Key, MemoryRenderBuffer)>,
    /// The frame control under the pointer, for hover states.
    pub hover: Option<(Window, DecoHit)>,
}

pub fn wants_frame(w: &Window) -> bool {
    w.toplevel().is_some_and(|t| t.with_pending_state(|s| s.decoration_mode) == Some(DecoMode::ServerSide))
}

fn title_of(w: &Window) -> String {
    let Some(t) = w.toplevel() else { return String::new() };
    with_states(t.wl_surface(), |states| {
        states
            .data_map
            .get::<XdgToplevelSurfaceData>()
            .and_then(|d| d.lock().ok().and_then(|a| a.title.clone().or_else(|| a.app_id.clone())))
            .unwrap_or_default()
    })
}

/// A friendly app name from the toplevel's app id ("org.kde.konsole" -> "Konsole").
pub fn app_name(w: &Window) -> String {
    let Some(t) = w.toplevel() else { return String::new() };
    let id = with_states(t.wl_surface(), |states| {
        states.data_map.get::<XdgToplevelSurfaceData>().and_then(|d| d.lock().ok().and_then(|a| a.app_id.clone()))
    })
    .unwrap_or_default();
    let last = id.rsplit('.').next().unwrap_or(&id).replace(['-', '_'], " ");
    let mut c = last.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

impl Smallvil {
    /// The window's content rectangle in output coordinates.
    pub fn content_rect(&self, w: &Window) -> Option<Rectangle<i32, Logical>> {
        let loc = self.space.element_location(w)?;
        let geo = w.geometry();
        Some(Rectangle::new(loc + geo.loc, geo.size))
    }

    pub fn is_focused(&self, w: &Window) -> bool {
        let focus = self.seat.get_keyboard().and_then(|k| k.current_focus());
        match (focus, w.toplevel()) {
            (Some(f), Some(t)) => &f == t.wl_surface(),
            _ => false,
        }
    }

    /// Which frame control is at `p`, top-most window first. A window's
    /// content above a lower window's title bar wins, as it is on screen.
    pub fn deco_hit(&self, p: Point<f64, Logical>) -> Option<(Window, DecoHit)> {
        let p = p.to_i32_round();
        for w in self.space.elements().rev() {
            let Some(c) = self.content_rect(w) else { continue };
            if c.contains(p) {
                return None;
            }
            if !wants_frame(w) {
                continue;
            }
            let bar = Rectangle::new((c.loc.x, c.loc.y - TITLE_H).into(), (c.size.w, TITLE_H).into());
            if !bar.contains(p) {
                continue;
            }
            let (lx, ly) = (p.x - bar.loc.x, p.y - bar.loc.y);
            let hit = match self.chrome.style {
                Style::Mac => {
                    let near = |cx: i32| (lx - cx).abs() <= 8 && (ly - TITLE_H / 2).abs() <= 8;
                    if near(MAC_DOTS[0]) {
                        DecoHit::Close
                    } else if near(MAC_DOTS[1]) {
                        DecoHit::Minimize
                    } else if near(MAC_DOTS[2]) {
                        DecoHit::Maximize
                    } else {
                        DecoHit::Title
                    }
                }
                Style::Windows => match (bar.size.w - lx) / WIN_BTN {
                    0 => DecoHit::Close,
                    1 => DecoHit::Maximize,
                    2 => DecoHit::Minimize,
                    _ => DecoHit::Title,
                },
            };
            return Some((w.clone(), hit));
        }
        None
    }

    /// The frame element for `w` (drawn behind its surfaces), if it has one.
    pub fn frame_element(
        &mut self,
        renderer: &mut GlesRenderer,
        w: &Window,
    ) -> Option<MemoryRenderBufferRenderElement<GlesRenderer>> {
        if !wants_frame(w) {
            return None;
        }
        let c = self.content_rect(w)?;
        let key = Key {
            size: (c.size.w, c.size.h),
            focused: self.is_focused(w),
            mode: self.chrome.mode,
            style: self.chrome.style,
            title: title_of(w),
            hover: self.deco.hover.as_ref().filter(|(hw, _)| hw == w).map(|(_, h)| *h),
        };
        self.deco.cache.retain(|(cw, _, _)| cw.alive());
        let fresh = self.deco.cache.iter().position(|(cw, k, _)| cw == w && *k == key);
        let idx = match fresh {
            Some(i) => i,
            None => {
                let buf = paint_frame(&mut self.chrome, &key)?;
                self.deco.cache.retain(|(cw, _, _)| cw != w);
                self.deco.cache.push((w.clone(), key, buf));
                self.deco.cache.len() - 1
            }
        };
        let at = Point::<i32, Logical>::from((c.loc.x - SHADOW, c.loc.y - TITLE_H - SHADOW));
        MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            at.to_f64().to_physical(1.0),
            &self.deco.cache[idx].2,
            None,
            None,
            None,
            Kind::Unspecified,
        )
        .ok()
    }
}

fn paint_frame(chrome: &mut crate::chrome::Chrome, k: &Key) -> Option<MemoryRenderBuffer> {
    let t = theme::theme(k.mode);
    let (cw, ch) = k.size;
    let (fw, fh) = (cw as f32, (ch + TITLE_H) as f32);
    let s = SHADOW as f32;
    let mut p = Pixmap::new((cw + 2 * SHADOW) as u32, (ch + TITLE_H + 2 * SHADOW) as u32)?;
    let r = theme::RADIUS_LG;

    // Shadow: deeper on the window in front.
    let sc = if k.focused { t.shadow_lg } else { t.shadow_md };
    draw::shadow(&mut p, s, s, fw, fh, r, if k.focused { 26.0 } else { 14.0 }, if k.focused { 10.0 } else { 4.0 }, sc);

    // Body (under the client, in case it is smaller than its geometry) and bar.
    if let Some(path) = draw::rrect_path(s, s, fw, fh, r, true) {
        p.fill_path(&path, &draw::paint(t.surface_base), tiny_skia::FillRule::Winding, tiny_skia::Transform::identity(), None);
    }
    let bar = if k.focused { t.surface_raised } else { t.surface_base };
    if let Some(path) = draw::rrect_path(s, s, fw, TITLE_H as f32, r, true) {
        p.fill_path(&path, &draw::paint(bar), tiny_skia::FillRule::Winding, tiny_skia::Transform::identity(), None);
    }
    fill_rect(&mut p, s, s + TITLE_H as f32 - 1.0, fw, 1.0, t.border_subtle);
    if let Some(path) = draw::rrect_path(s - 0.5, s - 0.5, fw + 1.0, fh + 1.0, r + 0.5, true) {
        let st = tiny_skia::Stroke { width: 1.0, ..Default::default() };
        p.stroke_path(&path, &draw::paint(t.border_subtle), &st, tiny_skia::Transform::identity(), None);
    }

    let mid = s + TITLE_H as f32 / 2.0;
    let ink = if k.focused { t.ink_primary } else { t.ink_muted };
    let font = &chrome.fonts.semibold;
    match k.style {
        Style::Mac => {
            let colors = [t.danger, t.warning, t.success];
            for (i, cx) in MAC_DOTS.iter().enumerate() {
                let c = if k.focused || k.hover.is_some() { colors[i] } else { with_alpha(t.border_strong, 110) };
                fill_circle(&mut p, s + *cx as f32, mid, 6.0, c);
                // Glyphs inside the dots while the pointer is over them.
                if k.hover.is_some_and(|h| h != DecoHit::Title) {
                    let g = Rgba3::dark();
                    let x = s + *cx as f32;
                    match i {
                        0 => {
                            line(&mut p, x - 2.5, mid - 2.5, x + 2.5, mid + 2.5, 1.3, g);
                            line(&mut p, x - 2.5, mid + 2.5, x + 2.5, mid - 2.5, 1.3, g);
                        }
                        1 => line(&mut p, x - 3.0, mid, x + 3.0, mid, 1.3, g),
                        _ => {
                            line(&mut p, x - 3.0, mid, x + 3.0, mid, 1.3, g);
                            line(&mut p, x, mid - 3.0, x, mid + 3.0, 1.3, g);
                        }
                    }
                }
            }
            let left = s + MAC_DOTS[2] as f32 + 16.0;
            let title = draw::fit_text(font, &k.title, 13.0, (fw - 2.0 * (left - s)).max(0.0));
            let tw = draw::text_width(font, &title, 13.0);
            draw::draw_text(&mut p, font, &title, s + (fw - tw) / 2.0, mid + 4.5, 13.0, ink);
        }
        Style::Windows => {
            let btns = 3 * WIN_BTN;
            let title = draw::fit_text(font, &k.title, 13.0, (fw - btns as f32 - 28.0).max(0.0));
            draw::draw_text(&mut p, font, &title, s + 14.0, mid + 4.5, 13.0, ink);
            let bx = |i: i32| s + fw - ((i + 1) * WIN_BTN) as f32; // 0 = close
            let bw = WIN_BTN as f32;
            let th = TITLE_H as f32 - 1.0;
            match k.hover {
                Some(DecoHit::Close) => {
                    // Fill the close button to the frame's rounded corner.
                    if let Some(path) = draw::rrect_path(bx(0), s, bw, th, r, true) {
                        p.fill_path(&path, &draw::paint(t.danger), tiny_skia::FillRule::Winding, tiny_skia::Transform::identity(), None);
                        fill_rect(&mut p, bx(0), s, bw - r, th, t.danger);
                    }
                }
                Some(DecoHit::Maximize) => fill_rect(&mut p, bx(1), s, bw, th, t.hover),
                Some(DecoHit::Minimize) => fill_rect(&mut p, bx(2), s, bw, th, t.hover),
                _ => {}
            }
            let close_ink = if k.hover == Some(DecoHit::Close) { theme::hexa(0xffffff) } else { ink };
            let cx = bx(0) + bw / 2.0;
            line(&mut p, cx - 5.0, mid - 5.0, cx + 5.0, mid + 5.0, 1.0, close_ink);
            line(&mut p, cx - 5.0, mid + 5.0, cx + 5.0, mid - 5.0, 1.0, close_ink);
            let mx = bx(1) + bw / 2.0;
            draw::stroke_rrect(&mut p, mx - 5.0, mid - 5.0, 10.0, 10.0, 1.5, ink);
            let nx = bx(2) + bw / 2.0;
            line(&mut p, nx - 5.0, mid + 0.5, nx + 5.0, mid + 0.5, 1.0, ink);
        }
    }
    Some(upload(&p))
}

/// Glyph colour inside the Mac dots: Redrob Black at 60%.
struct Rgba3;
impl Rgba3 {
    fn dark() -> theme::Rgba {
        theme::Rgba(10, 11, 12, 153)
    }
}
