//! Linux native views are `Painted` sources only — there is no platform
//! view to host (the compositor gives a window one surface), so zgx itself
//! paints each source's frames inside its node and there is one plane:
//! clips, corner radii, opacity and overlay paint all just work, and there
//! is nothing to place or commit. This host computes the split anyway —
//! `PlaneSplit::route` hit-tests the scene so a painted node gets the
//! pointer only when zgx content on top of it would not take it — and it
//! reports each source's laid-out size, routes its pointer, wheel and key
//! input, and keeps the node's focus with it.
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use winit::event::{ElementState, KeyEvent};
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::platform::scancode::PhysicalKeyExtScancode;
use winit::window::CursorIcon;
use zgx::{
    input::{Modifiers, PointerButton},
    native_view::{
        NativeContent, PaintedCursor, PaintedKey, PaintedResize, PaintedSource, PlaneSplit,
        PointerRoute, PointerStep,
    },
    scene::{NodeId, Rect, Scene},
    widgets::Ui,
};

use super::NativeHost;

/// A source's node and where its box starts in window points, so a pointer
/// position converts to the source's local space.
pub(super) struct Pointed {
    pub node: NodeId,
    pub source: Rc<dyn PaintedSource>,
    pub at: (f32, f32),
}
/// A press in progress on a painted node: the source keeps moves and takes
/// the releases, wherever the pointer goes, until every button is up.
pub(super) struct Held {
    pub node: NodeId,
    pub source: Rc<dyn PaintedSource>,
    pub buttons: usize,
}

/// How long and how far apart two presses still count as one click stream.
const CLICK: (std::time::Duration, f32) = (std::time::Duration::from_millis(500), 4.);

impl NativeHost {
    /// After the scene's flush: learn the painted sources' placements and
    /// tell each its size, scale and visibility. There is no overlay plane,
    /// so the split's damage is the window renderer's own.
    pub fn plan(
        &mut self,
        scene: &Scene,
        _: &winit::window::Window,
        _: &mut zgx_gpu::GpuRenderer,
        scale: f32,
        zoom: f32,
    ) -> Result<Vec<Rect>, zgx_gpu::GpuError> {
        if !self.active() {
            return Ok(Vec::new());
        }
        let (split, _) = PlaneSplit::compute(scene, &self.natives.nodes(), &self.split);
        let split = Rc::new(split);
        for placement in split.natives() {
            let Some(NativeContent::Painted(source)) = self.natives.content(placement.node)
            else {
                continue;
            };
            match placement.visible() {
                Some(_) => {
                    source.visible(true);
                    source.layout(placement.size.0, placement.size.1, scale.max(1e-3) * zoom);
                }
                None => source.visible(false),
            }
        }
        self.split = split.clone();
        self.natives.set_split(split);
        Ok(Vec::new())
    }
    /// The overlay plane does not exist: the window's renderer drew it all.
    pub fn render(&mut self, _: &Scene, _: &[Rect]) -> Result<(), zgx_gpu::GpuError> {
        Ok(())
    }
    pub fn begin(&self) -> Option<()> {
        None
    }
    pub fn present_overlay(&mut self) -> Result<(), zgx_gpu::GpuError> {
        Ok(())
    }
    pub fn apply(&mut self) {}
    pub fn flush(&mut self) {}
    pub fn trim(&mut self) {}

