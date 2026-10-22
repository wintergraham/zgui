//! Embedded web pages: a [`WebView`] controller and the [`web_view`] /
//! [`web_view_src`] components showing it.
//!
//! ```no_run
//! use zgui::{compose::prelude::*, web_view::{WebView, WebViewOptions}};
//! fn page(cx: &mut Context) -> View {
//!     let web = WebView::new(&cx.runtime(), WebViewOptions::default());
//!     web.load_url("https://example.com");
//!     let title = web.clone();
//!     column()
//!         .grow()
//!         .child(text_signal(move || title.title()))
//!         .child(web_view(&web).grow().rounded(12.))
//! }
//! ```
//!
//! The page is the platform's own view (WKWebView on macOS), laid out,
//! clipped, moved and faded as the node is; zgui content painted after it
//! (menus, popovers, a composer) draws above it, and glass over it blurs the
//! page (see [`crate::native_view`]). Without a platform backend (headless,
//! the software renderer, other platforms for now) the node renders a
//! labelled placeholder box.
//!
//! The controller outlives the node: unmounting hides the page and keeps it
//! loaded; mounting it again (in any window) shows it where it is placed.
//! Commands issued before the page exists are queued. Navigation state is
//! reactive ([`WebView::state`] and its accessors) and platform events reach
//! [`WebView::on_event`] subscribers on the UI thread at a safe point.
use crate::{
    compose::{NodeHandle, View, component, div, switch, text},
    input::InputEvent,
    native_view::{NativeContent, NativeRegistration, NativeViews},
    reactive::{Runtime, Signal},
    style::Styled,
};
use std::{
    any::Any,
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

/// Where a page keeps cookies, storage and caches.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum WebProfile {
    /// The app's default persistent store.
    #[default]
    Default,
    /// Nothing persists; each view gets its own store.
    Ephemeral,
    /// A persistent store of its own, shared by every view naming it (macOS
    /// 14 and later; earlier systems fall back to an ephemeral store).
    Named(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct WebViewOptions {
    /// Draw no page background of the view's own: the node's (and what lies
    /// under it) shows where the page is transparent.
    pub transparent: bool,
    pub profile: WebProfile,
    /// Allow Web Inspector (Safari's Develop menu, [`WebView::open_devtools`]).
    /// Defaults on in debug builds.
    pub inspectable: bool,
    pub user_agent: Option<String>,
    /// Pages may post to the app: `window.webkit.messageHandlers.zgui
    /// .postMessage(value)` arrives as [`WebViewEvent::Message`] (JSON).
    pub messages: bool,
    /// Pages may open windows (`window.open`, target=_blank): each request
    /// arrives as [`WebViewEvent::NewWindowRequested`] and opens nothing.
    pub new_windows: bool,
}
impl Default for WebViewOptions {
    fn default() -> Self {
        Self {
            transparent: false,
            profile: WebProfile::Default,
            inspectable: cfg!(debug_assertions),
            user_agent: None,
            messages: true,
            new_windows: true,
        }
    }
}

/// A page's navigation state, kept by the controller.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WebViewState {
    pub url: String,
    pub title: String,
    pub loading: bool,
    /// Estimated load progress, 0 to 1.
    pub progress: f64,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    /// The page has keyboard focus.
    pub focused: bool,
    /// The last navigation's failure, until the next starts.
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WebViewEvent {
    NavigationStarted {
        url: String,
    },
    /// The new page's content started arriving: its URL is final.
    NavigationCommitted {
        url: String,
    },
    NavigationFinished {
        url: String,
    },
    NavigationFailed {
        url: String,
        error: String,
    },
    TitleChanged {
        title: String,
    },
    UrlChanged {
        url: String,
    },
    /// A page message (see [`WebViewOptions::messages`]), as JSON.
    Message {
        body: String,
    },
    NewWindowRequested {
        url: String,
    },
    /// The page's process ended (crashed or was killed); reload to recover.
    ProcessTerminated,
}

/// A JavaScript evaluation's result: the value as JSON, or the error.
pub type ScriptResult = Result<String, String>;

/// What a controller asks of its platform view.
pub enum WebViewCommand {
    LoadUrl(String),
    LoadHtml {
        html: String,
        base_url: Option<String>,
    },
    Reload,
    Stop,
    GoBack,
    GoForward,
    Evaluate {
        script: String,
        reply: Option<Box<dyn FnOnce(ScriptResult)>>,
    },
    /// Give the page keyboard focus, or take it back (`false`).
    Focus(bool),
    OpenDevtools,
    /// Page zoom (1 is 100%).
    SetZoom(f64),
}
impl std::fmt::Debug for WebViewCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoadUrl(url) => f.debug_tuple("LoadUrl").field(url).finish(),
            Self::LoadHtml { base_url, .. } => f
                .debug_struct("LoadHtml")
                .field("base_url", base_url)
                .finish(),
            Self::Reload => f.write_str("Reload"),
            Self::Stop => f.write_str("Stop"),
            Self::GoBack => f.write_str("GoBack"),
            Self::GoForward => f.write_str("GoForward"),
            Self::Evaluate { script, .. } => {
                f.debug_struct("Evaluate").field("script", script).finish()
            }
            Self::Focus(focus) => f.debug_tuple("Focus").field(focus).finish(),
            Self::OpenDevtools => f.write_str("OpenDevtools"),
            Self::SetZoom(zoom) => f.debug_tuple("SetZoom").field(zoom).finish(),
        }
    }
}

