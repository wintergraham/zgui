//! The pointer over native views, as a desktop host follows it (headless):
//! over a page that takes input zgx hears one leave and then nothing, the
//! page hears where the pointer is, and coming back over zgx resumes moves.
use std::{cell::RefCell, rc::Rc};
use zgx::{
    compose::prelude::*,
    input::{InputEvent, PointerButton},
    native_view::{NativePointer, NativeViews, PlaneSplit, PointerRoute, PointerStep},
    web_view::{WebView, WebViewOptions},
    widgets::Ui,
};

struct Window {
    ui: Ui,
    natives: NativeViews,
    pointer: NativePointer,
    web: WebView,
    log: Rc<RefCell<Vec<String>>>,
    /// What follows the page's pointer, as a script reading `web.pointer`.
    follow: Option<zgx::reactive::Effect>,
}
impl Window {
    fn new() -> Self {
        let mut window = Self::unfollowed();
        let web = window.web.clone();
        window.follow = Some(window.ui.runtime.effect(move || {
            let _ = web.pointer();
        }));
        window
    }
    /// A page nothing follows the pointer over.
    fn unfollowed() -> Self {
        let mut ui = Ui::new(800., 600.);
        let natives = NativeViews::new();
        natives.set_supported(true);
        let web = WebView::new(&ui.runtime, WebViewOptions::default());
        let log = Rc::new(RefCell::new(Vec::new()));
        let hover = |name: &'static str| {
            let log = log.clone();
            move |cx: &mut zgx::input::EventContext| match cx.event {
                InputEvent::PointerEnter => log.borrow_mut().push(format!("{name}+")),
                InputEvent::PointerLeave => log.borrow_mut().push(format!("{name}-")),
                _ => {}
            }
        };
        // A pane: a 40-point bar over a page, and a button drawn over the
        // page's bottom right.
        ui.mount(provide(
            natives.clone(),
            overlay()
                .w(800.)
                .h(600.)
                .child(
                    column()
                        .w(600.)
                        .h(500.)
                        .on_event(hover("pane"))
                        .child(
                            div()
                                .w_full()
                                .h(40.)
                                .bg(rgb(0x303030))
                                .on_event(hover("bar")),
                        )
                        .child(web_view(&web).w(600.).h(460.)),
                )
                .child(
                    button()
                        .absolute()
                        .left(500.)
                        .top(440.)
                        .size(60., 40.)
                        .on_click(|| {})
                        .on_event(hover("button")),
                ),
        ));
        ui.prepare_frame();
        let split = {
            let scene = ui.scene.borrow();
            PlaneSplit::compute(&scene, &natives.nodes(), &PlaneSplit::default()).0
        };
        natives.set_split(Rc::new(split));
        Self {
            ui,
            natives,
            pointer: NativePointer::new(),
            web,
            log,
            follow: None,
        }
    }
    /// What the host does with a pointer move.
    fn move_to(&mut self, x: f32, y: f32) -> PointerStep {
        let route = {
            let scene = self.ui.scene.borrow();
            self.natives.split().route(&scene, &self.ui.input, x, y)
        };
        let held = self.ui.input.captured().is_some();
        let (step, report) = self.pointer.moved(&self.natives, route, held, x, y);
        report.apply();
        match step {
            PointerStep::Move => {
                self.ui.dispatch(InputEvent::PointerMove { x, y });
            }
            PointerStep::Leave(_) => {
                self.ui.dispatch(InputEvent::PointerLeave);
            }
            PointerStep::Over(_) => {}
        }
        step
    }
    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.log.borrow_mut())
    }
}

