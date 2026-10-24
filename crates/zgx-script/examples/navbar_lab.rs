//! Ask Motion Lab's Navbar page, rendered by native zgui nodes.
//! Run: cargo run --release -p zgui-script --example navbar_lab
use std::{path::Path, time::Duration};
use zgui::input::{Key, Modifiers};
use zgui_desktop::{AppMenu, AppMenuEntry, Application, MenuAccelerator, WindowOptions};
use zgui_gpu::text::FontData;
use zgui_script::{Command, ScriptHost};

const TITLE: &str = "Ask · Navbar";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/navbar_lab/app.luau");
    let mut application = Application::new();
    for bytes in [
        &include_bytes!("controls_lab/assets/Geist.ttf")[..],
        &include_bytes!("controls_lab/assets/GeistMono.ttf")[..],
    ] {
        application = application.font(FontData::new(bytes)?);
    }
    application
        .window(WindowOptions {
            title: TITLE.into(),
            width: 1180.,
            height: 1000.,
            transparent: true,
            titlebar: zgui_desktop::Titlebar::Inset,
            min_size: Some((720., 600.)),
            ..Default::default()
        })
        .run(move |cx| {
            let host = ScriptHost::new(&cx.ui.runtime, script);
            let window = cx.window.clone();
            host.action("drag_window", move |_| window.drag_window());
            if let Err(error) = host.reload() {
                eprintln!("navbar_lab: {error}");
            }
            cx.tasks.spawn(host.watch(Duration::from_millis(250)));
            // The script's commands are menu items, so ⌘L reaches the bar wherever focus is.
            let commands = host.commands();
            let windows = cx.windows.clone();
            let menus = cx.ui.runtime.effect(move || {
                if let Err(error) = windows.set_menus(menus(&commands.get())) {
                    eprintln!("navbar_lab: menus: {error}");
                }
            });
            let run = host.clone();
            cx.on_menu_action(move |action| {
                run.run_command(&action.0);
            });
            let keep = std::cell::RefCell::new(Some(menus));
            cx.on_closed(move || drop(keep.borrow_mut().take()));
            cx.render(host.view());
        })
}

/// Menus in the order the script names them.
fn menus(commands: &[Command]) -> Vec<AppMenu> {
    let mut menus: Vec<AppMenu> = Vec::new();
    for command in commands {
        let index = match menus.iter().position(|m| m.label == command.menu) {
            Some(index) => index,
            None => {
                menus.push(AppMenu::new(command.menu.clone(), []));
                menus.len() - 1
            }
        };
        let item = AppMenuEntry::item(command.title.clone(), command.id.as_str());
        menus[index]
            .entries
            .push(match command.key.as_deref().and_then(accelerator) {
                Some(key) => item.accelerator(key),
                None => item,
            });
    }
    menus
}

/// `cmd+l` as an accelerator: modifiers and one character.
fn accelerator(text: &str) -> Option<MenuAccelerator> {
    let mut modifiers = Modifiers::default();
    let mut parts: Vec<&str> = text.split('+').collect();
    let key = parts.pop()?.to_ascii_lowercase();
    for part in parts {
        match part {
            "cmd" => modifiers.meta = true,
            "ctrl" => modifiers.control = true,
            "alt" => modifiers.alt = true,
            "shift" => modifiers.shift = true,
            _ => return None,
        }
    }
    (key.chars().count() == 1).then(|| MenuAccelerator::new(Key::Character(key), modifiers))
}
