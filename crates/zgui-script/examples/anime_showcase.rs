//! anime.js's API in Luau, animated by zgui's Rust engine.
//! Run: cargo run --release -p zgui-script --example anime_showcase
//! Edit `examples/anime_showcase/app.luau` and save to reload it.
#[cfg(target_os = "macos")]
#[path = "../../zgui-desktop/examples/support/glass.rs"]
mod glass;

use std::{path::Path, time::Duration};
use zgui_desktop::{Application, WindowOptions};
use zgui_script::ScriptHost;

const TITLE: &str = "zgui · anime showcase";
const WIDTH: f64 = 760.;
const HEIGHT: f64 = 720.;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/anime_showcase/app.luau");
    Application::new()
        .window(WindowOptions {
            title: TITLE.into(),
            width: WIDTH,
            height: HEIGHT,
            transparent: true,
            decorations: cfg!(target_os = "macos"),
            min_size: Some((520., 420.)),
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