#[test]
fn entering_a_page_leaves_zgx_and_tells_the_page() {
    let mut w = Window::new();
    assert_eq!(w.move_to(100., 20.), PointerStep::Move);
    assert_eq!(w.take(), ["pane+", "bar+"]);
    assert_eq!(w.web.pointer(), None);
    // Onto the page: one leave for everything zgx hovered.
    assert!(matches!(w.move_to(100., 100.), PointerStep::Leave(_)));
    assert_eq!(w.take(), ["bar-", "pane-"]);
    assert_eq!(w.ui.input.hovered(), None);
    assert_eq!(w.web.pointer(), Some((100., 60.)));
    // Moving over it: zgx hears nothing; the page hears each move.
    assert!(matches!(w.move_to(300., 200.), PointerStep::Over(_)));
    assert!(w.take().is_empty());
    assert_eq!(w.web.pointer(), Some((300., 160.)));
    // Over zgx drawn on the page: zgx again, and the page still knows.
    assert_eq!(w.move_to(520., 460.), PointerStep::Move);
    assert_eq!(w.take(), ["button+"]);
    assert_eq!(w.web.pointer(), Some((520., 420.)));
    // Back onto the page, then up to the bar.
    assert!(matches!(w.move_to(300., 300.), PointerStep::Leave(_)));
    assert_eq!(w.take(), ["button-"]);
    assert_eq!(w.move_to(300., 30.), PointerStep::Move);
    assert_eq!(w.take(), ["pane+", "bar+"]);
    assert_eq!(w.web.pointer(), None, "the page heard it go");
}

#[test]
fn a_press_of_zgx_keeps_hearing_the_pointer_over_a_page() {
    let mut w = Window::new();
    w.move_to(100., 20.);
    w.take();
    // A press on the bar that captures the pointer (a pane being dragged).
    let node = w.ui.input.hovered().unwrap();
    let capture = w.ui.input.listen(node, |cx| {
        if matches!(cx.event, InputEvent::PointerDown { .. }) {
            cx.capture_pointer();
        }
    });
    w.ui.dispatch(InputEvent::PointerDown {
        x: 100.,
        y: 20.,
        button: PointerButton::Primary,
    });
    assert_eq!(w.ui.input.captured(), Some(node));
    assert_eq!(w.move_to(100., 200.), PointerStep::Move);
    assert_eq!(w.web.pointer(), Some((100., 160.)));
    drop(capture);
}

#[test]
fn a_page_nothing_follows_hears_nothing() {
    let mut w = Window::unfollowed();
    let route = |w: &Window, x, y| {
        let scene = w.ui.scene.borrow();
        w.natives.split().route(&scene, &w.ui.input, x, y)
    };
    w.move_to(100., 20.);
    let r = route(&w, 100., 100.);
    let (step, report) = w.pointer.moved(&w.natives, r, false, 100., 100.);
    assert!(matches!(step, PointerStep::Leave(_)));
    let r = route(&w, 120., 110.);
    let (step, report2) = w.pointer.moved(&w.natives, r, false, 120., 110.);
    // Over the page: zgx hears nothing and the page has nothing to hear,
    // so the host has nothing to deliver (no wake).
    assert!(matches!(step, PointerStep::Over(_)));
    assert!(report.is_empty() && report2.is_empty());
    drop(w.follow.take());
}

#[test]
fn leaving_the_window_clears_the_page() {
    let mut w = Window::new();
    w.move_to(100., 100.);
    assert!(w.web.pointer().is_some());
    w.pointer.left().apply();
    assert_eq!(w.web.pointer(), None);
    assert_eq!(w.pointer.over(), None);
    // Back over the page: a leave again (zgx may have re-hovered meanwhile).
    assert!(matches!(w.move_to(100., 100.), PointerStep::Leave(_)));
}

