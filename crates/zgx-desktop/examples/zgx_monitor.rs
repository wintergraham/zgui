//! Glass resource monitor: launches the kitchen sink, or another example with
//! `--app <example>` (or attaches with `--pid <pid>`, or neither with
//! `--app none`), and charts CPU, GPU and
//! memory over the last minute for any process on the machine. `--attach
//! <name>` launches nothing and narrows the sidebar to the processes of that
//! name and everything they started, as its Attach menu does. GPU time per
//! process comes from the GPU driver's per-client accounting (macOS: each
//! `AGXDeviceUserClient`'s `AppUsage`), the way Activity Monitor reads it.
//!
//! The interface is Luau (`examples/zgx_monitor/app.luau` and the modules it
//! requires); edit and save to reload it. It lays panels out as the terminal
//! example lays out panes: split beside or below, resized by their dividers,
//! moved by their headers onto another panel's edge (or its middle, to swap),
//! with processes dragged from the sidebar the same way.
//!
//! This file samples: every process, every half second, published to the
//! script as shared state (`procs`, `summary`), and the last minute of each
//! process a panel shows (`history:<pid>`, for the pids in `watch`).
//! Run: cargo run --release -p zgx-desktop --example zgx_monitor
//! (build the target first: cargo build --release -p zgx-desktop --example kitchen_sink)

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    process::{Child, Command},
    rc::Rc,
    time::{Duration, Instant},
};
use zgx::{reactive::Runtime, timer::sleep};
use zgx_desktop::{Application, FontData, Glass, Titlebar, WindowOptions};
use zgx_script::{ScriptHost, Value};

const TITLE: &str = "zgx monitor";
const WIDTH: f64 = 1280.;
const HEIGHT: f64 = 780.;
const INTERVAL: Duration = Duration::from_millis(500);
/// One minute of samples.
const CAPACITY: usize = 120;

#[derive(Clone, Copy, PartialEq)]
struct Sample {
    cpu: f32,
    /// Percent of one GPU's time, like `cpu` is of one core's.
    gpu: f32,
    memory_mb: f32,
}

/// Sampler state per process, kept past exit while a panel shows it.
struct Track {
    start: u64,
    name: Rc<str>,
    /// What the process's app calls it (a browser tab's title), if it says.
    label: Option<Rc<str>>,
    /// The parent when first seen.
    parent: i32,
    cpu_ns: u64,
    /// Accumulated GPU time, if the process has ever used the GPU.
    gpu_ns: Option<u64>,
    at: Instant,
    /// Smoothed CPU for a sort order that doesn't jitter every sample.
    smooth: f32,
    samples: VecDeque<Sample>,
}

