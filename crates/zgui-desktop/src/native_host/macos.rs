//! AppKit views and layers for a window's native views.
//!
//! The window's content view (winit's, flipped, its layer showing the base
//! plane) holds, in this order from the bottom:
//!
//! - per native view, in paint order, a stack of nested full-window "cut"
//!   views, each masking what it holds to one shape: the node's rectangular
//!   clip, then each rounded clip (its own corners first). The innermost holds
//!   the native view at the node's frame. Cut views never move: moving a
//!   `WKWebView`'s host while WebKit updates its remote layer tree can show
//!   the old origin with the new size (comet-native's finding);
//! - per overlay node with a backdrop blur, cuts for its ancestors' clips
//!   holding a `CABackdropLayer` view shaped to the node, blurring the
//!   composited layers under it (pages included: a within-window
//!   `NSVisualEffectView` may use a cached image without them);
//! - the overlay view, hosting the overlay plane's layer at the overlay
//!   paint's box.
//!
//! Only the outermost cut of each native view routes hit testing: it answers
//! for its node where `PlaneSplit::route` sends input there, so zgui gets
//! every point where it draws over a page or has an interactive node in front.
//!
//! Layer frames are placed in each owner layer's effective orientation:
//! `geometryFlipped` is relative to the superlayer (AppKit sets it to keep
//! layer-backed views' layers upright), so a layer's space is top-down when
//! an odd number of it and its superlayers are flipped. Cuts recheck it
//! every frame (AppKit may settle it after a view first shows).
use super::webkit;
use objc2::{
    ClassType, DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, NSObject},
};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSView, NSWindowOrderingMode};
use objc2_core_foundation::{CFRetained, CGFloat, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGColor, CGMutablePath};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use objc2_quartz_core::{CALayer, CAShapeLayer, CATransaction};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
use zgui::{
    input::InputDispatcher,
    native_view::{
        BackdropPlacement, NativeContent, NativePlacement, NativeViews, PlaneSplit, PointerRoute,
    },
    scene::{NodeId, Rect, RoundRect, Scene},
    widgets::Ui,
};

/// Cut levels around a native view: its rectangular clip, its own corners
/// and its two nearest rounded clipping ancestors (`RoundedClip`).
const NATIVE_CUTS: usize = 4;
/// Around a backdrop: its ancestors' rectangle and two rounded clips (its
/// own corners are its layer's).
const BACKDROP_CUTS: usize = 3;

thread_local! {
    /// Each window's router, by its content view: native views hand zgui
    /// the input they do not use.
    static ROUTERS: RefCell<Vec<(usize, Weak<Router>)>> = const { RefCell::new(Vec::new()) };
}

/// A secondary click a native view did not use (a terminal's right click
/// with no program asking for the mouse): zgui gets it as its own, at the
/// same point, so a context menu there opens.
#[allow(dead_code)]
pub(crate) fn forward_secondary_click(view: &NSView, event: &objc2_app_kit::NSEvent) {
    let Some(content) = view.window().and_then(|window| window.contentView()) else {
        return;
    };
    let point = content.convertPoint_fromView(event.locationInWindow(), None);
    let y = if content.isFlipped() {
        point.y
    } else {
        content.frame().size.height - point.y
    };
    let key = Retained::as_ptr(&content) as usize;
    let router = ROUTERS.with(|routers| {
        routers
            .borrow()
            .iter()
            .find(|(view, _)| *view == key)
            .and_then(|(_, router)| router.upgrade())
    });
    let Some(router) = router else {
        return;
    };
    let (x, y) = (point.x as f32, y as f32);
    zgui::native_view::post(move || {
        let Some(scene) = router.scene.upgrade() else {
            return;
        };
        use zgui::input::{InputEvent, PointerButton};
        let button = PointerButton::Secondary;
        router
            .input
            .dispatch(&scene, InputEvent::PointerDown { x, y, button });
        router
            .input
            .dispatch(&scene, InputEvent::PointerUp { x, y, button });
    });
}

/// The cursor zgui shows at `event`'s point in `view`'s window (what it
/// draws over a native view there: a pane's grip), if it sets one.
#[allow(dead_code)]
pub(crate) fn zgui_cursor_at(
    view: &NSView,
    event: &objc2_app_kit::NSEvent,
) -> Option<zgui::cursor::Cursor> {
    let content = view.window().and_then(|window| window.contentView())?;
    let point = content.convertPoint_fromView(event.locationInWindow(), None);
    let y = if content.isFlipped() {
        point.y
    } else {
        content.frame().size.height - point.y
    };
    let key = Retained::as_ptr(&content) as usize;
    let router = ROUTERS.with(|routers| {
        routers
            .borrow()
            .iter()
            .find(|(view, _)| *view == key)
            .and_then(|(_, router)| router.upgrade())
    })?;
    let scene = router.scene.upgrade()?;
    let scene = scene.try_borrow().ok()?;
    scene.cursor_at(point.x as f32, y as f32)
}

/// Whether `view` or a view in it has its window's keyboard focus.
fn has_focus(view: &NSView) -> bool {
    view.window()
        .and_then(|window| window.firstResponder())
        .and_then(|responder| responder.downcast::<NSView>().ok())
        .is_some_and(|responder| responder.isDescendantOf(view))
}

/// One Core Animation transaction, without implicit animation: committed on drop.
pub(crate) struct Transaction;
impl Transaction {
    pub fn begin() -> Self {
        CATransaction::begin();
        CATransaction::setDisableActions(true);
        Self
    }
}
impl Drop for Transaction {
    fn drop(&mut self) {
        CATransaction::commit();
    }
}

