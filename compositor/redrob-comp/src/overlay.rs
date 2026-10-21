//! Full-screen shell layers above the desktop:
//! - Finder: Spotlight (Mac) / Start menu (Windows) app search.
//! - Switcher: Command-Tab / Alt-Tab, held modifier, release to pick.
//! - Expose: Mission Control / Task view, every window scaled into a grid.
//! - Lock: lock screen with a PAM password check.
//! - Region: drag a rectangle for a screenshot.
//! Plus notification toasts, which are not modal.

use std::{
    collections::HashMap,
    sync::mpsc::{Receiver, channel},
    time::{Duration, Instant},
};

use smithay::{
    backend::renderer::{
        element::{
            AsRenderElements, Kind,
            memory::MemoryRenderBufferRenderElement,
            surface::WaylandSurfaceRenderElement,
        },
        gles::GlesRenderer,
    },
    desktop::Window,
    input::keyboard::{KeysymHandle, ModifiersState, keysyms},
    utils::{IsAlive, Logical, Physical, Point, Rectangle, Scale, Size},
};
use tiny_skia::Pixmap;

use crate::{
    apps,
    config::Style,
    draw::{self, fill_circle, fill_rect, fill_rrect, stroke_rrect, upload, with_alpha},
    notify::Note,
    state::Smallvil,
    theme::{self, Mode, Rgba},
};

const ICON_SEARCH: &str = include_str!("../assets/icons/search.svg");
const ICON_BELL: &str = include_str!("../assets/icons/bell.svg");
const ICON_CAMERA: &str = include_str!("../assets/icons/camera.svg");
const ICON_LOCK: &str = include_str!("../assets/icons/lock.svg");

pub enum Overlay {
    Finder { query: String, sel: usize, top: usize, matches: Vec<usize>, comp: crate::hangul::Composer },
    Settings { page: usize, capture: Option<crate::config::Act> },
    Switcher { items: Vec<Window>, sel: usize },
    Expose { slots: Vec<(Window, Rectangle<i32, Logical>, f64)>, hover: Option<usize> },
    Lock { input: String, error: bool, busy: Option<Receiver<bool>> },
    Region { start: Option<Point<i32, Logical>>, cur: Point<i32, Logical> },
}

pub struct Toast {
    pub note: Note,
    pub until: Instant,
}

/// Overlay state that lives on the compositor.
#[derive(Default)]
pub struct Layers {
    pub overlay: Option<Overlay>,
    pub dirty: bool,
    buf: Option<draw::Buf>,
    pub hits: Vec<(Rectangle<i32, Logical>, usize)>,
    /// Settings volume sliders (id, track) and the one being dragged.
    pub sliders: Vec<(usize, Rectangle<i32, Logical>)>,
    pub drag: Option<usize>,
    minute: u32,
    pub apps: Vec<apps::App>,
    icons: HashMap<(String, u32), Option<Pixmap>>,
    pub toasts: Vec<Toast>,
    /// The finder types Hangul (toggled with 한/영, right Alt or Shift+Space).
    pub ko: bool,
    toast_buf: Option<(draw::Buf, Point<i32, Logical>, Vec<(Rectangle<i32, Logical>, u32)>)>,
    toast_dirty: bool,
}

const FINDER_W: i32 = 640;
const ROW_H: i32 = 44;
const ROWS: usize = 8;
const FIELD_H: i32 = 56;
const TOAST_W: i32 = 360;
const TOAST_H: i32 = 76;

impl Layers {
    pub fn invalidate(&mut self) {
        self.dirty = true;
        self.toast_dirty = true;
        self.icons.clear();
    }

    /// `px` is logical; the icon is rendered at the output scale.
    fn icon(&mut self, name: &str, px: u32) -> Option<&Pixmap> {
        let px = (px as f32 * draw::k()).round() as u32;
        let key = (name.to_string(), px);
        if !self.icons.contains_key(&key) {
            let img = apps::icon(name, px);
            self.icons.insert(key.clone(), img);
        }
        self.icons.get(&key).and_then(|o| o.as_ref())
    }

    /// The icon for a window, via its app id's desktop entry.
    fn window_icon(&mut self, w: &Window, px: u32) -> Option<Pixmap> {
        let id = app_id(w);
        let name = apps::for_app_id(&self.apps, &id).and_then(|a| a.icon.clone()).unwrap_or(id);
        self.icon(&name, px).cloned()
    }
}

pub fn app_id(w: &Window) -> String {
    use smithay::wayland::{compositor::with_states, shell::xdg::XdgToplevelSurfaceData};
    let Some(t) = w.toplevel() else { return String::new() };
    with_states(t.wl_surface(), |s| {
        s.data_map.get::<XdgToplevelSurfaceData>().and_then(|d| d.lock().ok().and_then(|a| a.app_id.clone()))
    })
    .unwrap_or_default()
}

fn title(w: &Window) -> String {
    use smithay::wayland::{compositor::with_states, shell::xdg::XdgToplevelSurfaceData};
    let Some(t) = w.toplevel() else { return String::new() };
    with_states(t.wl_surface(), |s| {
        s.data_map.get::<XdgToplevelSurfaceData>().and_then(|d| d.lock().ok().and_then(|a| a.title.clone()))
    })
    .unwrap_or_default()
}

fn blit(dst: &mut Pixmap, src: &Pixmap, x: f32, y: f32) {
    draw::blit(dst, src, x, y, 1.0);
}

fn letter_tile(p: &mut Pixmap, font: &fontdue::Font, name: &str, x: f32, y: f32, size: f32, t: &theme::Theme) {
    fill_rrect(p, x, y, size, size, size * 0.225, t.surface_sunken);
    stroke_rrect(p, x + 0.5, y + 0.5, size - 1.0, size - 1.0, size * 0.225, t.border_subtle);
    let ch: String = name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
    let px = size * 0.48;
    let w = draw::text_width(font, &ch, px);
    draw::draw_text(p, font, &ch, x + (size - w) / 2.0, y + size * 0.66, px, t.ink_secondary);
}