/// The scripts: in the app bundle the monitor ships in, otherwise this checkout's.
fn script() -> PathBuf {
    let shipped = std::env::current_exe().ok().and_then(|exe| {
        let path = exe.parent()?.parent()?.join("Resources/monitor/app.luau");
        path.exists().then_some(path)
    });
    shipped.unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/zgx_monitor/app.luau")
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let script = script();
    let mut application = Application::new();
    // Geist, the interface's typeface (`font.sans` in zgx_monitor/glass.json).
    for bytes in [
        &include_bytes!("support/fonts/Geist.ttf")[..],
        include_bytes!("support/fonts/Geist-Medium.ttf"),
        include_bytes!("support/fonts/Geist-SemiBold.ttf"),
    ] {
        application = application.font(FontData::new(std::sync::Arc::<[u8]>::from(bytes))?);
    }
    application
        .window(WindowOptions {
            title: TITLE.into(),
            width: WIDTH,
            height: HEIGHT,
            decorations: cfg!(target_os = "macos"),
            min_size: Some((760., 520.)),
            titlebar: Titlebar::Inset,
            glass: Some(Glass::default()),
            ..Default::default()
        })
        .run(move |cx| {
            // Launch the target beside this binary unless attaching.
            let mut error = String::new();
            let child: Rc<RefCell<Option<Child>>> = Rc::default();
            let attach = attach_name();
            let launched = match attach_pid() {
                Some(pid) => Some(pid),
                None if target() == "none" || attach.is_some() => None,
                None => match launch_target() {
                    Ok(process) => {
                        let pid = process.id() as i32;
                        *child.borrow_mut() = Some(process);
                        Some(pid)
                    }
                    Err(message) => {
                        error = message;
                        None
                    }
                },
            };

            let host = ScriptHost::new(&cx.ui.runtime, script).app_id("zgx-monitor");
            // Panels stay concentric with the window's corners.
            let radius = host.shared("window_radius", 10.);
            let corner = cx.window_info.corner_radius.clone();
            let follow_radius = cx.ui.runtime.effect(move || {
                let r = corner.get();
                if r > 0. {
                    radius.set(r.into());
                }
            });
            host.shared(
                "launched",
                launched.map_or(Value::Nil, |pid| num(pid as f32)),
            );
            // The first panel shows the attached process, if it is running:
            // the largest, when several go by that name.
            let attach_pid = attach.as_deref().and_then(|name| {
                usage::pids()
                    .into_iter()
                    .filter(|pid| usage::name(*pid) == name)
                    .max_by_key(|pid| usage::read(*pid).map_or(0, |usage| usage.footprint))
            });
            host.shared("attach", attach.as_deref().unwrap_or(""));
            host.shared(
                "attach_pid",
                attach_pid.map_or(Value::Nil, |pid| num(pid as f32)),
            );
            host.shared("error", error.as_str());
            host.shared("interval", INTERVAL.as_millis() as f64);
            host.shared("capacity", CAPACITY as f64);
            let window = cx.window.clone();
            host.action("close_window", move |_| window.close());

            let sampler = Sampler {
                host: host.clone(),
                runtime: cx.ui.runtime.clone(),
                tracks: Rc::default(),
                procs: host.shared("procs", Value::List(Rc::new([]))),
                summary: host.shared("summary", Value::Nil),
                counts: host.shared("counts", Value::List(Rc::new([]))),
                watch: host.shared("watch", Value::List(Rc::new([]))),
            };
            // A panel opening on a process shows its last minute at once,
            // not at the next sample.
            let publish = {
                let sampler = sampler.clone();
                let watch = sampler.watch.clone();
                cx.ui.runtime.effect(move || {
                    watch.get();
                    sampler.publish_history();
                })
            };
            cx.tasks.spawn(async move {
                loop {
                    sampler.sample();
                    sleep(INTERVAL).await;
                }
            });

            let _ = host.reload();
            cx.tasks.spawn(host.watch(Duration::from_millis(250)));

            // The menus are the script's commands.
            let commands = host.commands();
            let windows = cx.windows.clone();
            let menus = cx.ui.runtime.effect(move || {
                let list = commands.get();
                if let Err(error) = windows.set_menus(zgx_run::menus(&list)) {
                    eprintln!("zgx_monitor: menus: {error}");
                }
            });
            let commands = host.clone();
            cx.on_menu_action(move |action| {
                commands.run_command(&action.0);
            });
            // The monitor owns the process it launched.
            let keep = RefCell::new(Some((menus, follow_radius, publish)));
            cx.on_closed(move || {
                drop(keep.borrow_mut().take());
                if let Some(mut process) = child.borrow_mut().take() {
                    let _ = process.kill();
                    let _ = process.wait();
                }
            });
            // `ui.window.titlebar_area`: where the title bar leaves the script room.
            host.window_chrome(
                cx.window_info.titlebar_area.clone(),
                cx.window_info.controls.clone(),
            );
            cx.render(host.view());
        })
}

fn attach_pid() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|arg| arg == "--pid")
        .and_then(|i| args.get(i + 1))
        .and_then(|pid| pid.parse().ok())
}

/// The process name whose tree the sidebar starts narrowed to.
fn attach_name() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|arg| arg == "--attach")
        .and_then(|i| args.get(i + 1))
        .cloned()
}

/// The example to launch: `--app <name>`, the kitchen sink by default;
/// `--app none` launches nothing.
fn target() -> String {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|arg| arg == "--app")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "kitchen_sink".into())
}

fn launch_target() -> Result<Child, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let name = target();
    let path = exe.with_file_name(&name);
    Command::new(&path).spawn().map_err(|e| {
        format!(
            "Could not start {} ({e}). Build it with: cargo build --release -p zgx-desktop --example {name}",
            path.display()
        )
    })
}

fn num(value: f32) -> Value {
    Value::Num(value as f64)
}

