//! Redrob shell chrome, drawn by the compositor itself (no Plasma, no COSMIC),
//! from the redrob-ui design system: its colour tokens, radii and shadows, its
//! glyph set, and the Redrob app icons (the masked superellipse tiles).
//!
//! Two layouts from `config::Style`:
//! - Mac: a translucent menu bar on top (Redrob symbol, the active app's name;
//!   search, Wi-Fi, volume, the light/dark switch and the Korean date-time at
//!   the right) and a floating dock of app tiles with a name tip on hover.
//! - Windows: one taskbar at the bottom with the start button, a search pill
//!   and app icons centred as a group; tray glyphs and time over date at the
//!   right.
//!
//! Painted on the CPU with tiny-skia, uploaded as textures. Colours come from
//! `theme.rs`.

use std::process::{Child, Command};

use smithay::{
    backend::renderer::{
        element::{
            Kind,
            memory::MemoryRenderBufferRenderElement,
        },
        gles::GlesRenderer,
    },
    utils::{Logical, Physical, Point, Rectangle, Size},
};
use tiny_skia::{FillRule, PathBuilder, Pixmap, Stroke};

use crate::config::{Act, Combo, Config, Style};
use crate::draw::{self, Fonts, IconCache, fill_circle, fill_rect, fill_rrect, stroke_rrect, upload, with_alpha};
use crate::theme::{self, Mode};

pub const ICON_TERMINAL: &str = include_str!("../assets/icons/terminal.svg");
pub const ICON_FOLDER: &str = include_str!("../assets/icons/folder.svg");
pub const ICON_SETTINGS: &str = include_str!("../assets/icons/settings.svg");
const ICON_SUN: &str = include_str!("../assets/icons/sun.svg");
const ICON_MOON: &str = include_str!("../assets/icons/moon.svg");
const ICON_SEARCH: &str = include_str!("../assets/icons/search.svg");
const ICON_WIFI: &str = include_str!("../assets/icons/wifi.svg");
const ICON_VOLUME: &str = include_str!("../assets/icons/volume.svg");
const ICON_WIFI_OFF: &str = include_str!("../assets/icons/wifi-off.svg");
const ICON_VOLUME_OFF: &str = include_str!("../assets/icons/volume-off.svg");

/// Redrob app icons, (light, dark) "icon-web" masters from redrob-ui.
const APP_REDROB: (&str, &str) =
    (include_str!("../assets/app-icons/redrob-icon-web.svg"), include_str!("../assets/app-icons/redrob-icon-web-dark.svg"));
const APP_BROWSER: (&str, &str) =
    (include_str!("../assets/app-icons/browser-icon-web.svg"), include_str!("../assets/app-icons/browser-icon-web-dark.svg"));
const APP_CODE: (&str, &str) =
    (include_str!("../assets/app-icons/code-icon-web.svg"), include_str!("../assets/app-icons/code-icon-web-dark.svg"));
const APP_OFFICE: (&str, &str) =
    (include_str!("../assets/app-icons/office-icon-web.svg"), include_str!("../assets/app-icons/office-icon-web-dark.svg"));
const REDROB_SYMBOL: &str = include_str!("../assets/app-icons/redrob-symbol.svg");

const FONT_SEMIBOLD: &[&str] = &["/usr/share/fonts/redrob/pretendard/Pretendard-SemiBold.otf"];
const FONT_MEDIUM: &[&str] = &[
    "/usr/share/fonts/redrob/pretendard/Pretendard-Medium.otf",
    "/usr/share/fonts/redrob/pretendard/Pretendard-SemiBold.otf",
];
const WALLPAPERS: &[&str] = &[
    "/usr/share/wallpapers/Redrob/contents/images/1920x1080.png",
    "/usr/share/wallpapers/Redrob/contents/images/2560x1440.png",
    "/usr/share/wallpapers/Redrob/contents/images/3840x2160.png",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Launch(usize),
    ToggleTheme,
    /// Start button / search / Command-Space: the app finder.
    Start,
    /// The volume glyph: mute / unmute.
    Volume,
    /// The network glyph: network settings.
    Network,
}

