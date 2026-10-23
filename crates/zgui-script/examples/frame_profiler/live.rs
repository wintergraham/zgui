//! The live feed: frames an app run with `ZGUI_PROFILE=1` streams (see
//! zgui-desktop's `profile` module), read on a thread and published to the
//! scripts as `ui.shared("live")`. It attaches to the newest app recording,
//! or the one `ZGUI_PROFILE_TARGET` names (a pid or a process name).
use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;
use zgui_script::Value;

/// Frames kept: 30 s at 120 Hz, the longest range.
const KEPT: usize = 3600;
/// Bars in the chart.
const BARS: usize = 120;
const PHASES: [&str; 7] = [
    "input", "update", "layout", "paint", "render", "present", "wait",
];

enum Event {
    Attached { pid: u32, name: String },
    Frame(Frame),
    Detached,
}
#[derive(Clone)]
struct Frame {
    n: f64,
    t: f64,
    fields: Vec<(String, f64)>,
}
impl Frame {
    fn get(&self, key: &str) -> f64 {
        self.fields
            .iter()
            .find(|(k, _)| k == key)
            .map_or(0., |(_, v)| *v)
    }
    fn cpu(&self) -> f64 {
        ["input", "update", "layout", "paint", "render", "present"]
            .iter()
            .map(|k| self.get(k))
            .sum()
    }
    fn value(&self) -> Value {
        let mut fields: Vec<(Rc<str>, Value)> = self
            .fields
            .iter()
            .map(|(k, v)| (k.as_str().into(), Value::Num(*v)))
            .collect();
        fields.push(("n".into(), Value::Num(self.n)));
        fields.push(("t".into(), Value::Num(self.t)));
        map(fields)
    }
}

fn map(mut fields: Vec<(Rc<str>, Value)>) -> Value {
    fields.sort_by(|a, b| a.0.cmp(&b.0));
    Value::Map(fields.into())
}

/// Recording state on the UI thread.
pub struct Live {
    events: mpsc::Receiver<Event>,
    target: Option<(u32, String)>,
    frames: VecDeque<Frame>,
}
impl Live {
    pub fn start() -> Self {
        let (send, events) = mpsc::channel();
        std::thread::Builder::new()
            .name("frame-profiler-feed".into())
            .spawn(move || feed(send))
            .expect("spawn the profiler feed");
        Self {
            events,
            target: None,
            frames: VecDeque::new(),
        }
    }
    /// A feed with no thread, read line by line (`read`): for tests.
    #[allow(dead_code)]
    pub fn offline() -> Self {
        let (_, events) = mpsc::channel();
        Self {
            events,
            target: None,
            frames: VecDeque::new(),
        }
    }
    /// Take one line of `pid`'s stream; the value to publish.
    #[allow(dead_code)]
    pub fn read(&mut self, pid: u32, line: &str) -> Value {
        if let Some(event) = parse(pid, line) {
            self.take(event);
        }
        self.value()
    }
    /// Take what arrived; `Some` when the published value changes.
    pub fn poll(&mut self) -> Option<Value> {
        let mut changed = false;
        while let Ok(event) = self.events.try_recv() {
            changed = true;
            self.take(event);
        }
        changed.then(|| self.value())
    }
    fn take(&mut self, event: Event) {
        match event {
            Event::Attached { pid, name } => {
                self.target = Some((pid, name));
                self.frames.clear();
            }
            Event::Frame(frame) => {
                if self.frames.len() == KEPT {
                    self.frames.pop_front();
                }
                self.frames.push_back(frame);
            }
            Event::Detached => self.target = None,
        }
    }
    fn value(&self) -> Value {
        let Some((pid, name)) = &self.target else {
            return Value::Bool(false);
        };
        let frames: Vec<&Frame> = self.frames.iter().collect();
        let last: Vec<Value> = frames[frames.len().saturating_sub(BARS)..]
            .iter()
            .map(|f| f.value())
            .collect();
        map(vec![
            ("pid".into(), Value::Num(*pid as f64)),
            ("name".into(), Value::Str(name.as_str().into())),
            ("count".into(), Value::Num(frames.len() as f64)),
            ("frames".into(), Value::List(last.into())),
            ("frames5".into(), self.buckets(5.)),
            ("frames30".into(), self.buckets(30.)),
            ("avg".into(), self.average(1000.)),
        ])
    }
    /// `seconds` up to the newest frame in 120 bars, each its slowest frame.
    fn buckets(&self, seconds: f64) -> Value {
        let Some(end) = self.frames.back().map(|f| f.t) else {
            return Value::List(Rc::from([]));
        };
        let width = seconds * 1000. / BARS as f64;
        let mut worst: Vec<Option<&Frame>> = vec![None; BARS];
        for frame in self.frames.iter().rev() {
            let back = ((end - frame.t) / width) as usize;
            if back >= BARS {
                break;
            }
            let slot = &mut worst[BARS - 1 - back];
            if slot.is_none_or(|w| frame.cpu() > w.cpu()) {
                *slot = Some(frame);
            }
        }
        let empty = map(vec![("empty".into(), Value::Bool(true))]);
        Value::List(
            worst
                .into_iter()
                .map(|f| f.map_or(empty.clone(), Frame::value))
                .collect(),
        )
    }
    /// Each field's mean over the last `ms` of frames.
    fn average(&self, ms: f64) -> Value {
        let Some(end) = self.frames.back().map(|f| f.t) else {
            return Value::Nil;
        };
        let recent: Vec<&Frame> = self
            .frames
            .iter()
            .rev()
            .take_while(|f| end - f.t <= ms)
            .collect();
        let keys = recent[0].fields.iter().map(|(k, _)| k.clone());
        let fields = keys
            .map(|k| {
                let mean = recent.iter().map(|f| f.get(&k)).sum::<f64>() / recent.len() as f64;
                (k.as_str().into(), Value::Num(mean))
            })
            .chain([("frames".into(), Value::Num(recent.len() as f64))])
            .collect();
        map(fields)
    }
}

