mod common;
use common::Harness;
use std::path::Path;
use zgx::input::{InputEvent, Key, Modifiers, PointerButton};
use zgx_script::ScriptHost;

fn navbar() -> Harness {
    let h = Harness::new(|runtime| {
        ScriptHost::new(
            runtime,
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/navbar_lab/app.luau"),
        )
    });
    h.ui.scene.borrow_mut().resize(1180., 1000.);
    h.ui.prepare_frame();
    h.settle();
    frames(&h, 10);
    h
}
fn frames(h: &Harness, count: usize) {
    for _ in 0..count {
        h.frame();
    }
}
/// Frames for `ms` of the 60 Hz clock.
fn ms(h: &Harness, ms: usize) {
    frames(h, ms.div_ceil(16));
}
fn healthy(h: &Harness) {
    assert_eq!(h.host.error().get(), None);
}
/// The lab's readout field, e.g. `field(h, "mode")` is "search".
fn field(h: &mut Harness, name: &str) -> String {
    let state = h.text("nb_state");
    let start = state
        .find(&format!("{name}:"))
        .unwrap_or_else(|| panic!("no `{name}` in {state}"))
        + name.len()
        + 1;
    state[start..].split(" · ").next().unwrap().to_owned()
}
fn key(h: &mut Harness, key: Key) {
    h.ui.dispatch(InputEvent::KeyDown {
        key: key.clone(),
        modifiers: Modifiers::default(),
        repeat: false,
    });
    h.ui.dispatch(InputEvent::KeyUp {
        key,
        modifiers: Modifiers::default(),
    });
    h.settle();
    frames(h, 1);
}
fn typing(h: &mut Harness, text: &str) {
    for c in text.chars() {
        key(h, Key::Character(c.to_string()));
        h.ui.dispatch(InputEvent::Text(c.to_string()));
        h.settle();
        frames(h, 1);
    }
}
fn center(h: &mut Harness, id: &str) -> (f32, f32) {
    let node = h.find(id).unwrap_or_else(|| panic!("no node `{id}`"));
    let b = h.ui.scene.borrow().bounds(node);
    (b.x + b.width / 2., b.y + b.height / 2.)
}
fn click_at(h: &mut Harness, x: f32, y: f32) {
    h.ui.dispatch(InputEvent::PointerMove { x, y });
    h.ui.dispatch(InputEvent::PointerDown {
        x,
        y,
        button: PointerButton::Primary,
    });
    h.ui.dispatch(InputEvent::PointerUp {
        x,
        y,
        button: PointerButton::Primary,
    });
    h.settle();
}
/// Every text under `id`, in order, joined by spaces.
fn texts(h: &mut Harness, id: &str) -> String {
    let node = h.find(id).unwrap_or_else(|| panic!("no node `{id}`"));
    let scene = h.ui.scene.borrow();
    let mut out = Vec::new();
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        if let zgx::scene::NodeKind::Text { text, .. } = scene.kind(n) {
            out.push(text.to_string());
        }
        stack.extend(scene.children(n).iter().rev());
    }
    out.join(" ")
}
fn editor_text(h: &Harness) -> (String, std::ops::Range<usize>) {
    let editor = h.ui.focused_editor().expect("a focused editor");
    let e = editor.editor.borrow();
    (e.text().to_owned(), e.selection().range())
}
fn open_address(h: &mut Harness) {
    h.host.run_command("focus_address");
    h.settle();
    ms(h, 400);
}