#[derive(Clone, Copy)]
enum Tile {
    /// A Redrob product: its own icon tile.
    App((&'static str, &'static str)),
    /// A system app: a neutral tile with a redrob-ui glyph.
    System(&'static str),
}

struct Launcher {
    name: &'static str,
    tile: Tile,
    command: Vec<String>,
    child: Option<Child>,
    /// Starts a separator before it in the dock.
    group: bool,
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
    style: Style,
    mode: Mode,
    size: (i32, i32),
    clock: (String, String),
    running: Vec<bool>,
    hover: Option<Action>,
    app: String,
    status: crate::status::Status,
}

pub struct Chrome {
    pub mode: Mode,
    pub style: Style,
    shortcuts: Vec<(Combo, Act)>,
    stamp: Option<(std::path::PathBuf, std::time::SystemTime)>,
    pub fonts: Fonts,
    pub icons: IconCache,
    launchers: Vec<Launcher>,
    key: Option<Key>,
    bar: Option<(draw::Buf, Point<i32, Logical>)>,
    dock: Option<(draw::Buf, Point<i32, Logical>)>,
    cursor: Option<(Mode, f32, draw::Buf)>,
    ground_px: Option<((Mode, i32, i32, u32), draw::Buf)>,
    wallpaper: Option<Option<Pixmap>>,
    frames: u32,
    ticks: u32,
    hits: Vec<(Rectangle<i32, Logical>, Action)>,
    hover: Option<Action>,
    /// Name of the focused app, for the Mac menu bar.
    pub app: String,
    pub status: crate::status::Status,
    /// The settings tile was clicked; the compositor opens the window.
    pub want_settings: bool,
}

fn load_font(env: &str, paths: &[&str]) -> fontdue::Font {
    let bytes = std::env::var(env)
        .ok()
        .into_iter()
        .chain(paths.iter().map(|s| s.to_string()))
        .find_map(|p| std::fs::read(p).ok())
        .expect("no Pretendard face found (set REDROB_FONT)");
    fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).expect("font parse")
}

impl Chrome {
    pub fn new() -> Self {
        let fonts = Fonts { semibold: load_font("REDROB_FONT", FONT_SEMIBOLD), medium: load_font("REDROB_FONT_MEDIUM", FONT_MEDIUM) };
        let cfg = Config::load();
        tracing::info!(style = ?cfg.style, theme = ?cfg.theme, shortcuts = cfg.shortcuts.len(), "desktop settings");
        Self {
            mode: cfg.theme,
            style: cfg.style,
            shortcuts: cfg.shortcuts,
            stamp: Config::stamp(),
            fonts,
            icons: IconCache::default(),
            launchers: default_launchers(),
            key: None,
            bar: None,
            dock: None,
            cursor: None,
            ground_px: None,
            wallpaper: None,
            frames: 0,
            ticks: 0,
            hits: vec![],
            hover: None,
            app: String::new(),
            status: Default::default(),
            want_settings: false,
        }
    }

    /// What the pointer hit, in output-logical coordinates, if anything.
    pub fn hit(&self, p: Point<f64, Logical>) -> Option<Action> {
        let p = p.to_i32_round();
        self.hits.iter().find(|(r, _)| r.contains(p)).map(|(_, a)| *a)
    }

    /// Track the pointer for hover states (dock name tips, taskbar wash).
    pub fn pointer(&mut self, p: Point<f64, Logical>) {
        self.hover = self.hit(p);
    }

    pub fn act(&mut self, action: Action) {
        match action {
            Action::ToggleTheme => self.toggle_theme(),
            Action::Start => {} // the compositor opens the finder (input.rs)
            Action::Volume => crate::status::toggle_mute(),
            Action::Network => crate::shell::spawn(&["systemsettings", "kcm_networkmanagement"]),
            Action::Launch(i) => {
                let l = &mut self.launchers[i];
                if l.command.first().map(String::as_str) == Some(SETTINGS) {
                    self.want_settings = true;
                    return;
                }
                if l.running() {
                    return;
                }
                if let Some((prog, args)) = l.command.split_first() {
                    l.child = Command::new(prog).args(args).spawn().ok();
                }
            }
        }
    }

    pub fn toggle_theme(&mut self) {
        self.mode = self.mode.toggled();
        Config::save_theme(self.mode);
        self.stamp = Config::stamp(); // our own write is not an outside edit
        apply_qt_colors(self.mode);
    }

    pub fn shortcuts(&self) -> &[(Combo, Act)] {
        &self.shortcuts
    }

    /// Re-read the settings file now (the settings window just wrote it).
    pub fn reload_now(&mut self) {
        self.stamp = None;
        self.reload_if_changed();
    }

    /// Re-read the settings file when it changed on disk (a settings app or an
    /// editor wrote it): style, theme and shortcuts apply live.
    fn reload_if_changed(&mut self) {
        let now = Config::stamp();
        if now == self.stamp {
            return;
        }
        self.stamp = now;
        let cfg = Config::load();
        tracing::info!(style = ?cfg.style, theme = ?cfg.theme, "desktop settings changed");
        if cfg.theme != self.mode {
            apply_qt_colors(cfg.theme);
        }
        if cfg.style != self.style {
            apply_button_layout(cfg.style);
        }
        self.style = cfg.style;
        self.mode = cfg.theme;
        self.shortcuts = cfg.shortcuts;
    }

