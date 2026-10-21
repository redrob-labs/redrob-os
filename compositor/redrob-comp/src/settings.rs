//! Redrob 설정: the shell's own settings window (style, theme, shortcuts,
//! input), drawn by the compositor like the other layers. It writes
//! ~/.config/redrob/desktop.conf, which the shell re-reads at once.

use smithay::{
    input::keyboard::{KeysymHandle, ModifiersState, keysyms, xkb},
    utils::Rectangle,
};
use tiny_skia::Pixmap;

use crate::{
    config::{self, ALT, Act, CTRL, Combo, SHIFT, SUPER, Style},
    draw::{self, fill_circle, fill_rect, fill_rrect, stroke_rrect, with_alpha},
    overlay::Overlay,
    state::Smallvil,
    theme::{self, Mode, Rgba},
};

const W: i32 = 840;
const H: i32 = 560;
const SIDE: i32 = 208;
const PAGES: [&str; 4] = ["화면 구성", "테마", "단축키", "입력"];

pub const ID_CLOSE: usize = 9_999;
const ID_PAGE: usize = 1_000;
const ID_ITEM: usize = 2_000;
const ID_RESET: usize = 3_000;
const ID_IME: usize = 3_001;

impl Smallvil {
    pub fn open_settings(&mut self) {
        if matches!(self.layers.overlay, Some(Overlay::Settings { .. })) {
            return self.close_overlay();
        }
        self.open(Overlay::Settings { page: 0, capture: None });
    }

    pub fn settings_click(&mut self, id: usize) {
        let Some(Overlay::Settings { page, capture }) = &mut self.layers.overlay else { return };
        match id {
            ID_CLOSE => return self.close_overlay(),
            i if (ID_PAGE..ID_PAGE + PAGES.len()).contains(&i) => {
                *page = i - ID_PAGE;
                *capture = None;
            }
            ID_RESET => {
                config::reset_shortcuts();
                self.chrome.reload_now();
            }
            ID_IME => crate::shell::spawn(&["fcitx5-configtool"]),
            i if i >= ID_ITEM => {
                let k = i - ID_ITEM;
                match *page {
                    0 => {
                        config::set_value("style", if k == 0 { "mac" } else { "windows" });
                        self.chrome.reload_now();
                    }
                    1 => {
                        let want = if k == 0 { Mode::Light } else { Mode::Dark };
                        if self.chrome.mode != want {
                            self.chrome.toggle_theme();
                        }
                    }
                    2 => *capture = Act::ALL.get(k).copied(),
                    _ => {}
                }
            }
            _ => {}
        }
        self.layers.dirty = true;
    }

    /// A key while the settings window is up: Esc closes (or cancels a
    /// capture); in capture mode the next combination becomes the shortcut.
    pub fn settings_key(&mut self, pressed: bool, mods: &ModifiersState, key: &KeysymHandle<'_>) {
        if !pressed {
            return;
        }
        let raw = key.modified_sym().raw();
        let Some(Overlay::Settings { capture, .. }) = &mut self.layers.overlay else { return };
        let Some(act) = *capture else {
            if raw == keysyms::KEY_Escape {
                self.close_overlay();
            }
            return;
        };
        let name = key.raw_syms().first().map(|s| xkb::keysym_get_name(*s).to_ascii_lowercase()).unwrap_or_default();
        if raw == keysyms::KEY_Escape {
            *capture = None;
            self.layers.dirty = true;
            return;
        }
        // Wait for the real key; a lone modifier press is part of the combo.
        if name.starts_with("super") || name.starts_with("control") || name.starts_with("alt") || name.starts_with("shift") || name.starts_with("meta") {
            return;
        }
        let mut bits = 0;
        if mods.logo {
            bits |= SUPER;
        }
        if mods.ctrl {
            bits |= CTRL;
        }
        if mods.alt {
            bits |= ALT;
        }
        if mods.shift {
            bits |= SHIFT;
        }
        let combo = Combo { mods: bits, key: name };
        let olds = config::default_combos(self.chrome.style, act);
        config::set_shortcut(&combo, Some(act), &olds);
        *capture = None;
        self.chrome.reload_now();
        self.layers.dirty = true;
    }

