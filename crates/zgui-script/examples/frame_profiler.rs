//! The zgui frame profiler design (pen: `zgui · Frame profiler` and its
//! popovers), built from Luau. Interactive, with sample data until an app
//! records frames for it: run one with `ZGUI_PROFILE=1` and the timeline
//! shows its frames (`frame_profiler/live.rs`).
//! Run: cargo run --release -p zgui-script --example frame_profiler
//! Edit `examples/frame_profiler/*.luau` and save to reload it.
#[cfg(target_os = "macos")]
#[path = "../../zgui-desktop/examples/support/glass.rs"]
mod glass;
#[cfg(unix)]
#[path = "frame_profiler/live.rs"]
mod live;

use std::{path::Path, sync::Arc, time::Duration};
use zgui_desktop::{Application, FontData, WindowOptions};
use zgui_script::ScriptHost;

const TITLE: &str = "zgui · Frame profiler";
const WIDTH: f64 = 1132.;
const HEIGHT: f64 = 1034.;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/frame_profiler/app.luau");
    let mut app = Application::new();
    for bytes in [
        &include_bytes!("../../zgui-desktop/examples/support/fonts/GeistVariable.ttf")[..],
        include_bytes!("../../zgui-desktop/examples/support/fonts/GeistMonoVariable.ttf"),
    ] {
        app = app.font(FontData::new(Arc::<[u8]>::from(bytes))?);
    }
    app.window(WindowOptions {
        title: TITLE.into(),
        width: WIDTH,
        height: HEIGHT,
        transparent: true,
        decorations: cfg!(target_os = "macos"),
        min_size: Some((980., 560.)),
        ..Default::default()
    })
    .run(move |cx| {
        #[cfg(target_os = "macos")]
        {
            let window = cx.window.clone();
            cx.tasks.spawn(async move {
                zgui::timer::sleep(Duration::from_millis(1)).await;
                if glass::inset_titlebar(TITLE) {
                    window.set_inner_size(WIDTH, HEIGHT);
                }
            });
        }
        let host = ScriptHost::new(&cx.ui.runtime, script);
        let window = cx.window.clone();
        host.action("drag_window", move |_| window.drag_window());
        if let Err(error) = host.reload() {
            eprintln!("frame_profiler: {error}");
        }
        cx.tasks.spawn(host.watch(Duration::from_millis(250)));
        #[cfg(unix)]
        {
            let shared = host.shared("live", false);
            let mut feed = live::Live::start();
            cx.tasks.spawn(async move {
                loop {
                    if let Some(value) = feed.poll() {
                        shared.set(value);
                    }
                    zgui::timer::sleep(Duration::from_millis(50)).await;
                }
            });
        }
        cx.render(host.view());
    })
}