/// What cut views ask where input goes (`hitTest:` runs inside AppKit's
/// event dispatch, possibly re-entrantly while zgui holds its state).
pub(crate) struct Router {
    pub scene: Weak<RefCell<Scene>>,
    pub input: InputDispatcher,
    pub natives: NativeViews,
}
impl Router {
    fn route(&self, x: f32, y: f32) -> PointerRoute {
        let split = self.natives.split();
        let Some(scene) = self.scene.upgrade() else {
            return PointerRoute::Ui;
        };
        match scene.try_borrow() {
            Ok(scene) => split.route(&scene, &self.input, x, y),
            // Mid-frame: the front-most page whose visible box holds the point.
            Err(_) => split
                .natives()
                .iter()
                .rev()
                .find(|placement| {
                    placement.visible().is_some_and(|r| {
                        x >= r.x && y >= r.y && x < r.x + r.width && y < r.y + r.height
                    })
                })
                .map_or(PointerRoute::Ui, |placement| {
                    PointerRoute::Native(placement.node)
                }),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    /// The outermost cut of a native view: routes hit testing for it.
    Route,
    /// Inside a routing cut: hit tests as any view.
    Inner,
    /// Around a backdrop: never hit.
    Passive,
}
pub(crate) struct CutState {
    node: Cell<Option<NodeId>>,
    role: Role,
    router: Option<Rc<Router>>,
}
define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "ZguiNativeCutView"]
    #[ivars = CutState]
    pub(crate) struct CutView;
    impl CutView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, point: NSPoint) -> *mut NSView {
            let state = self.ivars();
            let through = match state.role {
                Role::Passive => false,
                Role::Inner => true,
                Role::Route => match (state.node.get(), &state.router) {
                    (Some(node), Some(router)) => {
                        router.route(point.x as f32, point.y as f32) == PointerRoute::Native(node)
                    }
                    _ => false,
                },
            };
            if !through {
                return std::ptr::null_mut();
            }
            let hit: *mut NSView = unsafe { msg_send![super(self), hitTest: point] };
            // The cut itself is no target: only what it holds.
            if std::ptr::eq(hit, self as *const Self as *const NSView) {
                std::ptr::null_mut()
            } else {
                hit
            }
        }
    }
);

pub(crate) struct BackdropState {
    radius: Cell<f64>,
}
define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "ZguiBackdropView"]
    #[ivars = BackdropState]
    pub(crate) struct BackdropView;
    impl BackdropView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
        #[unsafe(method_id(makeBackingLayer))]
        fn make_backing_layer(&self) -> Retained<CALayer> {
            // Samples the composited layer tree live (pages included).
            unsafe {
                let layer: Retained<CALayer> = msg_send![objc2::class!(CABackdropLayer), layer];
                let _: () = msg_send![&*layer, setAllowsGroupBlending: true];
                let _: () = msg_send![&*layer, setAllowsGroupOpacity: true];
                let _: () = msg_send![&*layer, setIgnoresOffscreenGroups: true];
                let _: () = msg_send![&*layer, setAllowsInPlaceFiltering: false];
                // Blurred at a quarter of the pixels: a blur keeps no detail.
                let _: () = msg_send![&*layer, setScale: 0.25f64];
                layer
            }
        }
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, _point: NSPoint) -> *mut NSView {
            std::ptr::null_mut()
        }
    }
);

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "ZguiOverlayPlaneView"]
    pub(crate) struct OverlayView;
    impl OverlayView {
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, _point: NSPoint) -> *mut NSView {
            std::ptr::null_mut()
        }
    }
);