/// Attach to a recording app and forward its frames, reattaching when it goes.
fn feed(send: mpsc::Sender<Event>) {
    let wanted = std::env::var("ZGUI_PROFILE_TARGET").ok();
    loop {
        let Some((pid, stream)) = attach(wanted.as_deref()) else {
            std::thread::sleep(Duration::from_millis(500));
            continue;
        };
        let mut name = String::new();
        for line in BufReader::new(stream).lines() {
            let Ok(line) = line else { break };
            let Some(event) = parse(pid, &line) else {
                continue;
            };
            if let Event::Attached { name: attached, .. } = &event {
                name.clone_from(attached);
            }
            if send.send(event).is_err() {
                return;
            }
        }
        eprintln!("frame_profiler: {name} ({pid}) stopped recording");
        if send.send(Event::Detached).is_err() {
            return;
        }
    }
}

/// One line of the stream (see zgui-desktop's `profile` module).
fn parse(pid: u32, line: &str) -> Option<Event> {
    let mut words = line.split(' ');
    let kind = words.next()?;
    let mut pairs = words.filter_map(|w| w.split_once('='));
    match kind {
        "P" => {
            let name = pairs.find(|(k, _)| *k == "name").map_or("", |(_, v)| v);
            Some(Event::Attached {
                pid,
                name: name.into(),
            })
        }
        "F" => {
            let mut frame = Frame {
                n: 0.,
                t: 0.,
                fields: Vec::new(),
            };
            for (key, value) in pairs {
                let Ok(value) = value.parse::<f64>() else {
                    continue;
                };
                match key {
                    "n" => frame.n = value,
                    "t" => frame.t = value,
                    "win" => {}
                    _ => frame.fields.push((key.into(), value)),
                }
            }
            for phase in PHASES {
                if !frame.fields.iter().any(|(k, _)| k == phase) {
                    frame.fields.push((phase.into(), 0.));
                }
            }
            Some(Event::Frame(frame))
        }
        _ => None,
    }
}

/// The newest recording socket (or the wanted one) that accepts a connection.
/// Sockets nothing listens on are left by apps that quit; they are removed.
fn attach(wanted: Option<&str>) -> Option<(u32, UnixStream)> {
    let dir = std::env::temp_dir().join("zgui-profile");
    let own = std::process::id();
    let mut sockets: Vec<(std::time::SystemTime, u32, PathBuf)> = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let pid: u32 = path.file_stem()?.to_str()?.parse().ok()?;
            let modified = entry.metadata().ok()?.modified().ok()?;
            (pid != own && path.extension()? == "sock").then_some((modified, pid, path))
        })
        .collect();
    sockets.sort_by_key(|socket| std::cmp::Reverse(socket.0));
    for (_, pid, path) in sockets {
        if wanted.is_some_and(|w| w.parse() != Ok(pid) && !process_named(pid, w)) {
            continue;
        }
        match UnixStream::connect(&path) {
            Ok(stream) => return Some((pid, stream)),
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => {
                let _ = std::fs::remove_file(&path);
            }
            Err(_) => {}
        }
    }
    None
}

fn process_named(pid: u32, name: &str) -> bool {
    std::process::Command::new("ps")
        .args(["-o", "comm=", "-p", &pid.to_string()])
        .output()
        .is_ok_and(|out| String::from_utf8_lossy(&out.stdout).trim().ends_with(name))
}
