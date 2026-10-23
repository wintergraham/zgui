//! A window's native views (web pages, platform views) composited with its
//! zgui content; see `zgui::native_view` for the model.
//!
//! Each frame, after layout: split the scene's paint into the base plane (the
//! window renderer, under native views) and the overlay plane (a second
//! renderer presenting into a layer above them, sized to the overlay paint);
//! then present both planes and move the native views and the platform
//! backdrops under overlay glass in one Core Animation transaction, so they
//! never drift a frame apart. A window without native views does none of it.
#[cfg(all(target_os = "macos", feature = "ghostty"))]
mod ghostty;
#[cfg(all(target_os = "macos", feature = "ghostty"))]
pub(crate) use ghostty::processes_changed;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
mod webkit;

use std::rc::Rc;
use zgui::{
    native_view::{NativeViews, PlaneSplit, PointerStep},
    scene::{Rect, Scene},
    widgets::Ui,
};

#[cfg(target_os = "macos")]
pub use macos::{BackdropReport, NativeDiagnostics, NativeViewReport};
#[cfg(target_os = "macos")]
pub(crate) use macos::{disable_cursor_rects, set_cursor};
#[cfg(target_os = "macos")]
pub(crate) type NativeDiagnosticsHandle = NativeDiagnostics;
#[cfg(not(target_os = "macos"))]
pub(crate) type NativeDiagnosticsHandle = Rc<()>;

/// How long frames keep to the plain layer after native views move (see
/// `GpuRenderer::present_plain_for`): long enough to cover a scroll's
/// momentum or an animation's frames in between.
#[cfg(target_os = "macos")]
const SYNCED: std::time::Duration = std::time::Duration::from_millis(500);

pub(crate) struct NativeHost {
    pub natives: NativeViews,
    /// Shared with the window's context (`WindowContext::native_diagnostics`).
    pub diagnostics: NativeDiagnosticsHandle,
    split: Rc<PlaneSplit>,
    #[cfg(target_os = "macos")]
    surfaces: Option<macos::Surfaces>,
    #[cfg(target_os = "macos")]
    overlay: Option<zgui_gpu::GpuRenderer>,
    /// The overlay renderer drew this frame (present it).
    #[cfg(target_os = "macos")]
    overlay_drawn: bool,
    /// The scale the overlay renderer and its layer were made for.
    #[cfg(target_os = "macos")]
    overlay_scale: f32,
    /// The overlay layer shows its plane (it had paint at the last present).
    #[cfg(target_os = "macos")]
    overlay_live: bool,
    /// Native geometry changed and has not been applied yet.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pending: bool,
    /// What clip views' hit tests consult (macOS's clip views).
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    scene: std::rc::Weak<std::cell::RefCell<Scene>>,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    input: zgui::input::InputDispatcher,
    _waker: zgui::native_view::WakerGuard,
}

