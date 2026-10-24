//! Native views on Windows without the `webview2` feature: each is a child
//! window of the zgui window (the host's `HWND`, `NativeContent::Platform`),
//! placed, clipped and rounded with a window region. Child windows show over
//! the window's own composition (the base plane); the overlay plane presents
//! into a visual of a second, topmost target, which Windows composites above
//! child windows.
//!
//! Child windows take the pointer before their parent, so the boxes of
//! overlay paint that takes the pointer are cut out of their regions: there
//! zgui gets the input, as the macOS host's hit testing routes it.
use std::ffi::c_void;
use windows::Win32::{
    Foundation::HWND,
    Graphics::{
        DirectComposition::{
            DCompositionCreateDevice2, IDCompositionDevice, IDCompositionTarget,
            IDCompositionVisual,
        },
        Gdi::{CombineRgn, CreateRectRgn, DeleteObject, HRGN, RGN_AND, RGN_DIFF, SetWindowRgn},
    },
    UI::{
        HiDpi::GetDpiForWindow,
        Input::KeyboardAndMouse::{GetFocus, SetFocus},
        WindowsAndMessaging::{
            GWL_STYLE, GetParent, GetWindowLongPtrW, HWND_TOP, IsChild, IsWindowVisible,
            SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW,
            SetParent, SetWindowLongPtrW, SetWindowPos, WS_CHILD, WS_CLIPCHILDREN, WS_POPUP,
        },
    },
};
use zgui::{
    input::InputDispatcher,
    native_view::{NativeContent, NativePlacement, NativeViews, PlaneSplit},
    scene::{NodeId, Rect, RoundRect, Scene},
    widgets::Ui,
};

/// The macOS host's frame transaction has no counterpart here (`NativeHost::begin`).
pub(crate) struct Transaction;

/// An overlay box taking the pointer: its rounded box and its ancestors' clip (window points).
type Hole = (RoundRect, Option<Rect>);

/// A native view's child window and how it was last placed.
struct Clip {
    hwnd: HWND,
    /// Its placement, the cut-outs and the scale it was placed with.
    placed: Option<(NativePlacement, Vec<Hole>, f32)>,
    shown: bool,
}

/// The overlay plane's composition: a topmost target on the window, its root
/// and the visual the overlay renderer presents into.
struct Composition {
    device: IDCompositionDevice,
    _target: IDCompositionTarget,
    root: IDCompositionVisual,
    overlay: IDCompositionVisual,
    /// The overlay visual is in the tree, at this device-pixel offset.
    shown: Option<(f32, f32)>,
}

pub(crate) struct Surfaces {
    parent: HWND,
    composition: Option<Composition>,
    clips: Vec<(NodeId, Clip)>,
    /// Overlay boxes taking the pointer, from `cut_outs`.
    holes: Vec<Hole>,
    /// The view whose window held keyboard focus at the last check.
    focus: Option<NodeId>,
    /// zgui's keyboard focus at the last check.
    zgui_focus: Option<NodeId>,
}

