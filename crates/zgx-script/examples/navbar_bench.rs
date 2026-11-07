//! What Ask's navbar and omnibox cost, headless: `ui/scratch/navbar_bench.luau` alone, driven through mounting,
//! opening with ⌘L, typing, moving the selection, switching to Ask and closing, each phase measured from the input
//! through the frame that shows it, painted at 2x. Instructions are the number to compare between runs (they don't
//! move with what else the machine is doing); times are for a sense of scale, against 8.3 ms at 120 Hz.
//! Run: sh ui/scratch/bench.sh <label> (or RUNS=20 cargo run --release -p zgx-script --example navbar_bench -- <script>)
use std::{
    collections::BTreeMap,
    path::PathBuf,
    time::{Duration, Instant},
};
use zgx::input::{InputEvent, Key, Modifiers};
use zgx_gpu::GpuRenderer;
use zgx_script::{ScriptHost, testing::TestApp};

const W: f32 = 1280.;
const H: f32 = 820.;
const SCALE: f32 = 2.;
const QUERY: &str = "noise cancelling headphones for long flights";

/// Instructions this process has retired (macOS; 0 elsewhere).
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
    #[cfg(not(target_os = "macos"))]
    0
}

#[derive(Default)]
struct Samples(Vec<(u64, f64)>, Vec<(u64, usize, usize)>, BTreeMap<&'static str, u64>);
impl Samples {
    /// The median script calls, nodes laid out and nodes painted.
    fn work(&self) -> (u64, usize, usize) {
        let mut c: Vec<u64> = self.1.iter().map(|s| s.0).collect();
        let mut l: Vec<usize> = self.1.iter().map(|s| s.1).collect();
        let mut p: Vec<usize> = self.1.iter().map(|s| s.2).collect();
        c.sort_unstable();
        l.sort_unstable();
        p.sort_unstable();
        let m = c.len() / 2;
        if c.is_empty() { (0, 0, 0) } else { (c[m], l[m], p[m]) }
    }
    fn sorted(&self) -> (Vec<u64>, Vec<f64>) {
        let mut i: Vec<u64> = self.0.iter().map(|s| s.0).collect();
        let mut t: Vec<f64> = self.0.iter().map(|s| s.1).collect();
        i.sort_unstable();
        t.sort_by(f64::total_cmp);
        (i, t)
    }
}

struct Bench {
    h: TestApp,
    gpu: GpuRenderer,
    results: BTreeMap<String, Samples>,
    paint: bool,
    last: (usize, usize),
}
impl Bench {
    fn new(script: &PathBuf, paint: bool) -> Self {
        let mut gpu = GpuRenderer::new((W * SCALE) as u32, (H * SCALE) as u32).expect("GPU adapter");
        gpu.set_scale_factor(SCALE);
        for bytes in [
            &include_bytes!("../../zgx-desktop/examples/support/fonts/GeistVariable.ttf")[..],
            &include_bytes!("../../zgx-desktop/examples/support/fonts/GeistMonoVariable.ttf")[..],
        ] {
            zgx_gpu::text::FontData::new(bytes).unwrap().install(&mut gpu.text_system().borrow_mut());
        }
        let script = script.clone();
        let h = TestApp::new(|runtime| ScriptHost::new(runtime, script));
        gpu.install_text(&mut h.ui.scene.borrow_mut());
        h.ui.refresh_text_geometry();
        h.ui.scene.borrow_mut().resize(W, H);
        Self { h, gpu, results: BTreeMap::new(), paint, last: (0, 0) }
    }
    /// A frame as a window delivers it: lay out, then paint what changed. Whether anything had to be painted.
    fn frame(&mut self) -> bool {
        self.h.frame();
        let report = self.h.ui.scene.borrow_mut().flush();
        self.last = (report.layout_nodes, report.paint_nodes);
        let damaged = !report.damage.is_empty();
        if damaged && self.paint {
            self.gpu.render(&self.h.ui.scene.borrow(), &report.damage).unwrap();
        }
        damaged
    }
    /// `input`, then the frame that shows it: what it costs to see the result.
    fn phase(&mut self, name: &str, input: impl FnOnce(&mut Self)) {
        let calls = self.h.calls();
        zgx::profile::take();
        let (start, at) = (instructions(), Instant::now());
        input(self);
        self.h.settle();
        self.frame();
        let sample = (instructions() - start, at.elapsed().as_secs_f64() * 1e3);
        let work = (self.h.calls() - calls, self.last.0, self.last.1);
        let spent = zgx::profile::take();
        let entry = self.results.entry(name.into()).or_default();
        entry.0.push(sample);
        entry.1.push(work);
        for (phase, instr, _) in spent {
            *entry.2.entry(phase.name()).or_default() += instr;
        }
    }
    /// Lets animations run out and timers fire, unmeasured unless named.
    fn rest(&mut self, name: Option<&str>) {
        let (start, at) = (instructions(), Instant::now());
        for _ in 0..45 {
            self.frame();
        }
        let sample = (instructions() - start, at.elapsed().as_secs_f64() * 1e3);
        if let Some(name) = name {
            self.results.entry(name.into()).or_default().0.push(sample);
        }
        std::thread::sleep(Duration::from_millis(30));
        self.h.settle();
        self.frame();
    }
    fn shortcut(&mut self) {
        self.h.host.run_command("focus_address");
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
    fn nodes(&self) -> usize {
        self.h.ui.scene.borrow().len()
    }
    fn failed(&self) {
        if let Some(e) = self.h.error() {
            panic!("script error: {e}");
        }
    }
}

fn main() {
    let script: PathBuf = std::env::args_os().nth(1).expect("usage: navbar_bench <script.luau>").into();
    let runs: usize = std::env::var("RUNS").ok().and_then(|r| r.parse().ok()).unwrap_or(20);
    let paint = std::env::var_os("NO_PAINT").is_none();
    zgx::profile::set_recording(true);

    // Cold: from nothing to the first frame, each time a new script host.
    let mut all = BTreeMap::<String, Samples>::new();
    for _ in 0..5 {
        let (start, at) = (instructions(), Instant::now());
        let mut b = Bench::new(&script, paint);
        b.h.ui.prepare_frame();
        b.h.settle();
        b.frame();
        let sample = (instructions() - start, at.elapsed().as_secs_f64() * 1e3);
        all.entry("load everything (cold, first frame)".into()).or_default().0.push(sample);
        b.failed();
    }

    let mut b = Bench::new(&script, paint);
    b.h.ui.prepare_frame();
    b.h.settle();
    b.rest(None);
    std::thread::sleep(Duration::from_millis(130));
    b.h.settle();
    b.rest(None);
    b.failed();
    let nodes_closed = b.nodes();

    // The first open, then what a key typed straight after the shortcut does: in the same turn, before any frame.
    b.phase("open ⌘L (first)", |b| b.shortcut());
    let nodes_open = b.nodes();
    b.rest(Some("open, animation to rest"));
    b.press(Key::Escape);
    b.h.settle();
    b.rest(None);
    let t_calls = b.h.calls();
    let idle = {
        let (start, at) = (instructions(), Instant::now());
        let mut painted = 0;
        for _ in 0..120 {
            if b.frame() {
                painted += 1;
            }
        }
        (instructions() - start, at.elapsed().as_secs_f64() * 1e3, painted, b.h.calls() - t_calls)
    };

    let mut landed = 0;
    let mut tried = 0;
    for _ in 0..runs {
        b.phase("(remove navbar)", |b| { b.h.host.run_command("bench_rebuild"); });
        b.rest(None);
        b.phase("build navbar (modules loaded)", |b| { b.h.host.run_command("bench_rebuild"); });
        // What it builds after its first frame, if anything, is part of the bill.
        b.rest(Some("build navbar, then to rest"));
        std::thread::sleep(Duration::from_millis(130));
        b.phase("omnibox prebuild, idle after first frame", |_| {});
        b.rest(None);
        b.failed();
    }
    for run in 0..runs {
        b.phase("open ⌘L", |b| b.shortcut());
        b.rest(None);
        for (i, c) in QUERY.chars().enumerate() {
            let name = if i == 0 { "first key (results open)" } else { "next key" };
            b.phase(name, |b| b.typed(c));
        }
        b.rest(None);
        b.phase("arrow down", |b| b.press(Key::ArrowDown));
        b.phase("arrow down", |b| b.press(Key::ArrowDown));
        b.rest(None);
        // Select all and delete: the list empties back to its first rows.
        b.phase("close (Esc)", |b| b.press(Key::Escape));
        b.rest(Some("close, animation and tidying"));
        std::thread::sleep(Duration::from_millis(30));
        b.phase("open ⌘L", |b| b.shortcut());
        b.rest(None);
        b.phase("Tab to Ask", |b| b.press(Key::Tab));
        b.rest(Some("Tab to Ask, animation to rest"));
        std::thread::sleep(Duration::from_millis(150));
        b.h.settle();
        b.rest(None);
        b.phase("Esc, Ask back to Search", |b| b.press(Key::Escape));
        b.rest(None);
        b.phase("close (Esc)", |b| b.press(Key::Escape));
        b.rest(None);

        // Typed in the same turn as the shortcut: what the host's held keys do (mac.rs release_keys).
        if run % 2 == 0 {
            tried += 1;
            b.shortcut();
            b.h.settle();
            let on_field = b.h.ui.focused_editor().is_some();
            if on_field {
                landed += 1;
            }
            b.frame();
            b.rest(None);
            b.press(Key::Escape);
            b.h.settle();
            b.rest(None);
        }
        b.failed();
    }
    for (name, samples) in std::mem::take(&mut b.results) {
        let entry = all.entry(name).or_default();
        entry.0.extend(samples.0);
        entry.1.extend(samples.1);
        for (phase, instr) in samples.2 {
            *entry.2.entry(phase).or_default() += instr;
        }
    }

    let mut json = String::from("{");
    println!("{:<42} {:>5} {:>8} {:>8} {:>7} {:>7} {:>7} {:>7} {:>7}", "phase", "n", "instr M", "p95 M", "ms", "p95 ms", "calls", "layout", "paint");
    for (name, s) in &all {
        let (i, t) = s.sorted();
        let (med, p95) = (i[i.len() / 2] as f64 / 1e6, i[(i.len() * 95 / 100).min(i.len() - 1)] as f64 / 1e6);
        let (tm, tp) = (t[t.len() / 2], t[(t.len() * 95 / 100).min(t.len() - 1)]);
        let (calls, layout, painted) = s.work();
        println!("{name:<42} {:>5} {med:>8.2} {p95:>8.2} {tm:>7.2} {tp:>7.2} {calls:>7} {layout:>7} {painted:>7}", i.len());
        json += &format!("\"{name}\":{{\"instr_m\":{med:.3},\"p95_m\":{p95:.3},\"ms\":{tm:.3},\"p95_ms\":{tp:.3}}},");
    }
    println!("\nwhere it goes, M instructions per sample:");
    for (name, s) in &all {
        if s.2.is_empty() {
            continue;
        }
        let n = s.0.len() as f64;
        let parts: Vec<String> = s.2.iter().filter(|(_, i)| **i as f64 / n > 50_000.).map(|(p, i)| format!("{p} {:.2}", *i as f64 / n / 1e6)).collect();
        println!("  {name:<40} {}", parts.join("  "));
    }
    println!(
        "nodes: {nodes_closed} closed, {nodes_open} open\nidle, 120 frames: {:.2} M instructions, {:.2} ms, {} painted, {} script calls\nfield has the keyboard the moment ⌘L runs: {landed} of {tried}",
        idle.0 as f64 / 1e6, idle.1, idle.2, idle.3
    );
    json += &format!(
        "\"nodes_closed\":{nodes_closed},\"nodes_open\":{nodes_open},\"idle_m\":{:.3},\"idle_painted\":{},\"idle_calls\":{},\"focus_same_turn\":\"{landed}/{tried}\"}}",
        idle.0 as f64 / 1e6, idle.2, idle.3
    );
    if let Some(out) = std::env::var_os("OUT") {
        std::fs::write(out, json).unwrap();
    }
}