    /// Logical pixels the chrome keeps for itself at the (top, bottom) of the
    /// output, so maximized and snapped windows stop short of it. The Mac dock
    /// floats over windows, like the real one, so it reserves nothing.
    pub fn reserved(&self) -> (i32, i32) {
        match self.style {
            Style::Mac => (theme::BAR_H, 0),
            Style::Windows => (0, theme::TASKBAR_H),
        }
    }

    /// True about once every 4 s (one frame in 240), for a light frame trace.
    pub fn frame_log_due(&mut self) -> bool {
        self.frames = self.frames.wrapping_add(1);
        self.frames % 240 == 1
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
        self.ticks = self.ticks.wrapping_add(1);
        if self.ticks % 60 == 0 {
            self.reload_if_changed();
        }
        let running: Vec<bool> = self.launchers.iter_mut().map(|l| l.running()).collect();
        let key = Key {
            style: self.style,
            mode: self.mode,
            size: (out.w, out.h),
            clock: clock_text(),
            running,
            hover: self.hover,
            app: self.app.clone(),
            status: self.status.clone(),
        };
        if self.key.as_ref() != Some(&key) {
            self.repaint(&key);
            self.key = Some(key);
        }
        let mut v = Vec::new();
        for (buf, loc) in [&self.dock, &self.bar].into_iter().flatten() {
            if let Some(e) = draw::element(renderer, loc.to_f64(), buf, Kind::Unspecified) {
                v.push(e);
            }
        }
        v
    }

    /// The desktop ground as the back-most element: the Redrob wallpaper
    /// (Redrob Black crossed by the brand seam) in dark, the design system's
    /// --gradient-light in light. Opaque, so the DRM compositor treats it as
    /// the bottom of the stack.
    pub fn ground_element(
        &mut self,
        renderer: &mut GlesRenderer,
        out: Size<i32, Physical>,
    ) -> Option<MemoryRenderBufferRenderElement<GlesRenderer>> {
        let key = (self.mode, out.w, out.h, draw::k().to_bits());
        if self.ground_px.as_ref().map(|(k, _)| *k) != Some(key) {
            let t = theme::theme(self.mode);
            let mut p = draw::canvas(out.w as f32, out.h as f32)?;
            let wall = if self.mode == Mode::Dark { self.wallpaper(p.width() as i32, p.height() as i32) } else { None };
            match wall {
                Some(img) => draw::draw_cover(&mut p, img),
                None => draw::fill_gradient(&mut p, t.ground),
            }
            self.ground_px = Some((key, draw::upload_opaque(&p, true)));
        }
        let (_, buf) = self.ground_px.as_ref()?;
        draw::element(renderer, (0.0, 0.0).into(), buf, Kind::Unspecified)
    }

    /// A copy of the wallpaper for other layers (the lock screen).
    pub fn wallpaper_pixmap(&mut self, w: i32, h: i32) -> Option<Pixmap> {
        self.wallpaper(w, h).cloned()
    }

    /// The smallest packaged wallpaper that covers the output (or
    /// REDROB_WALLPAPER), decoded once.
    fn wallpaper(&mut self, w: i32, h: i32) -> Option<&Pixmap> {
        if self.wallpaper.is_none() {
            let mut paths: Vec<String> = std::env::var("REDROB_WALLPAPER").ok().into_iter().collect();
            let fit: Vec<&str> = WALLPAPERS
                .iter()
                .copied()
                .filter(|p| {
                    let dims = p.rsplit('/').next().and_then(|f| f.strip_suffix(".png")).and_then(|d| d.split_once('x'));
                    dims.and_then(|(a, b)| Some((a.parse::<i32>().ok()?, b.parse::<i32>().ok()?)))
                        .is_some_and(|(a, b)| a >= w && b >= h)
                })
                .collect();
            paths.extend(fit.iter().chain(WALLPAPERS.iter().rev()).map(|s| s.to_string()));
            let img = paths.iter().find_map(|p| Pixmap::load_png(p).ok());
            tracing::info!(found = img.is_some(), "wallpaper");
            self.wallpaper = Some(img);
        }
        self.wallpaper.as_ref().and_then(|w| w.as_ref())
    }