#[test]
fn mounts_where_the_source_measures_and_idles_at_rest() {
    let mut h = navbar();
    healthy(&h);
    assert_eq!(
        h.text("nb_state"),
        "focused:false · mode:search · collapsed:false · hover:false · popover:– · scroll:0 · progress:0.00"
    );
    // Ask's motion lab at 1180pt: the bar centred, 762 wide, 8 from the
    // window's top; the field in its 600pt slot.
    let (win, nav, field_node) = (
        h.find("nb_win_frame").unwrap(),
        h.find("nb_nav").unwrap(),
        h.find("nb_field").unwrap(),
    );
    let scene = h.ui.scene.borrow();
    let (w, n, f) = (
        scene.bounds(win),
        scene.bounds(nav),
        scene.bounds(field_node),
    );
    assert_eq!(
        (n.x - w.x - 1., n.y - w.y - 1., n.width, n.height),
        (184., 8., 762., 40.)
    );
    assert_eq!(
        (f.x - w.x - 1., f.y - w.y - 1., f.width, f.height),
        (278., 14., 600., 28.)
    );
    drop(scene);
    frames(&h, 60);
    assert!(
        !h.clock.wants_frame(),
        "a resting page must not request frames"
    );
}

#[test]
fn address_opens_completes_and_navigates() {
    let mut h = navbar();
    open_address(&mut h);
    assert_eq!(field(&mut h, "focused"), "true");
    // The current address arrives selected.
    let (text, selection) = editor_text(&h);
    assert_eq!(text, "https://www.theverge.com/tech/ask-browser-review");
    assert_eq!(selection, 0..text.len());
    // A prefix completes inline to the first matching tab, its suffix selected.
    typing(&mut h, "gi");
    let (text, selection) = editor_text(&h);
    assert_eq!(text, "github.com/ask-browser/ask/pull/412");
    assert_eq!(selection, 2..text.len());
    // Arrows move the row selection, not the caret.
    key(&mut h, Key::ArrowDown);
    key(&mut h, Key::ArrowUp);
    assert_eq!(editor_text(&h).1, 2..text.len());
    key(&mut h, Key::Enter);
    ms(&h, 2600);
    assert_eq!(field(&mut h, "focused"), "false");
    assert!(
        texts(&mut h, "nb_identity").contains("ask-browser 412")
            || texts(&mut h, "nb_identity").contains("github.com")
    );
    assert_eq!(
        field(&mut h, "progress"),
        "0.00",
        "the load finished and its wash settled"
    );
    healthy(&h);
}

#[test]
fn tab_switches_to_ask_and_escape_returns_to_search() {
    let mut h = navbar();
    open_address(&mut h);
    key(&mut h, Key::Tab);
    ms(&h, 400);
    assert_eq!(field(&mut h, "mode"), "ask");
    assert_eq!(h.ui.input.focused(), h.find("nb_prompt"));
    key(&mut h, Key::Escape);
    ms(&h, 400);
    assert_eq!(field(&mut h, "mode"), "search");
    assert_eq!(h.ui.input.focused(), h.find("nb_input"));
    key(&mut h, Key::Escape);
    ms(&h, 400);
    assert_eq!(field(&mut h, "focused"), "false");
}

#[test]
fn scrolling_collapses_to_the_pill_and_a_click_expands_it() {
    let mut h = navbar();
    let (x, y) = center(&mut h, "nb_win_frame");
    for _ in 0..2 {
        h.ui.dispatch(InputEvent::Scroll {
            x,
            y: y + 200.,
            delta_x: 0.,
            delta_y: 300.,
        });
        h.settle();
        frames(&h, 2);
    }
    ms(&h, 600);
    assert_eq!(field(&mut h, "collapsed"), "true");
    let nav = h.find("nb_nav").unwrap();
    assert_eq!(h.ui.scene.borrow().bounds(nav).height, 30.);
    let (px, py) = center(&mut h, "nb_nav");
    click_at(&mut h, px, py);
    ms(&h, 600);
    assert_eq!(field(&mut h, "collapsed"), "false");
    assert_eq!(h.ui.scene.borrow().bounds(nav).width, 762.);
}

