//! The frame profiler example: mounts, opens every menu, and comes to rest.
mod common;
use common::Harness;
use std::path::Path;
use zgui::input::{InputEvent, Key, Modifiers};
use zgui_script::ScriptHost;

fn profiler() -> Harness {
    let h = Harness::new(|runtime| {
        let host = ScriptHost::new(
            runtime,
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/frame_profiler/app.luau"),
        );
        host.action("drag_window", |_| {});
        host
    });
    h.ui.scene.borrow_mut().resize(1132., 1028.);
    h.ui.prepare_frame();
    h.settle();
    h
}
fn frames(h: &Harness, count: usize) {
    for _ in 0..count {
        h.frame();
    }
}
fn healthy(h: &Harness) {
    if let Some(e) = h.host.error().with_untracked(|e| e.clone()) {
        panic!("script error: {e}");
    }
}
fn key(h: &mut Harness, key: Key) {
    h.ui.dispatch(InputEvent::KeyDown {
        key,
        modifiers: Modifiers::default(),
        repeat: false,
    });
    h.settle();
}

/// Writes a software capture when FRAME_PROFILER_SNAPSHOT names a file.
fn snapshot(h: &Harness, suffix: &str) {
    let Some(path) = std::env::var_os("FRAME_PROFILER_SNAPSHOT") else {
        return;
    };
    let path = format!("{}{suffix}.ppm", path.to_string_lossy());
    h.ui.prepare_frame();
    let mut raster = zgui_desktop::raster::Raster::new(1132, 1028);
    for bytes in [
        &include_bytes!("../../zgui-desktop/examples/support/fonts/GeistVariable.ttf")[..],
        &include_bytes!("../../zgui-desktop/examples/support/fonts/GeistMonoVariable.ttf")[..],
    ] {
        raster.register_font(&zgui_gpu::text::FontData::new(bytes).unwrap());
    }
    raster.render(
        &h.ui.scene.borrow(),
        &[zgui::scene::Rect::new(0., 0., 1132., 1028.)],
    );
    raster.write_ppm(Path::new(&path)).unwrap();
}

#[test]
fn mounts_and_opens_every_menu() {
    let mut h = profiler();
    healthy(&h);
    frames(&h, 90);
    healthy(&h);
    assert!(h.find("profiler").is_some());
    assert_eq!(h.text("frame_title"), "Frame 48,213");
    snapshot(&h, "");
    for (button, panel) in [
        ("more_button", "more_menu"),
        ("overlays_button", "overlays_menu"),
        ("target_button", "target_menu"),
        ("search_button", "search_menu"),
        ("budget", "budget_menu"),
        ("fit_frame", "zoom_menu"),
        ("status_logs", "logs_menu"),
        ("status_buffer", "buffer_menu"),
    ] {
        h.click(button);
        frames(&h, 30);
        healthy(&h);
        assert!(h.find(panel).is_some(), "{button} opens {panel}");
        snapshot(&h, &format!("_{panel}"));
        key(&mut h, Key::Escape);
        frames(&h, 30);
        healthy(&h);
        assert!(h.find(panel).is_none(), "Escape closes {panel}");
    }
}

#[test]
fn paused_profiler_rests() {
    let mut h = profiler();
    frames(&h, 30);
    h.click("record_toggle");
    frames(&h, 120);
    healthy(&h);
    assert!(!h.clock.wants_frame(), "paused, nothing animates");
    // Resume, then clear: the stopped pill offers Record and rests too.
    h.click("record_toggle");
    frames(&h, 30);
    h.click("more_button");
    frames(&h, 30);
    h.click("more_item_7");
    frames(&h, 120);
    healthy(&h);
    assert!(
        h.find("more_menu").is_none(),
        "Clear capture closes the menu"
    );
    assert!(!h.clock.wants_frame(), "stopped, nothing animates");
}