    /// The pointer arrow (DRM backend only; nested runs use the host cursor):
    /// ink-primary fill with a surface-base outline so it reads on both grounds.
    pub fn cursor_element(
        &mut self,
        renderer: &mut GlesRenderer,
        at: Point<f64, Logical>,
    ) -> Option<MemoryRenderBufferRenderElement<GlesRenderer>> {
        if self.cursor.as_ref().map(|(m, k, _)| (*m, *k)) != Some((self.mode, draw::k())) {
            let t = theme::theme(self.mode);
            let mut p = draw::canvas(24.0, 24.0).unwrap();
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
                p.fill_path(&path, &draw::paint(t.ink_primary), FillRule::Winding, draw::t(), None);
                let s = Stroke { width: 1.5, ..Default::default() };
                p.stroke_path(&path, &draw::paint(t.surface_base), &s, draw::t(), None);
            }
            self.cursor = Some((self.mode, draw::k(), upload(&p)));
        }
        let (_, _, buf) = self.cursor.as_ref()?;
        draw::element(renderer, at, buf, Kind::Cursor)
    }

    fn repaint(&mut self, key: &Key) {
        self.hits.clear();
        match key.style {
            Style::Mac => {
                self.paint_menu_bar(key);
                self.paint_dock(key);
            }
            Style::Windows => self.paint_taskbar(key),
        }
    }

    /// Draw a launcher's tile: the product icon, or a neutral system tile.
    fn tile(&mut self, p: &mut Pixmap, tile: Tile, mode: Mode, x: f32, y: f32, size: f32) {
        let t = theme::theme(mode);
        match tile {
            Tile::App((light, dark)) => {
                self.icons.draw(p, if mode == Mode::Dark { dark } else { light }, None, x, y, size);
            }
            Tile::System(glyph) => {
                // The app-icon superellipse: ~22.5% corner radius, like the masters.
                let r = size * 0.225;
                let (top, bottom) = match mode {
                    Mode::Dark => (theme::hexa(0x2a2f38), theme::hexa(0x16191d)),
                    Mode::Light => (theme::hexa(0xffffff), theme::hexa(0xe7eaf0)),
                };
                let inset = size * 0.04;
                draw::fill_rrect_vgrad(p, x + inset, y + inset, size - 2.0 * inset, size - 2.0 * inset, r, top, bottom);
                stroke_rrect(p, x + inset + 0.5, y + inset + 0.5, size - 2.0 * inset - 1.0, size - 2.0 * inset - 1.0, r, t.border_subtle);
                let g = size * 0.5;
                self.icons.draw(p, glyph, Some(t.ink_primary), x + (size - g) / 2.0, y + (size - g) / 2.0, g);
            }
        }
    }

    // ---------------------------------------------------------------- Mac

    fn paint_menu_bar(&mut self, key: &Key) {
        let t = theme::theme(key.mode);
        let (w, _) = key.size;
        let bh = theme::BAR_H;
        let mut bar = draw::canvas(w.max(1) as f32, bh as f32).unwrap();
        fill_rect(&mut bar, 0.0, 0.0, w as f32, bh as f32, t.surface_material);
        fill_rect(&mut bar, 0.0, bh as f32 - 1.0, w as f32, 1.0, with_alpha(t.border_subtle, 160));
        let mid = bh as f32 / 2.0;

        // Left: the Redrob symbol (opens the Launcher), then the app in front.
        let sym = 16.0;
        let sym_x = 16.0;
        self.symbol(&mut bar, key.mode, sym_x, mid - sym / 2.0, sym);
        self.hits.push((Rectangle::new((6, 0).into(), (36, bh).into()), Action::Start));
        let name = if key.app.is_empty() { "Redrob" } else { key.app.as_str() };
        draw::draw_text(&mut bar, &self.fonts.semibold, name, sym_x + sym + 18.0, mid + 4.5, 13.0, t.ink_primary);

        // Right, from the edge: date-time, theme switch, volume, Wi-Fi, search.
        let mut x = w as f32 - 16.0;
        let clock = format!("{}  {}", key.clock.1, key.clock.0);
        let cw = draw::text_width(&self.fonts.medium, &clock, 13.0);
        x -= cw;
        draw::draw_text(&mut bar, &self.fonts.medium, &clock, x, mid + 4.5, 13.0, t.ink_primary);
        if let Some((pct, charging)) = key.status.battery {
            x -= 14.0 + 26.0;
            battery(&mut bar, x, mid - 6.0, pct, charging, &t);
            let label = format!("{pct}%");
            let lw = draw::text_width(&self.fonts.medium, &label, 12.0);
            x -= 6.0 + lw;
            draw::draw_text(&mut bar, &self.fonts.medium, &label, x, mid + 4.0, 12.0, t.ink_primary);
        }
        let g = 16.0;
        let glyphs: [(&'static str, Option<Action>); 4] = [
            (if key.mode == Mode::Dark { ICON_SUN } else { ICON_MOON }, Some(Action::ToggleTheme)),
            (vol_glyph(&key.status), Some(Action::Volume)),
            (net_glyph(&key.status), Some(Action::Network)),
            (ICON_SEARCH, Some(Action::Start)),
        ];
        for (glyph, action) in glyphs {
            x -= 14.0 + g;
            let hx = x as i32 - 6;
            if let Some(a) = action {
                if key.hover == Some(a) {
                    fill_rrect(&mut bar, hx as f32, 4.0, g + 12.0, bh as f32 - 8.0, theme::RADIUS_SM, t.hover);
                }
                self.hits.push((Rectangle::new((hx, 0).into(), (g as i32 + 12, bh).into()), a));
            }
            self.icons.draw(&mut bar, glyph, Some(t.ink_primary), x, mid - g / 2.0, g);
        }
        self.bar = Some((upload(&bar), (0, 0).into()));
    }

    /// The Redrob symbol in ink (the master is white-on-transparent).
    fn symbol(&mut self, p: &mut Pixmap, mode: Mode, x: f32, y: f32, size: f32) {
        let svg: &'static str = if mode == Mode::Dark { REDROB_SYMBOL } else { symbol_dark_ink() };
        self.icons.draw(p, svg, None, x, y, size);
    }

    fn paint_dock(&mut self, key: &Key) {
        let t = theme::theme(key.mode);
        let (w, h) = key.size;
        let tile = 52.0;
        let gap = 6.0;
        let pad = 8.0;
        let sep = 17.0; // 1px rule with 8px either side
        let n = self.launchers.len();
        let groups = self.launchers.iter().filter(|l| l.group).count() as f32;
        let dw = pad * 2.0 + n as f32 * tile + (n as f32 - 1.0) * gap + groups * (sep - gap);
        let dh = tile + pad * 2.0;
        // Room around the dock for its shadow and above it for the name tip.
        let (m, tip) = (28.0, 40.0);
        let bw = dw + 2.0 * m;
        let bh = dh + m + tip + 4.0;
        let mut p = draw::canvas(bw.ceil(), bh.ceil()).unwrap();
        let (x0, y0) = (m, tip + 4.0);
        let r = theme::RADIUS_2XL.min(dh / 2.0);
        draw::shadow(&mut p, x0, y0, dw, dh, r, 24.0, 8.0, t.shadow_lg);
        fill_rrect(&mut p, x0, y0, dw, dh, r, t.surface_material);
        stroke_rrect(&mut p, x0 + 0.5, y0 + 0.5, dw - 1.0, dh - 1.0, r, t.border_subtle);

        let bx = (w as f32 - bw) / 2.0;
        let by = h as f32 - bh - 6.0 + m; // dock bottom 6px over the screen edge
        let mut x = x0 + pad;
        let mut tip_at: Option<(f32, &'static str)> = None;
        for i in 0..n {
            if self.launchers[i].group {
                let rx = x + (sep - gap) / 2.0 - 0.5;
                draw::line(&mut p, rx, y0 + 14.0, rx, y0 + dh - 14.0, 1.0, t.border_subtle);
                x += sep - gap;
            }
            let ty = y0 + pad;
            let tl = self.launchers[i].tile;
            self.tile(&mut p, tl, key.mode, x, ty, tile);
            if key.running[i] {
                fill_circle(&mut p, x + tile / 2.0, y0 + dh - 3.5, 2.0, t.ink_primary);
            }
            let hit = Rectangle::new(
                ((bx + x).round() as i32, (by + ty).round() as i32).into(),
                (tile as i32, tile as i32).into(),
            );
            self.hits.push((hit, Action::Launch(i)));
            if key.hover == Some(Action::Launch(i)) {
                tip_at = Some((x + tile / 2.0, self.launchers[i].name));
            }
            x += tile + gap;
        }
        // Name tip over the hovered tile: a small material pill.
        if let Some((cx, name)) = tip_at {
            let tw = draw::text_width(&self.fonts.medium, name, 12.0) + 20.0;
            let tx = (cx - tw / 2.0).clamp(2.0, bw - tw - 2.0);
            let (th, tyy) = (26.0, 6.0);
            draw::shadow(&mut p, tx, tyy, tw, th, theme::RADIUS_MD, 10.0, 3.0, t.shadow_md);
            fill_rrect(&mut p, tx, tyy, tw, th, theme::RADIUS_MD, t.surface_raised);
            stroke_rrect(&mut p, tx + 0.5, tyy + 0.5, tw - 1.0, th - 1.0, theme::RADIUS_MD, t.border_subtle);
            draw::draw_text(&mut p, &self.fonts.medium, name, tx + 10.0, tyy + 17.5, 12.0, t.ink_primary);
        }
        self.dock = Some((upload(&p), (bx.round() as i32, by.round() as i32).into()));
    }

    // ---------------------------------------------------------------- Windows

    fn paint_taskbar(&mut self, key: &Key) {
        self.dock = None;
        let t = theme::theme(key.mode);
        let (w, h) = key.size;
        let th = theme::TASKBAR_H;
        let y0 = h - th; // taskbar top, in output coordinates
        let mut bar = draw::canvas(w.max(1) as f32, th as f32).unwrap();
        fill_rect(&mut bar, 0.0, 0.0, w as f32, th as f32, t.surface_material);
        fill_rect(&mut bar, 0.0, 0.0, w as f32, 1.0, with_alpha(t.border_subtle, 200));

        let slot = theme::TASK_SLOT as f32;
        let icon = 26.0;
        let gap = 4.0;
        let search_w = 196.0;
        let n = self.launchers.len() as f32;
        let group = slot + gap + search_w + gap + n * slot + (n - 1.0) * gap;
        let mut x = ((w as f32 - group) / 2.0).round();
        let sy = ((th as f32 - slot) / 2.0).round();
        let wash = |p: &mut Pixmap, x: f32, wd: f32, on: bool| {
            if on {
                fill_rrect(p, x, sy, wd, slot, theme::RADIUS_SM, t.hover);
            }
        };

        // Start: the Redrob app icon.
        wash(&mut bar, x, slot, key.hover == Some(Action::Start));
        let (light, dark) = APP_REDROB;
        self.icons.draw(&mut bar, if key.mode == Mode::Dark { dark } else { light }, None, x + (slot - icon) / 2.0, sy + (slot - icon) / 2.0, icon);
        self.hits.push((Rectangle::new((x as i32, y0 + sy as i32).into(), (slot as i32, slot as i32).into()), Action::Start));
        x += slot + gap;

        // Search pill (opens the Launcher too, like Windows search).
        let ph = 32.0;
        let py = ((th as f32 - ph) / 2.0).round();
        fill_rrect(&mut bar, x, py, search_w, ph, theme::RADIUS_FULL, t.surface_sunken);
        stroke_rrect(&mut bar, x + 0.5, py + 0.5, search_w - 1.0, ph - 1.0, ph / 2.0, t.border_subtle);
        self.icons.draw(&mut bar, ICON_SEARCH, Some(t.ink_secondary), x + 12.0, py + 8.0, 16.0);
        draw::draw_text(&mut bar, &self.fonts.medium, "검색", x + 36.0, py + 21.0, 13.0, t.ink_muted);
        self.hits.push((Rectangle::new((x as i32, y0 + py as i32).into(), (search_w as i32, ph as i32).into()), Action::Start));
        x += search_w + gap;

        for i in 0..self.launchers.len() {
            wash(&mut bar, x, slot, key.hover == Some(Action::Launch(i)));
            let tl = self.launchers[i].tile;
            self.tile(&mut bar, tl, key.mode, x + (slot - icon) / 2.0, sy + (slot - icon) / 2.0, icon);
            if key.running[i] {
                // Windows 11: a short pill under a running app, Redrob Blue.
                fill_rrect(&mut bar, x + slot / 2.0 - 3.0, th as f32 - 5.0, 6.0, 3.0, 1.5, t.action_primary);
            }
            self.hits.push((Rectangle::new((x as i32, y0 + sy as i32).into(), (slot as i32, slot as i32).into()), Action::Launch(i)));
            x += slot + gap;
        }

        // Right: time over date, then tray glyphs to its left, switch first.
        let right = w as f32 - 14.0;
        let time = &key.clock.0;
        let date_short = chrono::Local::now().format("%Y-%m-%d").to_string();
        let tw = draw::text_width(&self.fonts.medium, time, 12.0);
        let dw = draw::text_width(&self.fonts.medium, &date_short, 12.0);
        let block = tw.max(dw);
        draw::draw_text(&mut bar, &self.fonts.medium, time, right - tw, 21.0, 12.0, t.ink_primary);
        draw::draw_text(&mut bar, &self.fonts.medium, &date_short, right - dw, 37.0, 12.0, t.ink_primary);
        let mut gx = right - block - 14.0;
        let g = 16.0;
        let tray: [(&'static str, Option<Action>); 3] = [
            (vol_glyph(&key.status), Some(Action::Volume)),
            (net_glyph(&key.status), Some(Action::Network)),
            (if key.mode == Mode::Dark { ICON_SUN } else { ICON_MOON }, Some(Action::ToggleTheme)),
        ];
        for (glyph, action) in tray {
            gx -= g + 12.0;
            if let Some(a) = action {
                wash(&mut bar, gx - 6.0, g + 12.0, key.hover == Some(a));
                self.hits.push((Rectangle::new(((gx - 6.0) as i32, y0).into(), (g as i32 + 12, th).into()), a));
            }
            self.icons.draw(&mut bar, glyph, Some(t.ink_primary), gx, (th as f32 - g) / 2.0, g);
        }
        self.bar = Some((upload(&bar), (0, y0).into()));
    }
}

/// The Redrob symbol recoloured to Redrob Black for the light bar.
fn symbol_dark_ink() -> &'static str {
    use std::sync::OnceLock;
    static S: OnceLock<String> = OnceLock::new();
    // Only the painted rect: the mask's own white must stay white.
    S.get_or_init(|| REDROB_SYMBOL.replace("fill=\"#FFFFFF\" mask=", "fill=\"#0a0b0c\" mask="))
}

/// Match apps to the shell theme, live, open windows included:
/// - GTK / portals: org.gnome.desktop.interface color-scheme, and the
///   Redrob palette in ~/.config/gtk-4.0/gtk.css.
pub fn apply_qt_colors(mode: Mode) {
    if std::env::var_os("REDROB_NESTED").is_some() {
        return; // a development run must not retheme the host desktop
    }
    write_gtk_css();
    let gtk = if mode == Mode::Dark { "prefer-dark" } else { "default" };
    crate::shell::spawn(&["gsettings", "set", "org.gnome.desktop.interface", "color-scheme", gtk]);
}

impl Chrome {
    /// Drop every cached buffer (the output scale changed).
    pub fn invalidate(&mut self) {
        self.key = None;
        self.cursor = None;
        self.ground_px = None;
    }

    /// Bring apps in line with the current theme and style (session start).
    pub fn apply_app_theme(&self) {
        apply_qt_colors(self.mode);
        apply_button_layout(self.style);
    }
}

/// GTK window buttons on the side the style puts them.
pub fn apply_button_layout(style: Style) {
    if std::env::var_os("REDROB_NESTED").is_some() {
        return;
    }
    let layout = match style {
        Style::Mac => "close,minimize,maximize:",
        Style::Windows => "appmenu:minimize,maximize,close",
    };
    crate::shell::spawn(&["gsettings", "set", "org.gnome.desktop.wm.preferences", "button-layout", layout]);
}

const GTK_MARK: &str = "/* Redrob palette -- written by redrob-comp; delete this line to keep your own edits. */";

/// libadwaita / GTK 4 apps in the Redrob palette: the redrob-ui surface,
/// ink, border and accent tokens for both schemes. Only our own file is
/// replaced; a user's own gtk.css (no marker) is left alone.
fn write_gtk_css() {
    let Some(home) = std::env::var_os("HOME") else { return };
    let path = std::path::PathBuf::from(home).join(".config/gtk-4.0/gtk.css");
    if let Ok(old) = std::fs::read_to_string(&path) {
        // Ours, or the one-line file Plasma's GTK bridge writes (it imports
        // colours that only update while Plasma runs): both are replaced.
        if !old.starts_with(GTK_MARK) && old.trim() != "@import 'colors.css';" {
            return;
        }
    }
    let css = format!(
        "{GTK_MARK}\n{}",
        r#":root {
  --accent-bg-color: #2b52ff; --accent-fg-color: #ffffff; --accent-color: #1733d5;
  --window-bg-color: #ffffff; --window-fg-color: #0a0b0c;
  --view-bg-color: #ffffff; --view-fg-color: #0a0b0c;
  --headerbar-bg-color: #f8f9fb; --headerbar-fg-color: #0a0b0c; --headerbar-backdrop-color: #ffffff;
  --sidebar-bg-color: #f8f9fb; --sidebar-fg-color: #0a0b0c; --sidebar-backdrop-color: #ffffff;
  --secondary-sidebar-bg-color: #f8f9fb;
  --card-bg-color: #f8f9fb; --card-fg-color: #0a0b0c;
  --popover-bg-color: #ffffff; --popover-fg-color: #0a0b0c;
  --dialog-bg-color: #ffffff; --dialog-fg-color: #0a0b0c;
  --thumbnail-bg-color: #f8f9fb;
  --border-color: #dfe2e8;
  --destructive-bg-color: #ff4b4b; --success-bg-color: #29e474; --warning-bg-color: #ff9c1b;
  --window-radius: 12px;
}
@media (prefers-color-scheme: dark) {
  :root {
    --accent-color: #8aafff;
    --window-bg-color: #0a0b0c; --window-fg-color: #f8f9fb;
    --view-bg-color: #0a0b0c; --view-fg-color: #f8f9fb;
    --headerbar-bg-color: #141719; --headerbar-fg-color: #f8f9fb; --headerbar-backdrop-color: #0a0b0c;
    --sidebar-bg-color: #141719; --sidebar-fg-color: #f8f9fb; --sidebar-backdrop-color: #0a0b0c;
    --secondary-sidebar-bg-color: #141719;
    --card-bg-color: #141719; --card-fg-color: #f8f9fb;
    --popover-bg-color: #141719; --popover-fg-color: #f8f9fb;
    --dialog-bg-color: #141719; --dialog-fg-color: #f8f9fb;
    --thumbnail-bg-color: #141719;
    --border-color: #292e37;
  }
}
"#
    );
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&path, css);
}

