//! The terminal example's interface (zgui-desktop/examples/terminal), run
//! headless: its commands split, close and move panes as its menus would.
mod common;

use common::Harness;
use std::{cell::RefCell, rc::Rc};
use zgui::scene::Rect;
use zgui_script::{ScriptHost, Value};

const APP: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../zgui-desktop/examples/terminal/app.luau"
);

struct App {
    h: Harness,
    actions: Rc<RefCell<Vec<String>>>,
}
impl App {
    fn new() -> Self {
        let actions: Rc<RefCell<Vec<String>>> = Rc::default();
        let log = actions.clone();
        let h = Harness::new(move |runtime| {
            let host = ScriptHost::new(runtime, APP);
            for name in ["drag_window", "zoom_window", "close_window", "set_title"] {
                let log = log.clone();
                host.action(name, move |value| {
                    let arg = match value {
                        Value::Str(s) => format!(" {s}"),
                        _ => String::new(),
                    };
                    log.borrow_mut().push(format!("{name}{arg}"));
                });
            }
            host.shared("launch_dir", "/tmp");
            host.shared("home", "/Users/someone");
            host
        });
        assert_eq!(h.host.error().get(), None);
        Self { h, actions }
    }
    fn run(&mut self, id: &str) {
        assert!(self.h.host.run_command(id), "no command `{id}`");
        self.settle();
    }
    /// Half a second of display frames: panes finish easing into place.
    fn settle(&mut self) {
        self.h.settle();
        for _ in 0..40 {
            self.h.frame();
        }
    }
    /// Each mounted terminal's box.
    fn panes(&mut self) -> Vec<Rect> {
        self.h.ui.prepare_frame();
        let scene = self.h.ui.scene.borrow();
        let mut rects: Vec<Rect> = self
            .h
            .natives
            .nodes()
            .into_iter()
            .map(|n| scene.bounds(n))
            .collect();
        rects.sort_by(|a, b| (a.x, a.y).partial_cmp(&(b.x, b.y)).unwrap());
        rects
    }
}

#[test]
fn panes_split_beside_and_below_and_close_back_to_one() {
    let mut app = App::new();
    let one = app.panes();
    assert_eq!(one.len(), 1);
    assert!(one[0].width > 300. && one[0].height > 200., "{one:?}");

    app.run("split_right");
    let two = app.panes();
    assert_eq!(two.len(), 2, "{two:?}");
    assert!(
        two[0].x < two[1].x && (two[0].y - two[1].y).abs() < 1.,
        "{two:?}"
    );
    assert!(
        (two[0].width - two[1].width).abs() < 2.,
        "even halves: {two:?}"
    );

    app.run("split_down");
    let three = app.panes();
    assert_eq!(three.len(), 3, "{three:?}");
    // The right half is now two panes, one above the other.
    assert!(
        (three[1].x - three[2].x).abs() < 1. && three[1].y < three[2].y,
        "{three:?}"
    );

    app.run("close_pane");
    assert_eq!(app.panes().len(), 2);
    app.run("close_pane");
    assert_eq!(app.panes().len(), 1);
    assert_eq!(
        app.panes()[0].width,
        one[0].width,
        "one pane fills the tab again"
    );
}

#[test]
fn tabs_open_switch_and_closing_the_last_pane_closes_the_window() {
    let mut app = App::new();
    app.run("new_tab");
    app.run("split_right");
    assert_eq!(app.panes().len(), 2, "the new tab shows");
    app.run("tab_1");
    assert_eq!(app.panes().len(), 1, "the first tab has one pane");
    app.run("next_tab");
    assert_eq!(app.panes().len(), 2);
    // Close everything: the second tab's panes, then the first tab's.
    app.run("close_tab");
    assert_eq!(app.panes().len(), 1);
    app.run("close_pane");
    assert!(app.actions.borrow().iter().any(|a| a == "close_window"));
}

#[test]
fn pane_navigation_runs_without_changing_the_layout() {
    let mut app = App::new();
    app.run("split_right");
    app.run("split_down");
    let before = app.panes();
    for id in [
        "focus_left",
        "focus_right",
        "focus_up",
        "focus_down",
        "previous_pane",
        "next_pane",
        "zoom_pane",
    ] {
        app.run(id);
        assert_eq!(app.h.host.error().get(), None, "{id}");
    }
    // Zoomed: the focused pane alone; again: the split back.
    assert_eq!(app.panes().len(), 1);
    app.run("zoom_pane");
    assert_eq!(app.panes(), before);
}