    pub fn paint_settings(&mut self, p: &mut Pixmap, page: usize, capture: Option<Act>) {
        let mode = self.chrome.mode;
        let t = theme::theme(mode);
        let (sw, sh) = (p.width() as i32, p.height() as i32);
        fill_rect(p, 0.0, 0.0, sw as f32, sh as f32, Rgba(0, 0, 0, if mode == Mode::Dark { 110 } else { 50 }));
        let (x, y) = ((sw - W) / 2, (sh - H) / 2);
        let (xf, yf) = (x as f32, y as f32);
        let r = theme::RADIUS_XL_F;
        draw::shadow(p, xf, yf, W as f32, H as f32, r, 34.0, 14.0, t.shadow_lg);
        fill_rrect(p, xf, yf, W as f32, H as f32, r, t.surface_base);
        // Sidebar: raised, rounded on the left only.
        fill_rrect(p, xf, yf, SIDE as f32 + r, H as f32, r, t.surface_raised);
        fill_rect(p, xf + SIDE as f32, yf, r, H as f32, t.surface_base);
        fill_rect(p, xf + SIDE as f32, yf, 1.0, H as f32, t.border_subtle);
        stroke_rrect(p, xf + 0.5, yf + 0.5, W as f32 - 1.0, H as f32 - 1.0, r, t.border_subtle);
        self.layers.hits.push((Rectangle::new((x, y).into(), (W, H).into()), usize::MAX));

        let sb = &self.chrome.fonts.semibold;
        let md = &self.chrome.fonts.medium;
        draw::draw_text(p, sb, "설정", xf + 20.0, yf + 40.0, 20.0, t.ink_primary);
        for (i, name) in PAGES.iter().enumerate() {
            let ry = yf + 64.0 + i as f32 * 40.0;
            if i == page {
                fill_rrect(p, xf + 10.0, ry, SIDE as f32 - 20.0, 34.0, theme::RADIUS_MD, t.action_primary);
            }
            let ink = if i == page { Rgba(255, 255, 255, 255) } else { t.ink_primary };
            draw::draw_text(p, md, name, xf + 24.0, ry + 22.5, 14.0, ink);
            self.layers.hits.push((Rectangle::new((x + 10, ry as i32).into(), (SIDE - 20, 34).into()), ID_PAGE + i));
        }

        // Close button, top right.
        let (cx, cy) = (xf + W as f32 - 28.0, yf + 28.0);
        fill_circle(p, cx, cy, 13.0, t.surface_raised);
        draw::line(p, cx - 4.5, cy - 4.5, cx + 4.5, cy + 4.5, 1.6, t.ink_secondary);
        draw::line(p, cx - 4.5, cy + 4.5, cx + 4.5, cy - 4.5, 1.6, t.ink_secondary);
        self.layers.hits.push((Rectangle::new(((cx - 14.0) as i32, (cy - 14.0) as i32).into(), (28, 28).into()), ID_CLOSE));

        let (cx0, cy0) = (xf + SIDE as f32 + 32.0, yf + 40.0);
        let cw = (W - SIDE - 64) as f32;
        draw::draw_text(p, sb, PAGES[page], cx0, cy0, 20.0, t.ink_primary);
        let style = self.chrome.style;
        match page {
            0 => {
                draw::draw_text(p, md, "익숙한 배치와 단축키를 고르세요. 바로 바뀝니다.", cx0, cy0 + 28.0, 13.0, t.ink_secondary);
                for (k, (label, s, sub)) in [
                    ("맥 스타일", Style::Mac, "위 메뉴 바 · 떠 있는 도크 · ⌘ 단축키"),
                    ("윈도우 스타일", Style::Windows, "아래 작업 표시줄 · 시작 버튼 · Win 단축키"),
                ]
                .into_iter()
                .enumerate()
                {
                    let (bx, by) = (cx0 + k as f32 * (cw / 2.0 + 8.0), cy0 + 52.0);
                    let bw = cw / 2.0 - 8.0;
                    self.card(p, &t, bx, by, bw, 236.0, style == s);
                    mini_desktop(p, &t, s, bx + 14.0, by + 14.0, bw - 28.0, 140.0);
                    draw::draw_text(p, sb, label, bx + 16.0, by + 184.0, 15.0, t.ink_primary);
                    let sub = draw::fit_text(md, sub, 12.0, bw - 32.0);
                    draw::draw_text(p, md, &sub, bx + 16.0, by + 208.0, 12.0, t.ink_secondary);
                    self.layers.hits.push((Rectangle::new((bx as i32, by as i32).into(), (bw as i32, 236).into()), ID_ITEM + k));
                }
            }
            1 => {
                draw::draw_text(p, md, "앱 창까지 함께 바뀝니다.", cx0, cy0 + 28.0, 13.0, t.ink_secondary);
                for (k, (label, m)) in [("라이트", Mode::Light), ("다크", Mode::Dark)].into_iter().enumerate() {
                    let (bx, by) = (cx0 + k as f32 * (cw / 2.0 + 8.0), cy0 + 52.0);
                    let bw = cw / 2.0 - 8.0;
                    self.card(p, &t, bx, by, bw, 212.0, mode == m);
                    let tt = theme::theme(m);
                    // A small window on that theme's ground.
                    let (px, py, pw, ph) = (bx + 14.0, by + 14.0, bw - 28.0, 140.0);
                    fill_rrect(p, px, py, pw, ph, theme::RADIUS_MD, tt.surface_sunken);
                    fill_rrect(p, px + 18.0, py + 18.0, pw - 36.0, ph - 30.0, theme::RADIUS_MD, tt.surface_base);
                    fill_rect(p, px + 18.0, py + 40.0, pw - 36.0, 1.0, tt.border_subtle);
                    fill_rrect(p, px + 30.0, py + 56.0, pw * 0.4, 8.0, 4.0, tt.ink_primary);
                    fill_rrect(p, px + 30.0, py + 72.0, pw * 0.55, 6.0, 3.0, with_alpha(tt.ink_secondary, 160));
                    fill_rrect(p, px + 30.0, py + 90.0, 64.0, 20.0, theme::RADIUS_SM, tt.action_primary);
                    draw::draw_text(p, sb, label, bx + 16.0, by + 186.0, 15.0, t.ink_primary);
                    self.layers.hits.push((Rectangle::new((bx as i32, by as i32).into(), (bw as i32, 212).into()), ID_ITEM + k));
                }
            }
            2 => {
                let hint = match capture {
                    Some(a) => format!("「{}」에 쓸 키를 누르세요 · Esc 취소", a.label()),
                    None => "바꿀 항목을 누른 뒤 새 키를 누르세요.".to_string(),
                };
                draw::draw_text(p, md, &hint, cx0, cy0 + 28.0, 13.0, if capture.is_some() { t.action_primary } else { t.ink_secondary });
                let col_w = cw / 2.0 - 6.0;
                let shortcuts = self.chrome.shortcuts().to_vec();
                for (k, act) in Act::ALL.iter().enumerate() {
                    let (col, row) = (k / 9, k % 9);
                    let (rx, ry) = (cx0 + col as f32 * (col_w + 12.0), cy0 + 48.0 + row as f32 * 44.0);
                    let on = capture == Some(*act);
                    fill_rrect(p, rx, ry, col_w, 38.0, theme::RADIUS_MD, if on { with_alpha(t.action_primary, 40) } else { t.surface_raised });
                    if on {
                        stroke_rrect(p, rx + 0.5, ry + 0.5, col_w - 1.0, 37.0, theme::RADIUS_MD, t.action_primary);
                    }
                    draw::draw_text(p, md, act.label(), rx + 12.0, ry + 24.5, 13.0, t.ink_primary);
                    let keys: Vec<String> = shortcuts.iter().filter(|(_, a)| a == act).map(|(c, _)| c.display(style)).collect();
                    let text = if on { "…".to_string() } else if keys.is_empty() { "없음".into() } else { keys.join("  ") };
                    let text = draw::fit_text(md, &text, 12.0, col_w * 0.5);
                    let tw = draw::text_width(md, &text, 12.0) + 16.0;
                    let (kx, ky) = (rx + col_w - tw - 8.0, ry + 8.0);
                    fill_rrect(p, kx, ky, tw, 22.0, theme::RADIUS_SM, t.surface_sunken);
                    stroke_rrect(p, kx + 0.5, ky + 0.5, tw - 1.0, 21.0, theme::RADIUS_SM, t.border_subtle);
                    draw::draw_text(p, md, &text, kx + 8.0, ky + 15.5, 12.0, if keys.is_empty() { t.ink_muted } else { t.ink_primary });
                    self.layers.hits.push((Rectangle::new((rx as i32, ry as i32).into(), (col_w as i32, 38).into()), ID_ITEM + k));
                }
                self.button(p, &t, "기본값으로", cx0, yf + H as f32 - 52.0, ID_RESET);
            }
            _ => {
                let lines = [
                    ("한/영 전환", "한/영 키 · 오른쪽 Alt · Shift+Space"),
                    ("한자 변환", "한자 키 · 오른쪽 Ctrl"),
                    ("입력기", "Fcitx 5 · 한국어 두벌식"),
                ];
                for (k, (a, b)) in lines.iter().enumerate() {
                    let ry = cy0 + 48.0 + k as f32 * 56.0;
                    fill_rrect(p, cx0, ry, cw, 46.0, theme::RADIUS_MD, t.surface_raised);
                    draw::draw_text(p, md, a, cx0 + 14.0, ry + 28.5, 14.0, t.ink_primary);
                    let bw = draw::text_width(md, b, 13.0);
                    draw::draw_text(p, md, b, cx0 + cw - 14.0 - bw, ry + 28.5, 13.0, t.ink_secondary);
                }
                self.button(p, &t, "입력기 세부 설정", cx0, cy0 + 48.0 + 3.0 * 56.0 + 8.0, ID_IME);
            }
        }
    }

