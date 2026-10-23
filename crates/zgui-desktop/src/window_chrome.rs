//! Window chrome from `WindowOptions`: the title bar's style and a frosted
//! glass behind a translucent window. Applied when the window is made, on
//! macOS; other platforms keep their own title bars and draw no glass.

/// How a window's title bar looks (macOS; elsewhere the platform's own).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Titlebar {
    /// The platform's title bar.
    #[default]
    Standard,
    /// Transparent, without the title: content runs under it, the window
    /// buttons (the traffic lights) inset over it in a 40-point strip, as
    /// Electron's `hiddenInset`. Mark the strip `View::window_drag` so it
    /// moves the window.
    Inset,
    /// None: content fills the window and the window buttons are hidden.
    /// The window keeps its rounded corners and shadow.
    Hidden,
}

/// A blurred desktop behind a translucent window (macOS's behind-window
/// `NSVisualEffectView`, cut to the window's rounded corners). The window is
/// transparent where zgui paints nothing (or translucent colour).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glass {
    pub material: GlassMaterial,
    /// Only the blur: no tint from the desktop, no saturation boost, a
    /// 60-point gaussian, as gpui's `BlurredView` (for apps laying their own
    /// tint over it).
    pub plain: bool,
}
impl Glass {
    pub fn new(material: GlassMaterial) -> Self {
        Self {
            material,
            plain: false,
        }
    }
    /// The same glass, untinted and unsaturated (see `Glass::plain`).
    pub fn plain(mut self) -> Self {
        self.plain = true;
        self
    }
}
impl Default for Glass {
    fn default() -> Self {
        Self::new(GlassMaterial::Hud)
    }
}
/// The glass's material (`NSVisualEffectMaterial`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GlassMaterial {
    /// A dark, saturated heads-up-display glass.
    #[default]
    Hud,
    /// The colourless material under a window's own background.
    UnderWindowBackground,
    UnderPageBackground,
    WindowBackground,
    ContentBackground,
    Titlebar,
    HeaderView,
    Sidebar,
    Menu,
    Popover,
    Sheet,
    ToolTip,
    FullScreenUi,
    Selection,
}
impl GlassMaterial {
    /// Its `NSVisualEffectMaterial` value.
    pub fn appkit_value(self) -> isize {
        match self {
            Self::Titlebar => 3,
            Self::Selection => 4,
            Self::Menu => 5,
            Self::Popover => 6,
            Self::Sidebar => 7,
            Self::HeaderView => 10,
            Self::Sheet => 11,
            Self::WindowBackground => 12,
            Self::Hud => 13,
            Self::FullScreenUi => 15,
            Self::ToolTip => 17,
            Self::ContentBackground => 18,
            Self::UnderWindowBackground => 21,
            Self::UnderPageBackground => 22,
        }
    }
}

/// Style `window` as `titlebar` and `glass` ask: its corner radius in points
/// (content inset from the window's edge keeps its corners concentric with
/// it), 0 where the platform does not round windows or zgui cannot tell.
pub(crate) fn apply(
    window: &winit::window::Window,
    titlebar: Titlebar,
    glass: Option<Glass>,
) -> f64 {
    #[cfg(target_os = "macos")]
    {
        mac::apply(window, titlebar, glass)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, titlebar, glass);
        0.
    }
}

/// A double click in a window drag region: what a double click on the title
/// bar does. macOS follows the system setting (zoom, minimize or nothing);
/// elsewhere the window toggles maximized.
pub(crate) fn double_click(window: &winit::window::Window) {
    #[cfg(target_os = "macos")]
    mac::double_click(window);
    #[cfg(not(target_os = "macos"))]
    window.set_maximized(!window.is_maximized());
}