#[test]
fn dragging_a_pane_by_its_grip_previews_and_moves_it() {
    use zgui::input::{InputEvent, PointerButton};
    let mut app = App::new();
    app.run("split_right");
    let before = app.panes();
    let (left, right) = (before[0], before[1]);
    // The pointer near the left pane's top shows its grip.
    let node = app
        .h
        .natives
        .nodes()
        .into_iter()
        .find(|n| app.h.ui.scene.borrow().bounds(*n) == left)
        .unwrap();
    let Some(zgui::native_view::NativeContent::Terminal(term)) = app.h.natives.content(node) else {
        panic!("no terminal")
    };
    term.set_pointer(Some((left.width / 2., 10.)));
    app.settle();
    // The grip: centered, 6 points below the pane's top.
    let (gx, gy) = (left.x + left.width / 2., left.y + 1. + 6. + 11.);
    app.h.ui.dispatch(InputEvent::PointerMove { x: gx, y: gy });
    app.h.ui.dispatch(InputEvent::PointerDown {
        x: gx,
        y: gy,
        button: PointerButton::Primary,
    });
    // Over the right pane, nearest its bottom edge.
    let (tx, ty) = (right.x + right.width / 2., right.y + right.height - 20.);
    for step in 1..=8 {
        let t = step as f32 / 8.;
        app.h.ui.dispatch(InputEvent::PointerMove {
            x: gx + (tx - gx) * t,
            y: gy + (ty - gy) * t,
        });
    }
    app.settle();
    // The preview: the left pane moved under the right one, so the two
    // stack, each the full width.
    let preview = app.panes();
    assert_eq!(preview.len(), 2, "{preview:?}");
    assert!(
        preview.iter().all(|r| (r.x - left.x).abs() < 1.)
            && preview.iter().all(|r| r.width > left.width + right.width),
        "previewed stacked: {preview:?}"
    );
    app.h.ui.dispatch(InputEvent::PointerUp {
        x: tx,
        y: ty,
        button: PointerButton::Primary,
    });
    app.settle();
    // Dropped: kept as previewed, the dragged pane below the other.
    assert_eq!(app.panes(), preview);
    assert!(preview[0].y < preview[1].y && preview[0].height > 100.);
    assert_eq!(app.h.host.error().get(), None);
}

impl App {
    /// Every text the window shows.
    fn texts(&mut self) -> Vec<String> {
        use zgui::scene::NodeKind;
        self.h.ui.prepare_frame();
        let scene = self.h.ui.scene.borrow();
        let mut out = Vec::new();
        let mut stack = vec![scene.root()];
        while let Some(n) = stack.pop() {
            if let NodeKind::Text { text, .. } = scene.kind(n) {
                out.push(text.to_string());
            }
            stack.extend(scene.children(n).iter().copied());
        }
        out
    }
}

#[test]
fn a_tab_shows_an_icon_per_pane_and_counts_past_three() {
    let mut app = App::new();
    app.run("split_right");
    app.run("split_down");
    assert!(
        !app.texts().iter().any(|t| t.starts_with('+')),
        "three panes, three icons"
    );
    app.run("split_down");
    assert_eq!(app.panes().len(), 4);
    assert!(
        app.texts().iter().any(|t| t == "+2"),
        "two icons and +2: {:?}",
        app.texts()
    );
}

#[test]
fn dropping_a_pane_on_the_tab_bar_opens_it_as_a_tab_where_previewed() {
    use zgui::input::{InputEvent, PointerButton};
    let mut app = App::new();
    app.run("split_right");
    let left = app.panes()[0];
    let node = app
        .h
        .natives
        .nodes()
        .into_iter()
        .find(|n| app.h.ui.scene.borrow().bounds(*n) == left)
        .unwrap();
    let Some(zgui::native_view::NativeContent::Terminal(term)) = app.h.natives.content(node) else {
        panic!("no terminal")
    };
    term.set_pointer(Some((left.width / 2., 10.)));
    app.settle();
    let named = |app: &mut App| app.texts().iter().filter(|t| *t == "Terminal").count();
    let (gx, gy) = (left.x + left.width / 2., left.y + 1. + 6. + 11.);
    app.h.ui.dispatch(InputEvent::PointerMove { x: gx, y: gy });
    app.h.ui.dispatch(InputEvent::PointerDown {
        x: gx,
        y: gy,
        button: PointerButton::Primary,
    });
    app.h
        .ui
        .dispatch(InputEvent::PointerMove { x: gx, y: gy + 20. });
    app.settle();
    // Dragging: the tab's name and the preview's.
    let dragging = named(&mut app);
    // Up to the empty titlebar, right of the tabs: a slot opens at the end.
    let (tx, ty) = (300., 20.);
    for step in 1..=8 {
        let t = step as f32 / 8.;
        app.h.ui.dispatch(InputEvent::PointerMove {
            x: gx + (tx - gx) * t,
            y: gy + (ty - gy) * t,
        });
    }
    app.settle();
    assert_eq!(
        named(&mut app),
        dragging + 1,
        "a slot previews the new tab: {:?}",
        app.texts()
    );
    app.h.ui.dispatch(InputEvent::PointerUp {
        x: tx,
        y: ty,
        button: PointerButton::Primary,
    });
    app.settle();
    assert_eq!(app.panes().len(), 1, "the new tab, with the pane");
    app.run("tab_1");
    assert_eq!(app.panes().len(), 1, "the first tab, without it");
    assert_eq!(app.h.host.error().get(), None);
}
