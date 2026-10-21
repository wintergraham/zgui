//! A `WKWebView` behind a `zgui::web_view::WebView` controller.
//!
//! WebKit's callbacks (navigation delegate, key-value observation, script
//! replies, page messages) arrive on the main thread but can arrive inside
//! any zgui call into WebKit; each reads what it needs and posts the state
//! change (`zgui::native_view::post`), applied at the event loop's next safe
//! point. The page's view is mounted by the window host (macos.rs).
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::{Retained, Weak},
    runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject},
    sel,
};
use objc2_app_kit::{NSColor, NSView};
use objc2_foundation::{
    NSDictionary, NSError, NSKeyValueChangeKey, NSKeyValueObservingOptions, NSNumber,
    NSObjectNSKeyValueObserverRegistration, NSRect, NSString, NSURL, NSURLRequest, NSUUID,
};
use objc2_web_kit::{
    WKNavigation, WKNavigationAction, WKNavigationActionPolicy, WKNavigationDelegate,
    WKScriptMessage, WKScriptMessageHandler, WKUIDelegate, WKUserContentController, WKUserScript,
    WKUserScriptInjectionTime, WKWebView, WKWebViewConfiguration, WKWebsiteDataStore,
    WKWindowFeatures,
};
use std::{cell::RefCell, collections::HashMap, ffi::c_void, mem::ManuallyDrop, rc::Rc};
use zgui::web_view::{
    ScriptResult, WeakWebView, WebProfile, WebView, WebViewBackend, WebViewCommand, WebViewEvent,
};

#[link(name = "WebKit", kind = "framework")]
unsafe extern "C" {}

/// Observed for state: navigation, title, loading and history.
const OBSERVED: [&str; 6] = [
    "URL",
    "title",
    "loading",
    "estimatedProgress",
    "canGoBack",
    "canGoForward",
];
/// The page-side name of the app's message channel.
const CHANNEL: &str = "zgui";
/// Defines `window.zgui.postMessage(value)`, sending `value` as JSON.
const CHANNEL_SCRIPT: &str = "(() => { const handler = window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.zgui; if (!handler || window.zgui) return; Object.defineProperty(window, 'zgui', { value: Object.freeze({ postMessage(value) { handler.postMessage(JSON.stringify(value === undefined ? null : value)); } }) }); })();";

