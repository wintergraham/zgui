//! One Ghostty terminal in a frosted zgui window.
//! Run: cargo run --release -p zgui-desktop --features ghostty --example ghostty_pane
#[cfg(target_os = "macos")]
#[path = "support/glass.rs"]
mod glass;

use zgui::{
    compose::prelude::*,
    terminal::{Terminal, TerminalOptions, terminal},
};
use zgui_desktop::{Application, WindowOptions};

const TITLE: &str = "zgui · ghostty pane";
const WIDTH: f64 = 900.;
const HEIGHT: f64 = 600.;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let typed = std::env::args().nth(1);
    Application::new()
        .window(WindowOptions {
            title: TITLE.into(),
            width: WIDTH,
            height: HEIGHT,
            transparent: true,
            decorations: cfg!(target_os = "macos"),
            ..Default::default()
        })
        .run(move |cx| {
            #[cfg(target_os = "macos")]
            {
                let window = cx.window.clone();
                cx.tasks.spawn(async move {
                    zgui::timer::sleep(std::time::Duration::from_millis(1)).await;
                    if glass::inset_titlebar(TITLE) {
                        window.set_inner_size(WIDTH, HEIGHT);
                    }
                });
            }
            let term = Terminal::new(&cx.ui.runtime, TerminalOptions::default());
            if let Some(typed) = &typed {
                term.input(format!("{typed}\r"));
            }
            term.focus();
            let title = term.clone();
            cx.render(
                column()
                    .w_full()
                    .h_full()
                    .items_stretch()
                    .bg(rgba(0x1214168c))
                    .child(
                        row().h(40.).pl(84.).items_center().child(
                            text_signal(move || {
                                let t = title.title();
                                if t.is_empty() { "Terminal".into() } else { t }
                            })
                            .text_size(13.)
                            .text_color(rgba(0xffffffa6)),
                        ),
                    )
                    .child(
                        terminal(&term)
                            .grow()
                            .mx(6.)
                            .mb(6.)
                            .rounded(10.)
                            .bg(rgba(0x0c0d10d9)),
                    ),
            );
        })
}
