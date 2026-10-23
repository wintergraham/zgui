//! The Settings example: every section mounts, settings change in place,
//! search jumps to a setting, menus and dialogs open and close, preferences
//! restore from what the host saved, and the page comes to rest.
mod common;
use common::Harness;
use std::path::Path;
use zgui::input::{InputEvent, Key, Modifiers};
use zgui_script::{ScriptHost, Value};

const W: f32 = 1280.;
const H: f32 = 820.;

fn host(runtime: &zgui::reactive::Runtime) -> ScriptHost {
    ScriptHost::new(
        runtime,
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/settings/app.luau"),
    )
}
/// A window tall enough that every setting is on screen, so clicks reach
/// rows below the fold without scrolling.
const TALL: f32 = 3600.;
fn open(h: Harness) -> Harness {
    h.ui.scene.borrow_mut().resize(W, TALL);
    h.ui.prepare_frame();
    h.settle();
    frames(&h, 30);
    healthy(&h);
    h
}
fn settings() -> Harness {
    open(Harness::new(host))
}
/// Settings as the host restores them: `saved` is the stored model.
fn restored(saved: Value) -> Harness {
    open(Harness::new(|runtime| {
        let host = host(runtime);
        host.shared("settings", saved);
        host
    }))
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
fn typed(h: &mut Harness, text: &str) {
    h.ui.dispatch(InputEvent::Text(text.into()));
    h.settle();
}
fn go(h: &mut Harness, route: &str) {
    h.click(&format!("nav_{route}"));
    frames(h, 20);
    healthy(h);
}
/// What the script last saved for the host to store.
fn saved(h: &Harness) -> Value {
    h.host.shared("settings", Value::Nil).with_untracked(|v| v.clone())
}
fn field<'a>(v: &'a Value, path: &[&str]) -> &'a Value {
    path.iter()
        .fold(v, |v, k| v.field(k).unwrap_or_else(|| panic!("no `{k}` in {v}")))
}
fn map(fields: &[(&str, Value)]) -> Value {
    let mut fields: Vec<(std::rc::Rc<str>, Value)> = fields
        .iter()
        .map(|(k, v)| (std::rc::Rc::from(*k), v.clone()))
        .collect();
    fields.sort_by(|a, b| a.0.cmp(&b.0));
    Value::Map(fields.into())
}
/// Back to a quiet page with nothing animating.
fn rests(h: &mut Harness) {
    go(h, "general");
    h.ui.dispatch(InputEvent::PointerMove { x: 1., y: 1. });
    frames(h, 240);
    healthy(h);
    assert!(!h.clock.wants_frame(), "the page requests no frames at rest");
}
fn snapshot(h: &Harness, name: &str) {
    let Some(dir) = std::env::var_os("SETTINGS_SNAPSHOTS") else {
        return;
    };
    h.ui.scene.borrow_mut().resize(W, H);
    h.ui.prepare_frame();
    h.settle();
    let mut raster = zgui_desktop::raster::Raster::new(W as usize, H as usize);
    for bytes in [
        &include_bytes!("../examples/settings/assets/Geist.ttf")[..],
        &include_bytes!("../examples/settings/assets/GeistMono.ttf")[..],
    ] {
        raster.register_font(&zgui_gpu::text::FontData::new(bytes).unwrap());
    }
    raster.render(&h.ui.scene.borrow(), &[zgui::scene::Rect::new(0., 0., W, H)]);
    raster
        .write_ppm(&Path::new(&dir).join(format!("{name}.ppm")))
        .unwrap();
    h.ui.scene.borrow_mut().resize(W, TALL);
    h.ui.prepare_frame();
    h.settle();
}

const ROUTES: [&str; 13] = [
    "general",
    "appearance",
    "tabs",
    "assistant",
    "providers",
    "models",
    "confirmations",
    "passwords",
    "extensions",
    "downloads",
    "spaces",
    "about",
    "developer",
];