pub(crate) struct ObserverState {
    web: WeakWebView,
    view: RefCell<Weak<WKWebView>>,
    new_windows: bool,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "ZguiWebObserver"]
    #[ivars = ObserverState]
    pub(crate) struct Observer;
    unsafe impl NSObjectProtocol for Observer {}
    impl Observer {
        #[unsafe(method(observeValueForKeyPath:ofObject:change:context:))]
        fn observe(&self, _key: Option<&NSString>, _object: Option<&AnyObject>, _change: Option<&NSDictionary<NSKeyValueChangeKey, AnyObject>>, _context: *mut c_void) {
            self.changed();
        }
    }
    unsafe impl WKNavigationDelegate for Observer {
        #[unsafe(method(webView:decidePolicyForNavigationAction:decisionHandler:))]
        fn policy(&self, _view: &WKWebView, _action: &WKNavigationAction, decision: &block2::DynBlock<dyn Fn(WKNavigationActionPolicy)>) {
            decision.call((WKNavigationActionPolicy::Allow,));
        }
        #[unsafe(method(webView:didStartProvisionalNavigation:))]
        fn start(&self, view: &WKWebView, _navigation: Option<&WKNavigation>) {
            let url = url_of(view);
            self.post(move |web| {
                web.update_state(|state| state.error = None);
                web.emit(WebViewEvent::NavigationStarted { url });
            });
            self.changed();
        }
        #[unsafe(method(webView:didCommitNavigation:))]
        fn commit(&self, view: &WKWebView, _navigation: Option<&WKNavigation>) {
            let url = url_of(view);
            self.post(move |web| web.emit(WebViewEvent::NavigationCommitted { url }));
            self.changed();
        }
        #[unsafe(method(webView:didFinishNavigation:))]
        fn finish(&self, view: &WKWebView, _navigation: Option<&WKNavigation>) {
            self.changed();
            let url = url_of(view);
            self.post(move |web| web.emit(WebViewEvent::NavigationFinished { url }));
        }
        #[unsafe(method(webView:didFailProvisionalNavigation:withError:))]
        fn provisional_error(&self, view: &WKWebView, _navigation: Option<&WKNavigation>, error: &NSError) {
            self.failed(view, error);
        }
        #[unsafe(method(webView:didFailNavigation:withError:))]
        fn navigation_error(&self, view: &WKWebView, _navigation: Option<&WKNavigation>, error: &NSError) {
            self.failed(view, error);
        }
        #[unsafe(method(webViewWebContentProcessDidTerminate:))]
        fn terminated(&self, _view: &WKWebView) {
            self.post(|web| {
                web.update_state(|state| {
                    state.loading = false;
                    state.error = Some("The page stopped responding.".into());
                });
                web.emit(WebViewEvent::ProcessTerminated);
            });
        }
    }
    unsafe impl WKUIDelegate for Observer {
        #[unsafe(method_id(webView:createWebViewWithConfiguration:forNavigationAction:windowFeatures:))]
        fn create_window(&self, _view: &WKWebView, _configuration: &WKWebViewConfiguration, action: &WKNavigationAction, _features: &WKWindowFeatures) -> Option<Retained<WKWebView>> {
            if self.ivars().new_windows {
                let url = unsafe { action.request().URL() }
                    .and_then(|url| url.absoluteString())
                    .map(|url| url.to_string())
                    .unwrap_or_default();
                self.post(move |web| web.emit(WebViewEvent::NewWindowRequested { url }));
            }
            None
        }
    }
    unsafe impl WKScriptMessageHandler for Observer {
        #[unsafe(method(userContentController:didReceiveScriptMessage:))]
        fn message(&self, _controller: &WKUserContentController, message: &WKScriptMessage) {
            let body = unsafe { message.body() };
            let body = match body.downcast_ref::<NSString>() {
                Some(text) => text.to_string(),
                None => json(Some(&body)),
            };
            self.post(move |web| web.emit(WebViewEvent::Message { body }));
        }
    }
);

impl Observer {
    fn new(web: &WebView, mtm: MainThreadMarker) -> Retained<Self> {
        let object = mtm.alloc().set_ivars(ObserverState {
            web: web.downgrade(),
            view: RefCell::new(Weak::default()),
            new_windows: web.options().new_windows,
        });
        unsafe { msg_send![super(object), init] }
    }
    fn post(&self, delivery: impl FnOnce(&WebView) + 'static) {
        let web = self.ivars().web.clone();
        zgui::native_view::post(move || {
            if let Some(web) = web.upgrade() {
                delivery(&web);
            }
        });
    }
    /// Report the page's navigation state as it is now.
    fn changed(&self) {
        let Some(view) = self.ivars().view.borrow().load() else {
            return;
        };
        let url = url_of(&view);
        let (title, loading, progress, back, forward) = unsafe {
            (
                view.title()
                    .map(|title| title.to_string())
                    .unwrap_or_default(),
                view.isLoading(),
                view.estimatedProgress(),
                view.canGoBack(),
                view.canGoForward(),
            )
        };
        self.post(move |web| {
            let (old_url, old_title) = web
                .state()
                .with_untracked(|state| (state.url.clone(), state.title.clone()));
            web.update_state(|state| {
                if !url.is_empty() {
                    state.url = url.clone();
                }
                state.title = title.clone();
                state.loading = loading;
                state.progress = progress;
                state.can_go_back = back;
                state.can_go_forward = forward;
            });
            if !url.is_empty() && url != old_url {
                web.emit(WebViewEvent::UrlChanged { url });
            }
            if title != old_title {
                web.emit(WebViewEvent::TitleChanged { title });
            }
        });
    }
    fn failed(&self, view: &WKWebView, error: &NSError) {
        // NSURLErrorCancelled: a new navigation replaced this one.
        if error.code() == -999 {
            return;
        }
        let url = url_of(view);
        let message = error.localizedDescription().to_string();
        self.post(move |web| {
            web.update_state(|state| {
                state.loading = false;
                state.error = Some(message.clone());
            });
            web.emit(WebViewEvent::NavigationFailed {
                url,
                error: message,
            });
        });
        self.changed();
    }
}