impl Smallvil {
    pub fn out_size(&self) -> Size<i32, Logical> {
        self.space
            .outputs()
            .next()
            .and_then(|o| self.space.output_geometry(o))
            .map(|g| g.size)
            .unwrap_or_else(|| (1280, 800).into())
    }

    pub fn locked(&self) -> bool {
        matches!(self.layers.overlay, Some(Overlay::Lock { .. }))
    }

    pub fn open(&mut self, ov: Overlay) {
        if self.locked() {
            return;
        }
        self.layers.overlay = Some(ov);
        self.layers.dirty = true;
    }

    pub fn close_overlay(&mut self) {
        if self.locked() {
            return;
        }
        self.layers.overlay = None;
        self.layers.dirty = true;
    }

    // ------------------------------------------------------------ openers

    pub fn open_finder(&mut self) {
        if matches!(self.layers.overlay, Some(Overlay::Finder { .. })) {
            return self.close_overlay();
        }
        self.layers.apps = apps::load();
        // Start in the input mode the user is in (Fcitx: 2 = Korean active).
        if let Ok(o) = std::process::Command::new("fcitx5-remote").output() {
            self.layers.ko = String::from_utf8_lossy(&o.stdout).trim() == "2";
        }
        let mut ov = Overlay::Finder { query: String::new(), sel: 0, top: 0, matches: vec![], comp: Default::default() };
        refilter(&self.layers.apps, &mut ov);
        self.open(ov);
    }

    /// Command/Alt-Tab: windows front to back, then minimized ones.
    pub fn open_switcher(&mut self) {
        if let Some(Overlay::Switcher { items, sel }) = &mut self.layers.overlay {
            if !items.is_empty() {
                *sel = (*sel + 1) % items.len();
            }
            self.layers.dirty = true;
            return;
        }
        if self.layers.apps.is_empty() {
            self.layers.apps = apps::load();
        }
        let mut items: Vec<Window> = self.space.elements().rev().cloned().collect();
        items.extend(self.minimized.iter().rev().map(|(w, _)| w.clone()));
        if items.is_empty() {
            return;
        }
        let sel = if items.len() > 1 { 1 } else { 0 };
        self.open(Overlay::Switcher { items, sel });
    }

    fn commit_switcher(&mut self) {
        let Some(Overlay::Switcher { items, sel }) = self.layers.overlay.take() else { return };
        self.layers.dirty = true;
        if let Some(w) = items.get(sel).cloned() {
            self.bring(&w);
        }
    }

    /// Raise and focus `w`, bringing it back if it was minimized.
    pub fn bring(&mut self, w: &Window) {
        if let Some(i) = self.minimized.iter().position(|(m, _)| m == w) {
            let (m, loc) = self.minimized.remove(i);
            self.space.map_element(m, loc, true);
        }
        self.space.raise_element(w, true);
        if let Some(t) = w.toplevel() {
            let serial = smithay::utils::SERIAL_COUNTER.next_serial();
            let k = self.seat.get_keyboard().unwrap();
            k.set_focus(self, Some(t.wl_surface().clone()), serial);
        }
    }

    pub fn open_expose(&mut self) {
        if matches!(self.layers.overlay, Some(Overlay::Expose { .. })) {
            return self.close_overlay();
        }
        let wins: Vec<Window> = self.space.elements().cloned().collect();
        if wins.is_empty() {
            return;
        }
        let area = self.work_area();
        let n = wins.len();
        let cols = (n as f64).sqrt().ceil() as i32;
        let rows = ((n as i32) + cols - 1) / cols;
        let pad = 40;
        let cw = (area.size.w - pad) / cols;
        let ch = (area.size.h - pad) / rows;
        let mut slots = Vec::new();
        for (i, w) in wins.iter().enumerate() {
            let geo = w.geometry();
            let (c, r) = (i as i32 % cols, i as i32 / cols);
            let (bw, bh) = (cw - pad, ch - pad - 28);
            let s = (bw as f64 / geo.size.w as f64).min(bh as f64 / geo.size.h as f64).min(1.0);
            let (tw, th) = ((geo.size.w as f64 * s) as i32, (geo.size.h as f64 * s) as i32);
            let x = area.loc.x + pad / 2 + c * cw + (cw - tw) / 2;
            let y = area.loc.y + pad / 2 + r * ch + (ch - 28 - th) / 2;
            slots.push((w.clone(), Rectangle::new((x, y).into(), (tw, th).into()), s));
        }
        self.open(Overlay::Expose { slots, hover: None });
    }

    pub fn lock(&mut self) {
        self.close_overlay();
        self.layers.overlay = Some(Overlay::Lock { input: String::new(), error: false, busy: None });
        self.layers.dirty = true;
        tracing::info!("locked");
    }

    pub fn open_region(&mut self) {
        let p = self.seat.get_pointer().unwrap().current_location().to_i32_round();
        self.open(Overlay::Region { start: None, cur: p });
    }

    // ------------------------------------------------------------ input