/// A platform's page (implemented by the desktop host).
pub trait WebViewBackend {
    fn run(&self, command: WebViewCommand);
    fn as_any(&self) -> &dyn Any;
}

type Listener = Rc<RefCell<dyn FnMut(&WebViewEvent)>>;
struct Inner {
    id: u64,
    options: WebViewOptions,
    state: Signal<WebViewState>,
    attached: Signal<bool>,
    backend: RefCell<Option<Rc<dyn WebViewBackend>>>,
    queued: RefCell<Vec<WebViewCommand>>,
    listeners: RefCell<Vec<(u64, Listener)>>,
    next_listener: Cell<u64>,
    /// The last `set_src`, so an unchanged reactive source navigates once.
    src: RefCell<Option<String>>,
}
/// A web page's controller; clones share it.
#[derive(Clone)]
pub struct WebView(Rc<Inner>);
/// A weak controller reference, for platform callbacks.
#[derive(Clone)]
pub struct WeakWebView(Weak<Inner>);
impl WeakWebView {
    pub fn upgrade(&self) -> Option<WebView> {
        self.0.upgrade().map(WebView)
    }
}
/// Keeps an event subscription; dropping it unsubscribes.
pub struct WebViewSubscription {
    web: Weak<Inner>,
    id: u64,
}
impl Drop for WebViewSubscription {
    fn drop(&mut self) {
        if let Some(inner) = self.web.upgrade()
            && let Ok(mut listeners) = inner.listeners.try_borrow_mut()
        {
            listeners.retain(|(id, _)| *id != self.id);
        }
    }
}
thread_local! {
    static NEXT_ID: Cell<u64> = const { Cell::new(0) };
}
impl PartialEq for WebView {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl std::fmt::Debug for WebView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebView").field("id", &self.0.id).finish()
    }
}
impl WebView {
    /// Whether both are the same controller.
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
    pub fn new(runtime: &Runtime, options: WebViewOptions) -> Self {
        let id = NEXT_ID.with(|next| {
            let id = next.get() + 1;
            next.set(id);
            id
        });
        Self(Rc::new(Inner {
            id,
            options,
            state: runtime.signal(WebViewState::default()),
            attached: runtime.signal(false),
            backend: RefCell::new(None),
            queued: RefCell::new(Vec::new()),
            listeners: RefCell::new(Vec::new()),
            next_listener: Cell::new(0),
            src: RefCell::new(None),
        }))
    }
    /// Unique per controller in this thread.
    pub fn id(&self) -> u64 {
        self.0.id
    }
    pub fn options(&self) -> &WebViewOptions {
        &self.0.options
    }
    pub fn downgrade(&self) -> WeakWebView {
        WeakWebView(Rc::downgrade(&self.0))
    }
    /// The navigation state, reactive.
    pub fn state(&self) -> Signal<WebViewState> {
        self.0.state.clone()
    }
    pub fn url(&self) -> String {
        self.0.state.with(|state| state.url.clone())
    }
    pub fn title(&self) -> String {
        self.0.state.with(|state| state.title.clone())
    }
    pub fn loading(&self) -> bool {
        self.0.state.with(|state| state.loading)
    }
    pub fn progress(&self) -> f64 {
        self.0.state.with(|state| state.progress)
    }
    pub fn can_go_back(&self) -> bool {
        self.0.state.with(|state| state.can_go_back)
    }
    pub fn can_go_forward(&self) -> bool {
        self.0.state.with(|state| state.can_go_forward)
    }
    pub fn focused(&self) -> bool {
        self.0.state.with(|state| state.focused)
    }
    /// Whether a platform page exists for it (reactive).
    pub fn is_attached(&self) -> bool {
        self.0.attached.get()
    }
    fn run(&self, command: WebViewCommand) {
        let backend = self.0.backend.borrow().clone();
        match backend {
            Some(backend) => backend.run(command),
            None => self.0.queued.borrow_mut().push(command),
        }
    }
    pub fn load_url(&self, url: impl Into<String>) {
        let url = url.into();
        *self.0.src.borrow_mut() = Some(url.clone());
        // Shown at once (placeholders, address bars), confirmed by the page.
        self.0.state.update(|state| {
            state.url = url.clone();
            state.error = None;
        });
        self.run(WebViewCommand::LoadUrl(url));
    }
    /// Navigate to `url` unless it is what the last `set_src` (or
    /// `load_url`) asked for: a reactive source's equal writes do nothing.
    pub fn set_src(&self, url: &str) {
        if self.0.src.borrow().as_deref() == Some(url) {
            return;
        }
        self.load_url(url);
    }
    /// Show `html`; relative URLs and the page's origin resolve against
    /// `base_url` (none: `about:blank`, with no access to other origins).
    pub fn load_html(&self, html: impl Into<String>, base_url: Option<&str>) {
        *self.0.src.borrow_mut() = None;
        self.run(WebViewCommand::LoadHtml {
            html: html.into(),
            base_url: base_url.map(Into::into),
        });
    }
    pub fn reload(&self) {
        self.run(WebViewCommand::Reload);
    }
    pub fn stop(&self) {
        self.run(WebViewCommand::Stop);
    }
    pub fn go_back(&self) {
        self.run(WebViewCommand::GoBack);
    }
    pub fn go_forward(&self) {
        self.run(WebViewCommand::GoForward);
    }
    /// Evaluate `script` in the page; `reply` gets its result as JSON (an
    /// `undefined` result is `null`), on the UI thread at a safe point.
    pub fn evaluate_js(
        &self,
        script: impl Into<String>,
        reply: impl FnOnce(ScriptResult) + 'static,
    ) {
        self.run(WebViewCommand::Evaluate {
            script: script.into(),
            reply: Some(Box::new(reply)),
        });
    }
    /// Evaluate `script`, ignoring its result.
    pub fn run_js(&self, script: impl Into<String>) {
        self.run(WebViewCommand::Evaluate {
            script: script.into(),
            reply: None,
        });
    }
    /// Give the page keyboard focus.
    pub fn focus(&self) {
        self.run(WebViewCommand::Focus(true));
    }
    /// Take keyboard focus back from the page (to the window's zgui content).
    pub fn blur(&self) {
        self.run(WebViewCommand::Focus(false));
    }
    /// Open Web Inspector, when [`WebViewOptions::inspectable`].
    pub fn open_devtools(&self) {
        self.run(WebViewCommand::OpenDevtools);
    }
    pub fn set_zoom(&self, zoom: f64) {
        self.run(WebViewCommand::SetZoom(zoom));
    }
    /// Call `listener` with each platform event, until the subscription drops.
    pub fn on_event(&self, listener: impl FnMut(&WebViewEvent) + 'static) -> WebViewSubscription {
        let id = self.0.next_listener.get() + 1;
        self.0.next_listener.set(id);
        self.0
            .listeners
            .borrow_mut()
            .push((id, Rc::new(RefCell::new(listener))));
        WebViewSubscription {
            web: Rc::downgrade(&self.0),
            id,
        }
    }

