//! `ZGUI_PROFILE=1`: time each frame's phases and stream them to the frame
//! profiler (`zgui-script`'s `frame_profiler` example) over a Unix socket,
//! `$TMPDIR/zgui-profile/<pid>.sock`.
//!
//! The stream is text, one record a line, fields `key=value`:
//!
//! - `P pid=… name=…`: the process, first on every connection.
//! - `W win=… title=…`: a window (the title runs to the end of the line).
//! - `F win=… n=… t=… input=… update=… layout=… paint=… render=… present=…
//!   wait=… gpu=… draws=… passes=… blur=… instances=… shaped=… glyphs=…
//!   repaints=… hits=… damage=… nodes=…`: a frame that rendered or presented.
//!   Times are milliseconds: `t` since recording began, the phases' CPU time,
//!   `wait` the time held for the refresh, `gpu` the GPU's run of the frame
//!   (macOS, else absent). `damage` is damaged pixels, `nodes` laid out nodes.
//!
//! Phases: `input` handles window events, `update` runs tasks and timers
//! (scripts, effects, animations), `layout` settles layout, `paint` flushes
//! the scene into damage, `render` encodes it, `present` hands it over.
use std::time::{Duration, Instant};
use zgui_gpu::GpuStats;

#[derive(Clone, Copy)]
pub(crate) enum Phase {
    Input,
    Update,
    Layout,
    Paint,
    Render,
    Present,
    Wait,
}
const PHASES: [&str; 7] = [
    "input", "update", "layout", "paint", "render", "present", "wait",
];

pub(crate) struct FrameProfile {
    window: u64,
    index: u64,
    phases: [Duration; 7],
    /// Spans outside draws (event handling, task polls), innermost last,
    /// each counting since its instant. A draw pauses them, so the work
    /// before it in the same pass is that frame's.
    open: Vec<(Phase, Instant)>,
    /// The current draw: when it began, and its last checkpoint.
    draw: Option<(Instant, Instant)>,
    stats: Option<GpuStats>,
    nodes: usize,
    gpu: Option<Duration>,
    presented: bool,
    /// When the phases began adding up: work long before a frame (an idle
    /// window's timers) is dropped rather than charged to it.
    since: Instant,
}
impl FrameProfile {
    pub(crate) fn from_env(title: &str) -> Option<Self> {
        let sink = sink::get()?;
        let window = sink.window(title);
        Some(Self {
            window,
            index: 0,
            phases: Default::default(),
            open: Vec::new(),
            draw: None,
            stats: None,
            nodes: 0,
            gpu: None,
            presented: false,
            since: Instant::now(),
        })
    }
    pub(crate) fn title(&self, title: &str) {
        if let Some(sink) = sink::get() {
            sink.retitle(self.window, title);
        }
    }
    /// Count the time from now to `leave` as `phase`.
    pub(crate) fn enter(&mut self, phase: Phase) {
        let now = self.pause();
        self.open.push((phase, now));
    }
    pub(crate) fn leave(&mut self) {
        let now = self.pause();
        self.open.pop();
        if let Some((_, at)) = self.open.last_mut() {
            *at = now;
        }
    }
    /// Charge the innermost open span up to now.
    fn pause(&mut self) -> Instant {
        let now = Instant::now();
        if let Some((phase, at)) = self.open.last_mut() {
            self.phases[*phase as usize] += now - *at;
            *at = now;
        }
        now
    }
    pub(crate) fn begin_draw(&mut self) {
        let now = self.pause();
        self.draw = Some((now, now));
    }
    /// Count the time since the last checkpoint as `phase`.
    pub(crate) fn lap(&mut self, phase: Phase) {
        if let Some((_, last)) = &mut self.draw {
            let now = Instant::now();
            self.phases[phase as usize] += now - *last;
            *last = now;
        }
    }
    pub(crate) fn rendered(&mut self, stats: &GpuStats, nodes: usize) {
        self.stats = Some(*stats);
        self.nodes = nodes;
    }
    /// Move `time` counted as `from` to `to`.
    pub(crate) fn moved(&mut self, from: Phase, to: Phase, time: Duration) {
        let time = time.min(self.phases[from as usize]);
        self.phases[from as usize] -= time;
        self.phases[to as usize] += time;
    }
    pub(crate) fn presented(&mut self, gpu: Option<Duration>) {
        self.presented = true;
        self.gpu = gpu;
    }
    /// End a draw; one that rendered or presented sends its frame.
    pub(crate) fn end_draw(&mut self) {
        let Some((began, _)) = self.draw.take() else {
            return;
        };
        if let Some((_, at)) = self.open.last_mut() {
            *at = Instant::now();
        }
        if self.stats.is_none() && !self.presented {
            if self.since.elapsed() > Duration::from_millis(100) {
                self.phases = Default::default();
                self.since = Instant::now();
            }
            return;
        }
        self.index += 1;
        let Some(sink) = sink::get() else { return };
        let ms = |d: Duration| d.as_secs_f64() * 1000.;
        let mut line = format!(
            "F win={} n={} t={:.3}",
            self.window,
            self.index,
            ms(began.duration_since(sink.epoch))
        );
        for (name, time) in PHASES.iter().zip(self.phases) {
            line += &format!(" {name}={:.4}", ms(time));
        }
        if let Some(gpu) = self.gpu {
            line += &format!(" gpu={:.4}", ms(gpu));
        }
        let s = self.stats.take().unwrap_or_default();
        line += &format!(
            " draws={} passes={} blur={} instances={} shaped={} glyphs={} repaints={} hits={} damage={} nodes={}",
            s.draw_calls,
            s.render_passes,
            s.blur_passes,
            s.instances,
            s.shaped_nodes,
            s.glyph_uploads,
            s.layer_repaints,
            s.layer_cache_hits,
            s.damaged_pixels,
            self.nodes,
        );
        sink.send(line);
        self.phases = Default::default();
        self.since = Instant::now();
        self.nodes = 0;
        self.gpu = None;
        self.presented = false;
    }
}

