//! CPU drawing for the shell (tiny-skia): rounded shapes, soft shadows,
//! gradients, SVG icons (tinted or full-colour, cached), and text.

use std::collections::HashMap;

use resvg::usvg;
use smithay::{
    backend::{allocator::Fourcc, renderer::element::memory::MemoryRenderBuffer},
    utils::Transform,
};
use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Paint, PathBuilder, Pixmap, PixmapPaint, Point, Rect,
    Shader, SpreadMode, Stroke,
};

use crate::theme::Rgba;

// Crisp output scale: the shell lays out in logical pixels, but every
// pixmap is allocated and drawn at physical size (logical * k). Shapes go
// through `t()`, text and icons are rasterised at k times their size. Code
// that reads a pixmap's size uses `lw`/`lh` (logical).

/// The current output scale, as f32.
pub fn k() -> f32 {
    crate::display::scale() as f32
}

/// Logical -> physical transform for paths and rects.
pub fn t() -> tiny_skia::Transform {
    let k = k();
    tiny_skia::Transform::from_scale(k, k)
}

/// A transparent pixmap for a `w` x `h` logical area.
pub fn canvas(w: f32, h: f32) -> Option<Pixmap> {
    let k = k();
    Pixmap::new((w * k).ceil().max(1.0) as u32, (h * k).ceil().max(1.0) as u32)
}

/// Logical width / height of a pixmap made by `canvas`.
pub fn lw(p: &Pixmap) -> f32 {
    (p.width() as f32 / k()).round()
}
pub fn lh(p: &Pixmap) -> f32 {
    (p.height() as f32 / k()).round()
}

/// Draw `src` (already physical-size) into `dst` at logical (x, y).
pub fn blit(dst: &mut Pixmap, src: &Pixmap, x: f32, y: f32, opacity: f32) {
    let k = k();
    let pp = PixmapPaint { opacity, ..Default::default() };
    dst.draw_pixmap((x * k).round() as i32, (y * k).round() as i32, src.as_ref(), &pp, tiny_skia::Transform::identity(), None);
}

/// A physical-size shell buffer and the scale it was drawn at.
pub struct Buf {
    pub mem: MemoryRenderBuffer,
    pub px: (i32, i32),
    pub k: f32,
}

pub fn upload(p: &Pixmap) -> Buf {
    upload_opaque(p, false)
}

/// `opaque`: the whole buffer covers what is behind it (the desktop ground).
pub fn upload_opaque(p: &Pixmap, opaque: bool) -> Buf {
    // tiny-skia stores premultiplied RGBA bytes: that is DRM Abgr8888 (little-endian).
    let size = (p.width() as i32, p.height() as i32);
    let full = opaque.then(|| vec![smithay::utils::Rectangle::<i32, smithay::utils::Buffer>::from_size(size.into())]);
    Buf { mem: MemoryRenderBuffer::from_slice(p.data(), Fourcc::Abgr8888, size, 1, Transform::Normal, full), px: size, k: k() }
}

/// A render element for `b` with its top-left at logical `at`: the whole
/// buffer, shown at its logical size (so it is pixel-exact at the scale it
/// was drawn at).
pub fn element(
    renderer: &mut smithay::backend::renderer::gles::GlesRenderer,
    at: smithay::utils::Point<f64, smithay::utils::Logical>,
    b: &Buf,
    kind: smithay::backend::renderer::element::Kind,
) -> Option<smithay::backend::renderer::element::memory::MemoryRenderBufferRenderElement<smithay::backend::renderer::gles::GlesRenderer>> {
    let src = smithay::utils::Rectangle::<f64, smithay::utils::Logical>::from_size((b.px.0 as f64, b.px.1 as f64).into());
    let size: smithay::utils::Size<i32, smithay::utils::Logical> =
        ((b.px.0 as f32 / b.k).round() as i32, (b.px.1 as f32 / b.k).round() as i32).into();
    smithay::backend::renderer::element::memory::MemoryRenderBufferRenderElement::from_buffer(
        renderer,
        at.to_physical(crate::display::scale()),
        &b.mem,
        None,
        Some(src),
        Some(size),
        kind,
    )
    .ok()
}

pub fn paint(c: Rgba) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c.0, c.1, c.2, c.3);
    p.anti_alias = true;
    p
}

pub fn with_alpha(c: Rgba, a: u8) -> Rgba {
    Rgba(c.0, c.1, c.2, a)
}