/// What a drag over the window drops on (`PlaneSplit::native_at`): the page
/// wherever it shows, even under a scrim taking presses (which routes the
/// pointer to zgx), and not where zgx paints over it.
#[test]
fn a_drag_drops_on_the_page_wherever_it_shows() {
    let mut ui = Ui::new(800., 600.);
    let natives = NativeViews::new();
    natives.set_supported(true);
    let web = WebView::new(&ui.runtime, WebViewOptions::default());
    // A page under a 40-point bar, a scrim over everything (as an open
    // popover's), and the popover's panel over the page's top right.
    ui.mount(provide(
        natives.clone(),
        overlay()
            .w(800.)
            .h(600.)
            .child(
                column()
                    .w(600.)
                    .h(500.)
                    .child(div().w_full().h(40.).bg(rgb(0x303030)))
                    .child(web_view(&web).w(600.).h(460.)),
            )
            .child(
                div()
                    .absolute()
                    .left(0.)
                    .top(0.)
                    .size(800., 600.)
                    .on_event(|_| {}),
            )
            .child(
                div()
                    .absolute()
                    .left(400.)
                    .top(50.)
                    .size(180., 200.)
                    .bg(rgb(0x202020))
                    .on_event(|_| {}),
            ),
    ));
    ui.prepare_frame();
    let scene = ui.scene.borrow();
    let split = PlaneSplit::compute(&scene, &natives.nodes(), &PlaneSplit::default()).0;
    let page = natives.nodes()[0];
    // Pointer input there is the scrim's; a drop is the page's.
    assert_eq!(split.route(&scene, &ui.input, 100., 300.), PointerRoute::Ui);
    assert_eq!(split.native_at(&scene, 100., 300.), Some(page));
    // Over the panel painted on the page, the bar, and past the page.
    assert_eq!(split.native_at(&scene, 450., 100.), None);
    assert_eq!(split.native_at(&scene, 100., 20.), None);
    assert_eq!(split.native_at(&scene, 700., 300.), None);
}

/// A drag released over a popover open on the page drops nothing there:
/// over its rows (which paint nothing of their own, their text aside), in a
/// panel that fades in as a layer of its own, over the window-wide scrim
/// the popover opens with, as Ask's downloads menu is built.
#[test]
fn a_drag_over_a_popover_on_the_page_drops_nothing_there() {
    let mut ui = Ui::new(800., 600.);
    let natives = NativeViews::new();
    natives.set_supported(true);
    let web = WebView::new(&ui.runtime, WebViewOptions::default());
    let row = || {
        div()
            .w_full()
            .h(52.)
            .p(8.)
            .on_event(|_| {})
            .child(text("report.pdf"))
    };
    ui.mount(provide(
        natives.clone(),
        overlay()
            .w(800.)
            .h(600.)
            .child(
                column()
                    .w(800.)
                    .h(600.)
                    .child(div().w_full().h(40.).bg(rgb(0x303030)))
                    .child(web_view(&web).w(800.).h(560.)),
            )
            .child(
                div()
                    .absolute()
                    .left(0.)
                    .top(0.)
                    .size(800., 600.)
                    .on_event(|_| {}),
            )
            .child(
                div()
                    .absolute()
                    .left(0.)
                    .top(0.)
                    .size(800., 600.)
                    .pointer_events(false)
                    .child(
                        column()
                            .absolute()
                            .left(500.)
                            .top(44.)
                            .w(280.)
                            .opacity(0.9)
                            .child(
                                column()
                                    .w_full()
                                    .bg(rgb(0x202020))
                                    .rounded(12.)
                                    .overflow_hidden()
                                    .child(column().w_full().p(6.).child(row()).child(row())),
                            ),
                    ),
            ),
    ));
    ui.prepare_frame();
    let scene = ui.scene.borrow();
    let split = PlaneSplit::compute(&scene, &natives.nodes(), &PlaneSplit::default()).0;
    let page = natives.nodes()[0];
    // On a row, between its text and its edge, and in the panel's padding.
    assert_eq!(split.native_at(&scene, 760., 80.), None);
    assert_eq!(split.native_at(&scene, 503., 47.), None);
    // Beside the popover, the page.
    assert_eq!(split.native_at(&scene, 200., 300.), Some(page));
}
