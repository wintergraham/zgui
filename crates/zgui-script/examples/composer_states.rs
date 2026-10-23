//! The Ask motion lab's composer page in Luau: every composer state, and the
//! lab's animated transitions between them.
//! Run: cargo run --release -p zgui-script --example composer_states
//! Edit `examples/composer_states/app.luau` and save to reload it.
use std::{path::Path, sync::Arc, time::Duration};
use zgui_desktop::{Application, FontData, Glass, Titlebar, WindowOptions};
use zgui_script::ScriptHost;

const TITLE: &str = "zgui · composer states";
const WIDTH: f64 = 900.;
const HEIGHT: f64 = 1000.;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // app.luau requires states.luau: the lab's design data for every state,
    // generated from the lab (see its header).
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/composer_states");
    let mut app = Application::new();
    // The lab's own faces: variable Geist and Geist Mono 1.401 (every weight).
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
        titlebar: Titlebar::Inset,
        // The HUD glass the lab's pages sit on.
        glass: Some(Glass::default()),
        decorations: cfg!(target_os = "macos"),
        min_size: Some((640., 720.)),
        ..Default::default()
    })
    .run(move |cx| {
        let host = ScriptHost::new(&cx.ui.runtime, dir.join("app.luau"));
        // `COMPOSER_STATE=12` opens on the twelfth state (for screenshots).
        if let Some(n) = std::env::var("COMPOSER_STATE")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
        {
            host.shared("composer_state", n);
        }
        // `COMPOSER_COMPARE=1` paints the stage the lab's opaque #0A0A09.
        if std::env::var("COMPOSER_COMPARE").is_ok() {
            host.shared("composer_compare", true);
        }
        let _ = host.reload();
        cx.tasks.spawn(host.watch(Duration::from_millis(250)));
        cx.render(host.view());
    })
}