#[test]
fn every_section_mounts_and_the_page_rests() {
    let mut h = settings();
    assert!(h.find("nav").is_some());
    for route in ROUTES {
        go(&mut h, route);
        assert!(h.find("content").is_some(), "{route} has content");
        snapshot(&h, route);
    }
    rests(&mut h);
}

#[test]
fn every_section_mounts_when_the_page_opens_on_it() {
    for section in [
        "General",
        "Appearance",
        "Tabs & navigation",
        "Assistant",
        "Providers",
        "Models",
        "Confirmations",
        "Extensions",
        "Downloads",
        "About Ask",
    ] {
        let mut h = restored(map(&[("section", Value::Str(section.into()))]));
        assert!(h.find("content").is_some(), "{section} has content");
    }
}

#[test]
fn switches_flags_and_ai_features_save_and_restore() {
    let mut h = settings();
    h.click("ai_toggle");
    frames(&h, 10);
    assert_eq!(field(&saved(&h), &["ai"]), &Value::Bool(false));
    go(&mut h, "assistant");
    // The switch slides in above the settings it governs.
    assert!(h.find("ai_toggle_top").is_some());
    assert!(h.find("ai_toggle").is_none());
    h.click("ai_toggle_top");
    frames(&h, 10);
    assert_eq!(field(&saved(&h), &["ai"]), &Value::Bool(true));
    h.click("take_over");
    frames(&h, 10);
    assert_eq!(
        field(&saved(&h), &["flags", "Take over with Esc"]),
        &Value::Bool(false)
    );
    let store = saved(&h);
    let h = restored(store);
    assert_eq!(
        field(&saved(&h), &["flags", "Take over with Esc"]),
        &Value::Bool(false)
    );
    assert_eq!(
        field(&saved(&h), &["section"]),
        &Value::Str("Assistant".into())
    );
}

#[test]
fn menus_pick_values_and_close() {
    let mut h = settings();
    go(&mut h, "assistant");
    h.click("other_tabs");
    frames(&h, 15);
    h.click("other_tabs_option_3");
    frames(&h, 5);
    assert_eq!(h.text("other_tabs_value"), "Always");
    assert!(
        h.find("other_tabs_option_1").is_none(),
        "picking closes the menu"
    );
    // Multi-select stays open and counts.
    h.click("sources");
    frames(&h, 15);
    h.click("sources_option_2");
    frames(&h, 5);
    assert_eq!(h.text("sources_value"), "2 connected");
    assert!(
        h.find("sources_option_1").is_some(),
        "a multi-select stays open"
    );
    key(&mut h, Key::Escape);
    frames(&h, 5);
    assert!(h.find("sources_option_1").is_none(), "Escape closes it");
    h.click("manager");
    frames(&h, 15);
    h.click("manager_bitwarden");
    frames(&h, 5);
    assert_eq!(h.text("manager_value"), "Bitwarden");
    rests(&mut h);
}

#[test]
fn search_filters_opens_a_result_and_flashes_it() {
    let mut h = settings();
    h.click("search");
    typed(&mut h, "password");
    frames(&h, 10);
    assert!(h.find("result_1").is_some());
    key(&mut h, Key::Enter);
    frames(&h, 30);
    healthy(&h);
    assert_eq!(
        field(&saved(&h), &["section"]),
        &Value::Str("Assistant".into())
    );
    assert!(h.find("manager").is_some(), "jumped to the password manager");
    // Nothing matches: suggestions instead.
    h.click("search");
    typed(&mut h, "zzzz");
    frames(&h, 5);
    assert!(h.find("suggest_theme").is_some());
    h.click("suggest_theme");
    frames(&h, 5);
    assert!(h.find("result_1").is_some());
    h.click("search_clear");
    frames(&h, 5);
    assert!(h.find("result_1").is_none());
    rests(&mut h);
}

