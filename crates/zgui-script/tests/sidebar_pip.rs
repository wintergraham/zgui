//! The sidebar_pip example, driven headless through its states.
mod common;
use common::Harness;
use std::path::Path;
use zgui::input::{InputEvent, Key, Modifiers};
use zgui_script::ScriptHost;

const W: f32 = 1180.;
const H: f32 = 840.;

fn sidebar() -> Harness {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/sidebar_pip/app.luau");
    let h = Harness::new(|runtime| ScriptHost::new(runtime, path));
    h.ui.scene.borrow_mut().resize(W, H);
    h
}

/// Writes a GPU capture when SIDEBAR_PIP_SNAPSHOT names a file prefix.
fn snapshot(h: &Harness, suffix: &str) {
    let Some(path) = std::env::var_os("SIDEBAR_PIP_SNAPSHOT") else {
        return;
    };
    let path = format!("{}{suffix}.ppm", path.to_string_lossy());
    let mut gpu = zgui_gpu::GpuRenderer::new(W as u32, H as u32).expect("a GPU");
    for bytes in [
        &include_bytes!("../../zgui-desktop/examples/support/fonts/GeistVariable.ttf")[..],
        &include_bytes!("../../zgui-desktop/examples/support/fonts/GeistMonoVariable.ttf")[..],
    ] {
        zgui_gpu::text::FontData::new(bytes)
            .unwrap()
            .install(&mut gpu.text_system().borrow_mut());
    }
    gpu.install_text(&mut h.ui.scene.borrow_mut());
    h.ui.refresh_text_geometry();
    h.frame();
    h.ui.prepare_frame();
    gpu.render(
        &h.ui.scene.borrow(),
        &[zgui::scene::Rect::new(0., 0., W, H)],
    )
    .unwrap();
    let pixels = gpu.readback().unwrap();
    let mut out = format!("P6\n{} {}\n255\n", W as u32, H as u32).into_bytes();
    for rgba in pixels.chunks(4) {
        out.extend_from_slice(&rgba[..3]);
    }
    std::fs::write(&path, out).unwrap();
}
fn ok(h: &Harness) {
    if let Some(e) = h.host.error().with_untracked(|e| e.clone()) {
        panic!("script error: {e}");
    }
}
fn frames(h: &Harness, n: usize) {
    for _ in 0..n {
        h.frame();
        ok(h);
    }
}
fn key(h: &mut Harness, key: Key, modifiers: Modifiers) {
    h.ui.dispatch(InputEvent::KeyDown {
        key,
        modifiers,
        repeat: false,
    });
    h.settle();
    ok(h);
}
fn hover(h: &mut Harness, id: &str) {
    let node = h.find(id).unwrap_or_else(|| panic!("no node `{id}`"));
    let b = h.ui.scene.borrow().bounds(node);
    h.ui.dispatch(InputEvent::PointerMove {
        x: b.x + b.width / 2.,
        y: b.y + b.height / 2.,
    });
    h.settle();
    ok(h);
}

#[test]
fn opens_on_the_board_master() {
    let mut h = sidebar();
    frames(&h, 20);
    for id in [
        "space_name",
        "search",
        "pin_gmail",
        "pin_chatgpt",
        "chat_c_network",
        "tab_stripe",
        "tab_data",
        "group_g_daily",
        "group_g_reading",
        "tab_dither",
        "space_work",
        "space_research",
        "pip_front",
        "pip_others",
    ] {
        assert!(h.find(id).is_some(), "the master shows `{id}`");
    }
    // Google is resolving its favicon.
    snapshot(&h, "1-master");
    frames(&h, 60 * 4);
    snapshot(&h, "2-loaded");
}

