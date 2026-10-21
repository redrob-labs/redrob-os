//! Redrob shell chrome, drawn by the compositor itself (no Plasma, no COSMIC):
//! a top bar (Redrob mark, name, clock, light/dark switch) and a centred dock of
//! launchers. Painted on the CPU with tiny-skia (anti-aliased rounded corners),
//! text with fontdue (Pretendard), icons are the redrob-ui SVG set via resvg,
//! then uploaded as two textures. Colours and radii come from `theme.rs`.

use std::process::{Child, Command};

use resvg::{tiny_skia, usvg};
use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            element::{
                Kind,
                memory::{MemoryRenderBuffer, MemoryRenderBufferRenderElement},
            },
            gles::GlesRenderer,
        },
    },
    utils::{Logical, Physical, Point, Rectangle, Size, Transform},
};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke};

use crate::theme::{self, Mode, Rgba};

const ICON_TERMINAL: &str = include_str!("../assets/icons/terminal.svg");
const ICON_FOLDER: &str = include_str!("../assets/icons/folder.svg");
const ICON_BROWSER: &str = include_str!("../assets/icons/browser.svg");
const ICON_SETTINGS: &str = include_str!("../assets/icons/settings.svg");
const ICON_CODE: &str = include_str!("../assets/icons/code.svg");
const ICON_SUN: &str = include_str!("../assets/icons/sun.svg");
const ICON_MOON: &str = include_str!("../assets/icons/moon.svg");
const REDROB_MARK: &str = include_str!("../assets/redrob-os.svg");

const FONT_PATHS: &[&str] = &[
    "/usr/share/fonts/redrob/pretendard/Pretendard-SemiBold.otf",
    "/usr/share/fonts/redrob/pretendard/Pretendard-Medium.otf",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Launch(usize),
    ToggleTheme,
}

struct Launcher {
    icon: &'static str,
    command: Vec<String>,
    child: Option<Child>,
}

impl Launcher {
    fn running(&mut self) -> bool {
        match &mut self.child {
            Some(c) => matches!(c.try_wait(), Ok(None)),
            None => false,
        }
    }
}

/// What a frame of chrome depends on; repaint only when it changes.
#[derive(Clone, PartialEq, Eq)]
struct Key {
    mode: Mode,
    size: (i32, i32),
    clock: String,
    running: Vec<bool>,
}

pub struct Chrome {
    pub mode: Mode,
    font: fontdue::Font,
    launchers: Vec<Launcher>,
    key: Option<Key>,
    bar: Option<MemoryRenderBuffer>,
    cursor: Option<(Mode, MemoryRenderBuffer)>,
    ground_px: Option<(Mode, MemoryRenderBuffer)>,
    dock: Option<(MemoryRenderBuffer, Point<i32, Logical>)>,
    hits: Vec<(Rectangle<i32, Logical>, Action)>,
}

impl Chrome {
    pub fn new() -> Self {
        let font_bytes = std::env::var("REDROB_FONT")
            .ok()
            .into_iter()
            .chain(FONT_PATHS.iter().map(|s| s.to_string()))
            .find_map(|p| std::fs::read(p).ok())
            .expect("no Pretendard face found (set REDROB_FONT)");
        let font = fontdue::Font::from_bytes(font_bytes, fontdue::FontSettings::default())
            .expect("font parse");
        let mode = match std::env::var("REDROB_THEME").as_deref() {
            Ok("light") => Mode::Light,
            _ => Mode::Dark,
        };
        Self { mode, font, launchers: default_launchers(), key: None, bar: None, cursor: None, ground_px: None, dock: None, hits: vec![] }
    }

    /// What the pointer hit, in output-logical coordinates, if anything.
    pub fn hit(&self, p: Point<f64, Logical>) -> Option<Action> {
        let p = p.to_i32_round();
        self.hits.iter().find(|(r, _)| r.contains(p)).map(|(_, a)| *a)
    }

    pub fn act(&mut self, action: Action) {
        match action {
            Action::ToggleTheme => self.mode = self.mode.toggled(),
            Action::Launch(i) => {
                let l = &mut self.launchers[i];
                if l.running() {
                    return;
                }
                if let Some((prog, args)) = l.command.split_first() {
                    l.child = Command::new(prog).args(args).spawn().ok();
                }
            }
        }
    }

