//! Native views on Linux under X11: each is a child window of the zgx
//! window (an X11 window id, `NativeContent::Platform` holding a `u64`),
//! placed and clipped with the Shape extension, as `child.rs` does with
//! window regions on Windows. Wayland windows can't hold foreign child
//! windows, so there native views stay unsupported (placeholders).
//!
//! There is no overlay plane yet: zgx paint over a native view draws under
//! it. The boxes of overlay paint that take the pointer are cut out of the
//! views' shapes instead, so menus and popovers show (and take input) there.
//! Rounded clips are left square.
use x11rb::{
    connection::{Connection, RequestConnection as _},
    protocol::{
        shape::{self, ConnectionExt as _},
        xproto::{ConfigureWindowAux, ConnectionExt as _, InputFocus, Rectangle, StackMode, Window},
    },
    rust_connection::RustConnection,
};
use zgx::{
    input::InputDispatcher,
    native_view::{NativeContent, NativePlacement, NativeViews, PlaneSplit},
    scene::{NodeId, Rect, RoundRect, Scene},
    widgets::Ui,
};

/// The macOS host's frame transaction has no counterpart here.
pub(crate) struct Transaction;

/// An overlay box taking the pointer: its rounded box and its ancestors' clip (window points).
type Hole = (RoundRect, Option<Rect>);

struct Clip {
    window: Window,
    /// Its placement, the cut-outs and the scale it was placed with.
    placed: Option<(NativePlacement, Vec<Hole>, f32)>,
    shown: bool,
}

pub(crate) struct Surfaces {
    connection: RustConnection,
    root: Window,
    parent: Window,
    /// Device pixels per point of the window (its scale factor).
    window_scale: f32,
    clips: Vec<(NodeId, Clip)>,
    holes: Vec<Hole>,
    focus: Option<NodeId>,
    zgx_focus: Option<NodeId>,
    zoom: f32,
    shape: bool,
}

/// Device pixels of `rect` (window points) relative to `origin` (pixels).
fn pixels(rect: Rect, scale: f32, origin: (i32, i32)) -> (i32, i32, i32, i32) {
    let p = |v: f32| (v * scale).round() as i32;
    (
        p(rect.x) - origin.0,
        p(rect.y) - origin.1,
        p(rect.x + rect.width) - origin.0,
        p(rect.y + rect.height) - origin.1,
    )
}

/// `(left, top, right, bottom)` boxes covering `from` less `cut`.
fn subtract(from: (i32, i32, i32, i32), cut: (i32, i32, i32, i32)) -> Vec<(i32, i32, i32, i32)> {
    let (l, t, r, b) = from;
    let (cl, ct, cr, cb) = (cut.0.max(l), cut.1.max(t), cut.2.min(r), cut.3.min(b));
    if cl >= cr || ct >= cb {
        return vec![from];
    }
    [(l, t, r, ct), (l, cb, r, b), (l, ct, cl, cb), (cr, ct, r, cb)]
        .into_iter()
        .filter(|(l, t, r, b)| l < r && t < b)
        .collect()
}

fn rectangle((l, t, r, b): (i32, i32, i32, i32)) -> Rectangle {
    Rectangle {
        x: l as i16,
        y: t as i16,
        width: (r - l) as u16,
        height: (b - t) as u16,
    }
}