/// The Redrob session's apps: current GTK4 ones, Plasma's as the fallback.
pub const TERMINAL: &str = "exec ptyxis --new-window";
pub const FILES: &str = "exec nautilus --new-window";
/// Not a program: the dock asks the compositor to open Redrob 설정.
pub const SETTINGS: &str = "redrob:settings";

fn sh(script: &str) -> Vec<String> {
    vec!["sh".into(), "-c".into(), script.into()]
}

/// Dock / taskbar launchers. Redrob products open through Redrob Launcher's
/// `redrob://open/<app>` link: it starts the installed app or shows its
/// Install button. Override a command with REDROB_DOCK="name=cmd;...".
fn default_launchers() -> Vec<Launcher> {
    let mut v = vec![
        Launcher { name: "파일", tile: Tile::System(ICON_FOLDER), command: sh(FILES), child: None, group: false },
        Launcher { name: "터미널", tile: Tile::System(ICON_TERMINAL), command: sh(TERMINAL), child: None, group: false },
        Launcher {
            name: "Redrob Browser",
            tile: Tile::App(APP_BROWSER),
            command: sh("command -v redrob-browser >/dev/null && exec redrob-browser || exec redrob-launcher redrob://open/browser"),
            child: None,
            group: false,
        },
        Launcher {
            name: "Redrob Code",
            tile: Tile::App(APP_CODE),
            command: sh("command -v redrob >/dev/null && exec ptyxis --new-window -x redrob || exec redrob-launcher redrob://open/code"),
            child: None,
            group: false,
        },
        Launcher {
            name: "Redrob Office",
            tile: Tile::App(APP_OFFICE),
            command: vec!["redrob-launcher".into(), "redrob://open/office".into()],
            child: None,
            group: false,
        },
        Launcher { name: "설정", tile: Tile::System(ICON_SETTINGS), command: vec![SETTINGS.into()], child: None, group: false },
        Launcher { name: "Redrob Launcher", tile: Tile::App(APP_REDROB), command: vec!["redrob-launcher".into()], child: None, group: true },
    ];
    if let Ok(spec) = std::env::var("REDROB_DOCK") {
        for part in spec.split(';') {
            let Some((name, cmd)) = part.split_once('=') else { continue };
            if let Some(l) = v.iter_mut().find(|l| l.name.eq_ignore_ascii_case(name.trim())) {
                l.command = cmd.split_whitespace().map(String::from).collect();
            }
        }
    }
    v
}

