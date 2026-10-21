//! Screenshots: render the frame (without the pointer) into an offscreen
//! buffer, read it back, crop, and save a PNG to the Pictures folder.

use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            Bind, ExportMem, Offscreen,
            damage::OutputDamageTracker,
            gles::{GlesRenderer, GlesTexture},
        },
    },
    utils::{Logical, Physical, Rectangle, Size, Transform},
};
use tiny_skia::Pixmap;

use crate::scene::ShellElement;

/// Render `elements` at `size` and return the pixels, top row first.
pub fn capture(renderer: &mut GlesRenderer, elements: &[ShellElement], size: Size<i32, Physical>, clear: [f32; 4]) -> Option<Pixmap> {
    let buf_size = Size::from((size.w, size.h));
    let mut tex: GlesTexture = renderer.create_buffer(Fourcc::Abgr8888, buf_size).ok()?;
    let mut fb = renderer.bind(&mut tex).ok()?;
    let mut tracker = OutputDamageTracker::new(size, 1.0, Transform::Normal);
    tracker.render_output(renderer, &mut fb, 0, elements, clear).ok()?;
    let region = Rectangle::from_size(buf_size);
    let mapping = renderer.copy_framebuffer(&fb, region, Fourcc::Abgr8888).ok()?;
    let data = renderer.map_texture(&mapping).ok()?.to_vec();
    drop(fb);
    let (w, h) = (size.w as usize, size.h as usize);
    if data.len() < w * h * 4 {
        return None;
    }
    // GL reads bottom-up: flip, and force alpha opaque (the frame is opaque).
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        let src = &data[(h - 1 - y) * w * 4..(h - y) * w * 4];
        let dst = &mut out[y * w * 4..(y + 1) * w * 4];
        dst.copy_from_slice(src);
        for px in dst.chunks_exact_mut(4) {
            px[3] = 255;
        }
    }
    Pixmap::from_vec(out, tiny_skia::IntSize::from_wh(w as u32, h as u32)?)
}

/// Crop to `region` (output logical == physical at scale 1) and save in the
/// background. Returns the file path that will be written.
pub fn save(img: Pixmap, region: Option<Rectangle<i32, Logical>>) -> Option<std::path::PathBuf> {
    let img = match region {
        Some(r) => {
            let r = r.intersection(Rectangle::from_size((img.width() as i32, img.height() as i32).into()))?;
            let rect = tiny_skia::IntRect::from_xywh(r.loc.x, r.loc.y, r.size.w as u32, r.size.h as u32)?;
            img.clone_rect(rect)?
        }
        None => img,
    };
    let dir = pictures_dir().join("Screenshots");
    let name = chrono::Local::now().format("스크린샷 %Y-%m-%d %H.%M.%S.png").to_string();
    let path = dir.join(name);
    let p = path.clone();
    std::thread::spawn(move || {
        let _ = std::fs::create_dir_all(&dir);
        match img.save_png(&p) {
            Ok(()) => tracing::info!(path = %p.display(), "screenshot saved"),
            Err(e) => tracing::warn!(?e, "screenshot save failed"),
        }
    });
    Some(path)
}

/// XDG_PICTURES_DIR from user-dirs.dirs, else ~/Pictures.
fn pictures_dir() -> std::path::PathBuf {
    let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let cfg = std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from).unwrap_or_else(|| home.join(".config"));
    if let Ok(t) = std::fs::read_to_string(cfg.join("user-dirs.dirs")) {
        for line in t.lines() {
            if let Some(v) = line.strip_prefix("XDG_PICTURES_DIR=") {
                let v = v.trim_matches('"').replace("$HOME", &home.to_string_lossy());
                return std::path::PathBuf::from(v);
            }
        }
    }
    home.join("Pictures")
}