    fn card(&self, p: &mut Pixmap, t: &theme::Theme, x: f32, y: f32, w: f32, h: f32, on: bool) {
        fill_rrect(p, x, y, w, h, theme::RADIUS_LG, t.surface_raised);
        if on {
            for d in [0.5, 1.5] {
                stroke_rrect(p, x + d, y + d, w - 2.0 * d, h - 2.0 * d, theme::RADIUS_LG, t.action_primary);
            }
            fill_circle(p, x + w - 22.0, y + h - 24.0, 9.0, t.action_primary);
            draw::line(p, x + w - 26.5, y + h - 24.0, x + w - 23.0, y + h - 20.5, 1.8, Rgba(255, 255, 255, 255));
            draw::line(p, x + w - 23.0, y + h - 20.5, x + w - 17.5, y + h - 27.5, 1.8, Rgba(255, 255, 255, 255));
        } else {
            stroke_rrect(p, x + 0.5, y + 0.5, w - 1.0, h - 1.0, theme::RADIUS_LG, t.border_subtle);
        }
    }

    fn button(&mut self, p: &mut Pixmap, t: &theme::Theme, label: &str, x: f32, y: f32, id: usize) {
        let md = &self.chrome.fonts.medium;
        let w = draw::text_width(md, label, 13.0) + 28.0;
        fill_rrect(p, x, y, w, 34.0, theme::RADIUS_MD, t.surface_raised);
        stroke_rrect(p, x + 0.5, y + 0.5, w - 1.0, 33.0, theme::RADIUS_MD, t.border_strong);
        draw::draw_text(p, md, label, x + 14.0, y + 22.0, 13.0, t.ink_primary);
        self.layers.hits.push((Rectangle::new((x as i32, y as i32).into(), (w as i32, 34).into()), id));
    }
}

