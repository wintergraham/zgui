mod common;
use common::Harness;
use std::path::Path;
use zgui::{
    input::{InputEvent, Key, Modifiers},
    scene::{Color, NodeKind},
};
use zgui_script::ScriptHost;

fn controls() -> Harness {
    let h = Harness::new(|runtime| {
        ScriptHost::new(
            runtime,
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/controls_lab/app.luau"),
        )
    });
    h.ui.scene.borrow_mut().resize(1180., 12000.);
    h.ui.prepare_frame();
    h.settle();
    h
}

#[test]
fn mounts_controls_page() {
    let mut h = controls();
    assert!(h.find("controls_lab").is_some());
    assert!(h.find("save").is_some());
    assert_eq!(h.text("model_value"), "Grok 4.7 High");
    for id in [
        "buttons",
        "fields",
        "segmented",
        "selection",
        "sliders",
        "progress",
        "palette",
        "selects",
        "shortcut",
        "tints",
        "orbs",
        "modal",
        "links",
        "badges",
        "tooltips",
        "settings",
        "dropdowns",
        "menu",
        "alerts",
        "preview",
        "nav",
        "dialog",
        "toasts",
    ] {
        assert!(h.find(&format!("card_{id}")).is_some(), "missing {id}");
    }
    frames(&h, 100);
    healthy(&h);
    idle_viewport(&h);
    assert!(
        !h.clock.wants_frame(),
        "resting page must stop requesting frames"
    );
    if let Some(path) = std::env::var_os("CONTROLS_LAB_SNAPSHOT") {
        h.ui.scene.borrow_mut().resize(1180., 900.);
        h.ui.prepare_frame();
        let mut raster = zgui_desktop::raster::Raster::new(1180, 900);
        for bytes in [
            &include_bytes!("../examples/controls_lab/assets/Geist.ttf")[..],
            &include_bytes!("../examples/controls_lab/assets/GeistMono.ttf")[..],
        ] {
            raster.register_font(&zgui_gpu::text::FontData::new(bytes).unwrap());
        }
        raster.render(
            &h.ui.scene.borrow(),
            &[zgui::scene::Rect::new(0., 0., 1180., 900.)],
        );
        raster.write_ppm(Path::new(&path)).unwrap();
    }
}

fn idle_viewport(h: &Harness) {
    h.ui.scene.borrow_mut().resize(1180., 900.);
    h.ui.prepare_frame();
    h.settle();
    frames(h, 100);
}
fn frames(h: &Harness, count: usize) {
    for _ in 0..count {
        h.frame();
    }
}
fn healthy(h: &Harness) {
    assert_eq!(h.host.error().get(), None);
}
fn key(h: &mut Harness, key: Key, modifiers: Modifiers) {
    h.ui.dispatch(InputEvent::KeyDown {
        key,
        modifiers,
        repeat: false,
    });
    h.settle();
}
fn fill(h: &mut Harness, id: &str) -> Color {
    let node = h.find(id).unwrap();
    match h.ui.scene.borrow().kind(node) {
        NodeKind::Panel { quad, .. } | NodeKind::Quad(quad) => quad.fill,
        other => panic!("{id}: {other:?}"),
    }
}