    /// Keyboard while an overlay is up: everything goes to it.
    pub fn overlay_key(&mut self, pressed: bool, mods: &ModifiersState, key: &KeysymHandle<'_>) {
        let sym = key.modified_sym();
        let raw = sym.raw();
        let ch = sym.key_char().filter(|c| !c.is_control());
        let Some(ov) = self.layers.overlay.as_mut() else { return };
        match ov {
            Overlay::Switcher { items, sel } => {
                let is_mod = matches!(raw, keysyms::KEY_Alt_L | keysyms::KEY_Alt_R | keysyms::KEY_Super_L | keysyms::KEY_Super_R);
                if !pressed {
                    if is_mod {
                        self.commit_switcher();
                    }
                    return;
                }
                match raw {
                    keysyms::KEY_Tab | keysyms::KEY_ISO_Left_Tab | keysyms::KEY_Right if !items.is_empty() => {
                        *sel = if mods.shift || raw == keysyms::KEY_ISO_Left_Tab {
                            (*sel + items.len() - 1) % items.len()
                        } else {
                            (*sel + 1) % items.len()
                        };
                    }
                    keysyms::KEY_Left if !items.is_empty() => *sel = (*sel + items.len() - 1) % items.len(),
                    keysyms::KEY_Return => return self.commit_switcher(),
                    keysyms::KEY_Escape => {
                        self.layers.overlay = None;
                    }
                    _ => {}
                }
                self.layers.dirty = true;
            }
            _ if !pressed => {}
            Overlay::Settings { .. } => return self.settings_key(pressed, mods, key),
            Overlay::Finder { query, sel, top, matches, comp } => {
                let toggle = raw == keysyms::KEY_Hangul || (raw == keysyms::KEY_space && mods.shift && !mods.ctrl && !mods.logo);
                if toggle {
                    query.push_str(&comp.flush());
                    self.layers.ko = !self.layers.ko;
                    // Keep Fcitx in the same mode, so apps continue in it.
                    crate::shell::spawn(&["fcitx5-remote", if self.layers.ko { "-o" } else { "-c" }]);
                    self.layers.dirty = true;
                    return;
                }
                let ko = self.layers.ko;
                match raw {
                    keysyms::KEY_Escape => return self.close_overlay(),
                    keysyms::KEY_Return | keysyms::KEY_KP_Enter => {
                        if !comp.is_empty() {
                            query.push_str(&comp.flush());
                            refilter_parts(&self.layers.apps, query, sel, top, matches);
                        }
                        let pick = matches.get(*sel).copied();
                        self.close_overlay();
                        if let Some(i) = pick {
                            self.launch_app(i);
                        }
                        return;
                    }
                    keysyms::KEY_Down | keysyms::KEY_Tab => {
                        if *sel + 1 < matches.len() {
                            *sel += 1;
                        }
                    }
                    keysyms::KEY_Up => *sel = sel.saturating_sub(1),
                    keysyms::KEY_BackSpace => {
                        if !comp.back() {
                            query.pop();
                        }
                        let full = format!("{query}{}", comp.preedit());
                        refilter_parts(&self.layers.apps, &full, sel, top, matches);
                    }
                    _ if mods.ctrl || mods.logo || mods.alt => {
                        // Command/Win+Space again closes, like Spotlight.
                        if raw == keysyms::KEY_space {
                            return self.close_overlay();
                        }
                    }
                    _ => {
                        if let Some(c) = ch {
                            match (ko, if ko { comp.feed(c) } else { None }) {
                                (true, Some(done)) => query.push_str(&done),
                                _ => {
                                    query.push_str(&comp.flush());
                                    query.push(c);
                                }
                            }
                            let full = format!("{query}{}", comp.preedit());
                            refilter_parts(&self.layers.apps, &full, sel, top, matches);
                        }
                    }
                }
                if *sel < *top {
                    *top = *sel;
                } else if *sel >= *top + ROWS {
                    *top = *sel + 1 - ROWS;
                }
                self.layers.dirty = true;
            }
            Overlay::Expose { .. } | Overlay::Region { .. } => {
                if raw == keysyms::KEY_Escape {
                    self.close_overlay();
                }
            }
            Overlay::Lock { input, error, busy } => {
                if busy.is_some() {
                    return;
                }
                match raw {
                    keysyms::KEY_Return | keysyms::KEY_KP_Enter if !input.is_empty() => {
                        let pw = std::mem::take(input);
                        let (tx, rx) = channel();
                        let user = crate::greeter::current().0;
                        std::thread::spawn(move || {
                            let ok = if crate::greeter::active() { crate::greeter::login(&user, &pw) } else { crate::pam::check(&pw) };
                            let _ = tx.send(ok);
                        });
                        *busy = Some(rx);
                    }
                    keysyms::KEY_BackSpace => {
                        input.pop();
                    }
                    keysyms::KEY_Escape => input.clear(),
                    // The login screen: pick the user.
                    keysyms::KEY_Tab | keysyms::KEY_Right | keysyms::KEY_Down if crate::greeter::active() => {
                        crate::greeter::step(1);
                        input.clear();
                        *error = false;
                    }
                    keysyms::KEY_Left | keysyms::KEY_Up | keysyms::KEY_ISO_Left_Tab if crate::greeter::active() => {
                        crate::greeter::step(-1);
                        input.clear();
                        *error = false;
                    }
                    _ => {
                        if let Some(c) = ch {
                            input.push(c);
                            *error = false;
                        }
                    }
                }
                self.layers.dirty = true;
            }
        }
    }

