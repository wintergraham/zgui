//! An agent trace switching between states, animated from Luau.
//! Run: cargo run --release -p zgui-script --example agent_trace
//! Edit `examples/agent_trace/app.luau` and save to reload it.
#[cfg(target_os = "macos")]
#[path = "../../zgui-desktop/examples/support/glass.rs"]
mod glass;

use std::{path::Path, sync::Arc, time::Duration};
use zgui_desktop::{Application, FontData, WindowOptions};
use zgui_script::ScriptHost;

const TITLE: &str = "zgui · agent trace";
const WIDTH: f64 = 720.;
const HEIGHT: f64 = 1000.;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/agent_trace/app.luau");
    let mut app = Application::new();
    // The lab's faces: variable Geist and Geist Mono 1.401, every weight.
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
        min_size: Some((560., 520.)),
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
        let _ = host.reload();
        cx.tasks.spawn(host.watch(Duration::from_millis(250)));
        cx.render(host.view());
    })
}
