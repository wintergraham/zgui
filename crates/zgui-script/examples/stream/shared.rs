//! What the two stream examples share: the token source, its timing and the
//! window. Only the interface layer differs: Rust views in `stream_rust`, a
//! Luau script in `stream_luau`.
use std::{
    cell::Cell,
    future::Future,
    rc::Rc,
    time::{Duration, Instant},
};
use zgui_desktop::{WindowContext, WindowOptions};

/// One token per 60 Hz period, as the benchmark workload streams.
pub const PERIOD: Duration = Duration::from_nanos(16_666_667);

#[rustfmt::skip]
const WORDS: &[&str] = &[
    "the", "stream", "renders", "retained", "views", "while", "tokens", "arrive", "from", "a",
    "model", "and", "each", "update", "touches", "only", "its", "own", "text", "node", "layout",
    "settles", "before", "the", "next", "frame", "so", "nothing", "else", "repaints", "quickly",
    "measured", "against", "scripts", "describing", "identical", "trees", "in", "luau", "rust",
];

/// The same text every run.
pub struct Corpus {
    seed: u64,
    left: u32,
}
impl Corpus {
    pub fn new() -> Self {
        Self {
            seed: 0x9e37_79b9_7f4a_7c15,
            left: 0,
        }
    }
    /// The next word, and whether it ends its paragraph.
    pub fn next(&mut self) -> (&'static str, bool) {
        self.seed = self
            .seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let r = (self.seed >> 33) as usize;
        if self.left == 0 {
            self.left = 40 + (r % 60) as u32;
        }
        self.left -= 1;
        (WORDS[r % WORDS.len()], self.left == 0)
    }
}

/// Starts and stops one stream at a time.
#[derive(Clone, Default)]
pub struct Streamer {
    generation: Rc<Cell<u64>>,
}
impl Streamer {
    /// A stream calling `token` once a period until `stop` or the next
    /// `start`. Spawn it on the window's tasks.
    pub fn start(
        &self,
        mut token: impl FnMut(&'static str, bool) + 'static,
    ) -> impl Future<Output = ()> + 'static {
        let id = self.generation.get() + 1;
        self.generation.set(id);
        let generation = self.generation.clone();
        async move {
            let mut corpus = Corpus::new();
            let mut next = Instant::now();
            loop {
                next += PERIOD;
                zgui::timer::sleep_until(next).await;
                if generation.get() != id {
                    return;
                }
                let (word, ends) = corpus.next();
                token(word, ends);
            }
        }
    }
    pub fn stop(&self) {
        self.generation.set(self.generation.get() + 1);
    }
}

/// `STREAM_START_AFTER=<seconds>` starts streaming without a click, for
/// measuring.
pub fn autostart() -> Option<Duration> {
    let seconds: f64 = std::env::var("STREAM_START_AFTER").ok()?.parse().ok()?;
    Some(Duration::from_secs_f64(seconds.max(0.)))
}

const WIDTH: f64 = 760.;
const HEIGHT: f64 = 560.;

/// A transparent window: the app draws a translucent tint over a blurred
/// desktop (`frost`).
pub fn window(title: &str) -> WindowOptions {
    WindowOptions {
        title: title.into(),
        width: WIDTH,
        height: HEIGHT,
        transparent: true,
        decorations: cfg!(target_os = "macos"),
        min_size: Some((420., 320.)),
        ..Default::default()
    }
}

/// macOS: frosted glass behind the window and a transparent titlebar with
/// inset traffic lights, as the other glass examples have.
pub fn frost(cx: &mut WindowContext, title: &'static str) {
    #[cfg(target_os = "macos")]
    {
        // The native window is realised after the run closure returns.
        let window = cx.window.clone();
        cx.tasks.spawn(async move {
            zgui::timer::sleep(Duration::from_millis(1)).await;
            if crate::glass::inset_titlebar(title) {
                // Content now spans the titlebar; resize so the host sees it.
                window.set_inner_size(WIDTH, HEIGHT);
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (cx, title);
}