#[test]
fn span_context_menu_zooms_to_the_span() {
    use zgui::input::PointerButton;
    let mut h = profiler();
    frames(&h, 30);
    let node = h.find("span_glyphs").unwrap();
    let b = h.ui.scene.borrow().bounds(node);
    let (x, y) = (b.x + b.width / 2., b.y + b.height / 2.);
    h.ui.dispatch(InputEvent::PointerDown {
        x,
        y,
        button: PointerButton::Secondary,
    });
    h.ui.dispatch(InputEvent::PointerUp {
        x,
        y,
        button: PointerButton::Secondary,
    });
    frames(&h, 30);
    healthy(&h);
    assert!(
        h.find("span_menu").is_some(),
        "right-click opens the span menu"
    );
    h.click("span_menu_item_1");
    frames(&h, 60);
    healthy(&h);
    let glyphs = h.find("span_glyphs").unwrap();
    let after = h.ui.scene.borrow().bounds(glyphs);
    assert!(
        after.width > b.width * 4.,
        "zoomed to the span: {b:?} → {after:?}"
    );
    assert_eq!(h.text("fit_frame"), "Fit selection");
}

#[test]
fn hovering_a_control_shows_its_tooltip_after_a_delay() {
    let mut h = profiler();
    frames(&h, 30);
    let node = h.find("search_button").unwrap();
    let b = h.ui.scene.borrow().bounds(node);
    h.ui.dispatch(InputEvent::PointerMove {
        x: b.x + b.width / 2.,
        y: b.y + b.height / 2.,
    });
    frames(&h, 12);
    assert!(h.find("tooltip_search").is_none(), "not before 500 ms");
    frames(&h, 40);
    healthy(&h);
    assert!(h.find("tooltip_search").is_some(), "shown after the delay");
    h.ui.dispatch(InputEvent::PointerMove { x: 5., y: 500. });
    frames(&h, 30);
    assert!(
        h.find("tooltip_search").is_none(),
        "leaves with the pointer"
    );
}

#[test]
fn runtime_settings_open_from_more_and_dropdowns_choose() {
    let mut h = profiler();
    frames(&h, 30);
    h.click("more_button");
    frames(&h, 30);
    h.click("more_item_6");
    frames(&h, 30);
    healthy(&h);
    assert!(h.find("runtime_menu").is_some(), "More › Runtime settings… opens it");
    // A dropdown opens its options and closes on a choice.
    h.click("backend_select");
    frames(&h, 30);
    assert!(h.find("backend_select_list").is_some(), "the dropdown opens");
    h.click("backend_select_option_2");
    frames(&h, 30);
    healthy(&h);
    assert!(h.find("backend_select_list").is_none(), "a choice closes it");
    assert_eq!(h.text("backend_select"), "wgpu · Vulkan");
    assert!(h.find("runtime_menu").is_some(), "the popover stays open");
}

#[test]
fn capture_options_arm_a_timed_capture() {
    let mut h = profiler();
    frames(&h, 30);
    // Clear: the stopped control offers Record and its options.
    h.click("more_button");
    frames(&h, 30);
    h.click("more_item_7");
    frames(&h, 60);
    h.click("record_toggle");
    frames(&h, 30);
    healthy(&h);
    assert!(h.find("capture_menu").is_some(), "the chevron opens capture options");
    h.click("capture_menu_item_1"); // 5 seconds
    h.click("capture_menu_item_6"); // after a 3 s countdown
    frames(&h, 10);
    key(&mut h, Key::Escape);
    frames(&h, 30);
    h.click("record_start");
    frames(&h, 30);
    healthy(&h);
    assert!(h.text("record").starts_with("starts in"), "counting down: {}", h.text("record"));
    frames(&h, 60 * 4);
    healthy(&h);
    assert!(h.text("record").contains("/ 5 s"), "capturing: {}", h.text("record"));
    frames(&h, 60 * 6);
    assert!(h.text("record").contains(':'), "paused after the capture: {}", h.text("record"));
}