/// A tiny picture of a layout: ground, bar / dock or taskbar, one window.
fn mini_desktop(p: &mut Pixmap, t: &theme::Theme, s: Style, x: f32, y: f32, w: f32, h: f32) {
    let dark = theme::theme(Mode::Dark);
    let mut g = Pixmap::new(w as u32, h as u32).unwrap();
    draw::fill_gradient(&mut g, &[(0.0, 0x0a0b0c), (0.6, 0x0a0b0c), (1.0, 0x09209c)]);
    p.draw_pixmap(x as i32, y as i32, g.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
    let win = |p: &mut Pixmap, wx: f32, wy: f32| {
        fill_rrect(p, wx, wy, w * 0.5, h * 0.45, 4.0, dark.surface_raised);
        fill_rect(p, wx, wy + 9.0, w * 0.5, 1.0, dark.border_subtle);
    };
    match s {
        Style::Mac => {
            fill_rect(p, x, y, w, 8.0, dark.surface_material);
            win(p, x + w * 0.18, y + 22.0);
            for (i, c) in [dark.danger, dark.warning, dark.success].iter().enumerate() {
                fill_circle(p, x + w * 0.18 + 6.0 + i as f32 * 6.0, y + 26.5, 2.0, *c);
            }
            let (dw, dh) = (w * 0.46, 16.0);
            fill_rrect(p, x + (w - dw) / 2.0, y + h - dh - 6.0, dw, dh, 6.0, dark.surface_material);
            for i in 0..6 {
                fill_rrect(p, x + (w - dw) / 2.0 + 6.0 + i as f32 * (dw - 12.0) / 6.0, y + h - dh - 3.0, 10.0, 10.0, 3.0, if i == 2 { dark.action_primary } else { dark.border_strong });
            }
        }
        Style::Windows => {
            win(p, x + w * 0.2, y + 18.0);
            fill_rect(p, x + w * 0.2 + w * 0.5 - 22.0, y + 21.0, 4.0, 1.0, dark.ink_secondary);
            fill_rect(p, x, y + h - 14.0, w, 14.0, dark.surface_material);
            for i in 0..6 {
                fill_rrect(p, x + w / 2.0 - 30.0 + i as f32 * 11.0, y + h - 11.0, 8.0, 8.0, 2.0, if i == 0 { dark.action_primary } else { dark.border_strong });
            }
        }
    }
    stroke_rrect(p, x + 0.5, y + 0.5, w - 1.0, h - 1.0, theme::RADIUS_MD, t.border_subtle);
}