    /// A button with an overlay up. Returns true when the overlay took it.
    pub fn overlay_button(&mut self, pressed: bool, p: Point<f64, Logical>) -> bool {
        let pi = p.to_i32_round();
        // Toasts are not modal: a click dismisses one.
        if pressed && !self.locked() {
            if let Some((_, at, hits)) = &self.layers.toast_buf {
                let local = pi - *at;
                if let Some((_, id)) = hits.iter().find(|(r, _)| r.contains(local)) {
                    let id = *id;
                    self.layers.toasts.retain(|t| t.note.id != id);
                    self.layers.toast_dirty = true;
                    return true;
                }
            }
        }
        let Some(ov) = self.layers.overlay.as_mut() else { return false };
        match ov {
            Overlay::Lock { .. } => {}
            Overlay::Region { start, cur } => {
                if pressed {
                    *start = Some(pi);
                    *cur = pi;
                } else if let Some(s) = *start {
                    let r = Rectangle::from_extremities(
                        (s.x.min(pi.x), s.y.min(pi.y)),
                        (s.x.max(pi.x), s.y.max(pi.y)),
                    );
                    self.layers.overlay = None;
                    self.layers.dirty = true;
                    if r.size.w > 4 && r.size.h > 4 {
                        self.shot = Some(Some(r));
                    }
                }
                self.layers.dirty = true;
            }
            _ if !pressed => {
                self.layers.drag = None;
            }
            _ => {
                let hit = self.layers.hits.iter().rev().find(|(r, i)| *i != usize::MAX && r.contains(pi)).map(|(_, i)| *i);
                match (self.layers.overlay.as_ref().unwrap(), hit) {
                    (Overlay::Settings { .. }, Some(id)) if id == crate::settings::ID_OUT_SLIDER || id == crate::settings::ID_IN_SLIDER => {
                        self.layers.drag = Some(id);
                        self.settings_slider(id, pi.x);
                    }
                    (Overlay::Settings { .. }, Some(id)) => self.settings_click(id),
                    (Overlay::Finder { matches, .. }, Some(row)) => {
                        let pick = matches.get(row).copied();
                        self.close_overlay();
                        if let Some(i) = pick {
                            self.launch_app(i);
                        }
                    }
                    (Overlay::Switcher { items, .. }, Some(i)) => {
                        let w = items.get(i).cloned();
                        self.layers.overlay = None;
                        self.layers.dirty = true;
                        if let Some(w) = w {
                            self.bring(&w);
                        }
                    }
                    (Overlay::Expose { slots, .. }, Some(i)) => {
                        let w = slots.get(i).map(|s| s.0.clone());
                        self.close_overlay();
                        if let Some(w) = w {
                            self.bring(&w);
                        }
                    }
                    // A click outside the panel closes it.
                    (Overlay::Finder { .. } | Overlay::Switcher { .. } | Overlay::Expose { .. } | Overlay::Settings { .. }, None) => {
                        let inside = self.layers.hits.iter().any(|(r, i)| *i == usize::MAX && r.contains(pi));
                        if !inside {
                            self.close_overlay();
                        }
                    }
                    _ => {}
                }
            }
        }
        true
    }

    pub fn overlay_motion(&mut self, p: Point<f64, Logical>) {
        let pi = p.to_i32_round();
        let hit = self.layers.hits.iter().find(|(r, i)| *i != usize::MAX && r.contains(pi)).map(|(_, i)| *i);
        if let Some(id) = self.layers.drag {
            self.settings_slider(id, pi.x);
            return;
        }
        match self.layers.overlay.as_mut() {
            Some(Overlay::Region { cur, .. }) => {
                *cur = pi;
                self.layers.dirty = true;
            }
            Some(Overlay::Expose { hover, .. }) if *hover != hit => {
                *hover = hit;
                self.layers.dirty = true;
            }
            Some(Overlay::Finder { sel, top, .. }) => {
                if let Some(row) = hit {
                    if *sel != row {
                        *sel = row;
                        let _ = top;
                        self.layers.dirty = true;
                    }
                }
            }
            _ => {}
        }
    }

    fn launch_app(&mut self, i: usize) {
        let Some(app) = self.layers.apps.get(i) else { return };
        let args: Vec<&str> = app.exec.iter().map(String::as_str).collect();
        tracing::info!(app = %app.id, "finder: launch");
        crate::shell::spawn(&args);
    }