    /// The pointer moved to window point `(x, y)`: what zgx hears of it
    /// (see `pointer_moved` in `mod.rs`). `held`: a press of zgx's follows
    /// the pointer.
    pub(super) fn painted_pointer_moved(&mut self, x: f32, y: f32, held: bool) -> PointerStep {
        // A drag on a painted node goes to its source wherever it goes.
        if let Some(held_source) = &self.held {
            if let Some(at) = self.placement(held_source.node) {
                held_source.source.pointer_moved(x - at.0, y - at.1);
            }
            return PointerStep::Over(held_source.node);
        }
        let Some(scene) = self.scene.upgrade() else {
            return PointerStep::Move;
        };
        let scene = scene.borrow();
        match self.split.route(&scene, &self.input, x, y) {
            PointerRoute::Native(node) if !held => {
                let Some(pointed) = self.pointed(node) else {
                    self.leave();
                    return PointerStep::Move;
                };
                pointed
                    .source
                    .pointer_moved(x - pointed.at.0, y - pointed.at.1);
                if self.over.as_ref().is_some_and(|old| old.node == node) {
                    return PointerStep::Over(node);
                }
                if let Some(old) = self.over.take() {
                    old.source.pointer_left();
                }
                self.over = Some(pointed);
                PointerStep::Leave(node)
            }
            _ => {
                self.leave();
                PointerStep::Move
            }
        }
    }
    /// The painted node `node` shows, with its box's origin.
    fn pointed(&self, node: NodeId) -> Option<Pointed> {
        let at = self.placement(node)?;
        match self.natives.content(node) {
            Some(NativeContent::Painted(source)) => Some(Pointed { node, source, at }),
            _ => None,
        }
    }
    /// `node`'s box origin in window points, by the last split.
    fn placement(&self, node: NodeId) -> Option<(f32, f32)> {
        self.split
            .natives()
            .iter()
            .find(|placement| placement.node == node)
            .map(|placement| (placement.frame.x, placement.frame.y))
    }
    /// The pointer left the painted node it was over.
    fn leave(&mut self) {
        if let Some(old) = self.over.take() {
            old.source.pointer_left();
        }
    }
    /// The pointer left the window, or the window went to the back.
    pub(super) fn painted_pointer_left(&mut self) {
        self.leave();
    }
    /// The cursor the source under the pointer asks for, if any.
    pub fn painted_cursor(&self) -> Option<CursorIcon> {
        let pointed = self.over.as_ref()?;
        Some(match pointed.source.cursor()? {
            PaintedCursor::Pointer => CursorIcon::Default,
            PaintedCursor::Text => CursorIcon::Text,
            PaintedCursor::Hand => CursorIcon::Pointer,
            PaintedCursor::NotAllowed => CursorIcon::NotAllowed,
            PaintedCursor::Move => CursorIcon::Move,
            PaintedCursor::Resize(PaintedResize::Vertical) => CursorIcon::NsResize,
            PaintedCursor::Resize(PaintedResize::Horizontal) => CursorIcon::EwResize,
            PaintedCursor::Resize(PaintedResize::NorthWestSouthEast) => CursorIcon::NwseResize,
            PaintedCursor::Resize(PaintedResize::NorthEastSouthWest) => CursorIcon::NeswResize,
        })
    }
    /// A button went down or up at window point `(x, y)`. `true`: a
    /// painted source took it — the click is the page's, not zgx's.
    pub fn pointer_button(
        &mut self,
        x: f32,
        y: f32,
        button: PointerButton,
        pressed: bool,
    ) -> bool {
        if pressed {
            self.press(x, y, button)
        } else {
            self.release(x, y, button)
        }
    }
    fn press(&mut self, x: f32, y: f32, button: PointerButton) -> bool {
        // A second button while one is held stays with the same source.
        if self.held.is_some() {
            let node = self.held.as_ref().unwrap().node;
            let at = self
                .split
                .natives()
                .iter()
                .find(|placement| placement.node == node)
                .map(|placement| (placement.frame.x, placement.frame.y))
                .unwrap_or_default();
            let flags = self.flags();
            let held = self.held.as_mut().unwrap();
            held.buttons += 1;
            held.source.pointer_down(x - at.0, y - at.1, button, 1, flags);
            return true;
        }
        let Some(scene) = self.scene.upgrade() else {
            return false;
        };
        let scene = scene.borrow();
        let route = self.split.route(&scene, &self.input, x, y);
        let PointerRoute::Native(node) = route else {
            return false;
        };
        let Some(pointed) = self.pointed(node) else {
            return false;
        };
        let clicks = match &mut self.held_clicks {
            Some((last_button, at, lx, ly, count))
                if *last_button == button
                    && at.elapsed() < CLICK.0
                    && (x - *lx).abs() < CLICK.1
                    && (y - *ly).abs() < CLICK.1 =>
            {
                *count = (*count + 1).min(3);
                *at = Instant::now();
                *lx = x;
                *ly = y;
                *count
            }
            _ => {
                self.held_clicks = Some((button, Instant::now(), x, y, 1));
                1
            }
        };
        pointed.source.pointer_down(
            x - pointed.at.0,
            y - pointed.at.1,
            button,
            clicks,
            self.flags(),
        );
        // The node's keyboard focus follows a press into the page, as a
        // focusable view's does: its `Focusable` then tells the source.
        drop(scene);
        let scene = self.scene_handle();
        self.input.focus(&scene, Some(node));
        self.held = Some(Held {
            node,
            source: pointed.source,
            buttons: 1,
        });
        true
    }
    fn release(&mut self, x: f32, y: f32, button: PointerButton) -> bool {
        if self.held.is_none() {
            return false;
        }
        let node = self.held.as_ref().unwrap().node;
        let at = self
            .split
            .natives()
            .iter()
            .find(|placement| placement.node == node)
            .map(|placement| (placement.frame.x, placement.frame.y))
            .unwrap_or_default();
        let flags = self.flags();
        let held = self.held.as_mut().unwrap();
        held.source.pointer_up(x - at.0, y - at.1, button, flags);
        held.buttons -= 1;
        if held.buttons == 0 {
            self.held = None;
        }
        true
    }
    /// A wheel or trackpad scroll over a painted source is the page's;
    /// `true` keeps it from zgx's scrollers.
    pub fn wheel(&mut self, x: f32, y: f32, dx: f32, dy: f32) -> bool {
        let Some(pointed) = &self.over else {
            return false;
        };
        pointed
            .source
            .wheel(x - pointed.at.0, y - pointed.at.1, dx, dy, self.flags());
        true
    }
    /// The held modifiers as `PaintedKey::modifiers` bits.
    fn flags(&self) -> u32 {
        ((self.modifiers.shift as u32) << 1)
            | ((self.modifiers.control as u32) << 2)
            | ((self.modifiers.alt as u32) << 3)
            | ((self.modifiers.meta as u32) << 7)
    }
    /// What the keyboard holds now (for `PaintedKey::modifiers`).
    pub fn set_modifiers(&mut self, modifiers: Modifiers) {
        self.modifiers = modifiers;
    }
    /// A key event while a painted source's node holds focus; `true`
    /// keeps it from zgx's shortcuts (the page or the backend says so).
    pub fn key_event(&mut self, event: &KeyEvent) -> bool {
        let Some((_, source)) = &self.focused else {
            return false;
        };
        let pressed = event.state == ElementState::Pressed;
        let character = event
            .text
            .as_ref()
            .and_then(|text| text.chars().next())
            .map(|character| character as u16)
            .unwrap_or(0);
        let unmodified = event
            .key_without_modifiers()
            .to_text()
            .and_then(|text| text.chars().next())
            .map(|character| character as u16)
            .unwrap_or(character);
        source.key(&PaintedKey {
            pressed,
            // Linux has no virtual-key codes; the source recovers the
            // physical key from the scancode (`PhysicalKey::from_scancode`)
            // and maps that to its backend's code.
            key_code: 0,
            native_code: event.physical_key.to_scancode().unwrap_or(0),
            character,
            unmodified_character: unmodified,
            modifiers: self.flags(),
        })
    }
    /// The focused node's painted source, if it is one. A source that asks
    /// (its backend moved focus to the chrome) gives the focus back to zgx.
    pub fn sync_focus(&mut self, ui: &Ui) {
        if self.focused.as_ref().is_some_and(|(_, source)| source.take_blur()) {
            self.focused = None;
            self.input.focus(&ui.scene, None);
        }
        let next = ui.input.focused().and_then(|node| match self.natives.content(node) {
            Some(NativeContent::Painted(source)) => Some((node, source)),
            _ => None,
        });
        // Focus moves between painted sources and zgx's own: tell the
        // sources (their backends have their own focus).
        match (&self.focused, &next) {
            (Some((old_node, _)), Some((new_node, _))) if old_node == new_node => {}
            (Some((_, old)), _) => {
                old.focus(false);
                if let Some((_, source)) = &next {
                    source.focus(true);
                }
            }
            (None, Some((_, source))) => source.focus(true),
            (None, None) => {}
        }
        self.focused = next;
    }
    /// The window went away or restarted: every tracked source did too.
    pub(super) fn suspend_painted(&mut self) {
        self.over = None;
        self.held = None;
        self.focused = None;
        self.held_clicks = None;
    }
    /// The scene handle, for the focus move a press makes.
    fn scene_handle(&self) -> Rc<RefCell<Scene>> {
        self.scene.upgrade().expect("a live window's scene")
    }
}