/// A script table from its fields, keys shared across every table made.
fn map<const N: usize>(fields: [(&'static str, Value); N]) -> Value {
    thread_local! {
        static KEYS: RefCell<HashMap<&'static str, Rc<str>>> = RefCell::default();
    }
    let mut fields: Vec<(Rc<str>, Value)> = KEYS.with_borrow_mut(|keys| {
        fields
            .into_iter()
            .map(|(key, value)| (keys.entry(key).or_insert_with(|| key.into()).clone(), value))
            .collect()
    });
    fields.sort_by(|a, b| a.0.cmp(&b.0));
    Value::Map(fields.into())
}

/// Samples every readable process, so a newly opened panel already has its
/// last minute of history.
#[derive(Clone)]
struct Sampler {
    host: ScriptHost,
    runtime: Runtime,
    tracks: Rc<RefCell<HashMap<i32, Track>>>,
    /// `{ pid, ppid, name, cpu, gpu, mem, smooth }` for every live process.
    procs: zgx_script::Shared,
    /// `{ count, cpu, gpu }`: the whole machine.
    summary: zgx_script::Shared,
    /// `{ owner, label, count }`: what apps count of their own.
    counts: zgx_script::Shared,
    /// The pids the script's panels show (it writes this).
    watch: zgx_script::Shared,
}
impl Sampler {
    fn watched(&self) -> HashSet<i32> {
        self.watch.with_untracked(|watch| {
            watch
                .list()
                .iter()
                .filter_map(Value::as_f64)
                .map(|pid| pid as i32)
                .collect()
        })
    }
    fn sample(&self) {
        let watched = self.watched();
        let labels = labels::read();
        let procs = sample(&mut self.tracks.borrow_mut(), &watched, &labels.names);
        let counts: Vec<Value> = labels
            .counts
            .into_iter()
            .map(|(owner, label, count)| {
                map([
                    ("owner", num(owner as f32)),
                    ("label", Value::Str(label)),
                    ("count", num(count)),
                ])
            })
            .collect();
        let cpu: f32 = procs.iter().map(|p| p.2).sum();
        let summary = map([
            ("count", num(procs.len() as f32)),
            ("cpu", num(cpu)),
            ("gpu", usage::device_gpu().map_or(Value::Nil, num)),
        ]);
        let rows: Vec<Value> = procs
            .into_iter()
            .map(|(pid, name, cpu, gpu, memory_mb, smooth, parent, label)| {
                map([
                    ("pid", num(pid as f32)),
                    ("ppid", num(parent as f32)),
                    ("name", Value::Str(name)),
                    ("label", label.map_or(Value::Nil, Value::Str)),
                    ("cpu", num(cpu)),
                    ("gpu", num(gpu)),
                    ("mem", num(memory_mb)),
                    ("smooth", num(smooth)),
                ])
            })
            .collect();
        self.runtime.batch(|| {
            self.procs.set(Value::List(rows.into()));
            self.summary.set(summary);
            self.counts.set(Value::List(counts.into()));
            self.publish_history();
        });
    }
    /// Each watched process's last minute, as `history:<pid>`:
    /// `{ name, cpu = { … }, gpu = { … }, mem = { … } }`, oldest first.
    fn publish_history(&self) {
        let tracks = self.tracks.borrow();
        for pid in self.watched() {
            let Some(track) = tracks.get(&pid) else {
                continue;
            };
            let series = |read: fn(&Sample) -> f32| {
                Value::List(track.samples.iter().map(|s| num(read(s))).collect())
            };
            let history = map([
                ("name", Value::Str(track.name.clone())),
                ("label", track.label.clone().map_or(Value::Nil, Value::Str)),
                ("cpu", series(|s| s.cpu)),
                ("gpu", series(|s| s.gpu)),
                ("mem", series(|s| s.memory_mb)),
            ]);
            self.host
                .shared(&format!("history:{pid}"), Value::Nil)
                .set(history);
        }
    }
}

/// One reading per live process: pid, name, CPU, GPU, memory (MB), the
/// smoothed CPU, the parent's pid and the label its app gave it.
fn sample(
    tracks: &mut HashMap<i32, Track>,
    open: &HashSet<i32>,
    labels: &HashMap<i32, Rc<str>>,
) -> Vec<(i32, Rc<str>, f32, f32, f32, f32, i32, Option<Rc<str>>)> {
    let now = Instant::now();
    let mut seen = HashSet::new();
    let mut procs = Vec::new();
    let gpu_times = usage::gpu_times();
    for pid in usage::pids() {
        let Some(usage) = usage::read(pid) else {
            continue;
        };
        seen.insert(pid);
        let label = labels.get(&pid).cloned();
        let memory_mb = usage.footprint as f32 / (1024. * 1024.);
        let track = match tracks.get_mut(&pid) {
            // A reused pid is a different process.
            Some(track) if track.start == usage.start => track,
            _ => {
                let name: Rc<str> = usage::name(pid).into();
                let parent = usage::parent(pid);
                tracks.insert(
                    pid,
                    Track {
                        start: usage.start,
                        name: name.clone(),
                        label: label.clone(),
                        parent,
                        cpu_ns: usage.cpu_ns,
                        gpu_ns: gpu_times.get(&pid).copied(),
                        at: now,
                        smooth: 0.,
                        samples: VecDeque::new(),
                    },
                );
                procs.push((pid, name, 0., 0., memory_mb, 0., parent, label));
                continue;
            }
        };
        // Kept once the app stops saying, as when it quits with its helpers still going.
        if label.is_some() {
            track.label = label.clone();
        }
        let wall = now.duration_since(track.at).as_nanos().max(1) as f64;
        let cpu = (usage.cpu_ns.saturating_sub(track.cpu_ns) as f64 / wall * 100.) as f32;
        track.cpu_ns = usage.cpu_ns;
        let gpu_ns = gpu_times.get(&pid).copied();
        // A process that closes its last GPU client drops back to zero.
        let gpu = match (gpu_ns, track.gpu_ns) {
            (Some(now_ns), Some(then)) => (now_ns.saturating_sub(then) as f64 / wall * 100.) as f32,
            _ => 0.,
        };
        track.gpu_ns = gpu_ns;
        track.at = now;
        track.smooth = track.smooth * 0.8 + cpu * 0.2;
        if track.samples.len() == CAPACITY {
            track.samples.pop_front();
        }
        track.samples.push_back(Sample {
            cpu,
            gpu,
            memory_mb,
        });
        procs.push((
            pid,
            track.name.clone(),
            cpu,
            gpu,
            memory_mb,
            track.smooth,
            track.parent,
            track.label.clone(),
        ));
    }
    // Exited processes keep their history while a panel shows them.
    tracks.retain(|pid, _| seen.contains(pid) || open.contains(pid));
    procs
}

/// What apps call their own processes: a process can't be renamed for other
/// tools on macOS, so an app that wants its helpers told apart writes
/// `<temp>/zgx-monitor/labels-<its pid>`, one `pid<TAB>label` per line, and
/// `#<TAB>label<TAB>count` for a total of its own (its open tabs, say). The
/// heartbeat says someone is reading; an app stops writing when it goes stale.
#[cfg(target_os = "macos")]
mod labels {
    use std::collections::HashMap;
    use std::os::unix::fs::DirBuilderExt;
    use std::rc::Rc;
    use std::time::SystemTime;

    #[derive(Default)]
    pub struct Labels {
        pub names: HashMap<i32, Rc<str>>,
        /// An app's pid, what it counts and how many.
        pub counts: Vec<(i32, Rc<str>, f32)>,
    }

    pub fn read() -> Labels {
        let mut labels = Labels::default();
        let directory = std::env::temp_dir().join("zgx-monitor");
        let _ = std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&directory);
        let heartbeat = std::fs::File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(directory.join("heartbeat"));
        if let Ok(file) = heartbeat {
            let _ = file.set_modified(SystemTime::now());
        }
        let Ok(entries) = std::fs::read_dir(&directory) else {
            return labels;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(owner) = name
                .to_str()
                .and_then(|name| name.strip_prefix("labels-"))
                .and_then(|pid| pid.parse::<i32>().ok())
            else {
                continue;
            };
            // Left by an app that has quit.
            if super::usage::read(owner).is_none() {
                let _ = std::fs::remove_file(entry.path());
                continue;
            }
            let Ok(text) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            for line in text.lines() {
                if let Some(count) = line.strip_prefix("#\t") {
                    if let Some((label, count)) = count.split_once('\t')
                        && let Ok(count) = count.parse()
                    {
                        labels.counts.push((owner, label.into(), count));
                    }
                } else if let Some((pid, label)) = line.split_once('\t')
                    && let Ok(pid) = pid.parse()
                    && !label.is_empty()
                {
                    labels.names.insert(pid, label.into());
                }
            }
        }
        labels
    }
}
#[cfg(not(target_os = "macos"))]
mod labels {
    #[derive(Default)]
    pub struct Labels {
        pub names: std::collections::HashMap<i32, std::rc::Rc<str>>,
        pub counts: Vec<(i32, std::rc::Rc<str>, f32)>,
    }
    pub fn read() -> Labels {
        Labels::default()
    }
}

use zgx::process::platform as usage;

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::usage;

    #[test]
    fn creator_names_its_pid() {
        assert_eq!(usage::creator_pid("pid 395, WindowServer"), Some(395));
        assert_eq!(usage::creator_pid("pid 7,x"), Some(7));
        assert_eq!(usage::creator_pid("WindowServer"), None);
    }

    #[test]
    fn parent_reads_this_process_parent() {
        let pid = std::process::id() as i32;
        assert_eq!(
            usage::parent(pid),
            std::os::unix::process::parent_id() as i32
        );
    }

    #[test]
    fn window_server_has_gpu_time() {
        // Every logged-in Mac composites through WindowServer on the GPU.
        let times = usage::gpu_times();
        assert!(times.values().any(|ns| *ns > 0), "no GPU accounting read");
    }
}
