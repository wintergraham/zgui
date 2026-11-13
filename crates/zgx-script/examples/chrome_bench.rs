//! What Ask's tab strip, sidebar and navbar cost, headless and in the whole shell: `ui/scratch/chrome_bench.luau` over
//! the live host, with `ask.*` published as Ask's bridge publishes it (src/shell/bridge.rs: the whole list each time,
//! built afresh), from a fixture holding a real profile's tabs and chats. Each phase runs from the input through the frame
//! that shows it, painted at 2x. Instructions are the number to compare between runs.
//! Run: sh ui/scratch/chrome_bench.sh <label> <fixture.json>
use serde_json::{Value as J, json};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};
use zgx::input::{InputEvent, Key, Modifiers, PointerButton};
use zgx_gpu::GpuRenderer;
use zgx_script::{ScriptHost, Value, testing::TestApp};

const W: f32 = 1440.;
const H: f32 = 900.;
const SCALE: f32 = 2.;

fn instructions() -> u64 {
    #[cfg(target_os = "macos")]
    {
        #[repr(C)]
        struct Usage {
            uuid: [u8; 16],
            fields: [u64; 40],
        }
        unsafe extern "C" {
            fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut Usage) -> i32;
        }
        let mut usage = Usage { uuid: [0; 16], fields: [0; 40] };
        // RUSAGE_INFO_V4: `ri_instructions` is the 30th field after the uuid.
        unsafe { proc_pid_rusage(std::process::id() as i32, 4, &mut usage) };
        usage.fields[29]
    }
    // No instruction counter off macOS: the timings still compare.
    #[cfg(not(target_os = "macos"))]
    0
}

fn value(j: &J) -> Value {
    match j {
        J::Null => Value::Nil,
        J::Bool(b) => Value::Bool(*b),
        J::Number(n) => Value::Num(n.as_f64().unwrap_or(0.)),
        J::String(s) => Value::from(s.as_str()),
        J::Array(items) => Value::List(items.iter().map(value).collect::<Vec<_>>().into()),
        J::Object(fields) => {
            let mut fields: Vec<(Rc<str>, Value)> = fields.iter().map(|(k, v)| (Rc::<str>::from(k.as_str()), value(v))).collect();
            fields.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Map(fields.into())
        }
    }
}

/// The contract's action names (`local actions = {` to its closing brace), so a script's `ui.invoke` finds each.
fn actions(contract: &str) -> Vec<String> {
    let start = contract.find("local actions = {").expect("actions");
    let end = contract[start..].find("\n}").expect("end of actions") + start;
    let mut out = Vec::new();
    for line in contract[start..end].lines() {
        let line = line.split("--").next().unwrap_or("");
        let mut in_string = false;
        let mut word = String::new();
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '"' {
                in_string = !in_string;
                word.clear();
            } else if in_string {
            } else if c.is_alphanumeric() || c == '_' {
                word.push(c);
            } else if c == '=' {
                if !word.is_empty() {
                    out.push(word.clone());
                }
                word.clear();
            } else if c != ' ' {
                word.clear();
            }
        }
    }
    out
}

/// The model the bridge publishes from.
struct Model {
    tabs: Vec<J>,
    chats: Vec<J>,
    spaces: J,
    groups: J,
    active: i64,
    history: Vec<J>,
}
impl Model {
    fn tab(&mut self, id: i64) -> &mut J {
        self.tabs.iter_mut().find(|t| t["id"] == id).unwrap()
    }
    fn activate(&mut self, id: i64) {
        self.active = id;
        for t in &mut self.tabs {
            let on = t["id"] == id;
            t["active"] = J::Bool(on);
            if on {
                t["unread"] = J::Bool(false);
                t["asleep"] = J::Bool(false);
                if t["browser"].is_null() {
                    t["browser"] = json!(id + 5000);
                }
            }
        }
    }
    fn page(&self) -> J {
        let t = self.tabs.iter().find(|t| t["id"] == self.active).unwrap();
        json!({"browser": t["browser"], "title": t["title"], "url": t["url"], "loading": t["loading"], "native": false, "secure": true,
            "back": true, "forward": false, "zoom": 1.0})
    }
}