    pub fn ground(&self) -> [f32; 4] {
        let c = theme::theme(self.mode).surface_base;
        [c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0, 1.0]
    }

    pub fn elements(
        &mut self,
        renderer: &mut GlesRenderer,
        out: Size<i32, Physical>,
    ) -> Vec<MemoryRenderBufferRenderElement<GlesRenderer>> {
        let running: Vec<bool> = self.launchers.iter_mut().map(|l| l.running()).collect();
        let key = Key { mode: self.mode, size: (out.w, out.h), clock: clock_text(), running };
        if self.key.as_ref() != Some(&key) {
            self.repaint(&key);
            self.key = Some(key);
        }
        let mut v = Vec::new();
        if let Some((buf, loc)) = &self.dock {
            if let Ok(e) = MemoryRenderBufferRenderElement::from_buffer(
                renderer, loc.to_f64().to_physical(1.0), buf, None, None, None, Kind::Unspecified,
            ) {
                v.push(e);
            }
        }
        if let Some(buf) = &self.bar {
            if let Ok(e) = MemoryRenderBufferRenderElement::from_buffer(
                renderer, (0.0, 0.0), buf, None, None, None, Kind::Unspecified,
            ) {
                v.push(e);
            }
        }
        v
    }

    /// The desktop ground as an explicit back-most element (a 1x1 surface-base
    /// texel stretched over the output). The DRM compositor does not repaint the
    /// clear colour into areas no element covers, so the ground must be drawn.
    pub fn ground_element(
        &mut self,
        renderer: &mut GlesRenderer,
        out: Size<i32, Physical>,
    ) -> Option<MemoryRenderBufferRenderElement<GlesRenderer>> {
        if self.ground_px.as_ref().map(|(m, _)| *m) != Some(self.mode) {
            let c = theme::theme(self.mode).surface_base;
            let mut p = Pixmap::new(1, 1).unwrap();
            p.fill(tiny_skia::Color::from_rgba8(c.0, c.1, c.2, 255));
            self.ground_px = Some((self.mode, upload(p)));
        }
        let (_, buf) = self.ground_px.as_ref()?;
        let size: Size<i32, Logical> = (out.w, out.h).into();
        MemoryRenderBufferRenderElement::from_buffer(renderer, (0.0, 0.0), buf, None, None, Some(size), Kind::Unspecified).ok()
    }

    /// The pointer arrow (DRM backend only; nested runs use the host cursor):
    /// ink-primary fill with a surface-base outline so it reads on both grounds.
    pub fn cursor_element(
        &mut self,
        renderer: &mut GlesRenderer,
        at: Point<f64, Logical>,
    ) -> Option<MemoryRenderBufferRenderElement<GlesRenderer>> {
        if self.cursor.as_ref().map(|(m, _)| *m) != Some(self.mode) {
            let t = theme::theme(self.mode);
            let mut p = Pixmap::new(24, 24).unwrap();
            let mut pb = PathBuilder::new();
            pb.move_to(2.0, 2.0);
            pb.line_to(2.0, 19.0);
            pb.line_to(6.5, 15.0);
            pb.line_to(10.0, 22.0);
            pb.line_to(13.0, 20.5);
            pb.line_to(9.5, 14.0);
            pb.line_to(16.0, 14.0);
            pb.close();
            if let Some(path) = pb.finish() {
                p.fill_path(&path, &paint(t.ink_primary), FillRule::Winding, tiny_skia::Transform::identity(), None);
                let s = Stroke { width: 1.5, ..Default::default() };
                p.stroke_path(&path, &paint(t.surface_base), &s, tiny_skia::Transform::identity(), None);
            }
            self.cursor = Some((self.mode, upload(p)));
        }
        let (_, buf) = self.cursor.as_ref()?;
        MemoryRenderBufferRenderElement::from_buffer(renderer, at.to_physical(1.0), buf, None, None, None, Kind::Cursor).ok()
    }