#[test]
fn record_control_previews_every_state_and_morphs_in_place() {
    let mut h = profiler();
    frames(&h, 30);
    for (i, want) in ["", "", "Record", "starts in", "on first input", "/ 5 s"].iter().enumerate() {
        h.ui.dispatch(InputEvent::PointerDown {
            x: 0.,
            y: 0.,
            button: zgui::input::PointerButton::Primary,
        });
        let node = h.find("record").unwrap();
        let b = h.ui.scene.borrow().bounds(node);
        let (x, y) = (b.x + 12., b.y + b.height / 2.);
        for down in [true, false] {
            let button = zgui::input::PointerButton::Secondary;
            h.ui.dispatch(if down {
                InputEvent::PointerDown { x, y, button }
            } else {
                InputEvent::PointerUp { x, y, button }
            });
        }
        frames(&h, 20);
        healthy(&h);
        assert!(h.find("rec_states_menu").is_some(), "right-click opens the state menu");
        h.click(&format!("rec_states_menu_item_{}", i + 1));
        frames(&h, 40);
        healthy(&h);
        // One button throughout: the same node, resized, not replaced.
        assert_eq!(h.find("record"), Some(node), "state {i} keeps the button");
        let text = h.text("record");
        assert!(text.contains(want), "state {i}: {text}");
    }
}

#[test]
fn record_tooltip_stays_and_changes_with_the_state() {
    use zgui::input::PointerButton;
    let mut h = profiler();
    frames(&h, 30);
    let node = h.find("record_toggle").unwrap();
    let b = h.ui.scene.borrow().bounds(node);
    let (x, y) = (b.x + b.width / 2., b.y + b.height / 2.);
    h.ui.dispatch(InputEvent::PointerMove { x, y });
    frames(&h, 45);
    assert_eq!(h.text("tooltip_record"), "Stop recording");
    // Pressed, it stays open and says what a press does now.
    h.ui.dispatch(InputEvent::PointerDown { x, y, button: PointerButton::Primary });
    h.ui.dispatch(InputEvent::PointerUp { x, y, button: PointerButton::Primary });
    frames(&h, 2);
    healthy(&h);
    assert!(h.find("tooltip_record").is_some(), "the tooltip stays through the press");
    frames(&h, 20);
    assert_eq!(h.text("tooltip_record"), "Resume recording");
}

/// Every text under node `id`, joined with spaces.
fn all_text(h: &mut Harness, id: &str) -> String {
    let node = h.find(id).unwrap_or_else(|| panic!("no node `{id}`"));
    let scene = h.ui.scene.borrow();
    let (mut stack, mut out) = (vec![node], Vec::new());
    while let Some(n) = stack.pop() {
        if let zgui::scene::NodeKind::Text { text, .. } = scene.kind(n) {
            out.push(text.to_string());
        }
        stack.extend(scene.children(n).iter().rev());
    }
    out.join(" ")
}