impl Surfaces {
    pub fn new(window: &winit::window::Window) -> Result<Self, String> {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let handle = window.window_handle().map_err(|error| error.to_string())?;
        let parent = match handle.as_raw() {
            RawWindowHandle::Xlib(handle) => handle.window as Window,
            RawWindowHandle::Xcb(handle) => handle.window.get(),
            _ => return Err("native views need an X11 window (XWayland under Wayland)".into()),
        };
        let (connection, screen) = x11rb::connect(None).map_err(|error| error.to_string())?;
        let root = connection.setup().roots[screen].root;
        let shape = connection
            .extension_information(shape::X11_EXTENSION_NAME)
            .ok()
            .flatten()
            .is_some();
        Ok(Self {
            connection,
            root,
            parent,
            window_scale: window.scale_factor() as f32,
            clips: Vec::new(),
            holes: Vec::new(),
            focus: None,
            zgx_focus: None,
            zoom: 1.,
            shape,
        })
    }
    /// Device pixels per zgx point.
    fn scale(&self) -> f32 {
        self.zoom * self.window_scale
    }
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom;
    }
    /// Follow the window's scale factor (it moved to another monitor).
    pub fn set_window_scale(&mut self, scale: f32) -> bool {
        let changed = self.window_scale != scale;
        self.window_scale = scale;
        changed
    }
    pub fn differs(&self, split: &PlaneSplit, previous: &PlaneSplit) -> bool {
        split.natives() != previous.natives()
    }
    /// Note the overlay boxes that take the pointer: cut out of native views
    /// at the next `apply`. Returns whether they changed.
    pub fn cut_outs(&mut self, scene: &Scene, input: &InputDispatcher, split: &PlaneSplit) -> bool {
        let holes: Vec<Hole> = split
            .overlay_boxes()
            .iter()
            .filter(|(node, ..)| input.try_receives_pointer(scene, *node) == Some(true))
            .map(|(_, round, clip)| (*round, *clip))
            .collect();
        let changed = holes != self.holes;
        self.holes = holes;
        changed
    }
    fn hide(&self, clip: &Clip) {
        let _ = self.connection.unmap_window(clip.window);
    }
    /// Move native views to `split`'s placements.
    pub fn apply(&mut self, natives: &NativeViews, split: &PlaneSplit) {
        let scale = self.scale();
        let views = natives.views();
        let gone: Vec<Window> = self
            .clips
            .iter()
            .filter(|(node, clip)| clip.shown && !views.iter().any(|(other, _)| other == node))
            .map(|(_, clip)| clip.window)
            .collect();
        for window in gone {
            let _ = self.connection.unmap_window(window);
        }
        self.clips.retain(|(node, _)| views.iter().any(|(other, _)| other == node));
        let mut reorder = false;
        for (node, content) in &views {
            let NativeContent::Platform(view) = content else {
                continue;
            };
            let Some(&window) = view.downcast_ref::<u64>() else {
                continue;
            };
            let window = window as Window;
            if window == 0 {
                continue;
            }
            let index = match self.clips.iter().position(|(other, _)| other == node) {
                Some(index) if self.clips[index].1.window == window => index,
                Some(index) => {
                    self.clips[index].1 = Clip { window, placed: None, shown: false };
                    index
                }
                None => {
                    self.clips.push((*node, Clip { window, placed: None, shown: false }));
                    self.clips.len() - 1
                }
            };
            let parent = self
                .connection
                .query_tree(window)
                .ok()
                .and_then(|cookie| cookie.reply().ok())
                .map(|tree| tree.parent);
            if parent != Some(self.parent) {
                // Mounted: a child of this window from now on.
                let _ = self.connection.reparent_window(window, self.parent, 0, 0);
                self.clips[index].1.placed = None;
                self.clips[index].1.shown = false;
                reorder = true;
            }
            let was = self.clips[index].1.placed.as_ref().map(|(p, _, _)| p.order);
            let placement = split.native(*node).cloned();
            let holes = self.holes.clone();
            let mut clip = std::mem::replace(&mut self.clips[index].1, Clip { window, placed: None, shown: false });
            self.place(&mut clip, placement, &holes, scale);
            if clip.placed.as_ref().map(|(p, _, _)| p.order) != was {
                reorder = true;
            }
            self.clips[index].1 = clip;
        }
        if reorder {
            // Paint order: each later view above the ones before.
            let mut ordered: Vec<(usize, Window)> = self
                .clips
                .iter()
                .filter_map(|(_, clip)| clip.placed.as_ref().map(|(p, _, _)| (p.order, clip.window)))
                .collect();
            ordered.sort_by_key(|(order, _)| *order);
            for (_, window) in ordered {
                let _ = self
                    .connection
                    .configure_window(window, &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE));
            }
        }
        let _ = self.connection.flush();
    }
    fn place(&self, clip: &mut Clip, placement: Option<NativePlacement>, holes: &[Hole], scale: f32) {
        let visible = placement
            .as_ref()
            .filter(|placement| placement.opacity > 0.01)
            .and_then(|placement| placement.visible());
        let (Some(placement), Some(visible)) = (placement, visible) else {
            if clip.shown {
                self.hide(clip);
                clip.shown = false;
            }
            clip.placed = None;
            return;
        };
        let holes: Vec<Hole> = holes
            .iter()
            .filter(|(round, within)| {
                let reach = within.map_or(Some(round.rect), |within| round.rect.intersection(within));
                reach.is_some_and(|reach| reach.intersects(visible))
            })
            .copied()
            .collect();
        let moved = !clip.shown
            || clip
                .placed
                .as_ref()
                .is_none_or(|(old, _, old_scale)| *old != placement || *old_scale != scale);
        if !moved && clip.placed.as_ref().is_some_and(|(_, old, _)| *old == holes) {
            return;
        }
        let (left, top, right, bottom) = pixels(placement.frame, scale, (0, 0));
        let origin = (left, top);
        if self.shape {
            // The visible part less cut-outs, as rectangles in the view's own pixels.
            let mut boxes = vec![pixels(visible, scale, origin)];
            for (round, within) in &holes {
                let mut cut = pixels(round.rect, scale, origin);
                if let Some(within) = within {
                    let (l, t, r, b) = pixels(*within, scale, origin);
                    cut = (cut.0.max(l), cut.1.max(t), cut.2.min(r), cut.3.min(b));
                }
                boxes = boxes.into_iter().flat_map(|from| subtract(from, cut)).collect();
            }
            let rectangles: Vec<Rectangle> = boxes.into_iter().map(rectangle).collect();
            let _ = self.connection.shape_rectangles(
                shape::SO::SET,
                shape::SK::BOUNDING,
                x11rb::protocol::xproto::ClipOrdering::UNSORTED,
                clip.window,
                0,
                0,
                &rectangles,
            );
        }
        if moved {
            let aux = ConfigureWindowAux::new()
                .x(left)
                .y(top)
                .width((right - left).max(1) as u32)
                .height((bottom - top).max(1) as u32);
            let _ = self.connection.configure_window(clip.window, &aux);
            let _ = self.connection.map_window(clip.window);
        }
        clip.shown = true;
        clip.placed = Some((placement, holes, scale));
    }
    /// Whether `inner` is `outer` or inside it.
    fn within(&self, inner: Window, outer: Window) -> bool {
        let mut current = inner;
        for _ in 0..64 {
            if current == outer {
                return true;
            }
            let Some(tree) = self.connection.query_tree(current).ok().and_then(|c| c.reply().ok()) else {
                return false;
            };
            if tree.parent == self.root || tree.parent == x11rb::NONE {
                return false;
            }
            current = tree.parent;
        }
        false
    }
    fn input_focus(&self) -> Option<Window> {
        let focus = self.connection.get_input_focus().ok()?.reply().ok()?.focus;
        (focus > 1).then_some(focus)
    }
    /// The native view whose window (or a window inside it) has keyboard focus.
    fn focused_view(&self) -> Option<NodeId> {
        let focus = self.input_focus()?;
        self.clips
            .iter()
            .find_map(|(node, clip)| self.within(focus, clip.window).then_some(*node))
    }
    fn focus_window(&self, window: Window) {
        let _ = self
            .connection
            .set_input_focus(InputFocus::PARENT, window, x11rb::CURRENT_TIME);
        let _ = self.connection.flush();
    }
    /// A press zgx takes: keyboard focus leaves a native view for the window.
    pub fn pointer_down(&mut self) {
        if self.focused_view().is_some() {
            self.focus_window(self.parent);
        }
    }
    /// Agree zgx's focus with the window that has X11's keyboard focus.
    pub fn sync_focus(&mut self, ui: &Ui, natives: &NativeViews) {
        self.follow_zgx_focus(ui, natives);
        let focused = self.focused_view();
        if focused == self.focus {
            return;
        }
        let previous = std::mem::replace(&mut self.focus, focused);
        match focused {
            Some(node) if ui.input.focused() != Some(node) && natives.contains(node) => {
                ui.input.focus(&ui.scene, Some(node));
            }
            None if previous.is_some() && ui.input.focused() == previous => {
                ui.input.focus(&ui.scene, None);
            }
            _ => {}
        }
        self.zgx_focus = ui.input.focused();
    }
    fn follow_zgx_focus(&mut self, ui: &Ui, natives: &NativeViews) {
        let now = ui.input.focused();
        if now == self.zgx_focus {
            return;
        }
        self.zgx_focus = now;
        let window = now.filter(|node| natives.contains(*node)).and_then(|node| {
            self.clips
                .iter()
                .find(|(other, clip)| *other == node && clip.shown)
                .map(|(_, clip)| clip.window)
        });
        if let Some(window) = window {
            if !self.input_focus().is_some_and(|focus| self.within(focus, window)) {
                self.focus_window(window);
            }
        } else if self.focused_view().is_some() {
            self.focus_window(self.parent);
        }
    }
}