fn url_of(view: &WKWebView) -> String {
    unsafe { view.URL() }
        .and_then(|url| url.absoluteString())
        .map(|url| url.to_string())
        .unwrap_or_default()
}

/// A JavaScript value WebKit returned, as JSON.
fn json(value: Option<&AnyObject>) -> String {
    let Some(value) = value else {
        return "null".into();
    };
    if let Some(text) = value.downcast_ref::<NSString>() {
        return quote(&text.to_string());
    }
    if value.class().name().to_bytes() == b"__NSCFBoolean" {
        let on: bool = unsafe { msg_send![value, boolValue] };
        return if on { "true" } else { "false" }.into();
    }
    if let Some(number) = value.downcast_ref::<NSNumber>() {
        return number.stringValue().to_string();
    }
    if value.class().name().to_bytes() == b"NSNull" {
        return "null".into();
    }
    unsafe {
        let class = objc2::class!(NSJSONSerialization);
        let valid: bool = msg_send![class, isValidJSONObject: value];
        if valid {
            let data: *mut AnyObject = msg_send![class, dataWithJSONObject: value, options: 0usize, error: std::ptr::null_mut::<*mut NSError>()];
            if let Some(data) = data.as_ref() {
                let bytes: *const u8 = msg_send![data, bytes];
                let length: usize = msg_send![data, length];
                if !bytes.is_null() {
                    return String::from_utf8_lossy(std::slice::from_raw_parts(bytes, length))
                        .into_owned();
                }
            }
        }
        let description: Retained<NSString> = msg_send![value, description];
        quote(&description.to_string())
    }
}
fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

thread_local! {
    // Stores stay alive through process exit: releasing a data store in
    // AppKit's exit (Rust TLS destructors) removes its network session before
    // WebKit flushes cookies (learned in comet-native's browser).
    static STORES: ManuallyDrop<RefCell<HashMap<WebProfile, Retained<WKWebsiteDataStore>>>> =
        ManuallyDrop::new(RefCell::new(HashMap::new()));
}
fn data_store(profile: &WebProfile, mtm: MainThreadMarker) -> Retained<WKWebsiteDataStore> {
    // Each ephemeral view gets a store of its own.
    if *profile == WebProfile::Ephemeral {
        return unsafe { WKWebsiteDataStore::nonPersistentDataStore(mtm) };
    }
    STORES.with(|stores| {
        stores
            .borrow_mut()
            .entry(profile.clone())
            .or_insert_with(|| unsafe {
                match profile {
                    WebProfile::Named(name) if named_stores_supported() => {
                        let identifier = NSUUID::from_bytes(identifier(name));
                        WKWebsiteDataStore::dataStoreForIdentifier(&identifier, mtm)
                    }
                    WebProfile::Named(_) => WKWebsiteDataStore::nonPersistentDataStore(mtm),
                    _ => WKWebsiteDataStore::defaultDataStore(mtm),
                }
            })
            .clone()
    })
}
/// Persistent stores of their own need macOS 14.
fn named_stores_supported() -> bool {
    unsafe {
        msg_send![objc2::class!(WKWebsiteDataStore), respondsToSelector: sel!(dataStoreForIdentifier:)]
    }
}
/// A stable UUID for a profile name (FNV-1a, two lanes).
fn identifier(name: &str) -> [u8; 16] {
    let lane = |seed: u64| {
        name.bytes().fold(seed, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    };
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&lane(0xcbf2_9ce4_8422_2325).to_be_bytes());
    bytes[8..].copy_from_slice(&lane(0x6c62_272e_07bb_0142).to_be_bytes());
    // Version 5 shape, RFC 4122 variant.
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    bytes
}