/// (time, date) in Korean: ("오후 9:10", "10월 21일 (화)").
fn clock_text() -> (String, String) {
    use chrono::{Datelike, Timelike};
    let now = chrono::Local::now();
    let (h, m) = (now.hour(), now.minute());
    let ampm = if h < 12 { "오전" } else { "오후" };
    let h12 = match h % 12 {
        0 => 12,
        x => x,
    };
    let wd = ["월", "화", "수", "목", "금", "토", "일"][now.weekday().num_days_from_monday() as usize];
    (format!("{ampm} {h12}:{m:02}"), format!("{}월 {}일 ({wd})", now.month(), now.day()))
}

fn vol_glyph(s: &crate::status::Status) -> &'static str {
    if s.muted || s.volume == Some(0) { ICON_VOLUME_OFF } else { ICON_VOLUME }
}

fn net_glyph(s: &crate::status::Status) -> &'static str {
    if s.online { ICON_WIFI } else { ICON_WIFI_OFF }
}

/// A battery glyph 24x12: outline, nub, fill to the level (danger under 20%).
fn battery(p: &mut Pixmap, x: f32, y: f32, pct: u32, charging: bool, t: &theme::Theme) {
    stroke_rrect(p, x + 0.5, y + 0.5, 22.0, 11.0, 3.0, with_alpha(t.ink_primary, 160));
    fill_rrect(p, x + 23.0, y + 4.0, 2.0, 4.0, 1.0, with_alpha(t.ink_primary, 160));
    let c = if pct < 20 && !charging { t.danger } else if charging { t.success } else { t.ink_primary };
    let fw = 19.0 * (pct.min(100) as f32 / 100.0);
    fill_rrect(p, x + 2.0, y + 2.0, fw.max(1.0), 8.0, 1.5, c);
}
