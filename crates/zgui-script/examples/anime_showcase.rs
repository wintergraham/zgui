//! anime.js's API in Luau, animated by zgui's Rust engine.
//! Run: cargo run --release -p zgui-script --example anime_showcase
//! Edit `examples/anime_showcase/app.luau` and save to reload it.
use std::{path::Path, time::Duration};
use zgui_desktop::{Application, Glass, Titlebar, WindowOptions};
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
            decorations: cfg!(target_os = "macos"),
            min_size: Some((520., 420.)),
            titlebar: Titlebar::Inset,
            glass: Some(Glass::default()),
            ..Default::default()
        })
        .run(move |cx| {
            let host = ScriptHost::new(&cx.ui.runtime, script);
            let _ = host.reload();
            cx.tasks.spawn(host.watch(Duration::from_millis(250)));
            cx.render(host.view());
        })
}