pub fn fill_rect(p: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, c: Rgba) {
    if let Some(r) = Rect::from_xywh(x, y, w, h) {
        p.fill_rect(r, &paint(c), t(), None);
    }
}

/// Rounded rectangle; `top_only` keeps the bottom corners square (title bars).
pub fn rrect_path(x: f32, y: f32, w: f32, h: f32, r: f32, top_only: bool) -> Option<tiny_skia::Path> {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let rb = if top_only { 0.0 } else { r };
    let k = 0.552_284_8; // cubic circle approximation
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k * r, y, x + w, y + r - k * r, x + w, y + r);
    pb.line_to(x + w, y + h - rb);
    pb.cubic_to(x + w, y + h - rb + k * rb, x + w - rb + k * rb, y + h, x + w - rb, y + h);
    pb.line_to(x + rb, y + h);
    pb.cubic_to(x + rb - k * rb, y + h, x, y + h - rb + k * rb, x, y + h - rb);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k * r, x + r - k * r, y, x + r, y);
    pb.close();
    pb.finish()
}

pub fn fill_rrect(p: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, c: Rgba) {
    if let Some(path) = rrect_path(x, y, w, h, r, false) {
        p.fill_path(&path, &paint(c), FillRule::Winding, t(), None);
    }
}

pub fn stroke_rrect(p: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, c: Rgba) {
    if let Some(path) = rrect_path(x, y, w, h, r, false) {
        let s = Stroke { width: 1.0, ..Default::default() };
        p.stroke_path(&path, &paint(c), &s, t(), None);
    }
}

pub fn fill_circle(p: &mut Pixmap, cx: f32, cy: f32, r: f32, c: Rgba) {
    if let Some(path) = PathBuilder::from_circle(cx, cy, r) {
        p.fill_path(&path, &paint(c), FillRule::Winding, t(), None);
    }
}

pub fn line(p: &mut Pixmap, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, c: Rgba) {
    let mut pb = PathBuilder::new();
    pb.move_to(x0, y0);
    pb.line_to(x1, y1);
    if let Some(path) = pb.finish() {
        let s = Stroke { width, ..Default::default() };
        p.stroke_path(&path, &paint(c), &s, t(), None);
    }
}

/// A soft drop shadow under a rounded rect: `blur` px of falloff, offset
/// `dy` down, peak colour `c` (the token's rgba). Stacked translucent rounded
/// rects approximate the CSS gaussian closely enough at these sizes.
pub fn shadow(p: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, blur: f32, dy: f32, c: Rgba) {
    let steps = (blur / 2.0).ceil().max(1.0) as i32;
    for i in 0..steps {
        let t = i as f32 / steps as f32; // 0 = outermost
        let grow = blur * (1.0 - t) * 0.6;
        let a = (c.3 as f32 * (1.0 / steps as f32) * (0.35 + t)).min(255.0) as u8;
        fill_rrect(p, x - grow, y + dy - grow, w + 2.0 * grow, h + 2.0 * grow, r + grow, with_alpha(c, a));
    }
}

/// A 130deg linear gradient (the design system's brand angle) over the pixmap.
pub fn fill_gradient(p: &mut Pixmap, stops: &[(f32, u32)]) {
    let (w, h) = (p.width() as f32, p.height() as f32);
    // CSS 130deg: the gradient line runs from upper-left toward lower-right.
    let a = 130f32.to_radians();
    let (dx, dy) = (a.sin(), -a.cos());
    let half = (w * dx.abs() + h * dy.abs()) / 2.0;
    let (cx, cy) = (w / 2.0, h / 2.0);
    let start = Point::from_xy(cx - dx * half, cy - dy * half);
    let end = Point::from_xy(cx + dx * half, cy + dy * half);
    let stops: Vec<GradientStop> = stops
        .iter()
        .map(|(pos, c)| GradientStop::new(*pos, Color::from_rgba8((c >> 16) as u8, (c >> 8) as u8, *c as u8, 255)))
        .collect();
    if let Some(shader) = LinearGradient::new(start, end, stops, SpreadMode::Pad, tiny_skia::Transform::identity()) {
        let paint = Paint { shader, anti_alias: true, ..Default::default() };
        if let Some(r) = Rect::from_xywh(0.0, 0.0, w, h) {
            p.fill_rect(r, &paint, tiny_skia::Transform::identity(), None);
        }
    }
}