#[cfg(unix)]
mod sink {
    use std::io::{BufWriter, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::sync::{Arc, Mutex, OnceLock, mpsc};
    use std::time::Instant;

    pub(crate) struct Sink {
        pub(crate) epoch: Instant,
        lines: mpsc::Sender<String>,
        /// What a new connection is sent first: the process and its windows.
        intro: Arc<Mutex<Vec<String>>>,
        next_window: std::sync::atomic::AtomicU64,
    }
    impl Sink {
        pub(crate) fn window(&self, title: &str) -> u64 {
            let id = self
                .next_window
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.retitle(id, title);
            id
        }
        pub(crate) fn retitle(&self, window: u64, title: &str) {
            let line = format!("W win={window} title={}", title.replace('\n', " "));
            let mut intro = self.intro.lock().unwrap();
            let prefix = format!("W win={window} ");
            intro.retain(|known| !known.starts_with(&prefix));
            intro.push(line.clone());
            drop(intro);
            let _ = self.lines.send(line);
        }
        pub(crate) fn send(&self, line: String) {
            let _ = self.lines.send(line);
        }
    }

    pub(crate) fn get() -> Option<&'static Sink> {
        static SINK: OnceLock<Option<Sink>> = OnceLock::new();
        SINK.get_or_init(|| {
            std::env::var_os("ZGUI_PROFILE")?;
            match start() {
                Ok(sink) => Some(sink),
                Err(error) => {
                    eprintln!("ZGUI_PROFILE: {error}");
                    None
                }
            }
        })
        .as_ref()
    }

    fn start() -> std::io::Result<Sink> {
        let dir = std::env::temp_dir().join("zgui-profile");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path)?;
        eprintln!("ZGUI_PROFILE: recording frames at {}", path.display());
        let name = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "zgui".into());
        let intro = Arc::new(Mutex::new(vec![format!(
            "P pid={} name={name}",
            std::process::id()
        )]));
        let clients = Arc::new(Mutex::new(Vec::<BufWriter<UnixStream>>::new()));
        {
            let (intro, clients) = (intro.clone(), clients.clone());
            std::thread::Builder::new()
                .name("zgui-profile-accept".into())
                .spawn(move || {
                    for stream in listener.incoming().flatten() {
                        let mut client = BufWriter::new(stream);
                        let ok = intro
                            .lock()
                            .unwrap()
                            .iter()
                            .try_for_each(|line| writeln!(client, "{line}"))
                            .and_then(|()| client.flush());
                        if ok.is_ok() {
                            clients.lock().unwrap().push(client);
                        }
                    }
                })?;
        }
        let (lines, received) = mpsc::channel::<String>();
        std::thread::Builder::new()
            .name("zgui-profile-send".into())
            .spawn(move || {
                // Frames batch per wakeup; a client that fails is dropped.
                while let Ok(first) = received.recv() {
                    let mut clients = clients.lock().unwrap();
                    for line in std::iter::once(first).chain(received.try_iter()) {
                        clients.retain_mut(|client| writeln!(client, "{line}").is_ok());
                    }
                    clients.retain_mut(|client| client.flush().is_ok());
                }
            })?;
        Ok(Sink {
            epoch: Instant::now(),
            lines,
            intro,
            next_window: 1.into(),
        })
    }
}

#[cfg(not(unix))]
mod sink {
    pub(crate) struct Sink {
        pub(crate) epoch: std::time::Instant,
    }
    impl Sink {
        pub(crate) fn window(&self, _: &str) -> u64 {
            0
        }
        pub(crate) fn retitle(&self, _: u64, _: &str) {}
        pub(crate) fn send(&self, _: String) {}
    }
    pub(crate) fn get() -> Option<&'static Sink> {
        None
    }
}