#[test]
fn buttons_fields_switches_and_checkboxes_update_in_place() {
    let mut h = controls();
    frames(&h, 100);
    let root = h.find("controls_lab");
    h.click("save");
    frames(&h, 15);
    assert_eq!(h.text("save_label"), "Working");
    frames(&h, 110);
    assert_eq!(h.text("save_label"), "Saved");
    frames(&h, 100);
    assert_eq!(h.text("save_label"), "Save");
    h.click("search_input");
    h.ui.dispatch(InputEvent::Text("Lisbon".into()));
    h.settle();
    assert!(h.find("clear_search").is_some());
    h.click("clear_search");
    h.click("sync");
    assert_eq!(fill(&mut h, "sync"), Color(52, 52, 48, 255));
    h.click("all_sites");
    for id in ["site_1", "site_2", "site_3"] {
        assert_eq!(fill(&mut h, id), Color(237, 235, 229, 255));
    }
    h.click("site_2");
    healthy(&h);
    assert_eq!(fill(&mut h, "site_2"), Color(0, 0, 0, 0));
    h.click("nav_6");
    frames(&h, 100);
    let pill = h.find("nav_pill").unwrap();
    assert!((h.ui.scene.borrow().transform(pill).y - 170.).abs() < 0.1);
    assert_eq!(h.find("controls_lab"), root, "the page stays mounted");
    healthy(&h);
    idle_viewport(&h);
    assert!(!h.clock.wants_frame());
}

#[test]
fn menus_palette_shortcut_and_multi_select_are_local_and_live() {
    let mut h = controls();
    frames(&h, 100);
    h.click("model_select");
    frames(&h, 20);
    h.click("models_item_3");
    assert_eq!(h.text("model_value"), "Claude Sonnet 5.5");
    h.click("sharing_select");
    h.click("sharing_select_option_3");
    assert_eq!(h.text("sharing_select_label"), "3 selected");
    key(&mut h, Key::Escape, Modifiers::default());
    h.click("record_shortcut");
    key(
        &mut h,
        Key::Character("b".into()),
        Modifiers {
            meta: true,
            shift: true,
            ..Default::default()
        },
    );
    assert_eq!(h.text("shortcut_keys"), "⌘ ⇧ B");
    h.click("open_palette");
    h.click("palette_query");
    h.ui.dispatch(InputEvent::Text(">component".into()));
    h.settle();
    assert_eq!(h.text("palette_result_1"), "Components");
    assert!(h.find("palette_result_2").is_none());
    key(&mut h, Key::Enter, Modifiers::default());
    frames(&h, 100);
    healthy(&h);
    idle_viewport(&h);
    assert!(!h.clock.wants_frame());
}

#[test]
fn repeated_toast_bursts_are_bounded_and_expire_to_idle() {
    let mut h = controls();
    frames(&h, 100);
    let baseline = h.ui.scene.borrow().len();
    for _ in 0..12 {
        h.click("show_three");
        frames(&h, 3);
        healthy(&h);
        assert!(
            h.ui.scene.borrow().len() < baseline + 180,
            "toast nodes stay bounded"
        );
    }
    // Move away so hover cannot pause expiry.
    h.ui.dispatch(InputEvent::PointerMove { x: 1., y: 1. });
    frames(&h, 420);
    healthy(&h);
    assert_eq!(h.ui.scene.borrow().len(), baseline);
    idle_viewport(&h);
    assert!(!h.clock.wants_frame());
}

#[test]
fn responsive_layout_slider_keyboard_and_reduced_motion() {
    let mut h = controls();
    frames(&h, 100);
    assert_eq!(
        h.clock.demanded_rate(120.),
        Some(12.),
        "visible working orb uses 12 fps"
    );
    h.click("volume");
    let before = h.text("volume_readout");
    key(&mut h, Key::ArrowRight, Modifiers::default());
    assert_ne!(
        h.text("volume_readout"),
        before,
        "native slider keyboard input updates the authored track"
    );
    h.ui.scene.borrow_mut().resize(620., 12000.);
    frames(&h, 60);
    let first = h.find("card_segmented").unwrap();
    let second = h.find("card_selection").unwrap();
    let a = h.ui.scene.borrow().bounds(first);
    let b = h.ui.scene.borrow().bounds(second);
    assert!(
        a.width <= 572. && b.y >= a.y + a.height,
        "narrow window stacks the cards: {a:?} {b:?}"
    );
    h.click("reduce");
    frames(&h, 100);
    healthy(&h);
    assert!(!h.clock.wants_frame(), "reduce motion freezes active demos");
}