fn cg(rect: Rect) -> CGRect {
    CGRect::new(
        CGPoint::new(rect.x as CGFloat, rect.y as CGFloat),
        CGSize::new(
            rect.width.max(0.) as CGFloat,
            rect.height.max(0.) as CGFloat,
        ),
    )
}
fn ns(rect: Rect) -> NSRect {
    NSRect::new(
        NSPoint::new(rect.x as f64, rect.y as f64),
        NSSize::new(rect.width.max(0.) as f64, rect.height.max(0.) as f64),
    )
}
fn rect_of(rect: CGRect) -> Rect {
    Rect::new(
        rect.origin.x as f32,
        rect.origin.y as f32,
        rect.size.width as f32,
        rect.size.height as f32,
    )
}
/// Whether `layer`'s own space (its sublayers', its mask's) runs top-down.
fn top_down(layer: &CALayer) -> bool {
    let mut flips = 0;
    let mut current = Some(layer.retain());
    while let Some(layer) = current {
        flips += usize::from(layer.isGeometryFlipped());
        current = layer.superlayer();
    }
    flips % 2 == 1
}
/// A top-down rectangle in a layer's space of `height` with that orientation.
fn in_space(rect: Rect, height: f32, top_down: bool) -> CGRect {
    if top_down {
        cg(rect)
    } else {
        cg(Rect::new(
            rect.x,
            height - rect.y - rect.height,
            rect.width,
            rect.height,
        ))
    }
}
/// A layer frame in its owner's space, back to top-down coordinates.
fn from_space(frame: CGRect, height: f32, top_down: bool) -> Rect {
    let rect = rect_of(frame);
    if top_down {
        rect
    } else {
        Rect::new(
            rect.x,
            height - rect.y - rect.height,
            rect.width,
            rect.height,
        )
    }
}
/// A box of `size` with corner radii (top-left, top-right, bottom-right,
/// bottom-left) smoothed by `smoothing` (circular arcs at 0, else cubics
/// shaped as zgui's renderer draws them: `zgui::decoration::corner_shape`),
/// at the origin of a space with that orientation.
fn rounded_path(
    size: (f32, f32),
    radii: [f32; 4],
    smoothing: f32,
    top_down: bool,
) -> CFRetained<CGMutablePath> {
    let path = CGMutablePath::new();
    let (width, height) = (size.0 as CGFloat, size.1 as CGFloat);
    // In a bottom-up space the top corners are at y = height.
    let (top, bottom) = if top_down { (0., height) } else { (height, 0.) };
    let null = std::ptr::null();
    if smoothing <= 0. {
        let [tl, tr, br, bl] = radii.map(|radius| radius.max(0.) as CGFloat);
        unsafe {
            CGMutablePath::move_to_point(Some(&path), null, tl, top);
            CGMutablePath::add_arc_to_point(Some(&path), null, width, top, width, bottom, tr);
            CGMutablePath::add_arc_to_point(Some(&path), null, width, bottom, 0., bottom, br);
            CGMutablePath::add_arc_to_point(Some(&path), null, 0., bottom, 0., top, bl);
            CGMutablePath::add_arc_to_point(Some(&path), null, 0., top, width, top, tl);
        }
        CGMutablePath::close_subpath(Some(&path));
        return path;
    }
    // Each corner's extent along its edges, and how far short of the corner
    // its handles stop.
    let limit = size.0.min(size.1).max(0.) / 2.;
    let [tl, tr, br, bl] = radii.map(|radius| {
        let (extent, n) = zgui::decoration::corner_shape(radius, smoothing, limit);
        let short = extent * (1. - zgui::decoration::corner_handle(n));
        (extent as CGFloat, short as CGFloat)
    });
    // Toward the bottom edge from the top one.
    let down = if top_down { 1. } else { -1. };
    let p = Some(&*path);
    unsafe {
        CGMutablePath::move_to_point(p, null, tl.0, top);
        CGMutablePath::add_line_to_point(p, null, width - tr.0, top);
        CGMutablePath::add_curve_to_point(
            p,
            null,
            width - tr.1,
            top,
            width,
            top + down * tr.1,
            width,
            top + down * tr.0,
        );
        CGMutablePath::add_line_to_point(p, null, width, bottom - down * br.0);
        CGMutablePath::add_curve_to_point(
            p,
            null,
            width,
            bottom - down * br.1,
            width - br.1,
            bottom,
            width - br.0,
            bottom,
        );
        CGMutablePath::add_line_to_point(p, null, bl.0, bottom);
        CGMutablePath::add_curve_to_point(
            p,
            null,
            bl.1,
            bottom,
            0.,
            bottom - down * bl.1,
            0.,
            bottom - down * bl.0,
        );
        CGMutablePath::add_line_to_point(p, null, 0., top + down * tl.0);
        CGMutablePath::add_curve_to_point(p, null, 0., top + down * tl.1, tl.1, top, tl.0, top);
    }
    CGMutablePath::close_subpath(Some(&path));
    path
}
fn contains(outer: Rect, inner: Rect) -> bool {
    outer.x <= inner.x + 0.01
        && outer.y <= inner.y + 0.01
        && outer.x + outer.width >= inner.x + inner.width - 0.01
        && outer.y + outer.height >= inner.y + inner.height - 0.01
}
fn black() -> CFRetained<CGColor> {
    CGColor::new_srgb(0., 0., 0., 1.)
}
/// Changes to `layer` never animate implicitly.
fn no_actions(layer: &CALayer) {
    unsafe {
        let null = objc2_foundation::NSNull::null();
        let keys = [
            "bounds",
            "position",
            "frame",
            "path",
            "cornerRadius",
            "hidden",
            "opacity",
            "backgroundColor",
            "mask",
            "filters",
            "contents",
            "sublayers",
        ];
        let keys: Vec<Retained<NSString>> =
            keys.iter().map(|key| NSString::from_str(key)).collect();
        let values: Vec<&AnyObject> = keys.iter().map(|_| &**null as &AnyObject).collect();
        let keys: Vec<&NSString> = keys.iter().map(|key| &**key).collect();
        let actions =
            objc2_foundation::NSDictionary::<NSString, AnyObject>::from_slices(&keys, &values);
        let _: () = msg_send![layer, setActions: &*actions];
    }
}

/// One shape a cut masks to, in window coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Shape {
    Rect(Rect),
    Round(RoundRect),
}
/// A cut view and its mask.
struct Cut {
    view: Retained<CutView>,
    mask: Option<Retained<CALayer>>,
    /// What it masks to, the orientation and height it was placed for.
    shape: Option<(Option<Shape>, bool, f32)>,
}
impl Cut {
    fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        node: Option<NodeId>,
        role: Role,
        router: Option<Rc<Router>>,
    ) -> Self {
        let view: Retained<CutView> = unsafe {
            let object = mtm.alloc::<CutView>().set_ivars(CutState {
                node: Cell::new(node),
                role,
                router,
            });
            msg_send![super(object), initWithFrame: frame]
        };
        view.setWantsLayer(true);
        view.setAutoresizesSubviews(false);
        if role != Role::Inner {
            view.setHidden(true);
        }
        if let Some(layer) = view.layer() {
            no_actions(&layer);
        }
        Self {
            view,
            mask: None,
            shape: None,
        }
    }
    /// Mask what it holds to `shape` (none: nothing cut).
    fn cut(&mut self, shape: Option<Shape>) {
        let Some(layer) = self.view.layer() else {
            return;
        };
        let down = top_down(&layer);
        let height = layer.bounds().size.height as f32;
        let key = (shape, down, height);
        if self.shape == Some(key) {
            return;
        }
        self.shape = Some(key);
        let Some(shape) = shape else {
            unsafe { layer.setMask(None) };
            self.mask = None;
            return;
        };
        let (frame, radii, smoothing) = match shape {
            Shape::Rect(rect) => (rect, [0.; 4], 0.),
            Shape::Round(round) => (round.rect, round.radii, round.smoothing),
        };
        // A layer's own corner radius draws circles alone (`cornerCurve`'s
        // continuous corners are Apple's, not the renderer's): uneven or
        // smoothed corners take a shape.
        let wants_shape =
            !radii.iter().all(|r| (r - radii[0]).abs() < 0.01) || (smoothing > 0. && radii[0] > 0.);
        let reusable = self
            .mask
            .as_ref()
            .is_some_and(|mask| mask.isKindOfClass(CAShapeLayer::class()) == wants_shape);
        if !reusable {
            let mask: Retained<CALayer> = if wants_shape {
                Retained::into_super(CAShapeLayer::new())
            } else {
                let mask = CALayer::new();
                mask.setBackgroundColor(Some(&black()));
                mask
            };
            no_actions(&mask);
            unsafe { layer.setMask(Some(&mask)) };
            self.mask = Some(mask);
        }
        let mask = self.mask.as_ref().expect("made above");
        mask.setFrame(in_space(frame, height, down));
        if wants_shape {
            let shape = mask
                .downcast_ref::<CAShapeLayer>()
                .expect("made as a shape");
            shape.setPath(Some(&rounded_path(
                (frame.width, frame.height),
                radii,
                smoothing,
                down,
            )));
        } else {
            mask.setCornerRadius(radii[0] as CGFloat);
        }
    }
}
/// The shapes cutting a box: `clip`, unless every rounded shape lies inside
/// it, then each rounded rectangle; `None` where a level cuts nothing.
fn shapes(clip: Rect, rounded: &[RoundRect], levels: usize) -> Vec<Option<Shape>> {
    let rounded: Vec<RoundRect> = rounded
        .iter()
        .copied()
        .filter(|round| round.radii.iter().any(|r| *r > 0.))
        .collect();
    let mut shapes = Vec::with_capacity(levels);
    if rounded.is_empty() || !rounded.iter().all(|round| contains(clip, round.rect)) {
        shapes.push(Some(Shape::Rect(clip)));
    }
    shapes.extend(rounded.iter().map(|round| Some(Shape::Round(*round))));
    // Deeper nesting than the levels: the outermost go (zgui's own paint
    // keeps two rounded clips too).
    shapes.truncate(levels);
    shapes.resize(levels, None);
    shapes
}