    fn repaint(&mut self, key: &Key) {
        let t = theme::theme(key.mode);
        let (w, h) = key.size;
        self.hits.clear();

        // ---- Top bar: material + hairline, mark + name left, clock + switch right.
        let mut bar = Pixmap::new(w.max(1) as u32, theme::BAR_H as u32).unwrap();
        fill_rect(&mut bar, 0.0, 0.0, w as f32, theme::BAR_H as f32, t.surface_material);
        fill_rect(&mut bar, 0.0, theme::BAR_H as f32 - 1.0, w as f32, 1.0, t.border_subtle);
        draw_svg(&mut bar, REDROB_MARK, None, theme::SPACE_3 as f32, 7.0, 18.0);
        let x = theme::SPACE_3 as f32 + 18.0 + theme::SPACE_2 as f32;
        draw_text(&mut bar, &self.font, "Redrob OS", x, 21.0, 13.0, t.ink_primary);
        let switch_x = w - theme::SPACE_3 - 18;
        let icon = if key.mode == Mode::Dark { ICON_SUN } else { ICON_MOON };
        draw_svg(&mut bar, icon, Some(t.ink_primary), switch_x as f32, 7.0, 18.0);
        self.hits.push((Rectangle::new((switch_x - 6, 0).into(), (30, theme::BAR_H).into()), Action::ToggleTheme));
        let cw = text_width(&self.font, &key.clock, 13.0);
        draw_text(&mut bar, &self.font, &key.clock, switch_x as f32 - theme::SPACE_4 as f32 - cw, 21.0, 13.0, t.ink_primary);
        self.bar = Some(upload(bar));

        // ---- Dock: floating rounded material, launchers in rounded slots.
        let n = self.launchers.len() as i32;
        let pad = theme::SPACE_2;
        let dw = n * theme::SLOT + (n - 1) * theme::SPACE_2 + 2 * pad;
        let dh = theme::SLOT + 2 * pad;
        let mut dock = Pixmap::new(dw as u32, dh as u32).unwrap();
        let r = theme::RADIUS_2XL.min(dh as f32 / 2.0);
        fill_rrect(&mut dock, 0.5, 0.5, dw as f32 - 1.0, dh as f32 - 1.0, r, t.surface_material);
        stroke_rrect(&mut dock, 0.5, 0.5, dw as f32 - 1.0, dh as f32 - 1.0, r, t.border_subtle);
        let dx = (w - dw) / 2;
        let dy = h - dh - theme::SPACE_3;
        for (i, l) in self.launchers.iter().enumerate() {
            let sx = pad + i as i32 * (theme::SLOT + theme::SPACE_2);
            fill_rrect(&mut dock, sx as f32, pad as f32, theme::SLOT as f32, theme::SLOT as f32, theme::RADIUS_MD, t.surface_raised);
            draw_svg(&mut dock, l.icon, Some(t.ink_primary), sx as f32 + 12.0, pad as f32 + 12.0, 24.0);
            if key.running[i] {
                fill_rrect(&mut dock, sx as f32 + 18.0, dh as f32 - 5.0, 12.0, 3.0, theme::RADIUS_FULL.min(1.5), t.action_primary);
            }
            self.hits.push((
                Rectangle::new((dx + sx, dy + pad).into(), (theme::SLOT, theme::SLOT).into()),
                Action::Launch(i),
            ));
        }
        self.dock = Some((upload(dock), (dx, dy).into()));
    }

}

/// Dock launchers. Override with REDROB_DOCK="terminal=cmd args;folder=cmd;...".
fn default_launchers() -> Vec<Launcher> {
    let mut map: Vec<(&'static str, Vec<String>)> = vec![
        (ICON_TERMINAL, vec!["konsole".into()]),
        (ICON_FOLDER, vec!["dolphin".into()]),
        (ICON_BROWSER, vec!["xdg-open".into(), "https://redrob.ai".into()]),
        (ICON_SETTINGS, vec!["systemsettings".into()]),
        (ICON_CODE, vec!["kwrite".into()]),
    ];
    if let Ok(spec) = std::env::var("REDROB_DOCK") {
        for part in spec.split(';') {
            let Some((name, cmd)) = part.split_once('=') else { continue };
            let slot = match name {
                "terminal" => 0,
                "folder" => 1,
                "browser" => 2,
                "settings" => 3,
                "code" => 4,
                _ => continue,
            };
            map[slot].1 = cmd.split_whitespace().map(String::from).collect();
        }
    }
    map.into_iter().map(|(icon, command)| Launcher { icon, command, child: None }).collect()
}