#[cfg(target_os = "macos")]
mod mac {
    use super::{Glass, Titlebar};
    use objc2::{
        DefinedClass, MainThreadOnly, define_class, msg_send,
        rc::Retained,
        runtime::{AnyClass, AnyObject},
        sel,
    };
    use objc2_app_kit::{
        NSAutoresizingMaskOptions, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
        NSVisualEffectState, NSVisualEffectView, NSWindow, NSWindowStyleMask,
        NSWindowTitleVisibility,
    };
    use objc2_foundation::NSString;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    pub(super) fn ns_window(window: &winit::window::Window) -> Option<Retained<NSWindow>> {
        let handle = window.window_handle().ok()?;
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return None;
        };
        // SAFETY: winit's view, alive while its window is; the main thread.
        let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        view.window()
    }

    pub(super) fn apply(
        window: &winit::window::Window,
        titlebar: Titlebar,
        glass: Option<Glass>,
    ) -> f64 {
        let Some(window) = ns_window(window) else {
            return 0.;
        };
        match titlebar {
            Titlebar::Standard => {}
            Titlebar::Inset => {
                content_under_titlebar(&window);
                inset_buttons(&window);
            }
            Titlebar::Hidden => {
                content_under_titlebar(&window);
                // NSWindowCloseButton, MiniaturizeButton, ZoomButton.
                for button in [0_isize, 1, 2] {
                    unsafe {
                        let button: *mut AnyObject =
                            msg_send![&*window, standardWindowButton: button];
                        if let Some(button) = button.as_ref() {
                            let _: () = msg_send![button, setHidden: true];
                        }
                    }
                }
            }
        }
        if let Some(glass) = glass {
            frost(&window, glass);
        }
        radius(&window)
    }

    /// Content under a transparent, untitled title bar with no separator.
    fn content_under_titlebar(window: &NSWindow) {
        window.setStyleMask(window.styleMask() | NSWindowStyleMask::FullSizeContentView);
        window.setTitlebarAppearsTransparent(true);
        window.setTitleVisibility(NSWindowTitleVisibility::Hidden);
        // NSTitlebarSeparatorStyleNone
        unsafe {
            let _: () = msg_send![window, setTitlebarSeparatorStyle: 1_isize];
        }
    }

    /// An empty compact unified toolbar: it centres the traffic lights in a
    /// 40-point strip.
    fn inset_buttons(window: &NSWindow) {
        let Some(class) = AnyClass::get(c"NSToolbar") else {
            return;
        };
        unsafe {
            let id = NSString::from_str("zgui.titlebar.inset");
            let toolbar: Retained<AnyObject> =
                msg_send![msg_send![class, alloc], initWithIdentifier: &*id];
            let _: () = msg_send![&*toolbar, setShowsBaselineSeparator: false];
            let _: () = msg_send![window, setToolbar: &*toolbar];
            // NSWindowToolbarStyleUnifiedCompact
            let _: () = msg_send![window, setToolbarStyle: 4_isize];
        }
        toolbar_passes_through();
    }

    /// The empty toolbar is only there to centre the traffic lights: presses
    /// in it go to the content under it. Otherwise it keeps a press's drag
    /// and release events (zgui would start a drag and never see it end).
    /// The traffic lights are in the titlebar view, not the toolbar's, and
    /// stay clickable.
    fn toolbar_passes_through() {
        use objc2::runtime::Sel;
        use objc2_foundation::NSPoint;
        extern "C-unwind" fn nothing(
            _this: *mut AnyObject,
            _cmd: Sel,
            _point: NSPoint,
        ) -> *mut AnyObject {
            std::ptr::null_mut()
        }
        let Some(class) = AnyClass::get(c"NSToolbarView") else {
            return;
        };
        unsafe {
            let imp: objc2::runtime::Imp = std::mem::transmute(
                nothing as extern "C-unwind" fn(*mut AnyObject, Sel, NSPoint) -> *mut AnyObject,
            );
            objc2::ffi::class_replaceMethod(
                class as *const AnyClass as *mut AnyClass,
                sel!(hitTest:),
                imp,
                c"@@:{CGPoint=dd}".as_ptr(),
            );
        }
    }

    /// The window's corner radius: it varies by macOS version and toolbar
    /// style.
    pub(super) fn radius(window: &NSWindow) -> f64 {
        if window.class().responds_to(sel!(_cornerRadius)) {
            unsafe { msg_send![window, _cornerRadius] }
        } else {
            10.
        }
    }

    pub(crate) struct GlassState {
        plain: bool,
    }
    define_class!(
        /// A behind-window effect view that, plain, strips the layers AppKit
        /// builds for it each time it builds them (gpui's `BlurredView`).
        #[unsafe(super(NSVisualEffectView, NSView))]
        #[thread_kind = MainThreadOnly]
        #[name = "ZguiWindowGlass"]
        #[ivars = GlassState]
        pub(crate) struct GlassView;
        impl GlassView {
            #[unsafe(method(updateLayer))]
            fn update_layer(&self) {
                let _: () = unsafe { msg_send![super(self), updateLayer] };
                if self.ivars().plain
                    && let Some(layer) = self.layer()
                {
                    unsafe { strip_layer(&layer) };
                }
            }
        }
    );

    /// Put a behind-window effect view under the (transparent) zgui view.
    /// Unlike the private CGS window blur, it can be clipped to the window's
    /// rounded shape, so neither blur nor shadow is left square.
    fn frost(window: &NSWindow, glass: Glass) {
        let Some(content) = window.contentView() else {
            return;
        };
        let Some(frame) = (unsafe { content.superview() }) else {
            return;
        };
        let mtm = window.mtm();
        let effect: Retained<GlassView> = unsafe {
            let object = mtm
                .alloc::<GlassView>()
                .set_ivars(GlassState { plain: glass.plain });
            msg_send![super(object), initWithFrame: frame.bounds()]
        };
        effect.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        // Behind-window blending, always active.
        effect.setMaterial(NSVisualEffectMaterial(glass.material.appkit_value()));
        effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        effect.setState(NSVisualEffectState::Active);
        effect.setWantsLayer(true);
        if let Some(layer) = effect.layer() {
            layer.setCornerRadius(radius(window));
            unsafe {
                let continuous = NSString::from_str("continuous");
                let _: () = msg_send![&*layer, setCornerCurve: &*continuous];
            }
            layer.setMasksToBounds(true);
            if glass.plain {
                unsafe { strip_layer(&layer) };
            }
        }
        frame.addSubview_positioned_relativeTo(
            &effect,
            objc2_app_kit::NSWindowOrderingMode::Below,
            Some(&content),
        );
        window.invalidateShadow();
    }

    /// No desktop tint (`CAChameleonLayer` hidden), a 60-point gaussian and
    /// no saturation boost, through the effect's layer tree.
    unsafe fn strip_layer(layer: &AnyObject) {
        unsafe {
            let none: *const AnyObject = std::ptr::null();
            let _: () = msg_send![layer, setBackgroundColor: none];
            let class_name: Retained<NSString> = msg_send![layer, className];
            if class_name.to_string() == "CAChameleonLayer" {
                let _: () = msg_send![layer, setHidden: true];
                return;
            }
            let filters: *mut AnyObject = msg_send![layer, filters];
            if let Some(filters) = filters.as_ref() {
                let count: usize = msg_send![filters, count];
                let mut kept: Vec<*mut AnyObject> = Vec::with_capacity(count);
                for i in 0..count {
                    let filter: *mut AnyObject = msg_send![filters, objectAtIndex: i];
                    let Some(filter_ref) = filter.as_ref() else {
                        continue;
                    };
                    let description: Retained<NSString> = msg_send![filter_ref, description];
                    let description = description.to_string();
                    if description.contains("Saturat") {
                        continue;
                    }
                    if description.contains("Blur") {
                        let radius = objc2_foundation::NSNumber::new_f64(60.);
                        let key = NSString::from_str("inputRadius");
                        let _: () = msg_send![filter_ref, setValue: &*radius, forKey: &*key];
                    }
                    kept.push(filter);
                }
                if let Some(array_class) = AnyClass::get(c"NSArray") {
                    let array: Retained<AnyObject> = msg_send![
                        array_class,
                        arrayWithObjects: kept.as_ptr(),
                        count: kept.len()
                    ];
                    let _: () = msg_send![layer, setFilters: &*array];
                }
            }
            let sublayers: *mut AnyObject = msg_send![layer, sublayers];
            if let Some(sublayers) = sublayers.as_ref() {
                let count: usize = msg_send![sublayers, count];
                for i in 0..count {
                    let sublayer: *mut AnyObject = msg_send![sublayers, objectAtIndex: i];
                    if let Some(sublayer) = sublayer.as_ref() {
                        strip_layer(sublayer);
                    }
                }
            }
        }
    }

    /// The system's title bar double click (Desktop & Dock settings).
    pub(super) fn double_click(window: &winit::window::Window) {
        let Some(window) = ns_window(window) else {
            return;
        };
        let action = unsafe {
            let defaults: *mut AnyObject =
                msg_send![objc2::class!(NSUserDefaults), standardUserDefaults];
            let Some(defaults) = defaults.as_ref() else {
                return;
            };
            let key = NSString::from_str("AppleActionOnDoubleClick");
            let action: *mut NSString = msg_send![defaults, stringForKey: &*key];
            match action.as_ref() {
                Some(action) => action.to_string(),
                None => {
                    // Before the setting had choices: minimize or zoom.
                    let key = NSString::from_str("AppleMiniaturizeOnDoubleClick");
                    let minimize: bool = msg_send![defaults, boolForKey: &*key];
                    if minimize { "Minimize" } else { "Maximize" }.to_owned()
                }
            }
        };
        match action.as_str() {
            "Minimize" => window.miniaturize(None),
            "None" => {}
            // "Maximize" (zoom), "Fill" and anything newer: zoom.
            _ => window.zoom(None),
        }
    }
}