#[derive(Default)]
struct Samples(Vec<(u64, f64)>, Vec<(u64, usize, usize)>, BTreeMap<&'static str, (u64, f64)>);

struct Bench {
    h: TestApp,
    gpu: GpuRenderer,
    results: BTreeMap<String, Samples>,
    last: (usize, usize),
    model: Model,
    pressed: Rc<RefCell<Vec<(String, Value)>>>,
    commands: u64,
    traced: BTreeMap<String, BTreeMap<String, (u64, f64)>>,
    inspector: zgx::inspect::Inspector,
}
impl Bench {
    fn new(script: &PathBuf, contract: &str, fixture: &J, placement: &str) -> Self {
        let mut gpu = GpuRenderer::new((W * SCALE) as u32, (H * SCALE) as u32).expect("GPU adapter");
        gpu.set_scale_factor(SCALE);
        for bytes in [
            &include_bytes!("../../zgx-desktop/examples/support/fonts/GeistVariable.ttf")[..],
            &include_bytes!("../../zgx-desktop/examples/support/fonts/GeistMonoVariable.ttf")[..],
        ] {
            zgx_gpu::text::FontData::new(bytes).unwrap().install(&mut gpu.text_system().borrow_mut());
        }
        let pressed: Rc<RefCell<Vec<(String, Value)>>> = Rc::default();
        let script = script.clone();
        let names = actions(contract);
        let heard = pressed.clone();
        let placement = placement.to_owned();
        let look = fixture["appearance"].clone();
        let mut settings = fixture["settings"].clone();
        settings["navbar"] = json!(placement);
        let inspector = zgx::inspect::Inspector::off();
        inspector.set_warnings(std::env::var_os("TRACE_PHASES").is_some());
        let inspected = inspector.clone();
        let h = TestApp::inspected(move |runtime| {
            let host = ScriptHost::new(runtime, script);
            host.shared("ask.live", true);
            host.shared("bench.placement", placement.as_str());
            if std::env::var_os("DEFAULT_LOOK").is_none() {
                host.shared("ask.appearance", value(&look));
            }
            host.shared("ask.settings", value(&settings));
            for name in names {
                let heard = heard.clone();
                let named = name.clone();
                host.action(name, move |arg| heard.borrow_mut().push((named.clone(), arg)));
            }
            host
        }, &inspected);
        gpu.install_text(&mut h.ui.scene.borrow_mut());
        h.ui.refresh_text_geometry();
        h.ui.scene.borrow_mut().resize(W, H);
        let list = |k: &str| fixture[k].as_array().unwrap().clone();
        let model = Model { tabs: list("tabs"), chats: list("chats"), spaces: fixture["spaces"].clone(), groups: fixture["groups"].clone(), active: 0, history: list("history") };
        Self { h, gpu, results: BTreeMap::new(), last: (0, 0), model, pressed, commands: 0, traced: BTreeMap::new(), inspector }
    }
    fn set(&self, key: &str, v: &J) {
        self.h.host.shared(key, Value::Nil).set(value(v));
    }
    /// As `publish_model` does for a new revision: everything but the pages' data, which has fingerprints of its own.
    fn publish(&self, tabs: bool) {
        if tabs {
            self.set("ask.spaces", &self.model.spaces);
            self.set("ask.groups", &self.model.groups);
            self.set("ask.tabs", &J::Array(self.model.tabs.clone()));
        }
        self.set("ask.page", &self.model.page());
        self.set("ask.chats", &J::Array(self.model.chats.clone()));
        self.set("ask.projects", &json!([]));
        self.set("ask.context_tabs", &json!([]));
        self.set("ask.composer_attachments", &json!([]));
        self.set("ask.agents", &json!([]));
        self.set("ask.agent_tabs", &json!([]));
        self.set("ask.notice", &json!(""));
        self.set("ask.relaunch_notice", &json!(""));
        self.set("ask.exit_notice", &json!(""));
        self.set("ask.search_engine", &json!({"name": "Google", "url": "https://www.google.com/search?q=%s"}));
        self.set("ask.model", &json!({"id": "codex/gpt-6.1-sol", "effort": "high", "relentless": false}));
    }
    fn command(&mut self, name: &str) {
        self.commands += 1;
        self.set("ask.ui_command", &json!({"seq": self.commands, "name": name}));
    }
    fn frame(&mut self) -> bool {
        self.h.frame();
        let report = self.h.ui.scene.borrow_mut().flush();
        self.last = (report.layout_nodes, report.paint_nodes);
        let damaged = !report.damage.is_empty();
        if damaged {
            self.gpu.render(&self.h.ui.scene.borrow(), &report.damage).unwrap();
        }
        damaged
    }
    fn phase(&mut self, name: &str, input: impl FnOnce(&mut Self)) {
        if std::env::var_os("PRINT_PHASES").is_some() {
            eprintln!("phase {name}");
        }
        let calls = self.h.calls();
        zgx::profile::take();
        let mark = zgx::trace::frame_index();
        let (start, at) = (instructions(), Instant::now());
        input(self);
        self.h.settle();
        self.frame();
        let sample = (instructions() - start, at.elapsed().as_secs_f64() * 1e3);
        let work = (self.h.calls() - calls, self.last.0, self.last.1);
        if std::env::var_os("TRACE_PHASES").is_some() {
            let runs = self.traced.entry(name.into()).or_default();
            for frame in zgx::trace::frames(4).iter().filter(|f| f.index > mark) {
                for run in &frame.runs {
                    let key = run.name.as_deref().map_or_else(|| format!("effect {}", run.effect), str::to_owned);
                    let e = runs.entry(key).or_insert((0, 0.));
                    e.0 += 1;
                    e.1 += run.micros as f64;
                }
            }
        }
        let spent = zgx::profile::take();
        let entry = self.results.entry(name.into()).or_default();
        entry.0.push(sample);
        entry.1.push(work);
        for (phase, instr, time) in spent {
            let e = entry.2.entry(phase.name()).or_default();
            e.0 += instr;
            e.1 += time.as_secs_f64() * 1e3;
        }
    }
    /// Publishes alone: what setting the keys costs before any frame, under `name`.
    fn rest(&mut self, name: Option<&str>) {
        let (start, at) = (instructions(), Instant::now());
        for _ in 0..45 {
            self.frame();
        }
        let sample = (instructions() - start, at.elapsed().as_secs_f64() * 1e3);
        if let Some(name) = name {
            self.results.entry(name.into()).or_default().0.push(sample);
        }
        std::thread::sleep(Duration::from_millis(20));
        self.h.settle();
        self.frame();
    }
    fn center(&mut self, id: &str) -> Option<(f32, f32)> {
        self.h.find(id)?;
        let b = self.h.bounds(id);
        (b.width > 0.).then_some((b.x + b.width / 2., b.y + b.height / 2.))
    }
    fn press(&mut self, key: Key) {
        self.h.ui.dispatch(InputEvent::KeyDown { key: key.clone(), modifiers: Modifiers::default(), repeat: false });
        self.h.ui.dispatch(InputEvent::KeyUp { key, modifiers: Modifiers::default() });
    }
    fn typed(&mut self, c: char) {
        let key = Key::Character(c.to_string());
        self.h.ui.dispatch(InputEvent::KeyDown { key: key.clone(), modifiers: Modifiers::default(), repeat: false });
        self.h.ui.dispatch(InputEvent::Text(c.to_string()));
        self.h.ui.dispatch(InputEvent::KeyUp { key, modifiers: Modifiers::default() });
    }
    fn failed(&self) {
        if let Some(e) = self.h.error() {
            panic!("script error: {e}");
        }
    }
}

fn run(script: &PathBuf, contract: &str, fixture: &J, placement: &str, runs: usize, json_out: &mut String) {
    let mut b = Bench::new(script, contract, fixture, placement);
    let ids: Vec<i64> = b.model.tabs.iter().filter(|t| t["group"].is_null() && t["pinned"] == false).map(|t| t["id"].as_i64().unwrap()).collect();
    let first = ids[ids.len() / 2];
    b.model.activate(first);
    b.publish(true);
    let rows = J::Array(b.model.history.clone());
    b.set("ask.history", &rows);
    b.h.ui.prepare_frame();
    b.h.settle();
    b.rest(None);
    std::thread::sleep(Duration::from_millis(130));
    b.h.settle();
    b.rest(None);
    b.failed();
    let tab_id = |id: i64| if placement == "sidebar" { format!("{id}") } else { format!("strip_tab_{id}") };
    let shown: Vec<i64> = ids.iter().copied().filter(|id| b.h.find(&tab_id(*id)).is_some()).collect();
    assert!(!shown.is_empty(), "no tabs in the view:\n{}", b.h.tree());
    let nodes = b.h.ui.scene.borrow().len();

    // Idle, nothing changing.
    let calls = b.h.calls();
    let (start, at) = (instructions(), Instant::now());
    let mut painted = 0;
    for _ in 0..120 {
        if b.frame() {
            painted += 1;
        }
    }
    let idle = ((instructions() - start) as f64 / 1e6, at.elapsed().as_secs_f64() * 1e3, painted, b.h.calls() - calls);

    let mut serial = 0;
    for run in 0..runs {
        let other = shown[(run * 7 + 3) % shown.len()];
        let back = shown[(run * 5 + 1) % shown.len()];
        // A switch, from the host's side: the list and the page change, nothing else.
        b.phase("switch tab: publish, frame", |b| {
            b.model.activate(other);
            b.publish(true);
        });
        b.rest(Some("switch tab: then to rest (45 frames)"));
        b.phase("switch back: publish, frame", |b| {
            b.model.activate(back);
            b.publish(true);
        });
        b.rest(None);
        let active = b.model.active;
        let background = shown.iter().copied().find(|id| *id != active).unwrap();

        // A page's title, favicon and load state, one publish each, as a page loading sends them.
        serial += 1;
        b.phase("background tab title", |b| {
            b.model.tab(background)["title"] = json!(format!("A title that changed {serial}"));
            b.publish(true);
        });
        b.phase("active tab title", |b| {
            b.model.tab(active)["title"] = json!(format!("The active title changed {serial}"));
            b.publish(true);
        });
        b.phase("active tab loading on", |b| {
            b.model.tab(active)["loading"] = json!(true);
            b.publish(true);
        });
        b.rest(None);
        b.phase("active tab loading off", |b| {
            b.model.tab(active)["loading"] = json!(false);
            b.publish(true);
        });
        b.rest(None);
        b.phase("active tab favicon", |b| {
            let t = b.model.tab(active);
            let f = t["favicon"].as_str().unwrap().to_owned();
            t["favicon"] = json!(if f.ends_with("gmail.png") { f.replace("gmail.png", "figma.png") } else { f.replace(f.rsplit('/').next().unwrap(), "gmail.png") });
            b.publish(true);
        });
        b.rest(None);
        // A visit: the history page's list grows by one, as each page commit does.
        b.phase("visit: ask.history grows", |b| {
            let mut row = b.model.history[0].clone();
            row["id"] = json!(row["id"].as_i64().unwrap() + 1);
            row["visited"] = json!(row["visited"].as_i64().unwrap() + 1);
            b.model.history.insert(0, row);
            b.model.history.truncate(2000);
            let rows = J::Array(b.model.history.clone());
            b.set("ask.history", &rows);
        });
        b.rest(None);
        // The model changed elsewhere (a chat's status, an agent's step): every key but the tabs, all the same.
        b.phase("revision, tabs unchanged", |b| b.publish(false));
        b.phase("revision, tabs list rebuilt, same", |b| b.publish(true));

        // Hovering along the tabs.
        b.h.ui.dispatch(InputEvent::PointerEnter);
        for id in shown.iter().filter(|id| **id != b.model.active).take(6).copied().collect::<Vec<_>>() {
            let Some((x, y)) = b.center(&tab_id(id)) else { continue };
            b.phase("hover next tab", |b| { b.h.ui.dispatch(InputEvent::PointerMove { x, y }); });
            b.frame();
        }
        b.h.ui.dispatch(InputEvent::PointerMove { x: W / 2., y: H - 10. });
        b.h.settle();
        b.rest(None);
        b.pressed.borrow_mut().clear();

        // A new tab, as ⌘T: the list grows and it is active. Then it closes.
        let new = 100_000 + run as i64;
        b.phase("open tab: publish, frame", |b| {
            let mut t = b.model.tabs[0].clone();
            t["id"] = json!(new);
            t["browser"] = json!(new);
            t["title"] = json!("New tab");
            t["url"] = json!("https://example.com/");
            t["pinned"] = json!(false);
            t["group"] = J::Null;
            let at = b.model.tabs.len();
            b.model.tabs.insert(at, t);
            b.model.activate(new);
            b.publish(true);
        });
        b.rest(Some("open tab: then to rest (45 frames)"));
        b.phase("close tab: publish, frame", |b| {
            b.model.tabs.retain(|t| t["id"] != new);
            b.model.activate(active);
            b.publish(true);
        });
        b.rest(Some("close tab: then to rest (45 frames)"));

        // Pressing a tab and dragging it along.
        if placement != "sidebar" && let Some((x, y)) = b.center(&tab_id(shown[(run + 2) % shown.len()])) {
            let button = PointerButton::Primary;
            b.h.ui.dispatch(InputEvent::PointerMove { x, y });
            b.phase("drag: press", |b| { b.h.ui.dispatch(InputEvent::PointerDown { x, y, button }); });
            for i in 1..=24 {
                let x = x + i as f32 * 6.;
                b.phase("drag: move", |b| { b.h.ui.dispatch(InputEvent::PointerMove { x, y }); });
            }
            let x = x + 24. * 6.;
            b.phase("drag: release", |b| { b.h.ui.dispatch(InputEvent::PointerUp { x, y, button }); });
            b.rest(Some("drag: release to rest (45 frames)"));
            b.pressed.borrow_mut().clear();
            b.h.ui.dispatch(InputEvent::PointerMove { x: W / 2., y: H - 10. });
            b.h.settle();
            b.rest(None);
        }
        // The sidebar folding away and back.
        if placement == "sidebar" {
            b.phase("sidebar collapse", |b| b.command("toggle_sidebar"));
            b.rest(Some("sidebar collapse: to rest (45 frames)"));
            b.phase("sidebar expand", |b| b.command("toggle_sidebar"));
            b.rest(Some("sidebar expand: to rest (45 frames)"));
        }
        // A window drag-resizing.
        for i in 1..=10 {
            let w = W - i as f32 * 3.;
            b.phase("resize step", |b| b.h.ui.scene.borrow_mut().resize(w, H));
        }
        b.h.ui.scene.borrow_mut().resize(W, H);
        b.rest(None);

        // The navbar: ⌘L, keys, Esc.
        b.phase("⌘L open", |b| b.command("focus_address"));
        b.rest(Some("⌘L: to rest (45 frames)"));
        for c in "noise cancelling".chars() {
            b.phase("omnibox key", |b| {
                b.typed(c);
                b.h.settle();
                let query = b.pressed.borrow_mut().drain(..).rev().find_map(|(n, arg)| (n == "omnibox_query").then(|| arg.field("text").cloned())).flatten();
                if let Some(Value::Str(q)) = query {
                    let rows: Vec<J> = (0..8).map(|i| json!({"key": format!("h{i}"), "kind": "history", "title": format!("{q} result {i}"), "url": format!("https://example.com/{i}"), "score": 10 - i, "elsewhere": false, "correction": false})).collect();
                    b.set("ask.omnibox", &json!({"query": &*q, "stamp": 1, "rows": rows}));
                }
            });
        }
        b.rest(None);
        b.phase("omnibox close (Esc)", |b| b.press(Key::Escape));
        b.rest(Some("omnibox close: to rest (45 frames)"));
        b.pressed.borrow_mut().clear();
        // Hovering the address.
        if let Some((x, y)) = b.center("address") {
            b.phase("address hover in", |b| { b.h.ui.dispatch(InputEvent::PointerMove { x, y }); });
            b.rest(Some("address hover in: to rest (45 frames)"));
            b.phase("address hover out", |b| { b.h.ui.dispatch(InputEvent::PointerMove { x: W / 2., y: H - 10. }); });
            b.rest(None);
        }
        b.failed();
    }

    println!("\n== {placement}: {} tabs in view, {nodes} nodes ==", shown.len());
    if std::env::var_os("TRACE_IDLE").is_some() {
        zgx::trace::set_enabled(true);
        for _ in 0..3 {
            b.frame();
        }
        for f in zgx::trace::frames(3) {
            println!("frame {:?}", f);
        }
        zgx::trace::set_enabled(false);
        // What moves from one frame to the next, to the last bit, and what it is.
        let root = b.h.find("shell").unwrap();
        let walk = |b: &Bench| {
            let scene = b.h.ui.scene.borrow();
            let mut out = BTreeMap::new();
            let mut stack = vec![root];
            while let Some(n) = stack.pop() {
                let r = scene.bounds(n);
                out.insert(n.number(), (r.x, r.y, r.width, r.height));
                stack.extend(scene.children(n).iter().copied());
            }
            out
        };
        let tree = b.h.tree();
        let name = |n: u32| tree.lines().find(|l| l.trim_start().starts_with(&format!("@{n} "))).unwrap_or("?").trim().to_owned();
        let mut last = walk(&b);
        for _ in 0..6 {
            b.h.frame();
            let flush = b.h.ui.scene.borrow_mut().flush();
            let now = walk(&b);
            println!("-- layout {} paint {} composite {}", flush.layout_nodes, flush.paint_nodes, flush.composite_nodes);
            for (n, r) in &now {
                if last.get(n) != Some(r) {
                    println!("moved @{n} {:?} -> {:?} {}", last.get(n), r, name(*n));
                }
            }
            last = now;
        }
    }
    println!("{:<42} {:>5} {:>8} {:>8} {:>7} {:>7} {:>7} {:>7} {:>7}", "phase", "n", "instr M", "p95 M", "ms", "p95 ms", "calls", "layout", "paint");
    *json_out += &format!("\"{placement}\":{{");
    for (name, s) in &b.results {
        let mut i: Vec<u64> = s.0.iter().map(|x| x.0).collect();
        let mut t: Vec<f64> = s.0.iter().map(|x| x.1).collect();
        i.sort_unstable();
        t.sort_by(f64::total_cmp);
        let (med, p95) = (i[i.len() / 2] as f64 / 1e6, i[(i.len() * 95 / 100).min(i.len() - 1)] as f64 / 1e6);
        let (tm, tp) = (t[t.len() / 2], t[(t.len() * 95 / 100).min(t.len() - 1)]);
        let median = |mut v: Vec<usize>| {
            v.sort_unstable();
            if v.is_empty() { 0 } else { v[v.len() / 2] }
        };
        let calls = median(s.1.iter().map(|x| x.0 as usize).collect());
        let layout = median(s.1.iter().map(|x| x.1).collect());
        let paint = median(s.1.iter().map(|x| x.2).collect());
        println!("{name:<42} {:>5} {med:>8.2} {p95:>8.2} {tm:>7.2} {tp:>7.2} {calls:>7} {layout:>7} {paint:>7}", i.len());
        *json_out += &format!("\"{name}\":{{\"instr_m\":{med:.3},\"p95_m\":{p95:.3},\"ms\":{tm:.3},\"p95_ms\":{tp:.3}}},");
    }
    if let Ok(want) = std::env::var("TRACE_NODE") {
        let tree = b.h.tree();
        let name = |n: u32| tree.lines().find(|l| l.trim_start().starts_with(&format!("@{n} "))).map(|l| l.trim().to_owned());
        let scene = b.h.ui.scene.borrow();
        let mut stack = vec![b.h.ui.scene.borrow().root()];
        while let Some(n) = stack.pop() {
            if n.number().to_string() == want {
                let mut at = Some(n);
                while let Some(x) = at {
                    let r = scene.bounds(x);
                    println!("  {} {} {:?} {}", x.number(), zgx::inspect::kind_name(&scene, x), (r.x, r.y, r.width, r.height), name(x.number()).unwrap_or_default());
                    at = scene.parent(x);
                }
            }
            stack.extend(scene.children(n).iter().copied());
        }
    }
    if !b.traced.is_empty() {
        let tree = b.h.tree();
        let mut nodes = std::collections::HashMap::new();
        {
            let scene = b.h.ui.scene.borrow();
            let mut stack = vec![scene.root()];
            while let Some(n) = stack.pop() {
                nodes.insert(n.number().to_string(), n);
                stack.extend(scene.children(n).iter().copied());
            }
        }
        let who = |key: &str| -> String {
            let at = key.split_whitespace().next().unwrap_or("");
            if !at.starts_with('@') {
                return key.to_owned();
            }
            if let Some(line) = tree.lines().find(|l| l.trim_start().starts_with(&format!("{at} "))) {
                return line.trim().chars().take(70).collect();
            }
            let label = nodes.get(at.trim_start_matches('@')).and_then(|n| b.inspector.label_of(*n));
            match label {
                Some(l) => format!("{} {}", l.name, l.source.as_deref().map(|s| s.to_string()).unwrap_or_default()),
                None => key.to_owned(),
            }
        };
        for (name, runs) in &b.traced {
            let n = b.results.get(name).map_or(1, |s| s.0.len()) as f64;
            println!("runs in `{name}`, per sample:");
            let mut by: BTreeMap<String, (f64, f64)> = BTreeMap::new();
            for (key, (count, micros)) in runs {
                let e = by.entry(who(key)).or_default();
                e.0 += *count as f64 / n;
                e.1 += micros / n;
            }
            let mut rows: Vec<_> = by.into_iter().collect();
            rows.sort_by(|a, b| b.1.1.total_cmp(&a.1.1));
            for (key, (count, micros)) in rows.iter().take(14) {
                println!("  {micros:8.1} us x{count:<6.1} {key}");
            }
        }
    }
    println!("idle, 120 frames: {:.2} M instructions, {:.2} ms, {} painted, {} script calls", idle.0, idle.1, idle.2, idle.3);
    *json_out += &format!("\"idle_m\":{:.3},\"idle_painted\":{},\"idle_calls\":{},\"nodes\":{nodes}}},", idle.0, idle.2, idle.3);
    println!("where it goes, M instructions per sample:");
    for (name, s) in &b.results {
        if s.2.is_empty() {
            continue;
        }
        let n = s.0.len() as f64;
        let parts: Vec<String> = s.2.iter().filter(|(_, i)| i.0 as f64 / n > 100_000.).map(|(p, i)| format!("{p} {:.1}M/{:.1}ms", i.0 as f64 / n / 1e6, i.1 / n)).collect();
        println!("  {name:<40} {}", parts.join("  "));
    }
}

fn main() {
    let mut args = std::env::args_os().skip(1);
    let script: PathBuf = args.next().expect("usage: chrome_bench <script.luau> <fixture.json>").into();
    let fixture: J = serde_json::from_slice(&std::fs::read(args.next().expect("fixture")).unwrap()).unwrap();
    let contract = std::fs::read_to_string(std::env::var("CONTRACT").expect("CONTRACT")).unwrap();
    let runs: usize = std::env::var("RUNS").ok().and_then(|r| r.parse().ok()).unwrap_or(8);
    let placements = std::env::var("PLACEMENTS").unwrap_or_else(|_| "docked,compact,sidebar".into());
    zgx::profile::set_recording(true);
    if std::env::var_os("TRACE_PHASES").is_some() {
        zgx::trace::set_enabled(true);
    }
    let mut out = String::from("{");
    for placement in placements.split(',') {
        run(&script, &contract, &fixture, placement, runs, &mut out);
    }
    out.pop();
    out.push('}');
    if let Some(path) = std::env::var_os("OUT") {
        std::fs::write(path, out).unwrap();
    }
}