fn rect_region(left: i32, top: i32, right: i32, bottom: i32) -> HRGN {
    unsafe { CreateRectRgn(left, top, right, bottom) }
}
/// Remove from `region` what `cut` covers, then free `cut`.
fn subtract(region: HRGN, cut: HRGN) {
    unsafe {
        if !cut.is_invalid() {
            CombineRgn(Some(region), Some(region), Some(cut), RGN_DIFF);
            let _ = DeleteObject(cut.into());
        }
    }
}
/// Keep in `region` only what `keep` covers, then free `keep`.
fn intersect(region: HRGN, keep: HRGN) {
    unsafe {
        if !keep.is_invalid() {
            CombineRgn(Some(region), Some(region), Some(keep), RGN_AND);
            let _ = DeleteObject(keep.into());
        }
    }
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
/// A rounded box's region: the box less each corner's pixels wholly outside
/// its arc (aliased, as Windows regions are).
fn round_region(round: &RoundRect, scale: f32, origin: (i32, i32)) -> HRGN {
    let (left, top, right, bottom) = pixels(round.rect, scale, origin);
    let region = rect_region(left, top, right, bottom);
    if region.is_invalid() {
        return region;
    }
    let (width, height) = (right - left, bottom - top);
    // Top-left, top-right, bottom-right, bottom-left.
    for (corner, radius) in round.radii.iter().enumerate() {
        let radius = (radius * scale).max(0.).min(width.min(height) as f32 * 0.5);
        let reach = radius.ceil() as i32;
        for row in 0..reach {
            // Pixels of this row (from the corner's edge) outside the arc.
            let dy = radius - row as f32 - 0.5;
            let inside = (radius * radius - dy * dy).max(0.).sqrt();
            let span = (radius - inside).floor() as i32;
            if span <= 0 {
                continue;
            }
            let y = if corner < 2 {
                top + row
            } else {
                bottom - 1 - row
            };
            let x = if corner == 0 || corner == 3 {
                left
            } else {
                right - span
            };
            subtract(region, rect_region(x, y, x + span, y + 1));
        }
    }
    region
}

impl Surfaces {
    pub fn new(window: &winit::window::Window) -> Result<Self, String> {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let handle = window.window_handle().map_err(|error| error.to_string())?;
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return Err("not a Win32 window".into());
        };
        let parent = HWND(handle.hwnd.get() as *mut c_void);
        // Child windows keep their pixels out of the window's own.
        unsafe {
            let style = GetWindowLongPtrW(parent, GWL_STYLE);
            SetWindowLongPtrW(parent, GWL_STYLE, style | WS_CLIPCHILDREN.0 as isize);
        }
        let composition = match Self::composition(parent) {
            Ok(composition) => Some(composition),
            Err(error) => {
                eprintln!("zgui: no overlay plane over native views: {error}");
                None
            }
        };
        Ok(Self {
            parent,
            composition,
            clips: Vec::new(),
            holes: Vec::new(),
            focus: None,
            zgui_focus: None,
        })
    }
    fn composition(parent: HWND) -> windows::core::Result<Composition> {
        unsafe {
            let device: IDCompositionDevice = DCompositionCreateDevice2(None)?;
            // Topmost: above the window's child windows.
            let target = device.CreateTargetForHwnd(parent, true)?;
            let root = device.CreateVisual()?;
            target.SetRoot(&root)?;
            let overlay = device.CreateVisual()?;
            device.Commit()?;
            Ok(Composition {
                device,
                _target: target,
                root,
                overlay,
                shown: None,
            })
        }
    }
    /// The window's scale (device pixels per point).
    fn scale(&self) -> f32 {
        match unsafe { GetDpiForWindow(self.parent) } {
            0 => 1.,
            dpi => dpi as f32 / 96.,
        }
    }
    /// The overlay plane's renderer, presenting into the topmost target's
    /// visual (shown by `show_overlay`); none without the composition.
    pub fn overlay_renderer(
        &mut self,
        context: &zgui_gpu::GpuContext,
        scale: f32,
    ) -> Option<Result<zgui_gpu::GpuRenderer, zgui_gpu::GpuError>> {
        let composition = self.composition.as_ref()?;
        let renderer = zgui_gpu::GpuRenderer::for_overlay_visual(
            composition.overlay.clone(),
            composition.device.clone(),
            context,
            scale,
        );
        // The window's swap chain waits for a refresh; this plane presenting after it needn't wait again.
        Some(renderer.map(|mut renderer| {
            renderer.present_unthrottled();
            renderer
        }))
    }
    /// Whether paint over the child windows has a plane above them.
    pub fn has_overlay_plane(&self) -> bool {
        self.composition.is_some()
    }
    /// Whether `split` moves anything native relative to `previous`.
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
    /// Show the overlay visual at `rect` (window points), or take it out.
    pub fn show_overlay(&mut self, rect: Option<Rect>) {
        let scale = self.scale();
        let Some(composition) = &mut self.composition else {
            return;
        };
        let wanted = rect.map(|rect| ((rect.x * scale).floor(), (rect.y * scale).floor()));
        if wanted == composition.shown {
            return;
        }
        unsafe {
            match (wanted, composition.shown) {
                (None, Some(_)) => {
                    let _ = composition.root.RemoveVisual(&composition.overlay);
                }
                (Some((x, y)), shown) => {
                    let _ = composition.overlay.SetOffsetX2(x);
                    let _ = composition.overlay.SetOffsetY2(y);
                    if shown.is_none() {
                        let _ = composition.root.AddVisual(&composition.overlay, true, None);
                    }
                }
                (None, None) => {}
            }
        }
        composition.shown = wanted;
    }
    /// Show the composition's changes (the overlay's box, new content after
    /// a resize).
    pub fn commit(&self) {
        if let Some(composition) = &self.composition {
            unsafe {
                let _ = composition.device.Commit();
            }
        }
    }
    /// Move native views to `split`'s placements.
    pub fn apply(&mut self, natives: &NativeViews, split: &PlaneSplit) {
        let scale = self.scale();
        let views = natives.views();
        // Unmounted: hidden, left with their owners.
        self.clips.retain(|(node, clip)| {
            let kept = views.iter().any(|(other, _)| other == node);
            if !kept && clip.shown {
                unsafe {
                    let _ = SetWindowPos(
                        clip.hwnd,
                        None,
                        0,
                        0,
                        0,
                        0,
                        SWP_HIDEWINDOW | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            }
            kept
        });
        let mut reorder = false;
        for (node, content) in &views {
            let NativeContent::Platform(view) = content else {
                // Pages and terminals of zgui's own are macOS-only.
                continue;
            };
            let Some(&hwnd) = view.downcast_ref::<*mut c_void>() else {
                continue;
            };
            let hwnd = HWND(hwnd);
            let index = match self.clips.iter().position(|(other, _)| other == node) {
                Some(index) if self.clips[index].1.hwnd == hwnd => index,
                Some(index) => {
                    self.clips[index].1 = Clip {
                        hwnd,
                        placed: None,
                        shown: false,
                    };
                    index
                }
                None => {
                    self.clips.push((
                        *node,
                        Clip {
                            hwnd,
                            placed: None,
                            shown: false,
                        },
                    ));
                    self.clips.len() - 1
                }
            };
            let clip = &mut self.clips[index].1;
            if unsafe { GetParent(hwnd) }.ok() != Some(self.parent) {
                // Mounted: a child of this window from now on.
                unsafe {
                    let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
                    let style = (style & !(WS_POPUP.0 as isize)) | WS_CHILD.0 as isize;
                    SetWindowLongPtrW(hwnd, GWL_STYLE, style);
                    let _ = SetParent(hwnd, Some(self.parent));
                }
                clip.placed = None;
                reorder = true;
            }
            let was = clip
                .placed
                .as_ref()
                .map(|(placement, _, _)| placement.order);
            Self::place(clip, split.native(*node).cloned(), &self.holes, scale);
            if clip
                .placed
                .as_ref()
                .map(|(placement, _, _)| placement.order)
                != was
            {
                reorder = true;
            }
        }
        if reorder {
            // Paint order: each later view above the ones before.
            let mut ordered: Vec<(usize, HWND)> = self
                .clips
                .iter()
                .filter_map(|(_, clip)| clip.placed.as_ref().map(|(p, _, _)| (p.order, clip.hwnd)))
                .collect();
            ordered.sort_by_key(|(order, _)| *order);
            for (_, hwnd) in ordered {
                unsafe {
                    let _ = SetWindowPos(
                        hwnd,
                        Some(HWND_TOP),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                }
            }
        }
    }
    fn place(clip: &mut Clip, placement: Option<NativePlacement>, holes: &[Hole], scale: f32) {
        let visible = placement
            .as_ref()
            .filter(|placement| placement.opacity > 0.01)
            .and_then(|placement| placement.visible());
        let (Some(placement), Some(visible)) = (placement, visible) else {
            if clip.shown {
                unsafe {
                    let _ = SetWindowPos(
                        clip.hwnd,
                        None,
                        0,
                        0,
                        0,
                        0,
                        SWP_HIDEWINDOW | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
                clip.shown = false;
            }
            clip.placed = None;
            return;
        };
        let holes: Vec<Hole> = holes
            .iter()
            .filter(|(round, clip)| {
                let reach = clip.map_or(Some(round.rect), |clip| round.rect.intersection(clip));
                reach.is_some_and(|reach| reach.intersects(visible))
            })
            .copied()
            .collect();
        let moved = !clip.shown
            || clip
                .placed
                .as_ref()
                .is_none_or(|(old, _, old_scale)| *old != placement || *old_scale != scale);
        if !moved
            && clip
                .placed
                .as_ref()
                .is_some_and(|(_, old_holes, _)| *old_holes == holes)
        {
            return;
        }
        let (left, top, right, bottom) = pixels(placement.frame, scale, (0, 0));
        let origin = (left, top);
        unsafe {
            // The visible part, rounded by each rounded clip, less cut-outs.
            let (l, t, r, b) = pixels(visible, scale, origin);
            let region = rect_region(l, t, r, b);
            if !region.is_invalid() {
                for round in &placement.rounded {
                    intersect(region, round_region(round, scale, origin));
                }
                for (round, within) in &holes {
                    // A device pixel in from its edge: the box's antialiased rim and border blend over the page.
                    let inset = 1. / scale;
                    let rect = round.rect;
                    let inner = RoundRect {
                        rect: Rect::new(
                            rect.x + inset,
                            rect.y + inset,
                            (rect.width - 2. * inset).max(0.),
                            (rect.height - 2. * inset).max(0.),
                        ),
                        radii: round.radii.map(|radius| (radius - inset).max(0.)),
                    };
                    let hole = round_region(&inner, scale, origin);
                    if let Some(within) = within {
                        let (l, t, r, b) = pixels(*within, scale, origin);
                        intersect(hole, rect_region(l, t, r, b));
                    }
                    subtract(region, hole);
                }
                // The compositor clips a child window to its region as it shows it: only a move repaints.
                if SetWindowRgn(clip.hwnd, Some(region), moved && clip.shown) == 0 {
                    let _ = DeleteObject(region.into());
                }
            }
            if moved {
                let _ = SetWindowPos(
                    clip.hwnd,
                    None,
                    left,
                    top,
                    (right - left).max(1),
                    (bottom - top).max(1),
                    SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
        }
        clip.shown = true;
        clip.placed = Some((placement, holes, scale));
    }
    /// The native view whose window (or a window inside it) has keyboard focus.
    fn focused_view(&self) -> Option<NodeId> {
        let focus = unsafe { GetFocus() };
        if focus.is_invalid() {
            return None;
        }
        self.clips.iter().find_map(|(node, clip)| {
            (focus == clip.hwnd || unsafe { IsChild(clip.hwnd, focus) }.as_bool()).then_some(*node)
        })
    }
    /// A press zgui takes: keyboard focus leaves a native view for the window.
    pub fn pointer_down(&mut self) {
        if self.focused_view().is_some() {
            unsafe {
                let _ = SetFocus(Some(self.parent));
            }
        }
    }
    /// Agree zgui's focus with the window that has Windows' keyboard focus.
    pub fn sync_focus(&mut self, ui: &Ui, natives: &NativeViews) {
        self.follow_zgui_focus(ui, natives);
        let focused = self.focused_view();
        if focused == self.focus {
            return;
        }
        let previous = std::mem::replace(&mut self.focus, focused);
        match focused {
            // A click in the page, or the page focusing itself.
            Some(node) if ui.input.focused() != Some(node) && natives.contains(node) => {
                ui.input.focus(&ui.scene, Some(node));
            }
            // Focus left it (a click on zgui content, another window).
            None if previous.is_some() && ui.input.focused() == previous => {
                ui.input.focus(&ui.scene, None);
            }
            _ => {}
        }
        self.zgui_focus = ui.input.focused();
    }
    /// zgui moved focus onto a native view's node: its window takes keyboard
    /// focus; off one that held it: the zgui window takes it back.
    fn follow_zgui_focus(&mut self, ui: &Ui, natives: &NativeViews) {
        let now = ui.input.focused();
        if now == self.zgui_focus {
            return;
        }
        self.zgui_focus = now;
        let window = |node: NodeId| {
            natives
                .contains(node)
                .then(|| {
                    self.clips
                        .iter()
                        .find(|(other, _)| *other == node)
                        .map(|(_, clip)| clip.hwnd)
                })
                .flatten()
        };
        if let Some(hwnd) = now.and_then(window) {
            let focus = unsafe { GetFocus() };
            let inside = focus == hwnd || unsafe { IsChild(hwnd, focus) }.as_bool();
            if !inside && unsafe { IsWindowVisible(hwnd) }.as_bool() {
                unsafe {
                    let _ = SetFocus(Some(hwnd));
                }
            }
        } else if self.focused_view().is_some() {
            unsafe {
                let _ = SetFocus(Some(self.parent));
            }
        }
    }
}
