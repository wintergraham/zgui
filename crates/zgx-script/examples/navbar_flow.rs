//! Ask's omnibox, driven: `ui/scratch/navbar_flow.luau` on the mock host, each step's keys followed by what the model holds
//! and what went to the host. Run: sh ui/scratch/flow.sh
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
        std::thread::sleep(Duration::from_millis(20));
        self.h.settle();
        self.h.frame();
    }
    fn typed(&mut self, text: &str) {
        for c in text.chars() {
            let key = Key::Character(c.to_string());
            self.h.ui.dispatch(InputEvent::KeyDown { key: key.clone(), modifiers: Modifiers::default(), repeat: false });
            self.h.ui.dispatch(InputEvent::Text(c.to_string()));
            self.h.ui.dispatch(InputEvent::KeyUp { key, modifiers: Modifiers::default() });
            self.settle();
        }
    }
    fn press(&mut self, key: Key, modifiers: Modifiers) {
        self.h.ui.dispatch(InputEvent::KeyDown { key: key.clone(), modifiers, repeat: false });
        self.h.ui.dispatch(InputEvent::KeyUp { key, modifiers });
        self.settle();
    }
    /// Asking leaves the page on a chat; what follows wants a web page in front.
    fn web(&mut self) {
        self.h.host.run_command("flow_web");
        self.settle();
    }
    fn open(&mut self) {
        self.h.host.run_command("flow_clear");
        self.h.host.run_command("focus_address");
        let at_once = self.h.ui.focused_editor().is_some();
        self.settle();
        self.check("⌘L", &["open=true", "pristine=true"], &["field_focused"]);
        if !at_once {
            println!("FAIL  the field didn't have the keyboard in the same turn");
            self.failed += 1;
        }
    }
    /// Esc until it's closed, and past the close's animation and tidying.
    fn close(&mut self) {
        for _ in 0..3 {
            if self.h.text("flow_state").contains("open=true") {
                self.press(Key::Escape, Modifiers::default());
            }
        }
        for _ in 0..30 {
            self.h.frame();
        }
        std::thread::sleep(Duration::from_millis(200));
        self.settle();
        self.h.host.run_command("flow_clear");
        self.settle();
    }
    /// Past what's animating.
    fn rest(&mut self) {
        for _ in 0..60 {
            self.h.frame();
        }
        std::thread::sleep(Duration::from_millis(400));
        self.settle();
    }
    /// A switch: the model changed, the field still has the keyboard, the confirmation shows and the sheet didn't move for it.
    fn model_switch(&mut self, step: &str, model: &str, before: zgx::scene::Rect) {
        self.check(step, &["open=true", model], &[]);
        let (focused, shown, after) = (self.h.ui.focused_editor().is_some(), self.h.shown("composer-model-switched"), self.h.bounds("omnibox"));
        if !focused || !shown || after != before {
            println!("FAIL  {step}: field focused {focused}, confirmation {shown}, sheet {before:?} then {after:?}");
            self.failed += 1;
        }
    }
    fn check(&mut self, step: &str, state: &[&str], acts: &[&str]) {
        let (s, a) = (self.h.text("flow_state"), self.h.text("flow_acts"));
        let missing: Vec<&&str> = state.iter().filter(|w| !s.contains(**w)).chain(acts.iter().filter(|w| !a.contains(**w))).collect();
        if let Some(e) = self.h.error() {
            println!("FAIL  {step}: script error: {e}");
            self.failed += 1;
        } else if missing.is_empty() {
            println!("ok    {step}\n        {s}\n        {a}");
        } else {
            println!("FAIL  {step}: wanted {missing:?}\n        {s}\n        {a}");
            self.failed += 1;
        }
    }
}

