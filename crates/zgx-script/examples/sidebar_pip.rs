//! Ask's left sidebar and agent PiP, from ask.pen's "Sidebar · Navigation &
//! PiP" board, as one working browser window: spaces, pinned trays, chats,
//! tabs and groups, favicons resolving as pages load, the collapsed rail,
//! and agents working in a picture-in-picture that stacks and expands.
//! Run: cargo run --release -p zgx-script --example sidebar_pip
//! Edit `examples/sidebar_pip/app.luau` and save to reload it.
use std::{path::Path, sync::Arc, time::Duration};
use zgx_desktop::{Application, FontData, WindowOptions};
use zgx_script::ScriptHost;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/sidebar_pip");
    let mut app = Application::new();
    for bytes in [
        &include_bytes!("../../zgx-desktop/examples/support/fonts/GeistVariable.ttf")[..],
        include_bytes!("../../zgx-desktop/examples/support/fonts/GeistMonoVariable.ttf"),
    ] {
        app = app.font(FontData::new(Arc::<[u8]>::from(bytes))?);
    }
    app.window(WindowOptions {
        title: "zgx · sidebar & PiP".into(),
        width: 1180.,
        height: 840.,
        min_size: Some((760., 620.)),
        ..Default::default()
    })
    .run(move |cx| {
        let host = ScriptHost::new(&cx.ui.runtime, dir.join("app.luau"));
        let _ = host.reload();
        cx.tasks.spawn(host.watch(Duration::from_millis(250)));
        cx.render(host.view());
    })
}