impl NativeHost {
    pub fn new(ui: &Ui, wake: impl Fn() + 'static) -> Self {
        let natives = NativeViews::new();
        natives.set_supported(cfg!(target_os = "macos"));
        Self {
            natives,
            diagnostics: Default::default(),
            split: Rc::new(PlaneSplit::default()),
            #[cfg(target_os = "macos")]
            surfaces: None,
            #[cfg(target_os = "macos")]
            overlay: None,
            #[cfg(target_os = "macos")]
            overlay_drawn: false,
            #[cfg(target_os = "macos")]
            overlay_scale: 0.,
            #[cfg(target_os = "macos")]
            overlay_live: false,
            pending: false,
            scene: Rc::downgrade(&ui.scene),
            input: ui.input.clone(),
            _waker: zgui::native_view::set_waker(wake),
        }
    }
    /// Whether this window has native views to keep up (now or last frame).
    pub fn active(&self) -> bool {
        !self.natives.is_empty() || !self.split.is_empty()
    }
    /// Whether `node` is a native view's node (pointer over the page: the
    /// platform view owns the cursor).
    pub fn is_native(&self, node: zgui::scene::NodeId) -> bool {
        self.natives.contains(node)
    }
    /// The pointer moved to window point `(x, y)` (the host's own report):
    /// what zgui hears of it (see `NativePointer`). `held`: a press of
    /// zgui's follows the pointer.
    pub fn pointer_moved(&self, x: f32, y: f32, held: bool) -> PointerStep {
        #[cfg(target_os = "macos")]
        if let Some(surfaces) = &self.surfaces {
            let (step, report) = surfaces.router().track(x, y, held);
            report.apply();
            return step;
        }
        let _ = (x, y, held);
        PointerStep::Move
    }
    /// The pointer left the window, or the window went to the back.
    pub fn pointer_left(&self) {
        #[cfg(target_os = "macos")]
        if let Some(surfaces) = &self.surfaces {
            surfaces.router().pointer.left().apply();
        }
    }
    /// Whether the pointer is over a native view taking input (it shows
    /// its own cursor).
    pub fn pointer_over_native(&self) -> bool {
        #[cfg(target_os = "macos")]
        if let Some(surfaces) = &self.surfaces {
            return surfaces.router().pointer.over().is_some();
        }
        false
    }
    /// Where the pointer last moved to that the host did not hear itself
    /// (a native view had keyboard focus, so AppKit sent the move there).
    pub fn take_unheard_pointer(&self) -> Option<(f32, f32)> {
        #[cfg(target_os = "macos")]
        if let Some(surfaces) = &self.surfaces {
            return surfaces.router().unheard.take();
        }
        None
    }
    /// The native views went away with the window (suspend/recreate).
    pub fn suspend(&mut self) {
        #[cfg(target_os = "macos")]
        {
            self.overlay = None;
            self.surfaces = None;
            self.overlay_live = false;
            self.overlay_drawn = false;
        }
        self.split = Rc::new(PlaneSplit::default());
        self.natives.set_split(self.split.clone());
    }
}