#[test]
fn tabs_groups_and_pins_respond() {
    let mut h = sidebar();
    frames(&h, 10);
    // Unfold the reading list; select a tab in it.
    h.click("group_header_g_reading");
    frames(&h, 30);
    h.click("tab_rl_hn");
    frames(&h, 10);
    assert!(
        h.text("page_title").contains("Show HN"),
        "the page follows the selection"
    );
    // Close it from its own close button: the row's click must not reselect it.
    hover(&mut h, "tab_rl_hn");
    h.click("close_rl_hn");
    frames(&h, 10);
    assert!(h.find("tab_rl_hn").is_none(), "closed");
    // A sleeping tab wakes (and loads) when selected.
    h.click("tab_overview");
    frames(&h, 5);
    // Pins: open Figma.
    h.click("pin_figma");
    frames(&h, 10);
    assert!(h.text("page_title").contains("Figma"));
    // Muting a tab is the mute button's click, not the row's.
    h.click("tab_stripe");
    frames(&h, 10);
    hover(&mut h, "tab_spotify_tab");
    h.click("mute_spotify_tab");
    frames(&h, 10);
    assert!(
        h.text("page_title").contains("Stripe"),
        "muting does not select the tab"
    );
    h.click("tab_hn");
    frames(&h, 10);
    assert!(
        h.text("page_title").contains("Hacker News"),
        "the next row click still selects"
    );
    // Close a whole group.
    h.click("close_group_g_daily");
    frames(&h, 10);
    assert!(h.find("tab_inbox").is_none() && h.find("group_g_daily").is_none());
    hover(&mut h, "space_personal");
    frames(&h, 20);
    snapshot(&h, "3-edited");
}

#[test]
fn spaces_search_and_rail() {
    let mut h = sidebar();
    frames(&h, 10);
    h.click("space_research");
    frames(&h, 30);
    assert!(h.find("tab_r_bayer").is_some(), "the research space's tabs");
    assert!(h.find("tab_stripe").is_none());
    key(
        &mut h,
        Key::Character("1".into()),
        Modifiers {
            control: true,
            ..Default::default()
        },
    );
    frames(&h, 30);
    assert!(h.find("tab_stripe").is_some(), "⌃1 is back to work");
    // Type an address.
    key(
        &mut h,
        Key::Character("k".into()),
        Modifiers {
            meta: true,
            ..Default::default()
        },
    );
    for c in "github.com/zgui".chars() {
        h.ui.dispatch(InputEvent::Text(c.to_string()));
    }
    h.settle();
    key(&mut h, Key::Enter, Modifiers::default());
    frames(&h, 30);
    assert!(
        h.text("page").contains("github.com"),
        "the typed address opens"
    );
    snapshot(&h, "4-typed");
    // Collapse to the rail.
    h.click("toggle_sidebar");
    frames(&h, 40);
    assert!(h.find("sidebar_rail").is_some());
    assert!(h.find("rail_stripe").is_some());
    snapshot(&h, "5-rail");
    h.click("expand_sidebar");
    frames(&h, 40);
    assert!(h.find("sidebar_expanded").is_some());
}

#[test]
fn pip_stacks_expands_and_finishes() {
    let mut h = sidebar();
    frames(&h, 10);
    assert!(h.find("pip_stack_header").is_none());
    h.click("pip_others");
    frames(&h, 30);
    assert!(h.find("pip_stack_header").is_some(), "expanded");
    assert!(h.find("pip_agent_passport").is_some());
    snapshot(&h, "6-expanded");
    // Review brings the waiting agent to the front; approve it.
    h.click("pip_review_passport");
    frames(&h, 30);
    assert!(h.text("pip_front").contains("Renew passport"));
    h.click("pip_action");
    frames(&h, 10);
    // Stop the two others: one agent left is the master.
    h.click("pip_agent_dinner");
    frames(&h, 20);
    h.click("pip_action");
    frames(&h, 20);
    h.click("pip_agent_flights");
    frames(&h, 20);
    h.click("pip_action");
    frames(&h, 30);
    assert!(h.find("pip_stack_header").is_none() && h.find("pip_others").is_none());
    assert!(
        h.find("pip_open").is_some(),
        "a lone agent is the master card"
    );
    snapshot(&h, "7-master");
    h.click("pip_open");
    frames(&h, 20);
    assert!(
        h.find("tab_agent_passport").is_some(),
        "its tab opens in the sidebar"
    );
}

#[test]
fn tab_key_reaches_the_pins() {
    let mut h = sidebar();
    frames(&h, 10);
    for _ in 0..3 {
        key(&mut h, Key::Tab, Modifiers::default());
    }
    frames(&h, 5);
    snapshot(&h, "8-focus");
}
