//! Ask's first-run setup, from ask.pen's "Ask · Onboarding" board, in its
//! own 880 × 580 window: every step works, from the splash to Ready.
//! Run: cargo run --release -p zgui-script --example onboarding
//! Edit `examples/onboarding/*.luau` and save to reload it.
//! `ONBOARDING_STEP=appearance` opens on a step (for screenshots).
use std::{path::Path, sync::Arc, time::Duration};
use zgui_desktop::{Application, FontData, Titlebar, WindowOptions};
use zgui_script::ScriptHost;

const TITLE: &str = "Ask · Setup";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/onboarding/app.luau");
    let mut app = Application::new();
    for bytes in [
        &include_bytes!("../../zgui-desktop/examples/support/fonts/GeistVariable.ttf")[..],
        include_bytes!("../../zgui-desktop/examples/support/fonts/GeistMonoVariable.ttf"),
    ] {
        app = app.font(FontData::new(Arc::<[u8]>::from(bytes))?);
    }
    app.window(WindowOptions {
        title: TITLE.into(),
        width: 880.,
        height: 580.,
        resizable: false,
        // The board draws its own title bar: the window keeps its rounded
        // corners and shadow, without the traffic lights.
        titlebar: Titlebar::Hidden,
        ..Default::default()
    })
    .run(move |cx| {
        let host = ScriptHost::new(&cx.ui.runtime, script);
        if let Ok(step) = std::env::var("ONBOARDING_STEP") {
            host.shared("onboarding_step", step.as_str());
        }
        let window = cx.window.clone();
        host.action("close", move |_| window.close());
        let _ = host.reload();
        cx.tasks.spawn(host.watch(Duration::from_millis(250)));
        cx.render(host.view());
    })
}