struct Clip {
    cuts: Vec<Cut>,
    /// The native view it holds, and the controller when a page or a terminal.
    content: Option<Retained<NSView>>,
    web: Option<zgui::web_view::WebView>,
    term: Option<zgui::terminal::Terminal>,
    placed: Option<NativePlacement>,
    background: Option<Retained<objc2_app_kit::NSColor>>,
}
impl Clip {
    fn outer(&self) -> &CutView {
        &self.cuts[0].view
    }
    fn inner(&self) -> &CutView {
        &self.cuts[self.cuts.len() - 1].view
    }
}
struct Backdrop {
    cuts: Vec<Cut>,
    view: Retained<BackdropView>,
    own: Option<Retained<CALayer>>,
    placed: Option<BackdropPlacement>,
}

pub(crate) struct Surfaces {
    mtm: MainThreadMarker,
    parent: Retained<NSView>,
    router: Rc<Router>,
    clips: Vec<(NodeId, Clip)>,
    backdrops: Vec<Backdrop>,
    overlay_view: Retained<OverlayView>,
    overlay_root: Retained<CALayer>,
    overlay_host: Retained<CALayer>,
    /// The overlay's box as last shown (None: hidden), with the orientation
    /// and height of the space it was placed in.
    overlay_shown: Option<(Option<Rect>, bool, f32)>,
    /// The page that held keyboard focus at the last check.
    focus: Option<NodeId>,
    shared: Rc<RefCell<Shared>>,
}
/// A native view's cuts and what they hold, as diagnostics read them.
type ClipViews = (NodeId, Vec<Retained<CutView>>, Option<Retained<NSView>>);
/// What diagnostics read (live AppKit views).
#[derive(Default)]
struct Shared {
    parent: Option<Retained<NSView>>,
    clips: Vec<ClipViews>,
    backdrops: Vec<(Vec<Retained<CutView>>, Retained<BackdropView>)>,
    overlay_view: Option<Retained<OverlayView>>,
    overlay_host: Option<Retained<CALayer>>,
}