    // The platform side.

    /// Attach the platform page; queued commands run now, in order.
    pub fn attach(&self, backend: Rc<dyn WebViewBackend>) {
        *self.0.backend.borrow_mut() = Some(backend.clone());
        let queued = std::mem::take(&mut *self.0.queued.borrow_mut());
        for command in queued {
            backend.run(command);
        }
        self.0.attached.set(true);
    }
    /// Drop the platform page (it goes once nothing else holds it).
    pub fn detach(&self) -> Option<Rc<dyn WebViewBackend>> {
        let backend = self.0.backend.borrow_mut().take();
        self.0.attached.set(false);
        backend
    }
    pub fn backend(&self) -> Option<Rc<dyn WebViewBackend>> {
        self.0.backend.borrow().clone()
    }
    /// Update the state (the platform's report). Call from a delivery
    /// ([`crate::native_view::post`]), not from inside a platform callback.
    pub fn update_state(&self, change: impl FnOnce(&mut WebViewState)) {
        self.0.state.update(change);
    }
    /// Deliver `event` to subscribers. Call from a delivery, as above.
    pub fn emit(&self, event: WebViewEvent) {
        let listeners: Vec<Listener> = self
            .0
            .listeners
            .borrow()
            .iter()
            .map(|(_, listener)| listener.clone())
            .collect();
        for listener in listeners {
            if let Ok(mut listener) = listener.try_borrow_mut() {
                listener(&event);
            }
        }
    }
}
impl Drop for Inner {
    fn drop(&mut self) {
        // Pages go with their controller; the backend's own drop removes the view.
        self.backend.get_mut().take();
    }
}

