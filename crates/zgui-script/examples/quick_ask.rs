//! Ask's Quick ask, from ask.pen's "Ask from anywhere" board, as one working
//! element over four mock apps: summon it, pick context, type, run rows, read
//! a canned answer, paste it back, drop Finder files onto it.
//! Run: cargo run --release -p zgui-script --example quick_ask
//! Edit `examples/quick_ask/app.luau` and save to reload it.
use std::{path::Path, sync::Arc, time::Duration};
use zgui_desktop::{Application, FontData, WindowOptions};
use zgui_script::ScriptHost;

const TITLE: &str = "zgui · quick ask";
const WIDTH: f64 = 1280.;
const HEIGHT: f64 = 860.;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/quick_ask");
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
        titlebar: zgui_desktop::Titlebar::Inset,
        min_size: Some((1100., 720.)),
        ..Default::default()
    })
    .run(move |cx| {
        let host = ScriptHost::new(&cx.ui.runtime, dir.join("app.luau"));
        // `QUICK_ASK_SCENE=3` opens over the third app (Mail), for screenshots.
        if let Some(n) = std::env::var("QUICK_ASK_SCENE")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
        {
            host.shared("quick_ask_scene", n);
        }
        let window = cx.window.clone();
        host.action("drag_window", move |_| window.drag_window());
        let _ = host.reload();
        cx.tasks.spawn(host.watch(Duration::from_millis(250)));
        cx.render(host.view());
    })
}
