//! A generic runner for Ask's Luau UI: opens the script given as the first
//! argument in a glass window, reloading it whenever it (or what it
//! requires) changes, with its `ui.command`s as menu items. How the Luau UI
//! is iterated on before the Ask app hosts it.
//! Run: cargo run --release -p zgui-script --example ask_ui -- path/to/ui/gallery.luau
use std::{path::PathBuf, sync::Arc, time::Duration};
use zgui::input::{Key, Modifiers};
use zgui_desktop::{AppMenu, AppMenuEntry, Application, FontData, MenuAccelerator, WindowOptions};
use zgui_script::{Command, ScriptHost};

const TITLE: &str = "Ask · UI";
const WIDTH: f64 = 1280.;
const HEIGHT: f64 = 820.;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(script) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: ask_ui <script.luau>");
        std::process::exit(2);
    };
    let script = std::fs::canonicalize(&script).unwrap_or(script);
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
        min_size: Some((760., 560.)),
        ..Default::default()
    })
    .run(move |cx| {
        let host = ScriptHost::new(&cx.ui.runtime, script.clone());
        let window = cx.window.clone();
        host.action("drag_window", move |_| window.drag_window());
        if let Err(error) = host.reload() {
            eprintln!("ask_ui: {error}");
        }
        cx.tasks.spawn(host.watch(Duration::from_millis(250)));
        // The script's commands are menu items, so their shortcuts reach it
        // wherever focus is.
        let commands = host.commands();
        let windows = cx.windows.clone();
        let menus = cx.ui.runtime.effect(move || {
            if let Err(error) = windows.set_menus(menus(&commands.get())) {
                eprintln!("ask_ui: menus: {error}");
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
    (key.chars().count() == 1).then(|| MenuAccelerator::new(Key::Character(key.into()), modifiers))
}