#[test]
fn dragging_the_bar_docks_it_left() {
    let mut h = navbar();
    let frame = h.find("nb_win_frame").unwrap();
    let left = h.ui.scene.borrow().bounds(frame).x + 1.;
    let nav = h.find("nb_nav").unwrap();
    let b = h.ui.scene.borrow().bounds(nav);
    // The bar's empty edge, left of the history buttons.
    let (x, y) = (b.x + 3., b.y + 20.);
    h.ui.dispatch(InputEvent::PointerMove { x, y });
    h.ui.dispatch(InputEvent::PointerDown {
        x,
        y,
        button: PointerButton::Primary,
    });
    for i in 1..=20 {
        h.ui.dispatch(InputEvent::PointerMove {
            x: x - i as f32 * 20.,
            y,
        });
        h.settle();
        frames(&h, 1);
    }
    h.ui.dispatch(InputEvent::PointerUp {
        x: x - 400.,
        y,
        button: PointerButton::Primary,
    });
    h.settle();
    ms(&h, 900);
    let b = h.ui.scene.borrow().bounds(nav);
    assert!(
        (b.x - left - 8.).abs() < 0.5,
        "docked 8 from the left edge, at {}",
        b.x - left
    );
    assert_eq!(field(&mut h, "collapsed"), "true");
}

#[test]
fn popovers_open_close_and_set_permissions() {
    let mut h = navbar();
    for (button, name, probe) in [
        ("nb_logo", "site", "nb_read_pages"),
        ("nb_ext", "extensions", "nb_manage_ext"),
        ("nb_star", "bookmark", "nb_bm_done"),
    ] {
        h.click(button);
        ms(&h, 300);
        assert_eq!(field(&mut h, "popover"), name);
        assert!(h.find(probe).is_some(), "{name} shows {probe}");
        // The page beside the popover: the backdrop takes the press.
        let frame = h.find("nb_win_frame").unwrap();
        let b = h.ui.scene.borrow().bounds(frame);
        click_at(&mut h, b.x + 60., b.y + b.height - 60.);
        ms(&h, 300);
        assert_eq!(field(&mut h, "popover"), "–");
    }
    h.click("nb_logo");
    ms(&h, 300);
    h.click("nb_choice_Camera");
    ms(&h, 200);
    h.click("nb_set_Camera_block");
    ms(&h, 200);
    assert!(texts(&mut h, "nb_choice_Camera").contains("Block"));
    healthy(&h);
}

#[test]
fn cookies_arrive_in_their_popover() {
    let mut h = navbar();
    h.click("nb_logo");
    ms(&h, 300);
    h.click("nb_site_data");
    ms(&h, 1000);
    assert_eq!(field(&mut h, "popover"), "data");
    assert!(texts(&mut h, "nb_data").contains("5 cookies"));
}

/// Frame cost while the bar animates: script bindings, layout and the scene
/// flush (no GPU), per 60 Hz frame. Release builds only measure meaningfully.
#[test]
fn animating_frames_stay_cheap() {
    let mut h = navbar();
    let time = |h: &mut Harness, label: &str, act: &dyn Fn(&mut Harness), count: usize| {
        act(h);
        let start = std::time::Instant::now();
        frames(h, count);
        let per = start.elapsed().as_secs_f64() * 1000. / count as f64;
        println!("{label}: {per:.3} ms/frame");
        per
    };
    let mut worst: f64 = 0.;
    worst = worst.max(time(
        &mut h,
        "open",
        &|h| {
            h.host.run_command("focus_address");
            h.settle();
        },
        20,
    ));
    worst = worst.max(time(&mut h, "ask", &|h| key(h, Key::Tab), 20));
    worst = worst.max(time(
        &mut h,
        "close",
        &|h| {
            key(h, Key::Escape);
            key(h, Key::Escape);
        },
        20,
    ));
    let (x, y) = center(&mut h, "nb_win_frame");
    worst = worst.max(time(
        &mut h,
        "scroll",
        &move |h| {
            for _ in 0..3 {
                h.ui.dispatch(InputEvent::Scroll {
                    x,
                    y: y + 200.,
                    delta_x: 0.,
                    delta_y: 200.,
                });
                h.settle();
            }
        },
        30,
    ));
    worst = worst.max(time(&mut h, "load", &|h| h.click("nb_site_3"), 60));
    if !cfg!(debug_assertions) {
        assert!(worst < 4., "a frame took {worst:.2} ms");
    }
}