impl Surfaces {
    pub fn new(window: &winit::window::Window, router: Router) -> Result<Self, String> {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let mtm = MainThreadMarker::new().ok_or("native views need the main thread")?;
        let handle = window.window_handle().map_err(|error| error.to_string())?;
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return Err("not an AppKit window".into());
        };
        // SAFETY: winit's content view, alive while its window is.
        let parent: Retained<NSView> =
            unsafe { Retained::retain(handle.ns_view.as_ptr().cast()) }.ok_or("no content view")?;
        let parent_key = Retained::as_ptr(&parent) as usize;
        let overlay_root = CALayer::new();
        no_actions(&overlay_root);
        let overlay_host = CALayer::new();
        no_actions(&overlay_host);
        overlay_host.setHidden(true);
        overlay_root.addSublayer(&overlay_host);
        let overlay_view: Retained<OverlayView> = unsafe {
            let view: Retained<OverlayView> =
                msg_send![mtm.alloc::<OverlayView>(), initWithFrame: parent.bounds()];
            // Layer-hosting: our layer, set before it wants one.
            view.setLayer(Some(&overlay_root));
            view.setWantsLayer(true);
            view.setAutoresizingMask(
                NSAutoresizingMaskOptions::ViewWidthSizable
                    | NSAutoresizingMaskOptions::ViewHeightSizable,
            );
            view
        };
        let transaction = Transaction::begin();
        parent.addSubview(&overlay_view);
        drop(transaction);
        let shared = Rc::new(RefCell::new(Shared {
            parent: Some(parent.clone()),
            overlay_view: Some(overlay_view.clone()),
            overlay_host: Some(overlay_host.clone()),
            ..Default::default()
        }));
        Ok(Self {
            mtm,
            parent,
            router: {
                let router = Rc::new(router);
                ROUTERS.with(|routers| {
                    let mut routers = routers.borrow_mut();
                    routers.retain(|(_, weak)| weak.strong_count() > 0);
                    routers.push((parent_key, Rc::downgrade(&router)));
                });
                router
            },
            clips: Vec::new(),
            backdrops: Vec::new(),
            overlay_view,
            overlay_root,
            overlay_host,
            overlay_shown: None,
            focus: None,
            shared,
        })
    }
    /// The layer the overlay renderer presents into.
    pub fn overlay_host(&self, scale: f32) -> Retained<CALayer> {
        self.overlay_host.setContentsScale(scale as CGFloat);
        self.overlay_root.setContentsScale(scale as CGFloat);
        self.overlay_host.clone()
    }
    /// The overlay surface's box for overlay paint covering `needed`: the
    /// current one while it holds `needed` without being much larger, else
    /// `needed` with a margin to grow and move into (an animating popover
    /// keeps its surfaces), within the window.
    pub fn plane_rect(&self, current: Option<Rect>, needed: Rect, viewport: Rect) -> Rect {
        let needed = needed
            .intersection(viewport)
            .unwrap_or(Rect::new(viewport.x, viewport.y, 1., 1.));
        if let Some(current) = current
            && contains(current, needed)
            && current.width * current.height <= 3. * (needed.width * needed.height).max(64. * 64.)
        {
            return current;
        }
        let margin = 24f32.max(needed.width.max(needed.height) * 0.1);
        needed
            .expand(margin)
            .intersection(viewport)
            .unwrap_or(needed)
    }
    /// Whether `split` moves anything native relative to `previous`.
    pub fn differs(&self, split: &PlaneSplit, previous: &PlaneSplit) -> bool {
        split.natives() != previous.natives() || split.backdrops() != previous.backdrops()
    }
    /// Show the overlay layer at `rect`, or hide it.
    pub fn show_overlay(&mut self, rect: Option<Rect>) {
        let down = top_down(&self.overlay_root);
        let height = self.overlay_root.bounds().size.height as f32;
        let key = (rect, down, height);
        if self.overlay_shown == Some(key) {
            return;
        }
        self.overlay_shown = Some(key);
        match rect {
            None => self.overlay_host.setHidden(true),
            Some(rect) => {
                self.overlay_host.setFrame(in_space(rect, height, down));
                // The presenting layers fill it exactly. Autoresizing alone
                // drifts: it applies each change's delta to their old frames,
                // and a host resized more than once before they are laid out
                // leaves them taller than it, the plane's paint stretched and
                // shifted.
                let bounds = self.overlay_host.bounds();
                if let Some(layers) = unsafe { self.overlay_host.sublayers() } {
                    for layer in layers.iter() {
                        layer.setFrame(bounds);
                    }
                }
                self.overlay_host.setHidden(false);
            }
        }
    }
    /// Move native views and backdrops to `split`'s placements.
    pub fn apply(&mut self, natives: &NativeViews, split: &PlaneSplit) {
        let bounds = self.parent.bounds();
        // The overlay stays the window's size.
        if self.overlay_view.frame() != bounds {
            self.overlay_view.setFrame(bounds);
        }
        // Unmounted: their cuts go (pages stay with their controllers). A
        // view with keyboard focus that is mounted again elsewhere (its node
        // rebuilt) gets it back once placed.
        let views = natives.views();
        let mut order_changed = false;
        let mut refocus: Option<Retained<NSView>> = None;
        self.clips.retain(|(node, clip)| {
            let kept = views.iter().any(|(other, _)| other == node);
            if !kept {
                if let Some(content) = &clip.content {
                    if has_focus(content) {
                        refocus = Some(content.clone());
                    }
                    content.removeFromSuperview();
                }
                clip.outer().removeFromSuperview();
                order_changed = true;
            }
            kept
        });
        for (node, content) in &views {
            if !self.clips.iter().any(|(other, _)| other == node) {
                let clip = self.clip(*node, &bounds);
                self.clips.push((*node, clip));
                order_changed = true;
            }
            let clip = &mut self
                .clips
                .iter_mut()
                .find(|(other, _)| other == node)
                .unwrap()
                .1;
            // Mount what it shows (a page or a terminal is made on first mount).
            let mut term = None;
            let (view, web): (Option<Retained<NSView>>, Option<zgui::web_view::WebView>) =
                match content {
                    NativeContent::Web(web) => {
                        let backend = webkit::backend(web, self.mtm);
                        let page = backend
                            .as_any()
                            .downcast_ref::<webkit::MacWebView>()
                            .map(|page| page.view().retain());
                        (page, Some(web.clone()))
                    }
                    NativeContent::Terminal(terminal) => {
                        term = Some(terminal.clone());
                        (Self::terminal_view(terminal, self.mtm), None)
                    }
                    NativeContent::Platform(view) => {
                        (view.downcast_ref::<Retained<NSView>>().cloned(), None)
                    }
                };
            let inner: Retained<NSView> =
                Retained::into_super(clip.cuts.last().unwrap().view.clone());
            let held = |view: &NSView| {
                unsafe { view.superview() }.is_some_and(|parent| std::ptr::eq(&*parent, &*inner))
            };
            let same = match (&clip.content, &view) {
                (Some(a), Some(b)) => std::ptr::eq(&**a, &**b),
                (None, None) => true,
                _ => false,
            };
            if !same || view.as_deref().is_some_and(|view| !held(view)) {
                if let Some(old) = &clip.content
                    && held(old)
                {
                    old.removeFromSuperview();
                }
                if let Some(view) = &view {
                    // Moving a view out of its window drops first responder
                    // without asking it to resign: keep it where it was.
                    if has_focus(view) {
                        refocus = Some(view.clone());
                    }
                    view.removeFromSuperview();
                    inner.addSubview(view);
                }
                clip.content = view;
                clip.placed = None;
            }
            clip.web = web;
            clip.term = term;
            Self::place(clip, &bounds, split.native(*node).cloned());
        }
        // Pages in paint order, under backdrops and the overlay.
        let mut ordered: Vec<(usize, usize, NodeId)> = self
            .clips
            .iter()
            .enumerate()
            .map(|(index, (node, clip))| {
                (
                    clip.placed.as_ref().map_or(usize::MAX, |p| p.order),
                    index,
                    *node,
                )
            })
            .collect();
        ordered.sort_by_key(|(order, index, _)| (*order, *index));
        let wanted: Vec<NodeId> = ordered.iter().map(|(_, _, node)| *node).collect();
        let current: Vec<NodeId> = self.clips.iter().map(|(node, _)| *node).collect();
        if order_changed || wanted != current {
            self.clips
                .sort_by_key(|(node, _)| wanted.iter().position(|other| other == node));
            for (_, clip) in &self.clips {
                let above: Retained<NSView> = match self.backdrops.first() {
                    Some(backdrop) => Retained::into_super(backdrop.cuts[0].view.clone()),
                    None => Retained::into_super(self.overlay_view.clone()),
                };
                self.parent.addSubview_positioned_relativeTo(
                    clip.outer(),
                    NSWindowOrderingMode::Below,
                    Some(&above),
                );
            }
        }
        if let Some(view) = refocus
            && let Some(window) = view.window()
        {
            window.makeFirstResponder(Some(&view));
        }
        self.apply_backdrops(split.backdrops(), &bounds);
        let mut shared = self.shared.borrow_mut();
        shared.clips = self
            .clips
            .iter()
            .map(|(node, clip)| {
                (
                    *node,
                    clip.cuts.iter().map(|cut| cut.view.clone()).collect(),
                    clip.content.clone(),
                )
            })
            .collect();
        shared.backdrops = self
            .backdrops
            .iter()
            .map(|b| {
                (
                    b.cuts.iter().map(|cut| cut.view.clone()).collect(),
                    b.view.clone(),
                )
            })
            .collect();
    }
    /// Nested cuts: each next inside the one before, all the window's size.
    fn cuts(
        &self,
        levels: usize,
        frame: &NSRect,
        node: Option<NodeId>,
        outer: Role,
        inner: Role,
    ) -> Vec<Cut> {
        let mut cuts: Vec<Cut> = Vec::with_capacity(levels);
        for level in 0..levels {
            let (role, router) = match level {
                0 => (outer, (outer == Role::Route).then(|| self.router.clone())),
                _ => (inner, None),
            };
            let cut = Cut::new(self.mtm, *frame, node, role, router);
            if let Some(parent) = cuts.last() {
                parent.view.addSubview(&cut.view);
            }
            cuts.push(cut);
        }
        cuts
    }
    /// A terminal's view: its Ghostty surface (made on first mount), or
    /// none without the `ghostty` feature or once it closed.
    #[allow(unused_variables)]
    fn terminal_view(
        term: &zgui::terminal::Terminal,
        mtm: MainThreadMarker,
    ) -> Option<Retained<NSView>> {
        #[cfg(feature = "ghostty")]
        {
            let backend = super::ghostty::backend(term, mtm)?;
            backend
                .as_any()
                .downcast_ref::<super::ghostty::MacTerminal>()
                .map(|terminal| terminal.view().retain())
        }
        #[cfg(not(feature = "ghostty"))]
        None
    }
    fn clip(&self, node: NodeId, bounds: &NSRect) -> Clip {
        Clip {
            cuts: self.cuts(NATIVE_CUTS, bounds, Some(node), Role::Route, Role::Inner),
            content: None,
            web: None,
            term: None,
            placed: None,
            background: None,
        }
    }
    fn place(clip: &mut Clip, bounds: &NSRect, placement: Option<NativePlacement>) {
        let visible = placement
            .as_ref()
            .and_then(|p| p.visible())
            .filter(|_| clip.content.is_some());
        let (Some(placement), Some(visible)) = (placement, visible) else {
            if !clip.outer().isHidden() {
                // Hidden: keyboard focus leaves it.
                if let Some(web) = &clip.web
                    && web.focused()
                {
                    web.blur();
                }
                if let Some(term) = &clip.term
                    && term.focused()
                {
                    term.blur();
                }
                clip.outer().setHidden(true);
            }
            clip.placed = None;
            return;
        };
        for cut in &clip.cuts {
            if cut.view.frame() != *bounds {
                cut.view.setFrame(*bounds);
            }
        }
        // WebKit fills tiles a size change exposes a frame later: under them,
        // the page's own background rather than the window's.
        if let Some(web) = &clip.web
            && let Some(backend) = web.backend()
            && let Some(page) = backend.as_any().downcast_ref::<webkit::MacWebView>()
            && !page.transparent()
            && let Some(layer) = clip.inner().layer()
        {
            let color = page.under_page_color();
            if clip
                .background
                .as_ref()
                .is_none_or(|old| !old.isEqual(Some(&color)))
            {
                layer.setBackgroundColor(Some(&color.CGColor()));
                clip.background = Some(color);
            }
        }
        if clip.placed.as_ref() != Some(&placement) {
            if let Some(content) = &clip.content {
                // Origin and size together, in the cut's (flipped) space.
                content.setFrame(ns(placement.frame));
            }
            clip.outer().setAlphaValue(placement.opacity as f64);
        }
        // Every frame: cuts recheck the orientation AppKit gave their layers.
        let shapes = shapes(visible, &placement.rounded, clip.cuts.len());
        for (cut, shape) in clip.cuts.iter_mut().zip(shapes) {
            cut.cut(shape);
        }
        clip.placed = Some(placement);
        if clip.outer().isHidden() {
            clip.outer().setHidden(false);
        }
    }
    fn apply_backdrops(&mut self, placements: &[BackdropPlacement], bounds: &NSRect) {
        while self.backdrops.len() > placements.len() {
            let backdrop = self.backdrops.pop().unwrap();
            backdrop.cuts[0].view.removeFromSuperview();
        }
        while self.backdrops.len() < placements.len() {
            let cuts = self.cuts(BACKDROP_CUTS, bounds, None, Role::Passive, Role::Passive);
            let view: Retained<BackdropView> = unsafe {
                let object = self.mtm.alloc::<BackdropView>().set_ivars(BackdropState {
                    radius: Cell::new(-1.),
                });
                msg_send![super(object), initWithFrame: NSRect::ZERO]
            };
            view.setWantsLayer(true);
            if let Some(layer) = view.layer() {
                no_actions(&layer);
            }
            cuts.last().unwrap().view.addSubview(&view);
            for cut in &cuts[1..] {
                cut.view.setHidden(false);
            }
            self.parent.addSubview_positioned_relativeTo(
                &cuts[0].view,
                NSWindowOrderingMode::Below,
                Some(&self.overlay_view),
            );
            self.backdrops.push(Backdrop {
                cuts,
                view,
                own: None,
                placed: None,
            });
        }
        for (backdrop, placement) in self.backdrops.iter_mut().zip(placements) {
            let Some(visible) = placement.visible() else {
                backdrop.cuts[0].view.setHidden(true);
                backdrop.placed = Some(placement.clone());
                continue;
            };
            for cut in &backdrop.cuts {
                if cut.view.frame() != *bounds {
                    cut.view.setFrame(*bounds);
                }
            }
            for (cut, shape) in
                backdrop
                    .cuts
                    .iter_mut()
                    .zip(shapes(visible, &placement.rounded, BACKDROP_CUTS))
            {
                cut.cut(shape);
            }
            if backdrop.cuts[0].view.isHidden() {
                backdrop.cuts[0].view.setHidden(false);
            }
            if backdrop.placed.as_ref() == Some(placement) {
                continue;
            }
            backdrop.view.setFrame(ns(placement.rect));
            backdrop.cuts[0]
                .view
                .setAlphaValue(placement.opacity as f64);
            let Some(layer) = backdrop.view.layer() else {
                continue;
            };
            let radius = placement.sigma as f64;
            if backdrop.view.ivars().radius.replace(radius) != radius {
                // Filters are copied into the render tree: a changed value
                // takes a new filter.
                unsafe {
                    let filter: Retained<NSObject> = msg_send![
                        objc2::class!(CAFilter),
                        filterWithType: &*NSString::from_str("gaussianBlur")
                    ];
                    let value = objc2_foundation::NSNumber::new_f64(radius);
                    let yes = objc2_foundation::NSNumber::new_bool(true);
                    let _: () = msg_send![&*filter, setValue: &*value, forKey: &*NSString::from_str("inputRadius")];
                    let _: () = msg_send![&*filter, setValue: &*yes, forKey: &*NSString::from_str("inputNormalizeEdges")];
                    let filters = objc2_foundation::NSArray::from_retained_slice(&[filter]);
                    let _: () = msg_send![&*layer, setFilters: &*filters];
                }
            }
            // Its own corners: its layer's radius, or a shape when uneven
            // or smoothed.
            let (radii, smoothing) = (placement.radii, placement.smoothing);
            if radii.iter().all(|r| (r - radii[0]).abs() < 0.01)
                && (smoothing <= 0. || radii[0] <= 0.)
            {
                unsafe { layer.setMask(None) };
                backdrop.own = None;
                layer.setCornerRadius(radii[0] as CGFloat);
                layer.setMasksToBounds(true);
            } else {
                layer.setCornerRadius(0.);
                let mask = backdrop.own.get_or_insert_with(|| {
                    let mask = Retained::into_super(CAShapeLayer::new());
                    no_actions(&mask);
                    mask
                });
                let down = top_down(&layer);
                mask.setFrame(CGRect::new(CGPoint::new(0., 0.), layer.bounds().size));
                let shape = mask.downcast_ref::<CAShapeLayer>().expect("a shape");
                shape.setPath(Some(&rounded_path(
                    (placement.rect.width, placement.rect.height),
                    radii,
                    smoothing,
                    down,
                )));
                unsafe { layer.setMask(Some(mask)) };
            }
            backdrop.placed = Some(placement.clone());
        }
    }
    /// Agree zgui's focus with AppKit's first responder (see `NativeHost::sync_focus`).
    pub fn sync_focus(&mut self, ui: &Ui, natives: &NativeViews) {
        let responder = self
            .parent
            .window()
            .and_then(|window| window.firstResponder())
            .and_then(|responder| responder.downcast::<NSView>().ok());
        let focused = responder.and_then(|responder| {
            self.clips.iter().find_map(|(node, clip)| {
                clip.content
                    .as_ref()
                    .filter(|content| responder.isDescendantOf(content))
                    .map(|_| *node)
            })
        });
        if focused == self.focus {
            return;
        }
        let previous = std::mem::replace(&mut self.focus, focused);
        for (node, clip) in &self.clips {
            let has = focused == Some(*node);
            if let Some(web) = &clip.web
                && web.focused() != has
            {
                web.update_state(|state| state.focused = has);
            }
            if let Some(term) = &clip.term
                && term.focused() != has
            {
                term.update_state(|state| state.focused = has);
            }
        }
        match focused {
            // A click in the page, or the page focusing itself.
            Some(node) if ui.input.focused() != Some(node) && natives.contains(node) => {
                ui.input.focus(&ui.scene, Some(node));
            }
            // AppKit moved focus away (a click on zgui content).
            None if previous.is_some() && ui.input.focused() == previous => {
                ui.input.focus(&ui.scene, None);
            }
            _ => {}
        }
    }
    /// Let `diagnostics` read these surfaces.
    pub fn report_to(&self, diagnostics: &NativeDiagnostics) {
        *diagnostics.0.borrow_mut() = Rc::downgrade(&self.shared);
    }
}
impl Drop for Surfaces {
    fn drop(&mut self) {
        let transaction = Transaction::begin();
        for (_, clip) in &self.clips {
            if let Some(content) = &clip.content {
                content.removeFromSuperview();
            }
            clip.outer().removeFromSuperview();
        }
        for backdrop in &self.backdrops {
            backdrop.cuts[0].view.removeFromSuperview();
        }
        self.overlay_view.removeFromSuperview();
        drop(transaction);
    }
}

