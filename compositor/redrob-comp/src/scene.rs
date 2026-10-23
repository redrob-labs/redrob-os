//! The frame's element list, shared by the DRM and nested backends.
//! Front to back: cursor, shell chrome (bar / dock / taskbar), then each
//! window top-most first with its frame behind it, then the desktop ground.

use smithay::{
    backend::renderer::{
        element::{
            AsRenderElements, memory::MemoryRenderBufferRenderElement, surface::WaylandSurfaceRenderElement,
        },
        gles::GlesRenderer,
    },
    utils::{Physical, Scale, Size},
};

use crate::Smallvil;

smithay::backend::renderer::element::render_elements! {
    pub ShellElement<=GlesRenderer>;
    Window=WaylandSurfaceRenderElement<GlesRenderer>,
    Memory=MemoryRenderBufferRenderElement<GlesRenderer>,
}

pub fn elements(state: &mut Smallvil, renderer: &mut GlesRenderer, size: Size<i32, Physical>, cursor: bool) -> Vec<ShellElement> {
    // The shell lays out in logical pixels; elements are placed at
    // logical * scale and the renderer scales the buffers.
    let scale = crate::display::scale();
    let size: Size<i32, Physical> = ((size.w as f64 / scale).round() as i32, (size.h as f64 / scale).round() as i32).into();
    state.place_im_popups();
    // The menu bar names the app in front, and shows live status.
    let top = state.space.elements().last().cloned();
    state.chrome.app = top.as_ref().map(crate::deco::app_name).unwrap_or_default();
    if let Ok(s) = state.status.lock() {
        state.chrome.status = s.clone();
    }

    let mut out: Vec<ShellElement> = Vec::new();
    if cursor {
        let pointer = state.seat.get_pointer().unwrap().current_location();
        if let Some(c) = state.chrome.cursor_element(renderer, pointer) {
            out.push(ShellElement::Memory(c));
        }
    }
    let (front, thumbs) = state.overlay_elements(renderer, size);
    let locked = state.locked();
    let expose = matches!(state.layers.overlay, Some(crate::overlay::Overlay::Expose { .. }));
    if expose {
        // Mission Control: the bar stays, live thumbnails over a dim layer.
        out.extend(state.chrome.elements(renderer, size).into_iter().map(ShellElement::Memory));
        out.extend(thumbs.into_iter().map(ShellElement::Window));
        out.extend(front.into_iter().map(ShellElement::Memory));
    } else {
        out.extend(front.into_iter().map(ShellElement::Memory));
        if !locked {
            out.extend(state.chrome.elements(renderer, size).into_iter().map(ShellElement::Memory));
        }
    }
    if !locked && !expose {
        let windows: Vec<_> = state.space.elements().rev().cloned().collect();
        for window in windows {
            let Some(loc) = state.space.element_location(&window) else { continue };
            let w: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
                window.render_elements(renderer, loc.to_f64().to_physical(scale).to_i32_round(), Scale::from(scale), 1.0);
            out.extend(w.into_iter().map(ShellElement::Window));
            if let Some(f) = state.frame_element(renderer, &window) {
                out.push(ShellElement::Memory(f));
            }
        }
    }
    if let Some(g) = state.chrome.ground_element(renderer, size) {
        out.push(ShellElement::Memory(g));
    }
    out
}

/// A screen other than the main one: windows over it and the ground
/// (the bar, dock and overlays live on the main screen).
pub fn elements_secondary(
    state: &mut Smallvil,
    renderer: &mut GlesRenderer,
    origin: smithay::utils::Point<i32, smithay::utils::Logical>,
    size: Size<i32, Physical>,
) -> Vec<ShellElement> {
    let scale = crate::display::scale();
    let lsize: Size<i32, Physical> = ((size.w as f64 / scale).round() as i32, (size.h as f64 / scale).round() as i32).into();
    let area = smithay::utils::Rectangle::<i32, smithay::utils::Logical>::new(origin, (lsize.w, lsize.h).into());
    let mut out: Vec<ShellElement> = Vec::new();
    let pointer = state.seat.get_pointer().unwrap().current_location();
    if area.to_f64().contains(pointer) {
        if let Some(c) = state.chrome.cursor_element(renderer, pointer - origin.to_f64()) {
            out.push(ShellElement::Memory(c));
        }
    }
    if !state.locked() {
        let windows: Vec<_> = state.space.elements().rev().cloned().collect();
        for window in windows {
            let Some(loc) = state.space.element_location(&window) else { continue };
            let geo = window.geometry();
            if !area.overlaps(smithay::utils::Rectangle::new(loc, geo.size)) {
                continue;
            }
            let at = (loc - origin).to_f64().to_physical(scale).to_i32_round();
            let w: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = window.render_elements(renderer, at, Scale::from(scale), 1.0);
            out.extend(w.into_iter().map(ShellElement::Window));
            if let Some(f) = state.frame_element_at(renderer, &window, origin) {
                out.push(ShellElement::Memory(f));
            }
        }
    }
    if let Some(g) = state.chrome.ground_element(renderer, lsize) {
        out.push(ShellElement::Memory(g));
    }
    out
}

/// Take a pending screenshot of this frame (pointer excluded), then tell the
/// user where it went.
pub fn take_shot(state: &mut Smallvil, renderer: &mut GlesRenderer, size: Size<i32, Physical>) {
    let Some(region) = state.shot.take() else { return };
    let elements = elements(state, renderer, size, false);
    let img = crate::shot::capture(renderer, &elements, size, state.chrome.ground());
    let saved = img.and_then(|i| crate::shot::save(i, region));
    let note = crate::notify::Note {
        id: u32::MAX - 1,
        app: crate::i18n::tr("레드롭").into(),
        icon: String::new(),
        summary: crate::i18n::tr(if saved.is_some() { "스크린샷을 저장했어요" } else { "스크린샷을 찍지 못했어요" }).into(),
        body: saved.map(|p| p.display().to_string()).unwrap_or_default(),
        timeout: 4000,
    };
    state.toast(note);
}