/// A page: the `WKWebView` and its delegate.
pub(crate) struct MacWebView {
    view: Retained<WKWebView>,
    observer: Retained<Observer>,
    messages: bool,
    transparent: bool,
    /// The last HTML shown and its base: WebKit reloads such a page by
    /// fetching its URL (`about:blank`, a blank page), so reload shows it again.
    html: RefCell<Option<Html>>,
}
/// A page's HTML and its base URL.
type Html = (Retained<NSString>, Option<Retained<NSURL>>);
impl MacWebView {
    pub fn new(web: &WebView, mtm: MainThreadMarker) -> Self {
        let options = web.options().clone();
        let observer = Observer::new(web, mtm);
        let view = unsafe {
            let configuration = WKWebViewConfiguration::new(mtm);
            configuration.setWebsiteDataStore(&data_store(&options.profile, mtm));
            if options.messages {
                let content = configuration.userContentController();
                content.addScriptMessageHandler_name(
                    ProtocolObject::from_ref(&*observer),
                    &NSString::from_str(CHANNEL),
                );
                let script = WKUserScript::initWithSource_injectionTime_forMainFrameOnly(
                    mtm.alloc(),
                    &NSString::from_str(CHANNEL_SCRIPT),
                    WKUserScriptInjectionTime::AtDocumentStart,
                    true,
                );
                content.addUserScript(&script);
            }
            WKWebView::initWithFrame_configuration(mtm.alloc(), NSRect::ZERO, &configuration)
        };
        *observer.ivars().view.borrow_mut() = Weak::from_retained(&view);
        unsafe {
            view.setNavigationDelegate(Some(ProtocolObject::from_ref(&*observer)));
            view.setUIDelegate(Some(ProtocolObject::from_ref(&*observer)));
            view.setAllowsBackForwardNavigationGestures(true);
            for key in OBSERVED {
                view.addObserver_forKeyPath_options_context(
                    &observer,
                    &NSString::from_str(key),
                    NSKeyValueObservingOptions::empty(),
                    std::ptr::null_mut(),
                );
            }
            if let Some(agent) = &options.user_agent {
                view.setCustomUserAgent(Some(&NSString::from_str(agent)));
            }
            let inspectable: bool = msg_send![&*view, respondsToSelector: sel!(setInspectable:)];
            if inspectable {
                let _: () = msg_send![&*view, setInspectable: options.inspectable];
            }
            if options.transparent {
                let no = NSNumber::new_bool(false);
                let _: () = msg_send![&*view, setValue: &*no, forKey: &*NSString::from_str("drawsBackground")];
                let clear = NSColor::clearColor();
                let _: () = msg_send![&*view, setUnderPageBackgroundColor: &*clear];
            }
        }
        Self {
            view,
            observer,
            messages: options.messages,
            transparent: options.transparent,
            html: RefCell::new(None),
        }
    }
    pub fn view(&self) -> &NSView {
        &self.view
    }
    pub fn transparent(&self) -> bool {
        self.transparent
    }
    /// The colour WebKit shows beyond the page (and under tiles not drawn
    /// yet, while it catches up with a new size).
    pub fn under_page_color(&self) -> Retained<NSColor> {
        unsafe { self.view.underPageBackgroundColor() }
    }
    fn has_focus(&self) -> bool {
        self.view
            .window()
            .and_then(|window| window.firstResponder())
            .and_then(|responder| responder.downcast::<NSView>().ok())
            .is_some_and(|view| view.isDescendantOf(&self.view))
    }
    fn load(&self, url: &str) {
        let parsed = NSURL::URLWithString(&NSString::from_str(url));
        let parsed = parsed.or_else(|| {
            // A path: show the file.
            std::path::Path::new(url)
                .is_absolute()
                .then(|| NSURL::fileURLWithPath(&NSString::from_str(url)))
        });
        let Some(parsed) = parsed else {
            let url = url.to_owned();
            self.observer.post(move |web| {
                web.update_state(|state| state.error = Some("Not a URL".into()));
                web.emit(WebViewEvent::NavigationFailed {
                    url,
                    error: "Not a URL".into(),
                });
            });
            return;
        };
        unsafe {
            if parsed.isFileURL() {
                let directory = parsed
                    .URLByDeletingLastPathComponent()
                    .unwrap_or(parsed.clone());
                self.view
                    .loadFileURL_allowingReadAccessToURL(&parsed, &directory);
            } else {
                self.view
                    .loadRequest(&NSURLRequest::requestWithURL(&parsed));
            }
        }
    }
}
impl WebViewBackend for MacWebView {
    fn run(&self, command: WebViewCommand) {
        unsafe {
            match command {
                WebViewCommand::LoadUrl(url) => {
                    self.html.replace(None);
                    self.load(&url);
                }
                WebViewCommand::LoadHtml { html, base_url } => {
                    let base =
                        base_url.and_then(|base| NSURL::URLWithString(&NSString::from_str(&base)));
                    let html = NSString::from_str(&html);
                    self.view.loadHTMLString_baseURL(&html, base.as_deref());
                    self.html.replace(Some((html, base)));
                }
                WebViewCommand::Reload => {
                    // Still on the HTML's page (not a link followed from it).
                    let html = self.html.borrow().clone().filter(|(_, base)| {
                        let shown = base
                            .as_ref()
                            .and_then(|base| base.absoluteString())
                            .map_or_else(|| "about:blank".into(), |url| url.to_string());
                        url_of(&self.view) == shown
                    });
                    match html {
                        Some((html, base)) => {
                            self.view.loadHTMLString_baseURL(&html, base.as_deref());
                        }
                        None => {
                            self.view.reload();
                        }
                    }
                }
                WebViewCommand::Stop => self.view.stopLoading(),
                WebViewCommand::GoBack => {
                    self.view.goBack();
                }
                WebViewCommand::GoForward => {
                    self.view.goForward();
                }
                WebViewCommand::Evaluate { script, reply } => {
                    let reply = RefCell::new(reply);
                    let web = self.observer.ivars().web.clone();
                    let completion =
                        block2::RcBlock::new(move |value: *mut AnyObject, error: *mut NSError| {
                            let Some(reply) = reply.borrow_mut().take() else {
                                return;
                            };
                            let result: ScriptResult = match error.as_ref() {
                                Some(error) => Err(error.localizedDescription().to_string()),
                                None => Ok(json(value.as_ref())),
                            };
                            let web = web.clone();
                            zgui::native_view::post(move || {
                                // The reply runs even if the controller went.
                                let _ = web.upgrade();
                                reply(result);
                            });
                        });
                    self.view.evaluateJavaScript_completionHandler(
                        &NSString::from_str(&script),
                        Some(&completion),
                    );
                }
                WebViewCommand::Focus(focus) => {
                    let Some(window) = self.view.window() else {
                        return;
                    };
                    if focus && !self.has_focus() {
                        window.makeFirstResponder(Some(&self.view));
                    } else if !focus && self.has_focus() {
                        window.makeFirstResponder(
                            window
                                .contentView()
                                .as_deref()
                                .map(|view| -> &objc2_app_kit::NSResponder { view }),
                        );
                    }
                }
                WebViewCommand::OpenDevtools => {
                    let has: bool = msg_send![&*self.view, respondsToSelector: sel!(_inspector)];
                    if has {
                        let inspector: *mut AnyObject = msg_send![&*self.view, _inspector];
                        if let Some(inspector) = inspector.as_ref() {
                            let _: () = msg_send![inspector, show];
                        }
                    }
                }
                WebViewCommand::SetZoom(zoom) => {
                    let has: bool = msg_send![&*self.view, respondsToSelector: sel!(setPageZoom:)];
                    if has {
                        let _: () = msg_send![&*self.view, setPageZoom: zoom];
                    }
                }
            }
        }
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl Drop for MacWebView {
    fn drop(&mut self) {
        unsafe {
            if self.has_focus()
                && let Some(window) = self.view.window()
            {
                window.makeFirstResponder(
                    window
                        .contentView()
                        .as_deref()
                        .map(|view| -> &objc2_app_kit::NSResponder { view }),
                );
            }
            for key in OBSERVED {
                self.view
                    .removeObserver_forKeyPath(&self.observer, &NSString::from_str(key));
            }
            self.view.setNavigationDelegate(None);
            self.view.setUIDelegate(None);
            if self.messages {
                let content = self.view.configuration().userContentController();
                content.removeScriptMessageHandlerForName(&NSString::from_str(CHANNEL));
            }
            self.view.stopLoading();
        }
        self.view.removeFromSuperview();
    }
}

/// The platform page behind `web`, made now if it has none.
pub(crate) fn backend(web: &WebView, mtm: MainThreadMarker) -> Rc<dyn WebViewBackend> {
    if let Some(backend) = web.backend()
        && backend.as_any().is::<MacWebView>()
    {
        return backend;
    }
    let backend: Rc<dyn WebViewBackend> = Rc::new(MacWebView::new(web, mtm));
    web.attach(backend.clone());
    backend
}
