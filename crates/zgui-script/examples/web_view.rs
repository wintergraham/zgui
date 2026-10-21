//! A web page in a Luau interface, with a frosted popover over it.
//! Run: cargo run --release -p zgui-script --example web_view
//! Edit `examples/web_view/app.luau` and save to reload it.
#[cfg(target_os = "macos")]
#[path = "../../zgui-desktop/examples/support/glass.rs"]
mod glass;

use std::{path::Path, time::Duration};
use zgui_desktop::{Application, WindowOptions};
use zgui_script::ScriptHost;

const TITLE: &str = "zgui · web view";
const WIDTH: f64 = 1100.;
const HEIGHT: f64 = 760.;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/web_view/app.luau");
    Application::new()
        .window(WindowOptions {
            title: TITLE.into(),
            width: WIDTH,
            height: HEIGHT,
            transparent: true,
            decorations: cfg!(target_os = "macos"),
            min_size: Some((560., 400.)),
            ..Default::default()
        })
        .run(move |cx| {
            let host = ScriptHost::new(&cx.ui.runtime, script);
            // The page's corners follow the window's (`ui.shared` in the script).
            let radius = host.shared("window_radius", 10.);
            #[cfg(target_os = "macos")]
            {
                let window = cx.window.clone();
                cx.tasks.spawn(async move {
                    zgui::timer::sleep(Duration::from_millis(1)).await;
                    if glass::inset_titlebar(TITLE) {
                        window.set_inner_size(WIDTH, HEIGHT);
                    }
                    if let Some(r) = glass::corner_radius(TITLE) {
                        radius.set(r.into());
                    }
                });
            }
            #[cfg(not(target_os = "macos"))]
            let _ = radius;
            let window = cx.window.clone();
            host.action("drag_window", move |_| window.drag_window());
            let _ = host.reload();
            cx.tasks.spawn(host.watch(Duration::from_millis(250)));
            cx.render(host.view());
        })
}
