//! Ask Motion Lab's Controls page, rendered by native zgx nodes.
//! Run: cargo run --release -p zgx-script --example controls_lab
use std::{path::Path, time::Duration};
use zgx_desktop::{Application, WindowOptions};
use zgx_gpu::text::FontData;
use zgx_script::ScriptHost;

const TITLE: &str = "Ask · Controls";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/controls_lab/app.luau");
    Application::new()
        .font(FontData::new(
            &include_bytes!("controls_lab/assets/Geist.ttf")[..],
        )?)
        .font(FontData::new(
            &include_bytes!("controls_lab/assets/GeistMono.ttf")[..],
        )?)
        .window(WindowOptions {
            title: TITLE.into(),
            width: 1180.,
            height: 900.,
            transparent: true,
            titlebar: zgx_desktop::Titlebar::Inset,
            min_size: Some((620., 460.)),
            ..Default::default()
        })
        .run(move |cx| {
            let host = ScriptHost::new(&cx.ui.runtime, script);
            let window = cx.window.clone();
            host.action("drag_window", move |_| window.drag_window());
            if let Err(error) = host.reload() {
                eprintln!("controls_lab: {error}");
            }
            cx.tasks.spawn(host.watch(Duration::from_millis(250)));
            cx.render(host.view());
        })
}