fn main() {
    let script: PathBuf = std::env::args_os().nth(1).expect("usage: navbar_flow <script.luau>").into();
    let h = TestApp::new(|runtime| ScriptHost::new(runtime, script));
    h.ui.scene.borrow_mut().resize(1280., 820.);
    let mut f = Flow { h, failed: 0 };
    let none = Modifiers::default();
    let meta = Modifiers { meta: true, ..Modifiers::default() };
    f.h.ui.prepare_frame();
    f.settle();
    std::thread::sleep(Duration::from_millis(160));
    f.settle();

    f.open();
    f.check("opens on the page", &["mode=search", "input=[http"], &[]);
    f.typed("hello w");
    f.check("typing replaces the URL", &["input=[hello w]", "pristine=false", "first=[hello w]"], &[]);
    f.press(Key::Enter, none);
    f.check("↵ searches", &["open=false"], &["navigate(https://www.google.com/search?q=hello+w)"]);
    f.close();

    f.check("tidied while closed", &["input=[http", "pristine=true", "mode=search"], &[]);
    f.open();
    f.press(Key::Enter, none);
    f.check("↵ untouched reloads the URL", &["open=false"], &["navigate(http"]);
    f.close();

    f.web();
    f.open();
    f.press(Key::ArrowDown, none);
    f.press(Key::Enter, none);
    f.check("↓↵ takes the first row", &["open=false"], &["ask(Summarize this page +page)"]);
    f.close();

    f.web();
    f.open();
    f.press(Key::Tab, none);
    f.check("⇥ untouched goes to Ask, empty", &["mode=ask", "input=[]"], &[]);
    f.typed("why");
    f.press(Key::Enter, none);
    f.check("↵ asks with the page", &["open=false"], &["ask(why +page)"]);
    f.close();

    f.web();
    f.open();
    f.typed("?hi");
    f.check("? turns to Ask", &["mode=ask", "input=[hi]"], &[]);
    f.press(Key::Enter, meta);
    f.check("⌘↵ runs it behind", &["open=false"], &["new_chat", "attach_tab", "send(hi)"]);
    f.close();

    f.open();
    f.typed("gh ");
    f.check("a keyword and a space takes its scope", &["scope=gh", "input=[]", "rows=0"], &[]);
    f.typed("zgx");
    f.check("scope lists the query", &["scope=gh", "first=[zgx]"], &[]);
    f.press(Key::Enter, none);
    f.check("↵ searches the site", &["open=false"], &["navigate(https://github.com/search?q=zgx)"]);
    f.close();

    f.open();
    f.typed("w ");
    f.check("a one-letter word doesn't", &["scope=-", "input=[w ]"], &[]);
    f.close();

    f.open();
    f.typed("tabs ");
    f.check("tabs without its @ doesn't", &["scope=-", "input=[tabs ]"], &[]);
    f.close();

    f.open();
    f.typed("@tabs ");
    f.check("@tabs takes Tabs", &["scope=tabs"], &[]);
    f.press(Key::Backspace, none);
    f.check("⌫ clears the scope", &["scope=-", "mode=search"], &[]);
    f.close();

    f.open();
    f.typed("@");
    f.check("@ lists the scopes", &["first=[@tabs]", "rows=5"], &[]);
    f.press(Key::Tab, none);
    f.check("⇥ takes the highlighted one", &["scope=tabs", "input=[]"], &[]);
    f.press(Key::Escape, none);
    f.check("Esc leaves the scope", &["open=true", "scope=-"], &[]);
    f.press(Key::Escape, none);
    f.check("Esc closes", &["open=false"], &[]);
    f.close();

    f.open();
    f.typed("exa");
    f.check("an address completes", &["input=[example.com/docs]", "typed=[exa]"], &[]);
    f.typed("m");
    f.check("typing on replaces the completion", &["input=[exam]", "typed=[]"], &[]);
    f.close();

    f.open();
    f.typed("exa");
    f.press(Key::Enter, none);
    f.check("↵ takes the top row", &["open=false"], &["omnibox_pick", "navigate(https://example.com/docs)"]);
    f.close();

    f.open();
    f.typed("two words");
    f.press(Key::Tab, none);
    f.check("first Tab with text stays in Search", &["mode=search", "input=[two words]"], &[]);
    f.press(Key::Tab, none);
    f.check("second Tab switches", &["mode=ask", "input=[two words]"], &[]);
    f.press(Key::Tab, none);
    f.check("first Tab back stays in Ask", &["mode=ask"], &[]);
    f.press(Key::Tab, none);
    f.check("second Tab goes back", &["mode=search", "input=[two words]"], &[]);
    f.close();

    f.web();
    f.open();
    f.typed("half typed");
    f.h.host.run_command("focus_address");
    f.settle();
    f.check("⌘L again closes it", &["open=false"], &["focus_page"]);
    f.close();
    f.check("and it's put back", &["input=[http", "pristine=true"], &[]);

    f.web();
    f.open();
    // At rest: the click goes to where the field is, and mid-open it has yet to arrive there.
    for _ in 0..60 {
        f.h.frame();
    }
    std::thread::sleep(Duration::from_millis(400));
    f.settle();
    f.h.click("navbar_omnibox_field");
    f.settle();
    std::thread::sleep(Duration::from_millis(200));
    f.settle();
    let hit = f.h.ui.focused_editor().is_some();
    f.check("a click in the field keeps it open", &["open=true"], &[]);
    println!("        field focused after the click: {hit}");
    f.close();

    // ⌥1–⌥6 and ⌥Tab are menu commands (features/composer/loadouts.luau): the field keeps the keyboard and its text, the sheet
    // stays put, and the switch says so under it. This harness has no menu bar, so the commands run directly.
    f.web();
    f.open();
    f.typed("half typed");
    f.rest();
    let before = f.h.bounds("omnibox");
    f.h.host.run_command("model_2");
    f.settle();
    f.model_switch("⌥2 in Search", "model=codex/gpt-6-astra", before);
    f.check("  and keeps the text", &["mode=search", "input=[half typed]"], &[]);
    f.h.host.run_command("cycle_model");
    f.settle();
    f.model_switch("⌥⇥ in Search", "model=codex/gpt-6-luna", before);
    f.h.host.run_command("model_picker");
    f.settle();
    let sheet_w = before.width as i32;
    f.check("⌘⇧M in Search opens from the box", &[&format!("anchor={sheet_w}x")], &[]);
    f.h.host.run_command("flow_no_anchor");
    f.press(Key::Tab, none);
    f.press(Key::Tab, none);
    f.rest();
    let before = f.h.bounds("omnibox");
    f.h.host.run_command("model_1");
    f.settle();
    f.model_switch("⌥1 in Ask", "model=codex/gpt-6.1-sol", before);
    f.check("  and keeps the text", &["mode=ask", "input=[half typed]"], &[]);
    f.h.host.run_command("model_picker");
    f.settle();
    let pill = f.h.bounds("navbar_omnibox_model_below");
    f.check("⌘⇧M in Ask opens from the pill", &[&format!("anchor={}x{}", pill.width as i32, pill.height as i32)], &[]);
    f.h.host.run_command("flow_no_anchor");
    f.close();
    f.h.host.run_command("model_picker");
    f.settle();
    f.check("⌘⇧M with it closed opens from nothing out of sight", &["open=false", "anchor=none"], &[]);

    println!("{}", if f.failed == 0 { "all passed".to_string() } else { format!("{} failed", f.failed) });
    std::process::exit(if f.failed == 0 { 0 } else { 1 });
}