/// Live AppKit state of a window's native views, read back for fixtures and
/// diagnostics: what is actually where, in window coordinates (y down).
/// Clones share it; it reads nothing until the window first shows a native view.
#[derive(Clone, Default)]
pub struct NativeDiagnostics(Rc<RefCell<Weak<RefCell<Shared>>>>);
/// A native view as AppKit has it.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeViewReport {
    pub node: NodeId,
    pub hidden: bool,
    pub alpha: f64,
    /// The native view's frame in the window.
    pub frame: Option<Rect>,
    /// Each cut's mask in the window: its box and corner radius (a shape
    /// mask reports radius -1).
    pub masks: Vec<(Rect, f64)>,
    /// Its outermost cut's subview index in the content view (paint order).
    pub index: usize,
}
#[derive(Clone, Debug, PartialEq)]
pub struct BackdropReport {
    pub hidden: bool,
    pub frame: Rect,
    pub alpha: f64,
    /// Its layer's corner radius (-1: a shape mask).
    pub corner_radius: f64,
    /// Masks of the cuts around it (its ancestors' clips), in the window.
    pub masks: Vec<(Rect, f64)>,
    pub index: usize,
    pub class: String,
    /// The blur's radius, as its filter holds it.
    pub blur: Option<f64>,
}
fn masks_of(cuts: &[Retained<CutView>]) -> Vec<(Rect, f64)> {
    cuts.iter()
        .filter_map(|cut| {
            let layer = cut.layer()?;
            let mask = layer.mask()?;
            let rect = from_space(
                mask.frame(),
                layer.bounds().size.height as f32,
                top_down(&layer),
            );
            let radius = if mask.isKindOfClass(CAShapeLayer::class()) {
                -1.
            } else {
                mask.cornerRadius()
            };
            Some((rect, radius))
        })
        .collect()
}
fn index_in(parent: &NSView, view: &NSView) -> usize {
    parent
        .subviews()
        .iter()
        .position(|other| std::ptr::eq(&*other, view))
        .unwrap_or(usize::MAX)
}
impl NativeDiagnostics {
    fn with<R>(&self, f: impl FnOnce(&Shared) -> R) -> Option<R> {
        let shared = self.0.borrow().upgrade()?;
        let shared = shared.borrow();
        Some(f(&shared))
    }
    pub fn natives(&self) -> Vec<NativeViewReport> {
        self.with(|shared| {
            let parent = shared.parent.as_ref().unwrap();
            shared
                .clips
                .iter()
                .map(|(node, cuts, content)| NativeViewReport {
                    node: *node,
                    hidden: cuts[0].isHidden(),
                    alpha: cuts[0].alphaValue(),
                    frame: content.as_ref().map(|content| {
                        rect_of(content.convertRect_toView(content.bounds(), Some(parent)))
                    }),
                    masks: masks_of(cuts),
                    index: index_in(parent, &cuts[0]),
                })
                .collect()
        })
        .unwrap_or_default()
    }
    pub fn backdrops(&self) -> Vec<BackdropReport> {
        self.with(|shared| {
            let parent = shared.parent.as_ref().unwrap();
            shared
                .backdrops
                .iter()
                .map(|(cuts, view)| {
                    let layer = view.layer();
                    let blur = layer.as_ref().and_then(|layer| unsafe {
                        let filters: *mut AnyObject = msg_send![&**layer, filters];
                        let filter: *mut AnyObject = msg_send![filters.as_ref()?, firstObject];
                        let value: *mut AnyObject = msg_send![
                            filter.as_ref()?,
                            valueForKey: &*NSString::from_str("inputRadius")
                        ];
                        let value: f64 = msg_send![value.as_ref()?, doubleValue];
                        Some(value)
                    });
                    BackdropReport {
                        hidden: cuts[0].isHidden(),
                        frame: rect_of(view.convertRect_toView(view.bounds(), Some(parent))),
                        alpha: cuts[0].alphaValue(),
                        corner_radius: layer.as_ref().map_or(0., |layer| {
                            if layer.mask().is_some() {
                                -1.
                            } else {
                                layer.cornerRadius()
                            }
                        }),
                        masks: masks_of(cuts),
                        index: index_in(parent, &cuts[0]),
                        class: layer
                            .map(|layer| layer.class().name().to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        blur,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
    }
    /// The overlay plane's layer frame in the window (y down), when shown,
    /// and the overlay view's subview index.
    pub fn overlay(&self) -> Option<(Rect, usize)> {
        self.with(|shared| {
            let parent = shared.parent.as_ref().unwrap();
            let host = shared.overlay_host.as_ref()?;
            if host.isHidden() {
                return None;
            }
            let root = host.superlayer()?;
            let rect = from_space(
                host.frame(),
                root.bounds().size.height as f32,
                top_down(&root),
            );
            let overlay = shared.overlay_view.as_ref()?;
            Some((rect, index_in(parent, overlay)))
        })
        .flatten()
    }
    /// Which native view AppKit's hit testing at window point `(x, y)`
    /// lands in, as a click there would.
    pub fn native_hit(&self, x: f32, y: f32) -> Option<NodeId> {
        self.with(|shared| {
            let parent = shared.parent.as_ref().unwrap();
            // hitTest: takes the point in the receiver's superview's space.
            let point = NSPoint::new(x as f64, y as f64);
            let point = match unsafe { parent.superview() } {
                Some(superview) => parent.convertPoint_toView(point, Some(&superview)),
                None => point,
            };
            let hit = parent.hitTest(point)?;
            shared.clips.iter().find_map(|(node, _, content)| {
                content
                    .as_ref()
                    .filter(|content| hit.isDescendantOf(content))
                    .map(|_| *node)
            })
        })
        .flatten()
    }
    /// The window's number (`screencapture -l` takes it).
    pub fn window_number(&self) -> Option<isize> {
        self.with(|shared| {
            shared
                .parent
                .as_ref()?
                .window()
                .map(|window| window.windowNumber())
        })
        .flatten()
    }
    /// The content view's layer tree, described (for debugging).
    pub fn layer_tree(&self) -> String {
        fn describe(layer: &CALayer, depth: usize, out: &mut String) {
            if depth > 9 {
                return;
            }
            let frame = layer.frame();
            let mask = layer
                .mask()
                .map(|mask| {
                    let f = mask.frame();
                    format!(
                        " mask=({:.1},{:.1} {:.1}x{:.1} r{:.1})",
                        f.origin.x,
                        f.origin.y,
                        f.size.width,
                        f.size.height,
                        mask.cornerRadius()
                    )
                })
                .unwrap_or_default();
            out.push_str(&format!(
                "{}{} frame=({:.1},{:.1} {:.1}x{:.1}) hidden={} flipped={}{mask}\n",
                "  ".repeat(depth),
                layer.class().name().to_string_lossy(),
                frame.origin.x,
                frame.origin.y,
                frame.size.width,
                frame.size.height,
                layer.isHidden(),
                layer.isGeometryFlipped(),
            ));
            if let Some(sublayers) = unsafe { layer.sublayers() } {
                for sublayer in sublayers.iter().take(24) {
                    describe(&sublayer, depth + 1, out);
                }
            }
        }
        self.with(|shared| {
            let mut out = String::new();
            if let Some(layer) = shared.parent.as_ref().and_then(|parent| parent.layer()) {
                describe(&layer, 0, &mut out);
            }
            out
        })
        .unwrap_or_default()
    }
}