#[cfg(target_os = "macos")]
impl NativeHost {
    /// After the scene's flush: split this frame's paint around native views
    /// and give each plane's renderer its part. Returns damage the split adds
    /// (paint that moved between planes repaints on both).
    pub fn plan(
        &mut self,
        scene: &Scene,
        window: &winit::window::Window,
        renderer: &mut zgui_gpu::GpuRenderer,
        scale: f32,
    ) -> Result<Vec<Rect>, zgui_gpu::GpuError> {
        // Frames go with native geometry, in one transaction, while there is any.
        renderer.present_with_native_views(self.active());
        if !self.active() {
            return Ok(Vec::new());
        }
        if self.surfaces.is_none() {
            let router = macos::Router {
                scene: self.scene.clone(),
                input: self.input.clone(),
                natives: self.natives.clone(),
                pointer: Default::default(),
                unheard: Default::default(),
            };
            match macos::Surfaces::new(window, router) {
                Ok(surfaces) => {
                    surfaces.report_to(&self.diagnostics);
                    self.surfaces = Some(surfaces);
                }
                Err(error) => {
                    eprintln!("zgui: native views unavailable: {error}");
                    self.natives.set_supported(false);
                    return Ok(Vec::new());
                }
            }
        }
        let (split, damage) = PlaneSplit::compute(scene, &self.natives.nodes(), &self.split);
        let split = Rc::new(split);
        let surfaces = self.surfaces.as_mut().expect("made above");
        if surfaces.differs(&split, &self.split) {
            self.pending = true;
            renderer.present_plain_for(SYNCED);
        }
        if split.is_empty() {
            renderer.clear_plane();
        } else {
            renderer.set_plane(split.clone(), zgui::native_view::Plane::Base);
            if split.has_overlay() {
                // Base and overlay frames show together, in the transaction.
                renderer.present_plain_for(SYNCED);
            }
        }
        // The overlay plane: made for the first paint over a native view,
        // kept, and made again for a new display scale (its layer's too).
        if self.overlay.is_some() && self.overlay_scale != scale {
            self.overlay = None;
            self.overlay_live = false;
        }
        if split.has_overlay() && self.overlay.is_none() {
            self.overlay_scale = scale;
            let overlay = zgui_gpu::GpuRenderer::for_overlay_layer(
                surfaces.overlay_host(scale),
                &renderer.context(),
                scale,
            )?;
            self.overlay = Some(overlay);
        }
        if let Some(overlay) = &mut self.overlay {
            overlay.set_plane(split.clone(), zgui::native_view::Plane::Overlay);
            if let Some(needed) = split.overlay_bounds() {
                let viewport = scene.bounds(scene.root());
                let rect = surfaces.plane_rect(overlay.plane_rect(), needed, viewport);
                overlay.set_plane_rect(rect);
                if !self.overlay_live {
                    overlay.repaint_all();
                }
            }
        }
        self.split = split.clone();
        self.natives.set_split(split);
        Ok(damage)
    }
    /// Render the overlay plane's part of this frame's `damage`.
    pub fn render(&mut self, scene: &Scene, damage: &[Rect]) -> Result<(), zgui_gpu::GpuError> {
        self.overlay_drawn = false;
        if let Some(overlay) = &mut self.overlay
            && self.split.has_overlay()
        {
            overlay.render(scene, damage)?;
            self.overlay_drawn = true;
        }
        Ok(())
    }
    /// Open the transaction a frame's planes and native geometry commit in.
    pub fn begin(&self) -> Option<macos::Transaction> {
        self.active().then(macos::Transaction::begin)
    }
    /// Within `begin`'s transaction: show the overlay plane's frame (before
    /// the base plane's, which waits for its refresh point).
    pub fn present_overlay(&mut self) -> Result<(), zgui_gpu::GpuError> {
        let Some(surfaces) = &mut self.surfaces else {
            return Ok(());
        };
        let live = self.split.has_overlay();
        if live != self.overlay_live {
            zgui_gpu::present_log::log(|| format!("overlay live={live}"));
        }
        if let Some(overlay) = &mut self.overlay
            && live
            && self.overlay_drawn
        {
            overlay.present()?;
        }
        if let Some(overlay) = &self.overlay {
            let rect = live.then(|| overlay.plane_rect()).flatten();
            zgui_gpu::present_log::log(|| {
                format!("overlay show rect={rect:?} drawn={}", self.overlay_drawn)
            });
            surfaces.show_overlay(rect);
        }
        if self.overlay_live
            && !live
            && let Some(overlay) = &mut self.overlay
        {
            // Hidden while nothing paints on it: one surface is enough.
            overlay.trim();
        }
        self.overlay_live = live;
        self.overlay_drawn = false;
        Ok(())
    }
    /// Within the frame's transaction: move native views and backdrops to
    /// this frame's placements.
    pub fn apply(&mut self) {
        if let Some(surfaces) = &mut self.surfaces {
            surfaces.apply(&self.natives, &self.split);
        }
        self.pending = false;
    }
    /// Apply geometry that changed in a frame without zgui pixels to show
    /// (a native node moved, painting nothing), in a transaction of its own.
    pub fn flush(&mut self) {
        if self.pending {
            let transaction = macos::Transaction::begin();
            self.apply();
            drop(transaction);
        }
    }
    /// Keep zgui's focus and the page's keyboard focus agreeing (a click in
    /// a page focuses its node; a click on zgui content leaves the page).
    pub fn sync_focus(&mut self, ui: &Ui) {
        if let Some(surfaces) = &mut self.surfaces {
            surfaces.sync_focus(ui, &self.natives);
        }
    }
    /// Release what an idle window does not need.
    pub fn trim(&mut self) {
        if !self.split.has_overlay()
            && let Some(overlay) = &mut self.overlay
        {
            overlay.trim();
        }
    }
}

#[cfg(not(target_os = "macos"))]
impl NativeHost {
    pub fn plan(
        &mut self,
        _: &Scene,
        _: &winit::window::Window,
        _: &mut zgui_gpu::GpuRenderer,
        _: f32,
    ) -> Result<Vec<Rect>, zgui_gpu::GpuError> {
        Ok(Vec::new())
    }
    pub fn render(&mut self, _: &Scene, _: &[Rect]) -> Result<(), zgui_gpu::GpuError> {
        Ok(())
    }
    pub fn begin(&self) -> Option<()> {
        None
    }
    pub fn present_overlay(&mut self) -> Result<(), zgui_gpu::GpuError> {
        Ok(())
    }
    pub fn apply(&mut self) {}
    pub fn flush(&mut self) {}
    pub fn sync_focus(&mut self, _: &Ui) {}
    pub fn trim(&mut self) {}
}