#[test]
fn search_engines_add_validate_and_remove() {
    let mut h = settings();
    h.click("engine");
    frames(&h, 15);
    h.click("engine_kagi");
    frames(&h, 5);
    assert_eq!(h.text("engine_value"), "Kagi");
    h.click("engine");
    frames(&h, 15);
    h.click("engine_add_open");
    frames(&h, 20);
    assert!(h.find("add_engine").is_some());
    h.click("engine_add");
    frames(&h, 5);
    assert_eq!(
        h.text("engine_error"),
        "Enter the full address, starting with https://"
    );
    h.click("engine_name");
    typed(&mut h, "Wikipedia");
    h.click("engine_url");
    typed(&mut h, "https://en.wikipedia.org/w/index.php?search=");
    frames(&h, 2);
    assert_eq!(h.text("engine_error"), "Put %s where the search terms go.");
    typed(&mut h, "%s");
    h.click("engine_add");
    frames(&h, 20);
    healthy(&h);
    assert!(h.find("add_engine").is_none(), "adding closes the dialog");
    assert_eq!(h.text("engine_value"), "Wikipedia");
    rests(&mut h);
}

#[test]
fn account_dialog_opens_and_closes() {
    let mut h = settings();
    h.click("account");
    frames(&h, 30);
    assert!(h.find("account_dialog").is_some());
    key(&mut h, Key::Escape);
    frames(&h, 20);
    assert!(h.find("account_dialog").is_none());
    h.click("account");
    frames(&h, 20);
    h.click("account_close");
    frames(&h, 20);
    assert!(h.find("account_dialog").is_none());
    rests(&mut h);
}

#[test]
fn appearance_theme_tint_and_dither() {
    let mut h = settings();
    go(&mut h, "appearance");
    h.click("theme_light");
    frames(&h, 30);
    healthy(&h);
    assert_eq!(field(&saved(&h), &["theme"]), &Value::Str("light".into()));
    h.click("tint_sage");
    frames(&h, 30);
    assert_eq!(field(&saved(&h), &["tint"]), &Value::Str("sage".into()));
    h.click("intensity_0");
    frames(&h, 10);
    assert_eq!(field(&saved(&h), &["dither", "master"]), &Value::Num(0.));
    h.click("grain_coarse");
    h.click("dither_reset");
    frames(&h, 10);
    assert_eq!(field(&saved(&h), &["dither", "master"]), &Value::Num(1.));
    assert_eq!(
        field(&saved(&h), &["dither", "grain"]),
        &Value::Str("default".into())
    );
    h.click("theme_dark");
    frames(&h, 30);
    h.click("reduce_motion");
    frames(&h, 10);
    assert_eq!(field(&saved(&h), &["reduced"]), &Value::Bool(true));
    rests(&mut h);
}

#[test]
fn tabs_and_navigation_drive_the_preview() {
    let mut h = settings();
    go(&mut h, "tabs");
    h.click("tab_layout_horizontal");
    h.click("bar_position_inset");
    frames(&h, 40);
    healthy(&h);
    assert_eq!(
        field(&saved(&h), &["tabs"]),
        &Value::Str("horizontal".into())
    );
    assert_eq!(field(&saved(&h), &["navbar"]), &Value::Str("inset".into()));
    h.click("show_full_url");
    frames(&h, 10);
    assert_eq!(
        field(&saved(&h), &["flags", "Show full URL"]),
        &Value::Bool(true)
    );
    rests(&mut h);
}

#[test]
fn providers_connect_after_signing_in() {
    let mut h = settings();
    go(&mut h, "providers");
    assert_eq!(h.text("providers_total"), "1 of 4 connected");
    h.click("provider_openrouter_connect");
    frames(&h, 10);
    assert!(h.find("provider_openrouter_cancel").is_some());
    frames(&h, 60 * 3);
    healthy(&h);
    assert!(h.find("provider_openrouter_disconnect").is_some());
    assert_eq!(h.text("providers_total"), "2 of 4 connected");
    h.click("provider_claude-code_import");
    frames(&h, 5);
    assert_eq!(h.text("providers_total"), "3 of 4 connected");
    h.click("provider_opencode_key");
    frames(&h, 10);
    typed(&mut h, "sk-test");
    key(&mut h, Key::Enter);
    frames(&h, 5);
    assert_eq!(h.text("providers_total"), "4 of 4 connected");
    rests(&mut h);
}

