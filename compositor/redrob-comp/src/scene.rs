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
    // The menu bar names the app in front.
    let top = state.space.elements().last().cloned();
    state.chrome.app = top.as_ref().map(crate::deco::app_name).unwrap_or_default();

    let mut out: Vec<ShellElement> = Vec::new();
    if cursor {
        let pointer = state.seat.get_pointer().unwrap().current_location();
        if let Some(c) = state.chrome.cursor_element(renderer, pointer) {
            out.push(ShellElement::Memory(c));
        }
    }
    out.extend(state.chrome.elements(renderer, size).into_iter().map(ShellElement::Memory));
    let windows: Vec<_> = state.space.elements().rev().cloned().collect();
    for window in windows {
        let Some(loc) = state.space.element_location(&window) else { continue };
        let w: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
            window.render_elements(renderer, loc.to_physical(1), Scale::from(1.0), 1.0);
        out.extend(w.into_iter().map(ShellElement::Window));
        if let Some(f) = state.frame_element(renderer, &window) {
            out.push(ShellElement::Memory(f));
        }
    }
    if let Some(g) = state.chrome.ground_element(renderer, size) {
        out.push(ShellElement::Memory(g));
    }
    out
}
