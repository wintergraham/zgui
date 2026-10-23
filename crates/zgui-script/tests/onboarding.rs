//! The onboarding example, walked headless from the splash to "Open Ask" as
//! a person would: choosing browsers and data, answering the Keychain coach,
//! changing the appearance and the agent's rules.
mod common;
use common::Harness;
use std::{cell::Cell, path::Path, rc::Rc};
use zgui::input::Key;
use zgui_script::{ScriptHost, testing::TestApp};

fn onboarding(step: &str, closed: Rc<Cell<bool>>) -> Harness {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/onboarding/app.luau");
    let step = step.to_string();
    let h = Harness::new(move |runtime| {
        let host = ScriptHost::new(runtime, path);
        host.shared("onboarding_step", step.as_str());
        // The import, twenty times as fast.
        host.shared("onboarding_speed", 20.);
        host.action("close", move |_| closed.set(true));
        host
    });
    h.ui.scene.borrow_mut().resize(880., 580.);
    h
}
fn ok(h: &Harness) {
    if let Some(e) = h.error() {
        panic!("script error: {e}");
    }
}
// The import has stopped for the Keychain coach.
fn waiting(h: &mut TestApp) -> bool {
    h.texts().iter().any(|t| t == "Waiting for Keychain access")
}
// Lets a step's entrance and the last one's exit play out.
fn settle(h: &mut Harness) {
    h.frames(45);
    ok(h);
}

#[test]
fn every_step_works_from_splash_to_open_ask() {
    let closed = Rc::new(Cell::new(false));
    let mut h = onboarding("splash", closed.clone());
    settle(&mut h);
    assert!(h.find("splash").is_some(), "opens on the splash");
    h.click("splash");
    settle(&mut h);

    // 01 Welcome → 02 Choose browsers.
    assert!(
        h.texts().iter().any(|t| t == "Welcome to Ask"),
        "{:?}",
        h.texts()
    );
    h.click("primary");
    settle(&mut h);
    assert!(
        h.texts()
            .iter()
            .any(|t| t == "Bring your browsing with you")
    );
    h.click("tile:arc");
    h.click("tile:aside");
    settle(&mut h);
    h.click("primary");
    settle(&mut h);

    // 03 Choose data: Chrome and Arc, extensions on too.
    assert!(
        h.texts()
            .iter()
            .any(|t| t.contains("from Chrome") && t.contains("from Arc")),
        "the bookmarks row names the picks: {:?}",
        h.texts()
    );
    h.click("kind:extensions");
    h.click("primary");
    settle(&mut h);

    // 04 Importing: Chrome's passwords bring up the Keychain coach.
    // (The coach's content stays mounted while it is closed: wait for the
    // import to stop for it.)
    assert!(
        h.wait_until(|h| waiting(h)),
        "the import stops for Chrome's passwords"
    );
    assert!(h.texts().iter().any(|t| t == "One more step for Chrome"));
    settle(&mut h);
    // Always Allow with nothing typed stays on step 1.
    h.click("keychain-always");
    assert!(h.texts().iter().any(|t| t == "Type your Mac password"));
    h.click("keychain-password");
    for c in ["h", "u", "n", "t", "e", "r", "2"] {
        h.key(Key::Character(c.into()));
    }
    assert_eq!(h.text("keychain-password"), "●●●●●●●");
    h.key(Key::Enter);
    settle(&mut h);
    h.click("keychain-always");
    settle(&mut h);
    // Arc is Chromium too: deny it, and its passwords are skipped.
    assert!(h.wait_until(|h| waiting(h) && h.texts().iter().any(|t| t == "One more step for Arc")));
    settle(&mut h);
    h.click("keychain-deny");
    assert!(
        h.wait_until(|h| h.texts().iter().any(|t| t == "Import complete")),
        "the import finishes"
    );
    assert!(
        h.texts().iter().any(|t| t.contains("passwords skipped")),
        "Arc's row says what it skipped: {:?}",
        h.texts()
    );
    h.click("primary");
    settle(&mut h);

    // 05 Appearance.
    h.click("theme:Light");
    h.click("tint:sage");
    h.click("intensity:Rich");
    h.click("grain:Coarse");
    settle(&mut h);
    h.click("reduce-motion");
    h.click("reserve");
    settle(&mut h);
    assert!(h.texts().iter().any(|t| t == "Booked · 8:00"));
    h.click("primary");
    settle(&mut h);

    // 06 Agent → 06B Choose per action.
    h.click("level:delegate");
    settle(&mut h);
    assert!(
        h.texts()
            .iter()
            .any(|t| t == "Pays at checkout with your saved cards")
    );
    h.click("customize");
    settle(&mut h);
    h.click("preset:Navigate");
    settle(&mut h);
    h.click("back");
    settle(&mut h);
    assert!(
        h.texts().iter().any(|t| t == "and 4 more"),
        "Navigate asks before all eight: {:?}",
        h.texts()
    );
    h.click("ai-off");
    settle(&mut h);
    assert!(h.texts().iter().any(|t| t == "Turn AI on"));
    h.key(Key::Enter);
    settle(&mut h);

    // 07 Ready.
    assert!(h.texts().iter().any(|t| t == "You're all set."));
    assert!(h.texts().iter().any(|t| t == "Agent off"));
    assert!(h.texts().iter().any(|t| t == "Imported Chrome & Arc"));
    h.click("make-default");
    settle(&mut h);
    assert!(h.texts().iter().any(|t| t == "Ask is your default"));
    h.click("primary");
    assert!(h.wait_until(|_| closed.get()), "Open Ask closes the window");
    ok(&h);
}

#[test]
fn nothing_runs_at_rest_under_reduce_motion() {
    let closed = Rc::new(Cell::new(false));
    let mut h = onboarding("appearance", closed);
    settle(&mut h);
    h.click("reduce-motion");
    settle(&mut h);
    for step in ["primary", "primary"] {
        h.click(step);
        settle(&mut h);
    }
    // Ready's radiance field and spinner are still: no frames, no script.
    h.frames(120);
    let calls = h.calls();
    h.frames(60);
    assert_eq!(h.calls(), calls, "no script runs at rest");
    assert!(!h.clock.wants_frame(), "nothing animates at rest");
    ok(&h);
}

#[test]
fn skipping_import_goes_straight_to_appearance() {
    let closed = Rc::new(Cell::new(false));
    let mut h = onboarding("browsers", closed);
    settle(&mut h);
    h.click("skip-import");
    settle(&mut h);
    h.click("primary");
    settle(&mut h);
    assert!(h.texts().iter().any(|t| t == "Make it yours"));
    assert!(h.find("import-status").is_none(), "no import, no status");
    ok(&h);
}
