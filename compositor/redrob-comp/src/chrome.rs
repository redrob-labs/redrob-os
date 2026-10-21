//! Redrob shell chrome, drawn by the compositor itself (no Plasma, no COSMIC).
//! Milestone 1: solid shapes only -- background, top bar with the Redrob tile,
//! and a centred dock. Text, icons and hit-testing come next.
//! Colours are the Redrob Group Design System tokens.

use smithay::{
    backend::renderer::{
        element::{Id, Kind, solid::SolidColorRenderElement},
        utils::CommitCounter,
    },
    utils::{Physical, Rectangle, Size},
};

/// Redrob deep ground (#0A0B0C) -- the clear colour behind everything.
pub const GROUND: [f32; 4] = [0x0A as f32 / 255.0, 0x0B as f32 / 255.0, 0x0C as f32 / 255.0, 1.0];
const BAR: [f32; 4] = [0x14 as f32 / 255.0, 0x16 as f32 / 255.0, 0x1A as f32 / 255.0, 1.0];
const DOCK: [f32; 4] = [0x1C as f32 / 255.0, 0x1F as f32 / 255.0, 0x24 as f32 / 255.0, 1.0];
const SLOT: [f32; 4] = [0x2A as f32 / 255.0, 0x2E as f32 / 255.0, 0x35 as f32 / 255.0, 1.0];
/// Redrob Blue #2B52FF.
const BLUE: [f32; 4] = [0x2B as f32 / 255.0, 0x52 as f32 / 255.0, 0xFF as f32 / 255.0, 1.0];

pub const BAR_H: i32 = 32;
const DOCK_SLOTS: i32 = 5;
const SLOT_PX: i32 = 48;
const GAP: i32 = 12;

/// Stable element ids so the damage tracker only redraws what changed.
pub struct Chrome {
    ids: Vec<Id>,
    commit: CommitCounter,
}

impl Chrome {
    pub fn new() -> Self {
        Self { ids: (0..16).map(|_| Id::new()).collect(), commit: CommitCounter::default() }
    }

    pub fn elements(&self, out: Size<i32, Physical>) -> Vec<SolidColorRenderElement> {
        let mut rects: Vec<(Rectangle<i32, Physical>, [f32; 4])> = Vec::new();
        // Top bar + Redrob tile (logo placeholder) at the left.
        rects.push((Rectangle::new((0, 0).into(), (out.w, BAR_H).into()), BAR));
        rects.push((Rectangle::new((10, 6).into(), (20, 20).into()), BLUE));
        // Dock: a pill-shaped bar centred at the bottom, sized to its slots.
        let dock_w = DOCK_SLOTS * SLOT_PX + (DOCK_SLOTS + 1) * GAP;
        let dock_h = SLOT_PX + 2 * GAP;
        let dx = (out.w - dock_w) / 2;
        let dy = out.h - dock_h - 12;
        rects.push((Rectangle::new((dx, dy).into(), (dock_w, dock_h).into()), DOCK));
        for i in 0..DOCK_SLOTS {
            let sx = dx + GAP + i * (SLOT_PX + GAP);
            rects.push((Rectangle::new((sx, dy + GAP).into(), (SLOT_PX, SLOT_PX).into()), SLOT));
        }
        // Running-app indicator under the first slot.
        rects.push((
            Rectangle::new((dx + GAP + SLOT_PX / 2 - 8, dy + dock_h - 6).into(), (16, 3).into()),
            BLUE,
        ));
        // The elements list is front-to-back: reverse so the bar/dock paint above
        // their slots' backgrounds correctly (slots and indicator first).
        rects
            .into_iter()
            .rev()
            .enumerate()
            .map(|(i, (geo, color))| {
                SolidColorRenderElement::new(self.ids[i].clone(), geo, self.commit, color, Kind::Unspecified)
            })
            .collect()
    }
}