/// Vertical gradient inside a rounded rect (system app tiles).
pub fn fill_rrect_vgrad(p: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, top: Rgba, bottom: Rgba) {
    let Some(path) = rrect_path(x, y, w, h, r, false) else { return };
    let c = |c: Rgba| Color::from_rgba8(c.0, c.1, c.2, c.3);
    let Some(shader) = LinearGradient::new(
        Point::from_xy(x, y),
        Point::from_xy(x, y + h),
        vec![GradientStop::new(0.0, c(top)), GradientStop::new(1.0, c(bottom))],
        SpreadMode::Pad,
        tiny_skia::Transform::identity(),
    ) else {
        return;
    };
    let paint: Paint<'_> = Paint { shader, anti_alias: true, ..Default::default() };
    let _: &Shader = &paint.shader;
    p.fill_path(&path, &paint, FillRule::Winding, t(), None);
}

/// Cover-scale `src` into `dst` (centre crop), like a wallpaper.
pub fn draw_cover(dst: &mut Pixmap, src: &Pixmap) {
    let (dw, dh) = (dst.width() as f32, dst.height() as f32);
    let (sw, sh) = (src.width() as f32, src.height() as f32);
    let s = (dw / sw).max(dh / sh);
    let tx = (dw - sw * s) / 2.0;
    let ty = (dh - sh * s) / 2.0;
    let pp = PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() };
    dst.draw_pixmap(0, 0, src.as_ref(), &pp, tiny_skia::Transform::from_row(s, 0.0, 0.0, s, tx, ty), None);
}

/// Rendered SVGs, keyed by (source pointer, tint, size): the app icons are
/// 1024-unit masked tiles and cost real time to rasterise every repaint.
#[derive(Default)]
pub struct IconCache {
    map: HashMap<(usize, Option<(u8, u8, u8, u8)>, u32), Pixmap>,
}

impl IconCache {
    /// Draw `svg` into `p` at (x, y), `size` px square. `ink` replaces
    /// `currentColor` (the redrob-ui glyphs are stroke="currentColor").
    pub fn draw(&mut self, p: &mut Pixmap, svg: &'static str, ink: Option<Rgba>, x: f32, y: f32, size: f32) {
        let kk = k();
        let (x, y) = (x * kk, y * kk);
        let px = (size * kk).round().max(1.0) as u32;
        let key = (svg.as_ptr() as usize, ink.map(|c| (c.0, c.1, c.2, c.3)), px);
        if !self.map.contains_key(&key) {
            let Some(img) = render_svg(svg, ink, px) else { return };
            self.map.insert(key, img);
        }
        let img = &self.map[&key];
        let pp = PixmapPaint { opacity: ink.map(|c| c.3 as f32 / 255.0).unwrap_or(1.0), ..Default::default() };
        p.draw_pixmap(x.round() as i32, y.round() as i32, img.as_ref(), &pp, tiny_skia::Transform::identity(), None);
    }
}

fn render_svg(svg: &str, ink: Option<Rgba>, px: u32) -> Option<Pixmap> {
    let src = match ink {
        Some(c) => svg.replace("currentColor", &format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)),
        None => svg.to_string(),
    };
    let tree = usvg::Tree::from_str(&src, &usvg::Options::default()).ok()?;
    let s = tree.size();
    let scale = px as f32 / s.width().max(s.height());
    let mut out = Pixmap::new(px, px)?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut out.as_mut());
    Some(out)
}

pub struct Fonts {
    pub semibold: fontdue::Font,
    pub medium: fontdue::Font,
}

pub fn text_width(font: &fontdue::Font, s: &str, px: f32) -> f32 {
    s.chars().map(|c| font.metrics(c, px).advance_width).sum()
}

/// Draw a single line of text with its baseline at `baseline` (logical
/// units; rasterised at the output scale).
pub fn draw_text(p: &mut Pixmap, font: &fontdue::Font, s: &str, x: f32, baseline: f32, px: f32, c: Rgba) {
    let kk = k();
    let (x, baseline, px) = (x * kk, baseline * kk, px * kk);
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

/// Text truncated with an ellipsis to fit `max` px.
pub fn fit_text(font: &fontdue::Font, s: &str, px: f32, max: f32) -> String {
    if text_width(font, s, px) <= max {
        return s.to_string();
    }
    let mut out = String::new();
    for ch in s.chars() {
        let next = format!("{out}{ch}…");
        if text_width(font, &next, px) > max {
            break;
        }
        out.push(ch);
    }
    out + "…"
}