fn clock_text() -> String {
    chrono::Local::now().format("%H:%M").to_string()
}

fn upload(p: Pixmap) -> MemoryRenderBuffer {
    // tiny-skia stores premultiplied RGBA bytes: that is DRM Abgr8888 (little-endian).
    let size = (p.width() as i32, p.height() as i32);
    MemoryRenderBuffer::from_slice(p.data(), Fourcc::Abgr8888, size, 1, Transform::Normal, None)
}

fn paint(c: Rgba) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c.0, c.1, c.2, c.3);
    p.anti_alias = true;
    p
}

fn fill_rect(p: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, c: Rgba) {
    if let Some(r) = tiny_skia::Rect::from_xywh(x, y, w, h) {
        p.fill_rect(r, &paint(c), tiny_skia::Transform::identity(), None);
    }
}

fn rrect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<tiny_skia::Path> {
    let r = r.min(w / 2.0).min(h / 2.0);
    let k = 0.552_284_8 * r; // cubic circle approximation
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish()
}

fn fill_rrect(p: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, c: Rgba) {
    if let Some(path) = rrect_path(x, y, w, h, r) {
        p.fill_path(&path, &paint(c), FillRule::Winding, tiny_skia::Transform::identity(), None);
    }
}

fn stroke_rrect(p: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, c: Rgba) {
    if let Some(path) = rrect_path(x, y, w, h, r) {
        let s = Stroke { width: 1.0, ..Default::default() };
        p.stroke_path(&path, &paint(c), &s, tiny_skia::Transform::identity(), None);
    }
}

/// Render an SVG into `p` at (x, y), `size` px square. `ink` replaces
/// `currentColor` (the redrob-ui icons are stroke="currentColor").
fn draw_svg(p: &mut Pixmap, svg: &str, ink: Option<Rgba>, x: f32, y: f32, size: f32) {
    let src = match ink {
        Some(c) => svg.replace("currentColor", &format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)),
        None => svg.to_string(),
    };
    let Ok(tree) = usvg::Tree::from_str(&src, &usvg::Options::default()) else { return };
    let s = tree.size();
    let scale = size / s.width().max(s.height());
    let ts = tiny_skia::Transform::from_translate(x, y).pre_scale(scale, scale);
    resvg::render(&tree, ts, &mut p.as_mut());
}

fn text_width(font: &fontdue::Font, s: &str, px: f32) -> f32 {
    s.chars().map(|c| font.metrics(c, px).advance_width).sum()
}

/// Draw a single line of text with its baseline at `baseline`.
fn draw_text(p: &mut Pixmap, font: &fontdue::Font, s: &str, x: f32, baseline: f32, px: f32, c: Rgba) {
    let (pw, ph) = (p.width() as i32, p.height() as i32);
    let mut pen = x;
    for ch in s.chars() {
        let (m, cov) = font.rasterize(ch, px);
        let gx = (pen + m.xmin as f32).round() as i32;
        let gy = (baseline - m.height as f32 - m.ymin as f32).round() as i32;
        let data = p.data_mut();
        for row in 0..m.height as i32 {
            for col in 0..m.width as i32 {
                let (tx, ty) = (gx + col, gy + row);
                if tx < 0 || ty < 0 || tx >= pw || ty >= ph {
                    continue;
                }
                let a = cov[(row * m.width as i32 + col) as usize] as u32 * c.3 as u32 / 255;
                if a == 0 {
                    continue;
                }
                let i = ((ty * pw + tx) * 4) as usize;
                // src-over in premultiplied space
                let inv = 255 - a;
                for (k, ch_v) in [c.0, c.1, c.2].iter().enumerate() {
                    data[i + k] = ((*ch_v as u32 * a + data[i + k] as u32 * inv) / 255) as u8;
                }
                data[i + 3] = (a + data[i + 3] as u32 * inv / 255) as u8;
            }
        }
        pen += m.advance_width;
    }
}