    /// Per-frame bookkeeping: lock results, notifications, toast expiry.
    pub fn poll_layers(&mut self) {
        // Reap exited children (apps, helper tools) so none linger as zombies.
        unsafe { while libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) > 0 {} }
        if let Some(Overlay::Lock { busy: Some(rx), error, .. }) = &mut self.layers.overlay {
            if let Ok(ok) = rx.try_recv() {
                if ok && crate::greeter::active() {
                    // greetd starts the session once the greeter is gone.
                    tracing::info!("greeter: logged in");
                    std::process::exit(0);
                }
                if ok {
                    self.layers.overlay = None;
                    tracing::info!("unlocked");
                } else {
                    *error = true;
                    if let Some(Overlay::Lock { busy, .. }) = &mut self.layers.overlay {
                        *busy = None;
                    }
                }
                self.layers.dirty = true;
            }
        }
        while let Ok(m) = self.note_rx.try_recv() {
            match m {
                crate::notify::Msg::Show(n) => self.toast(n),
                crate::notify::Msg::Close(id) => {
                    self.layers.toasts.retain(|t| t.note.id != id);
                    self.layers.toast_dirty = true;
                }
            }
        }
        let now = Instant::now();
        let before = self.layers.toasts.len();
        self.layers.toasts.retain(|t| t.until > now);
        if self.layers.toasts.len() != before {
            self.layers.toast_dirty = true;
        }
        if let Some(Overlay::Lock { .. }) = &self.layers.overlay {
            use chrono::Timelike;
            let m = chrono::Local::now().minute();
            if m != self.layers.minute {
                self.layers.minute = m;
                self.layers.dirty = true;
            }
        }
        if let Some(Overlay::Expose { slots, .. }) = &mut self.layers.overlay {
            let n = slots.len();
            slots.retain(|s| s.0.alive());
            if slots.len() != n {
                self.layers.dirty = true;
            }
        }
    }

    pub fn toast(&mut self, n: Note) {
        let ms = if n.timeout > 0 { n.timeout.min(30_000) as u64 } else { 5_000 };
        self.layers.toasts.retain(|t| t.note.id != n.id);
        self.layers.toasts.push(Toast { note: n, until: Instant::now() + Duration::from_millis(ms) });
        while self.layers.toasts.len() > 3 {
            self.layers.toasts.remove(0);
        }
        self.layers.toast_dirty = true;
    }

    // ------------------------------------------------------------ paint

    /// Elements for the overlay (front first). `thumbs` are the Expose
    /// window thumbnails, drawn above the overlay's dim layer.
    pub fn overlay_elements(
        &mut self,
        renderer: &mut GlesRenderer,
        out: Size<i32, Physical>,
    ) -> (Vec<MemoryRenderBufferRenderElement<GlesRenderer>>, Vec<WaylandSurfaceRenderElement<GlesRenderer>>) {
        self.poll_layers();
        let mut front = Vec::new();
        let mut thumbs = Vec::new();
        if self.layers.toast_dirty {
            self.paint_toasts(out.w, out.h);
            self.layers.toast_dirty = false;
        }
        if !self.locked() {
            if let Some((buf, at, _)) = &self.layers.toast_buf {
                if let Some(e) = draw::element(renderer, at.to_f64(), buf, Kind::Unspecified) {
                    front.push(e);
                }
            }
        }
        if self.layers.overlay.is_none() {
            return (front, thumbs);
        }
        if self.layers.dirty || self.layers.buf.is_none() {
            self.paint_overlay(out.w, out.h);
            self.layers.dirty = false;
        }
        if let Some(buf) = &self.layers.buf {
            if let Some(e) = draw::element(renderer, (0.0, 0.0).into(), buf, Kind::Unspecified) {
                front.push(e);
            }
        }
        if let Some(Overlay::Expose { slots, .. }) = &self.layers.overlay {
            for (w, r, s) in slots.clone() {
                let geo = w.geometry();
                let at = Point::<f64, Logical>::from((r.loc.x as f64 - geo.loc.x as f64 * s, r.loc.y as f64 - geo.loc.y as f64 * s));
                let els: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
                    w.render_elements(renderer, at.to_physical(crate::display::scale()).to_i32_round(), Scale::from(s * crate::display::scale()), 1.0);
                thumbs.extend(els);
            }
        }
        (front, thumbs)
    }

    fn paint_overlay(&mut self, w: i32, h: i32) {
        self.layers.hits.clear();
        self.layers.sliders.clear();
        let mode = self.chrome.mode;
        let t = theme::theme(mode);
        let Some(mut p) = draw::canvas(w.max(1) as f32, h.max(1) as f32) else { return };
        let style = self.chrome.style;
        let ov = self.layers.overlay.take();
        match &ov {
            Some(Overlay::Finder { query, sel, top, matches, comp }) => {
                let shown = format!("{query}{}", comp.preedit());
                self.paint_finder(&mut p, &t, style, &shown, *sel, *top, matches)
            }
            Some(Overlay::Settings { page, capture }) => self.paint_settings(&mut p, *page, *capture),
            Some(Overlay::Switcher { items, sel }) => self.paint_switcher(&mut p, &t, items, *sel),
            Some(Overlay::Expose { slots, hover }) => {
                fill_rect(&mut p, 0.0, 0.0, w as f32, h as f32, if mode == Mode::Dark { Rgba(0, 0, 0, 140) } else { Rgba(10, 11, 12, 70) });
                for (i, (win, r, _)) in slots.iter().enumerate() {
                    let ring = if *hover == Some(i) { t.action_primary } else { with_alpha(t.border_subtle, 200) };
                    let (x, y, rw, rh) = (r.loc.x as f32, r.loc.y as f32, r.size.w as f32, r.size.h as f32);
                    draw::shadow(&mut p, x, y, rw, rh, theme::RADIUS_MD, 20.0, 8.0, t.shadow_lg);
                    fill_rrect(&mut p, x - 3.0, y - 3.0, rw + 6.0, rh + 6.0, theme::RADIUS_MD + 3.0, ring);
                    fill_rect(&mut p, x, y, rw, rh, t.surface_base);
                    let name = draw::fit_text(&self.chrome.fonts.medium, &title(win), 13.0, rw.max(60.0));
                    let tw = draw::text_width(&self.chrome.fonts.medium, &name, 13.0) + 20.0;
                    let (px, py) = (x + (rw - tw) / 2.0, y + rh + 10.0);
                    fill_rrect(&mut p, px, py, tw, 24.0, theme::RADIUS_FULL, t.surface_material);
                    draw::draw_text(&mut p, &self.chrome.fonts.medium, &name, px + 10.0, py + 16.5, 13.0, t.ink_primary);
                    self.layers.hits.push((Rectangle::new((r.loc.x - 3, r.loc.y - 3).into(), (r.size.w + 6, r.size.h + 40).into()), i));
                }
            }
            Some(Overlay::Lock { input, error, busy }) => self.paint_lock(&mut p, input, *error, busy.is_some()),
            Some(Overlay::Region { start, cur }) => {
                let dim = Rgba(0, 0, 0, 90);
                match start {
                    None => fill_rect(&mut p, 0.0, 0.0, w as f32, h as f32, dim),
                    Some(s) => {
                        let (x0, y0) = (s.x.min(cur.x) as f32, s.y.min(cur.y) as f32);
                        let (x1, y1) = (s.x.max(cur.x) as f32, s.y.max(cur.y) as f32);
                        fill_rect(&mut p, 0.0, 0.0, w as f32, y0, dim);
                        fill_rect(&mut p, 0.0, y1, w as f32, h as f32 - y1, dim);
                        fill_rect(&mut p, 0.0, y0, x0, y1 - y0, dim);
                        fill_rect(&mut p, x1, y0, w as f32 - x1, y1 - y0, dim);
                        let white = Rgba(255, 255, 255, 230);
                        fill_rect(&mut p, x0, y0, x1 - x0, 1.0, white);
                        fill_rect(&mut p, x0, y1 - 1.0, x1 - x0, 1.0, white);
                        fill_rect(&mut p, x0, y0, 1.0, y1 - y0, white);
                        fill_rect(&mut p, x1 - 1.0, y0, 1.0, y1 - y0, white);
                        let label = format!("{} × {}", (x1 - x0) as i32, (y1 - y0) as i32);
                        let lw = draw::text_width(&self.chrome.fonts.medium, &label, 12.0) + 16.0;
                        fill_rrect(&mut p, x1 - lw, y1 + 6.0, lw, 22.0, theme::RADIUS_SM, Rgba(10, 11, 12, 200));
                        draw::draw_text(&mut p, &self.chrome.fonts.medium, &label, x1 - lw + 8.0, y1 + 21.0, 12.0, white);
                    }
                }
                // Hint pill at the top.
                let hint = "드래그해서 영역을 고르세요 · Esc 취소";
                let hw = draw::text_width(&self.chrome.fonts.medium, hint, 13.0) + 28.0;
                fill_rrect(&mut p, (w as f32 - hw) / 2.0, 48.0, hw, 30.0, theme::RADIUS_FULL, t.surface_material);
                draw::draw_text(&mut p, &self.chrome.fonts.medium, hint, (w as f32 - hw) / 2.0 + 14.0, 68.0, 13.0, t.ink_primary);
            }
            None => {}
        }
        self.layers.overlay = ov;
        self.layers.buf = Some(upload(&p));
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_finder(&mut self, p: &mut Pixmap, t: &theme::Theme, style: Style, query: &str, sel: usize, top: usize, matches: &[usize]) {
        let (w, h) = (draw::lw(p) as i32, draw::lh(p) as i32);
        let rows = matches.len().saturating_sub(top).min(ROWS) as i32;
        let list_h = if rows > 0 { rows * ROW_H + 12 } else { 52 };
        let ph = FIELD_H + 1 + list_h;
        let x = (w - FINDER_W) / 2;
        let y = match style {
            Style::Mac => (h as f32 * 0.2) as i32,
            Style::Windows => h - theme::TASKBAR_H - 12 - ph,
        };
        let (xf, yf, wf, hf) = (x as f32, y as f32, FINDER_W as f32, ph as f32);
        let r = theme::RADIUS_XL_F;
        draw::shadow(p, xf, yf, wf, hf, r, 30.0, 12.0, t.shadow_lg);
        fill_rrect(p, xf, yf, wf, hf, r, t.surface_raised);
        stroke_rrect(p, xf + 0.5, yf + 0.5, wf - 1.0, hf - 1.0, r, t.border_subtle);
        self.layers.hits.push((Rectangle::new((x, y).into(), (FINDER_W, ph).into()), usize::MAX));

        // Field.
        let mid = yf + FIELD_H as f32 / 2.0;
        self.chrome.icons.draw(p, ICON_SEARCH, Some(t.ink_secondary), xf + 20.0, mid - 11.0, 22.0);
        let font = &self.chrome.fonts.medium;
        let tx = xf + 56.0;
        if query.is_empty() {
            let ph_text = match style {
                Style::Mac => "Redrob 검색",
                Style::Windows => "앱, 설정 검색",
            };
            draw::draw_text(p, font, ph_text, tx, mid + 7.0, 19.0, t.ink_muted);
            fill_rect(p, tx - 1.0, mid - 12.0, 2.0, 24.0, t.action_primary);
        } else {
            draw::draw_text(p, font, query, tx, mid + 7.0, 19.0, t.ink_primary);
            let qw = draw::text_width(font, query, 19.0);
            fill_rect(p, tx + qw + 2.0, mid - 12.0, 2.0, 24.0, t.action_primary);
        }
        // Input mode pill at the right of the field: 가 (Hangul) / A.
        let mode_label = if self.layers.ko { "가" } else { "A" };
        let (mx, my) = (xf + wf - 52.0, mid - 13.0);
        fill_rrect(p, mx, my, 32.0, 26.0, theme::RADIUS_SM, if self.layers.ko { t.action_primary } else { t.surface_sunken });
        let lw = draw::text_width(&self.chrome.fonts.semibold, mode_label, 13.0);
        draw::draw_text(p, &self.chrome.fonts.semibold, mode_label, mx + (32.0 - lw) / 2.0, my + 18.0, 13.0, if self.layers.ko { Rgba(255, 255, 255, 255) } else { t.ink_secondary });
        fill_rect(p, xf, yf + FIELD_H as f32, wf, 1.0, t.border_subtle);

        // Results.
        let ly = y + FIELD_H + 1 + 6;
        if rows == 0 {
            draw::draw_text(p, font, "결과 없음", xf + 24.0, ly as f32 + 30.0, 14.0, t.ink_muted);
            return;
        }
        let head = if query.is_empty() { "모든 앱" } else { "앱" };
        let _ = head;
        for (k, &ai) in matches.iter().skip(top).take(ROWS).enumerate() {
            let idx = top + k;
            let ry = ly + k as i32 * ROW_H;
            let selected = idx == sel;
            if selected {
                fill_rrect(p, xf + 6.0, ry as f32, wf - 12.0, ROW_H as f32, theme::RADIUS_MD, t.action_primary);
            }
            let app = self.layers.apps[ai].clone();
            let ix = xf + 18.0;
            let iy = ry as f32 + (ROW_H as f32 - 28.0) / 2.0;
            let icon = app.icon.as_deref().and_then(|n| self.layers.icon(n, 28).cloned());
            match icon {
                Some(img) => blit(p, &img, ix, iy),
                None => letter_tile(p, &self.chrome.fonts.semibold, &app.name, ix, iy, 28.0, t),
            }
            let ink = if selected { Rgba(255, 255, 255, 255) } else { t.ink_primary };
            let name = draw::fit_text(&self.chrome.fonts.semibold, &app.name, 14.0, wf - 120.0);
            draw::draw_text(p, &self.chrome.fonts.semibold, &name, ix + 40.0, ry as f32 + 27.0, 14.0, ink);
            if selected {
                let hint = "↵ 열기";
                let hw = draw::text_width(&self.chrome.fonts.medium, hint, 12.0);
                draw::draw_text(p, &self.chrome.fonts.medium, hint, xf + wf - 24.0 - hw, ry as f32 + 27.0, 12.0, Rgba(255, 255, 255, 200));
            }
            self.layers.hits.push((Rectangle::new((x + 6, ry).into(), (FINDER_W - 12, ROW_H).into()), idx));
        }
    }

    fn paint_switcher(&mut self, p: &mut Pixmap, t: &theme::Theme, items: &[Window], sel: usize) {
        let (w, h) = (draw::lw(p), draw::lh(p));
        let card = 112.0;
        let gap = 8.0;
        let pad = 16.0;
        let n = items.len().min(9) as f32;
        let pw = pad * 2.0 + n * card + (n - 1.0) * gap;
        let ph = pad * 2.0 + card + 4.0;
        let (x, y) = ((w - pw) / 2.0, (h - ph) / 2.0);
        draw::shadow(p, x, y, pw, ph, theme::RADIUS_XL_F, 30.0, 12.0, t.shadow_lg);
        fill_rrect(p, x, y, pw, ph, theme::RADIUS_XL_F, t.surface_material);
        stroke_rrect(p, x + 0.5, y + 0.5, pw - 1.0, ph - 1.0, theme::RADIUS_XL_F, t.border_subtle);
        self.layers.hits.push((Rectangle::new((x as i32, y as i32).into(), (pw as i32, ph as i32).into()), usize::MAX));
        for (i, win) in items.iter().take(9).enumerate() {
            let cx = x + pad + i as f32 * (card + gap);
            let cy = y + pad;
            if i == sel {
                fill_rrect(p, cx, cy, card, card + 4.0, theme::RADIUS_LG, t.hover);
                stroke_rrect(p, cx + 1.0, cy + 1.0, card - 2.0, card + 2.0, theme::RADIUS_LG, t.action_primary);
                stroke_rrect(p, cx + 1.5, cy + 1.5, card - 3.0, card + 1.0, theme::RADIUS_LG, t.action_primary);
            }
            let icon = self.layers.window_icon(win, 64);
            let (ix, iy) = (cx + (card - 64.0) / 2.0, cy + 12.0);
            let name = crate::deco::app_name(win);
            match icon {
                Some(img) => blit(p, &img, ix, iy),
                None => letter_tile(p, &self.chrome.fonts.semibold, &name, ix, iy, 64.0, t),
            }
            let label = draw::fit_text(&self.chrome.fonts.medium, &title(win), 12.0, card - 12.0);
            let lw = draw::text_width(&self.chrome.fonts.medium, &label, 12.0);
            draw::draw_text(p, &self.chrome.fonts.medium, &label, cx + (card - lw) / 2.0, cy + 98.0, 12.0, t.ink_primary);
            self.layers.hits.push((Rectangle::new((cx as i32, cy as i32).into(), (card as i32, card as i32).into()), i));
        }
    }

    fn paint_lock(&mut self, p: &mut Pixmap, input: &str, error: bool, busy: bool) {
        use chrono::{Datelike, Timelike};
        let (w, h) = (draw::lw(p), draw::lh(p));
        // Always the dark ground: the lock screen reads the same in both themes.
        let t = theme::theme(Mode::Dark);
        draw::fill_gradient(p, t.ground);
        if let Some(img) = self.chrome.wallpaper_pixmap(p.width() as i32, p.height() as i32) {
            draw::draw_cover(p, &img);
        }
        fill_rect(p, 0.0, 0.0, w, h, Rgba(0, 0, 0, 90));
        let white = Rgba(248, 249, 251, 255);
        let now = chrono::Local::now();
        let wd = ["월", "화", "수", "목", "금", "토", "일"][now.weekday().num_days_from_monday() as usize];
        let date = format!("{}월 {}일 {wd}요일", now.month(), now.day());
        let time = format!("{}:{:02}", now.hour(), now.minute());
        let f = &self.chrome.fonts.semibold;
        let dw = draw::text_width(f, &date, 20.0);
        draw::draw_text(p, f, &date, (w - dw) / 2.0, h * 0.18, 20.0, with_alpha(white, 220));
        let tw = draw::text_width(f, &time, 96.0);
        draw::draw_text(p, f, &time, (w - tw) / 2.0, h * 0.18 + 100.0, 96.0, white);

        // Avatar, name, password pill.
        let user = if crate::greeter::active() {
            crate::greeter::current().1
        } else {
            std::env::var("USER").unwrap_or_else(|_| "user".into())
        };
        let cy = h * 0.62;
        fill_circle(p, w / 2.0, cy - 70.0, 36.0, with_alpha(white, 40));
        let initial: String = user.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
        let iw = draw::text_width(f, &initial, 30.0);
        draw::draw_text(p, f, &initial, (w - iw) / 2.0, cy - 59.0, 30.0, white);
        let uw = draw::text_width(f, &user, 16.0);
        draw::draw_text(p, f, &user, (w - uw) / 2.0, cy - 16.0, 16.0, white);
        let (pw, ph) = (260.0, 40.0);
        let (px, py) = ((w - pw) / 2.0, cy + 2.0);
        fill_rrect(p, px, py, pw, ph, theme::RADIUS_FULL, Rgba(255, 255, 255, 46));
        stroke_rrect(p, px + 0.5, py + 0.5, pw - 1.0, ph - 1.0, ph / 2.0, Rgba(255, 255, 255, if error { 0 } else { 70 }));
        if error {
            stroke_rrect(p, px + 0.5, py + 0.5, pw - 1.0, ph - 1.0, ph / 2.0, t.danger);
        }
        let m = &self.chrome.fonts.medium;
        if input.is_empty() {
            let hint = if busy { "확인 중…" } else { "암호 입력" };
            let hw = draw::text_width(m, hint, 14.0);
            draw::draw_text(p, m, hint, (w - hw) / 2.0, py + 25.0, 14.0, with_alpha(white, 150));
        } else {
            let n = input.chars().count().min(24) as f32;
            let dots = n * 12.0;
            for i in 0..n as i32 {
                fill_circle(p, (w - dots) / 2.0 + 6.0 + i as f32 * 12.0, py + ph / 2.0, 3.5, white);
            }
        }
        self.chrome.icons.draw(p, ICON_LOCK, Some(with_alpha(white, 160)), px + 14.0, py + 12.0, 16.0);
        if crate::greeter::active() && crate::greeter::user_count() > 1 {
            let hint = "Tab 키로 사용자 바꾸기";
            let hw = draw::text_width(m, hint, 13.0);
            draw::draw_text(p, m, hint, (w - hw) / 2.0, py + ph + 52.0, 13.0, with_alpha(white, 140));
        }
        if error {
            let e = "암호가 맞지 않습니다";
            let ew = draw::text_width(m, e, 13.0);
            draw::draw_text(p, m, e, (w - ew) / 2.0, py + ph + 26.0, 13.0, t.danger);
        }
    }

    fn paint_toasts(&mut self, w: i32, h: i32) {
        if self.layers.toasts.is_empty() {
            self.layers.toast_buf = None;
            return;
        }
        let t = theme::theme(self.chrome.mode);
        let n = self.layers.toasts.len() as i32;
        let m = 24; // shadow room
        let gap = 10;
        let bw = TOAST_W + 2 * m;
        let bh = n * TOAST_H + (n - 1) * gap + 2 * m;
        let Some(mut p) = draw::canvas(bw as f32, bh as f32) else { return };
        let mut hits = Vec::new();
        let notes: Vec<Note> = self.layers.toasts.iter().rev().map(|t| t.note.clone()).collect();
        for (i, note) in notes.iter().enumerate() {
            let (x, y) = (m as f32, (m + i as i32 * (TOAST_H + gap)) as f32);
            let (tw, th) = (TOAST_W as f32, TOAST_H as f32);
            draw::shadow(&mut p, x, y, tw, th, theme::RADIUS_LG, 18.0, 6.0, t.shadow_md);
            fill_rrect(&mut p, x, y, tw, th, theme::RADIUS_LG, t.surface_raised);
            stroke_rrect(&mut p, x + 0.5, y + 0.5, tw - 1.0, th - 1.0, theme::RADIUS_LG, t.border_subtle);
            // Icon: the app's, else a glyph tile.
            let icon_name = if !note.icon.is_empty() {
                Some(note.icon.clone())
            } else {
                apps::for_app_id(&self.layers.apps, &note.app).and_then(|a| a.icon.clone())
            };
            let img = icon_name.and_then(|n| self.layers.icon(&n, 36).cloned());
            let (ix, iy) = (x + 14.0, y + (th - 36.0) / 2.0);
            match img {
                Some(img) => blit(&mut p, &img, ix, iy),
                None => {
                    fill_rrect(&mut p, ix, iy, 36.0, 36.0, 8.0, t.surface_sunken);
                    let g = if note.app == "Redrob" { ICON_CAMERA } else { ICON_BELL };
                    self.chrome.icons.draw(&mut p, g, Some(t.ink_secondary), ix + 9.0, iy + 9.0, 18.0);
                }
            }
            let tx = ix + 50.0;
            let maxw = tw - (tx - x) - 14.0;
            let app = draw::fit_text(&self.chrome.fonts.medium, &note.app, 12.0, maxw - 30.0);
            draw::draw_text(&mut p, &self.chrome.fonts.medium, &app, tx, y + 22.0, 12.0, t.ink_muted);
            let now = "지금";
            let nw = draw::text_width(&self.chrome.fonts.medium, now, 12.0);
            draw::draw_text(&mut p, &self.chrome.fonts.medium, now, x + tw - 14.0 - nw, y + 22.0, 12.0, t.ink_muted);
            let sum = draw::fit_text(&self.chrome.fonts.semibold, &note.summary, 14.0, maxw);
            draw::draw_text(&mut p, &self.chrome.fonts.semibold, &sum, tx, y + 43.0, 14.0, t.ink_primary);
            let body = draw::fit_text(&self.chrome.fonts.medium, &note.body.replace('\n', " "), 13.0, maxw);
            draw::draw_text(&mut p, &self.chrome.fonts.medium, &body, tx, y + 62.0, 13.0, t.ink_secondary);
            hits.push((Rectangle::new((m, m + i as i32 * (TOAST_H + gap)).into(), (TOAST_W, TOAST_H).into()), note.id));
        }
        let at: Point<i32, Logical> = match self.chrome.style {
            Style::Mac => (w - bw - 12 + m, theme::BAR_H + 10 - m).into(),
            Style::Windows => (w - bw - 12 + m, h - theme::TASKBAR_H - 12 - bh + m).into(),
        };
        self.layers.toast_buf = Some((upload(&p), at, hits));
    }
}

fn refilter(apps: &[apps::App], ov: &mut Overlay) {
    if let Overlay::Finder { query, sel, top, matches, .. } = ov {
        refilter_parts(apps, query, sel, top, matches);
    }
}

fn refilter_parts(all: &[apps::App], query: &str, sel: &mut usize, top: &mut usize, matches: &mut Vec<usize>) {
    let q = query.to_lowercase();
    let mut m: Vec<(u32, usize)> = all.iter().enumerate().filter_map(|(i, a)| apps::score(a, &q).map(|s| (s, i))).collect();
    m.sort_by_key(|(s, i)| (*s, all[*i].name.to_lowercase()));
    *matches = m.into_iter().map(|(_, i)| i).collect();
    *sel = 0;
    *top = 0;
}

impl Smallvil {
    pub fn layers_hit(&mut self, r: Rectangle<i32, Logical>, id: usize) {
        self.layers.hits.push((r, id));
    }
}
