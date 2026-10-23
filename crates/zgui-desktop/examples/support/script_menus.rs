//! A Luau app's commands (`ui.command`) as the native menu bar: shared by
//! the examples whose interface is a script (terminal, zgui_monitor).
use zgui::input::{Key, Modifiers};
use zgui_desktop::{AppMenu, AppMenuEntry, MenuAccelerator};
use zgui_script::Command;

/// Menus in the order the script first names them, items in its order.
pub fn menus(commands: &[Command]) -> Vec<AppMenu> {
    let mut menus: Vec<AppMenu> = Vec::new();
    for command in commands {
        let index = match menus.iter().position(|m| m.label == command.menu) {
            Some(index) => index,
            None => {
                menus.push(AppMenu::new(command.menu.clone(), []));
                menus.len() - 1
            }
        };
        let entry = if command.separator {
            AppMenuEntry::Separator
        } else {
            let item = AppMenuEntry::item(command.title.clone(), command.id.as_str());
            match command.key.as_deref().and_then(accelerator) {
                Some(key) => item.accelerator(key),
                None => item,
            }
        };
        menus[index].entries.push(entry);
    }
    menus
}

/// `cmd+shift+d`, `cmd+alt+arrowleft`, `cmd+1`, `cmd+[` as an accelerator.
pub fn accelerator(text: &str) -> Option<MenuAccelerator> {
    let mut modifiers = Modifiers::default();
    let mut parts: Vec<&str> = text.split('+').collect();
    // `cmd++` would be a plus key; none of ours use it.
    let key = parts.pop()?.to_ascii_lowercase();
    for part in parts {
        match part.to_ascii_lowercase().as_str() {
            "cmd" | "command" | "super" | "meta" => modifiers.meta = true,
            "ctrl" | "control" => modifiers.control = true,
            "alt" | "opt" | "option" => modifiers.alt = true,
            "shift" => modifiers.shift = true,
            other => {
                eprintln!("menus: unknown modifier `{other}` in `{text}`");
                return None;
            }
        }
    }
    let key = match key.as_str() {
        "arrowleft" | "left" => Key::ArrowLeft,
        "arrowright" | "right" => Key::ArrowRight,
        "arrowup" | "up" => Key::ArrowUp,
        "arrowdown" | "down" => Key::ArrowDown,
        "enter" | "return" => Key::Enter,
        "tab" => Key::Tab,
        "escape" | "esc" => Key::Escape,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "space" => Key::Space,
        f if f.len() > 1 && f.starts_with('f') && f[1..].parse::<u8>().is_ok() => {
            Key::Function(f[1..].parse().ok()?)
        }
        single if single.chars().count() == 1 => Key::Character(single.into()),
        other => {
            eprintln!("menus: unknown key `{other}` in `{text}`");
            return None;
        }
    };
    Some(MenuAccelerator::new(key, modifiers))
}
