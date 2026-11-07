//! Ask's agents dropdown, driven: `ui/scratch/agents_flow.luau` on the mock host, each click and key followed by what's
//! listed and what went to the host. Run: sh ui/scratch/agents_flow.sh
use std::{path::PathBuf, time::Duration};
use zgx::input::{InputEvent, Key, Modifiers};
use zgx_script::{ScriptHost, testing::TestApp};

struct Flow {
    h: TestApp,
    failed: usize,
}
impl Flow {
    fn settle(&mut self) {
        for _ in 0..3 {
            self.h.settle();
            self.h.frame();
        }
        std::thread::sleep(Duration::from_millis(220));
        self.h.settle();
        self.h.frame();
    }
    fn press(&mut self, key: Key) {
        self.h.ui.dispatch(InputEvent::KeyDown { key: key.clone(), modifiers: Modifiers::default(), repeat: false });
        self.h.ui.dispatch(InputEvent::KeyUp { key, modifiers: Modifiers::default() });
        self.settle();
    }
    fn click(&mut self, id: &str) {
        self.h.host.run_command("flow_clear");
        self.h.click(id);
        self.settle();
    }
    fn check(&mut self, step: &str, state: &[&str], acts: &[&str]) {
        if let Some(error) = self.h.error() {
            println!("FAIL  {step}: script error {error}");
            self.failed += 1;
        }
        let shown = self.h.text("flow_state");
        let sent = self.h.text("flow_acts");
        let missing: Vec<_> = state.iter().filter(|s| !shown.contains(**s)).chain(acts.iter().filter(|a| !sent.contains(**a))).collect();
        if missing.is_empty() {
            println!("ok    {step}");
        } else {
            println!("FAIL  {step}\n      wanted {missing:?}\n      state {shown}\n      acts  {sent}");
            self.failed += 1;
        }
    }
    fn same_top(&mut self, step: &str, a: &str, b: &str) {
        for _ in 0..12 {
            self.settle();
        }
        let (at, want) = (self.h.bounds(a).y, self.h.bounds(b).y);
        if (at - want).abs() < 1.5 {
            println!("ok    {step}");
        } else {
            println!("FAIL  {step}: {a} at {at}, {b} at {want}");
            self.failed += 1;
        }
    }
    fn shown(&mut self, step: &str, id: &str, wanted: bool) {
        // `hidden` keeps a node, taking its size.
        let found = self.h.find(id).is_some() && self.h.shown(id) && self.h.bounds(id).width > 0.;
        if found == wanted {
            println!("ok    {step}");
        } else {
            println!("FAIL  {step}: {id} shown={found}");
            self.failed += 1;
        }
    }
}

fn main() {
    let script: PathBuf = std::env::args_os().nth(1).expect("usage: agents_flow <script.luau>").into();
    let h = TestApp::new(|runtime| ScriptHost::new(runtime, script));
    h.ui.scene.borrow_mut().resize(1280., 820.);
    let mut f = Flow { h, failed: 0 };
    f.h.ui.prepare_frame();
    f.settle();

    f.check("at rest", &["open=false", "chip=5"], &[]);
    f.shown("the panel isn't built closed", "agents_panel", false);
    f.click("agents_chip");
    f.check("opens, what needs you first", &["open=true", "records=[Book dinner for 4:waiting:1:Paused before “Complete reservation”"], &[]);
    f.check("finished last, on its timer", &["Morning brief:done:2:Tabs close in"], &[]);
    f.shown("the panel", "agents_panel", true);
    f.shown("Review on what needs you", "agents_review_202", true);
    f.shown("a driven tab's row", "agents_tab_921", true);

    // Keyboard: down to the first row, Esc closes.
    f.press(Key::ArrowDown);
    f.press(Key::ArrowDown);
    f.check("keys don't act until ↵", &["open=true"], &[]);
    f.press(Key::Escape);
    f.check("Esc closes", &["open=false"], &[]);
    f.settle();
    f.shown("gone once closed", "agents_panel", false);

    // The selector follows the keyboard and the pointer, and rows act.
    f.click("agents_chip");
    f.press(Key::ArrowDown);
    f.press(Key::ArrowDown);
    f.same_top("↓↓ puts the selector on the first tab", "agents_selector", "agents_tab_921");
    let at = f.h.bounds("agents_agent_222");
    f.h.ui.dispatch(InputEvent::PointerMove { x: at.x + 40., y: at.y + 10. });
    f.settle();
    f.same_top("the pointer moves it to the row it's over", "agents_selector", "agents_agent_222");
    f.shown("Stop shows on the row it's on", "agents_stop_222", true);
    f.h.host.run_command("flow_clear");
    f.press(Key::ArrowDown);
    f.press(Key::Delete);
    f.check("⌫ closes the tab it's on", &["open=true", "Compare noise cancelling headphones:working:3:"], &["close_tab(1121)"]);
    f.settle();
    f.same_top("the selector takes the row now in its place", "agents_selector", "agents_tab_1122");
    f.h.host.run_command("flow_clear");
    f.press(Key::ArrowUp);
    f.press(Key::Enter);
    f.check("↵ on a record opens its chat", &["open=false"], &["open_chat("]);
    f.settle();

    f.click("agents_chip");
    f.click("agents_review_202");
    f.check("Review opens the chat and shows its tab", &["open=false"], &["activate_tab(", "open_chat(202)"]);

    f.click("agents_chip");
    f.click("agents_tab_8001");
    f.check("a finished run's tab: shown, and leaves the list", &["open=false", "Morning brief:done:1:"], &["activate_tab(8001)"]);

    f.click("agents_chip");
    f.h.host.run_command("flow_clear");
    f.press(Key::End);
    f.press(Key::Enter);
    f.check("↵ on the footer runs Show agents and closes", &["open=false", "shows=1"], &[]);

    f.click("agents_chip");
    f.h.host.run_command("flow_stop_first");
    f.settle();
    f.check("a stopped run moves to finished, its tabs on the clock, without closing the panel", &["open=true", "Tabs close in 15 min"], &[]);
    f.press(Key::Escape);

    f.h.host.run_command("flow_ai_off");
    f.settle();
    f.check("browser use off: nothing listed", &["n=0"], &[]);
    f.shown("the control hides", "agents_chip", false);
    f.h.host.run_command("flow_ai_on");
    f.settle();
    f.check("back on", &["open=false"], &[]);
    f.shown("the control returns", "agents_chip", true);

    if f.failed > 0 {
        println!("{} failed", f.failed);
        std::process::exit(1);
    }
    println!("all passed");
}
