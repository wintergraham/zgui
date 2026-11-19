//! How evenly a script app's frames take their time, headless, every frame
//! timed: the dashboard's live feed over 100,000 rows (`refresh` each
//! frame), and mail read through message by message (the next one opened
//! each frame, new mail received every 50th). Prints each scene's
//! distribution and slowest frames, and what a full collection of its
//! script heap takes: the pause the memory watch's collection (halfway to
//! the memory limit) would put in a frame.
//! Run: cargo run --release -p zgx-script --example frame_spikes
//! (`FRAMES` sets the frames, 600 by default; `ONLY=dashboard|mail`.)
use std::time::{Duration, Instant};
use zgx::input::Key;
use zgx_script::{Limits, ScriptHost, testing::TestApp};

const DASHBOARD: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/dashboard/app.luau");
const MAIL: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/mail/app.luau");

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

/// Time `frames` frames of `app`, each after `step(frame)`, and report.
fn scene(name: &str, mut app: TestApp, frames: usize, mut step: impl FnMut(&mut TestApp, usize)) {
    let mut times = Vec::with_capacity(frames);
    for frame in 0..frames {
        let started = Instant::now();
        step(&mut app, frame);
        app.frame();
        app.ui.scene.borrow_mut().flush();
        times.push(started.elapsed());
    }
    assert_eq!(app.error(), None, "{name} ran without an error");
    let mut sorted = times.clone();
    sorted.sort();
    let at = |q: f64| sorted[((sorted.len() - 1) as f64 * q) as usize];
    println!(
        "{name}: {frames} frames, p50 {:.2} ms, p90 {:.2}, p99 {:.2}, max {:.2}; over 1 ms {}, over 8 ms {}",
        ms(at(0.5)),
        ms(at(0.9)),
        ms(at(0.99)),
        ms(*sorted.last().unwrap()),
        times.iter().filter(|t| t.as_secs_f64() > 1e-3).count(),
        times.iter().filter(|t| t.as_secs_f64() > 8e-3).count(),
    );
    let mut slowest: Vec<(usize, Duration)> = times.iter().copied().enumerate().collect();
    slowest.sort_by_key(|&(_, t)| std::cmp::Reverse(t));
    let shown: Vec<String> = slowest[..8.min(slowest.len())]
        .iter()
        .map(|(i, t)| format!("#{i} {:.2} ms", ms(*t)))
        .collect();
    println!("  slowest: {}", shown.join(", "));
    let started = Instant::now();
    let held = app.host.script_memory();
    println!(
        "  a full collection: {:.2} ms, {:.1} MB held",
        ms(started.elapsed()),
        held as f64 / 1e6
    );
}

fn main() {
    let frames: usize = std::env::var("FRAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(600);
    let only = std::env::var("ONLY").ok();
    let wanted = |name: &str| only.as_deref().is_none_or(|only| only == name);
    let limits = Limits {
        load: Duration::from_secs(5),
        // `MEM_MB` caps a VM's heap so the memory watch's collection runs
        // in the measured frames: its in-frame cost shows in the tail.
        memory: std::env::var("MEM_MB")
            .ok()
            .and_then(|mb| mb.parse().ok())
            .map_or(1 << 30, |mb: usize| mb << 20),
        ..Limits::default()
    };
    if wanted("dashboard") {
        let app = TestApp::new(|runtime| ScriptHost::new(runtime, DASHBOARD).limits(limits));
        app.ui.scene.borrow_mut().resize(1280., 800.);
        app.frames(2);
        scene("dashboard", app, frames, |app, _| {
            assert!(app.host.run_command("refresh"));
        });
    }
    if wanted("gc") {
        // Nothing but garbage churn: each "churn" drops tens of megabytes
        // of dead tables faster than the VM's own pacing frees them, so
        // under `MEM_MB` the memory watch's collections are the tail.
        let app = TestApp::new(|runtime| {
            ScriptHost::inline(
                runtime,
                "gc",
                r#"
ui.command { id = "churn", run = function()
	local junk = {}
	for i = 1, 200000 do
		junk[i] = { i, "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx" }
	end
end }
return ui.column { ui.text { "gc" } }
"#,
            )
            .limits(limits)
        });
        app.frames(2);
        scene("gc", app, frames, |app, _| {
            assert!(app.host.run_command("churn"));
        });
    }
    if wanted("mail") {
        let mut app = TestApp::new(|runtime| ScriptHost::new(runtime, MAIL).limits(limits));
        app.ui.scene.borrow_mut().resize(1180., 760.);
        app.frames(2);
        app.click_row("messages", 1);
        app.frames(1);
        scene("mail", app, frames, |app, frame| {
            if frame % 50 == 49 {
                assert!(app.host.run_command("receive"));
            } else {
                app.key(Key::ArrowDown);
            }
        });
    }
}