#[test]
fn loadouts_edit_add_and_remove() {
    let mut h = settings();
    go(&mut h, "models");
    h.click("loadout_3");
    frames(&h, 10);
    assert_eq!(h.text("loadout_name_value"), "Opus 5.5 · Deep");
    h.click("loadout_effort");
    frames(&h, 15);
    h.click("loadout_effort_option_Low");
    frames(&h, 5);
    assert_eq!(h.text("loadout_effort_value"), "Low");
    // The model browser: filter, then pick with the keyboard.
    h.click("loadout_model");
    frames(&h, 20);
    assert!(h.find("model_browser").is_some());
    h.click("browser_filter_grok");
    frames(&h, 5);
    assert_eq!(h.text("browser_detail_name"), "Grok 4.7");
    key(&mut h, Key::ArrowDown);
    key(&mut h, Key::Enter);
    frames(&h, 10);
    assert_eq!(h.text("loadout_model_value"), "Grok 4.7 Fast");
    assert!(h.find("loadout_effort").is_none(), "no reasoning to set");
    h.click("new_loadout");
    frames(&h, 10);
    assert_eq!(h.text("loadout_name_value"), "New loadout");
    assert!(h.find("loadout_7").is_some());
    h.click("loadout_delete");
    frames(&h, 10);
    assert!(h.find("loadout_7").is_none());
    h.click("loadout_record");
    h.ui.dispatch(InputEvent::KeyDown {
        key: Key::Character("k".into()),
        modifiers: Modifiers {
            alt: true,
            ..Default::default()
        },
        repeat: false,
    });
    h.settle();
    frames(&h, 5);
    let loadouts = field(&saved(&h), &["models", "loadouts"]).list().to_vec();
    assert_eq!(
        field(&loadouts[0], &["keys"]),
        &Value::List(vec![Value::Str("⌥".into()), Value::Str("K".into())].into())
    );
    rests(&mut h);
}

#[test]
fn confirmations_presets_and_rules() {
    let mut h = settings();
    go(&mut h, "confirmations");
    // The stage plays a pass and moves to the next category.
    frames(&h, 60 * 8);
    healthy(&h);
    h.click("rule_forms_always");
    frames(&h, 5);
    assert_eq!(h.text("approach_balanced"), "Custom");
    h.click("approach_every");
    frames(&h, 5);
    assert_eq!(
        field(&saved(&h), &["confirm", "mode"]),
        &Value::Str("every".into())
    );
    h.click("approach_read");
    frames(&h, 5);
    assert_eq!(
        field(&saved(&h), &["confirm", "mode"]),
        &Value::Str("read".into())
    );
    go(&mut h, "assistant");
    assert_eq!(h.text("confirm_summary_label"), "Read only");
    h.click("confirm_summary");
    frames(&h, 10);
    assert_eq!(
        field(&saved(&h), &["section"]),
        &Value::Str("Confirmations".into())
    );
    rests(&mut h);
}

#[test]
fn extensions_downloads_and_about() {
    let mut h = settings();
    go(&mut h, "extensions");
    assert_eq!(h.text("extensions_count"), "INSTALLED · 4 OF 5 ON");
    h.click("extension_toggle_eimadpbcbfnmbkopoojfekhnkhdbieeh");
    frames(&h, 5);
    assert_eq!(h.text("extensions_count"), "INSTALLED · 5 OF 5 ON");
    h.click("developer_mode");
    frames(&h, 5);
    assert!(h.find("load_unpacked").is_some());
    go(&mut h, "downloads");
    h.click("downloads_clear");
    frames(&h, 5);
    assert!(h.find("downloads_clear").is_none());
    go(&mut h, "about");
    h.click("check_updates");
    frames(&h, 10);
    assert_eq!(h.text("check_updates_label"), "Checking…");
    frames(&h, 120);
    assert_eq!(h.text("check_updates_label"), "Check for updates");
    h.click("package_search");
    typed(&mut h, "serde");
    frames(&h, 5);
    healthy(&h);
    rests(&mut h);
}