#[test]
fn areas_tab_expands_selects_and_filters() {
    let mut h = profiler();
    frames(&h, 30);
    h.click("tabs_2");
    frames(&h, 60);
    healthy(&h);
    assert_eq!(h.text("areas_detail_title"), "Toasts");
    assert!(h.find("areas_row_retry").is_some(), "toast_3 starts open");
    snapshot(&h, "_areas");
    // Status orbs starts collapsed: its chevron opens it.
    assert!(h.find("areas_row_orb_thinking").is_none());
    h.click("areas_toggle_orbs");
    frames(&h, 30);
    healthy(&h);
    assert!(h.find("areas_row_orb_thinking").is_some(), "the chevron opens the row");
    // A click selects a row and the detail follows it.
    h.click("areas_row_palette");
    frames(&h, 30);
    healthy(&h);
    assert_eq!(h.text("areas_detail_title"), "Command palette");
    assert!(all_text(&mut h, "areas_detail").contains("results list grows"));
    assert!(h.find("areas_layer").is_none(), "no isolated layer");
    snapshot(&h, "_areas_palette");
    // The scope switches the values; a finding's link selects its row.
    let before = all_text(&mut h, "areas_row_toasts");
    h.click("areas_scope_1");
    frames(&h, 30);
    let after = all_text(&mut h, "areas_row_toasts");
    assert!(before.contains("53") && after.contains("61"), "1 s avg {before} → Frame {after}");
    h.click("areas_finding_3_link");
    frames(&h, 30);
    healthy(&h);
    assert_eq!(h.text("areas_detail_title"), "Status orbs");
    // Collapsing hides the children.
    h.click("areas_toggle_toasts");
    frames(&h, 30);
    assert!(h.find("areas_row_toast_3").is_none(), "collapsed");
    assert!(h.find("areas_row_toasts").is_some());
    // The filter keeps matches and their ancestors, opened; keys typed
    // into it stay out of the window's shortcuts.
    h.click("areas_filter_input");
    h.ui.dispatch(InputEvent::Text("slider_".into()));
    key(&mut h, Key::Space);
    key(&mut h, Key::ArrowLeft);
    h.ui.dispatch(InputEvent::Text("v".into()));
    key(&mut h, Key::ArrowRight);
    key(&mut h, Key::Backspace);
    assert_eq!(h.text("areas_filter_input"), "sliderv", "←/→ move the caret");
    key(&mut h, Key::Backspace);
    h.ui.dispatch(InputEvent::Text("_v".into()));
    frames(&h, 30);
    healthy(&h);
    assert_eq!(h.text("areas_filter_input"), "slider_v");
    let clock = h.text("record");
    frames(&h, 30);
    assert_ne!(h.text("record"), clock, "space typed into the filter, still recording");
    assert_eq!(h.text("frame_title"), "Frame 48,213", "←/→ left the Timeline's frame");
    assert!(h.find("areas_row_volume").is_some(), "a child id matches");
    assert!(h.find("areas_row_sliders").is_some(), "its parent stays");
    assert!(h.find("areas_row_toasts").is_none(), "others leave");
    snapshot(&h, "_areas_filtered");
}

#[cfg(unix)]
#[path = "../examples/frame_profiler/live.rs"]
#[allow(dead_code)]
mod live;

/// Frames streamed by a `ZGUI_PROFILE=1` app, through the example's own
/// parser, replace the sample data in the chart, trace and counters.
#[cfg(unix)]
#[test]
fn live_frames_drive_the_timeline() {
    let mut h = profiler();
    let shared = h.host.shared("live", false);
    let mut feed = live::Live::offline();
    let mut value = feed.read(4242, "P pid=4242 name=controls_lab");
    // A full 30 s buffer at 120 Hz, the most the feed keeps.
    for n in 1..=3600 {
        let update = if n == 3577 { 9.0 } else { 0.4 };
        value = feed.read(
            4242,
            &format!(
                "F win=1 n={n} t={:.3} input=0.1 update={update} layout=1.5 paint=0.3 render=2.0 \
                 present=0.5 wait=4.0 gpu=1.2 draws=40 passes=1 blur=0 instances=300 shaped=0 \
                 glyphs=0 repaints=0 hits=1 damage=100000 nodes=12",
                n as f64 * 8.33
            ),
        );
    }
    shared.set(value);
    frames(&h, 60);
    healthy(&h);
    // The selected bar, 97th of the last 120, is frame 3,577.
    assert_eq!(h.text("frame_title"), "Frame 3,577");
    assert_eq!(h.text("badge_cpu"), "CPU 13.40 ms");
    assert_eq!(h.text("badge_gpu"), "GPU 1.20 ms");
    assert_eq!(h.text("target_button"), "controls_lab");
    snapshot(&h, "_live");
}