/// Show `web`'s page in a node laid out like any other: give it a size (or
/// let it grow), round it, clip it, move and fade it.
pub fn web_view(web: &WebView) -> View {
    native(NativeContent::Web(web.clone()), Some(web.clone()), None)
}
/// [`web_view`] navigating to `src` whenever its tracked reads change it.
pub fn web_view_src(web: &WebView, src: impl FnMut() -> String + 'static) -> View {
    native(
        NativeContent::Web(web.clone()),
        Some(web.clone()),
        Some(Box::new(src)),
    )
}
/// A node showing `content` (see [`crate::native_view`]). On macOS the
/// desktop host mounts a `Retained<NSView>` given as
/// `NativeContent::Platform(Rc::new(view))`.
pub fn native_view(content: NativeContent) -> View {
    let web = match &content {
        NativeContent::Web(web) => Some(web.clone()),
        _ => None,
    };
    native(content, web, None)
}
/// A node showing what `content` returns, following its tracked reads (the
/// active tab's page, a view the host makes later). When it returns other
/// content the node stays, with its layout, clips and paint order, and the
/// host swaps the platform view inside it. `Err(label)` shows a placeholder
/// box with that label, as content does without a platform backend.
pub fn native_view_when(
    mut content: impl FnMut() -> Result<NativeContent, String> + 'static,
) -> View {
    component(move |cx| {
        let natives = cx.try_service::<NativeViews>();
        let supported = natives.as_ref().is_some_and(|natives| natives.supported());
        let runtime = cx.runtime();
        let current: Rc<RefCell<Option<NativeContent>>> = Rc::default();
        // What the placeholder says; None while a platform view shows.
        let label: Signal<Option<String>> = runtime.signal(None);
        let shown = label.clone();
        let focused = current.clone();
        div()
            .focusable(true)
            .on_event(move |event| {
                let Some(focus) = focused.borrow().as_ref().and_then(Focusable::of) else {
                    return;
                };
                match event.event {
                    InputEvent::Focus if !focus.focused() => focus.focus(true),
                    InputEvent::Blur if focus.focused() => focus.focus(false),
                    _ => {}
                }
            })
            .child(switch(
                move || shown.get(),
                |label, _| match label {
                    Some(label) => placeholder_box(text(label)),
                    None => div(),
                },
            ))
            .on_mount(move |handle| {
                let node = handle.node();
                let painter = handle.clone();
                let registration: RefCell<Option<NativeRegistration>> = RefCell::default();
                let effect = runtime.clone().effect(move || {
                    let next = content();
                    if let (Ok(next), Some(now)) = (&next, &*current.borrow())
                        && next.same(now)
                    {
                        return;
                    }
                    // The old view goes before the new one comes.
                    drop(registration.borrow_mut().take());
                    let (painted, placeholder) = match &next {
                        Ok(content) => {
                            if let Some(natives) = &natives {
                                *registration.borrow_mut() =
                                    Some(natives.register(node, content.clone()));
                            }
                            (supported, (!supported).then(|| content.label()))
                        }
                        Err(missing) => (false, Some(missing.clone())),
                    };
                    platform_painted(&painter, painted);
                    *current.borrow_mut() = next.ok();
                    label.set(placeholder);
                });
                handle.retain(effect);
            })
    })
}
/// Mark the node painted by the platform (its own paint skipped), now or,
/// while the scene is borrowed, at the next safe point.
fn platform_painted(handle: &NodeHandle, painted: bool) {
    let Some(scene) = handle.scene() else {
        return;
    };
    match scene.try_borrow_mut() {
        Ok(mut scene) => scene.set_platform_painted(handle.node(), painted),
        Err(_) => {
            let handle = handle.clone();
            crate::native_view::post(move || platform_painted(&handle, painted));
        }
    }
}
/// What takes keyboard focus with a native node.
enum Focusable {
    Web(WebView),
    Terminal(crate::terminal::Terminal),
    /// A host's own view: AppKit's first responder follows the node's focus
    /// in the desktop host (`Surfaces::sync_focus`).
    Platform,
}
impl Focusable {
    fn of(content: &NativeContent) -> Option<Self> {
        match content {
            NativeContent::Web(web) => Some(Self::Web(web.clone())),
            NativeContent::Terminal(term) => Some(Self::Terminal(term.clone())),
            NativeContent::Platform(_) => Some(Self::Platform),
        }
    }
    fn focused(&self) -> bool {
        match self {
            Self::Web(web) => web.focused(),
            Self::Terminal(term) => term.focused(),
            Self::Platform => true,
        }
    }
    fn focus(&self, on: bool) {
        match (self, on) {
            (Self::Web(web), true) => web.focus(),
            (Self::Web(web), false) => web.blur(),
            (Self::Terminal(term), true) => term.focus(),
            (Self::Terminal(term), false) => term.blur(),
            (Self::Platform, _) => {}
        }
    }
}
fn native(
    content: NativeContent,
    web: Option<WebView>,
    src: Option<Box<dyn FnMut() -> String>>,
) -> View {
    component(move |cx| {
        let natives = cx.try_service::<NativeViews>();
        let supported = natives.as_ref().is_some_and(|natives| natives.supported());
        if let (Some(web), Some(mut src)) = (web.clone(), src) {
            let effect = cx.runtime().effect(move || {
                let url = src();
                web.set_src(&url);
            });
            cx.retain(effect);
        }
        let focus = Focusable::of(&content);
        let mut root = div().focusable(focus.is_some()).on_event(move |event| {
            // The view's keyboard focus follows the node's: Tab into it,
            // or focus moved away from it in zgui.
            let Some(focus) = &focus else { return };
            match event.event {
                InputEvent::Focus if !focus.focused() => focus.focus(true),
                InputEvent::Blur if focus.focused() => focus.focus(false),
                _ => {}
            }
        });
        if !supported {
            let label = content.clone();
            root = root.child(placeholder(label));
        }
        let registered = content.clone();
        root.on_mount(move |handle| {
            if let Some(natives) = natives {
                if supported
                    && let Some(scene) = handle.scene()
                    && let Ok(mut scene) = scene.try_borrow_mut()
                {
                    scene.set_platform_painted(handle.node(), true);
                }
                handle.retain(natives.register(handle.node(), registered));
            }
        })
    })
}
/// What a native node shows without a platform backend.
fn placeholder(content: NativeContent) -> View {
    let label = match &content {
        NativeContent::Web(web) => {
            let web = web.clone();
            crate::compose::text_signal(move || {
                let url = web.url();
                if url.is_empty() {
                    "Web view".into()
                } else {
                    format!("Web view · {url}")
                }
            })
        }
        NativeContent::Terminal(term) => {
            let term = term.clone();
            crate::compose::text_signal(move || {
                let title = term.title();
                if title.is_empty() {
                    "Terminal".into()
                } else {
                    format!("Terminal · {title}")
                }
            })
        }
        NativeContent::Platform(_) => text(content.label()),
    };
    placeholder_box(label)
}
/// A labelled box filling a native node.
fn placeholder_box(label: crate::compose::View) -> View {
    div()
        .absolute()
        .left(0.)
        .top(0.)
        .w_full()
        .h_full()
        .bg(crate::style::rgba(0x8080_8020))
        .border(1.)
        .border_color(crate::style::rgba(0x8080_8060))
        .items_center()
        .justify_center()
        .overflow_hidden()
        .child(
            label
                .text_size(12.)
                .text_color(crate::style::rgba(0xa0a0_a0ff)),
        )
}
