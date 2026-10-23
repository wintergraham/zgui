//! Ask's Settings page in Luau, drawn by native zgui nodes: thirteen
//! sections, settings search, dither previews, menus and dialogs.
//! Run: cargo run --release -p zgui-script --example settings
//! Edit `examples/settings/*.luau` and save to reload it.
//!
//! Preferences persist as JSON in `target/settings/preferences.json`, or in
//! the directory `ZGUI_SETTINGS_PROFILE` names.
#[cfg(target_os = "macos")]
#[path = "../../zgui-desktop/examples/support/glass.rs"]
mod glass;

use std::{
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};
use zgui_desktop::{Application, FontData, WindowOptions};
use zgui_script::{ScriptHost, Value};

const TITLE: &str = "Ask · Settings";
const WIDTH: f64 = 1280.;
const HEIGHT: f64 = 820.;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/settings");
    let file = std::env::var_os("ZGUI_SETTINGS_PROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/settings"))
        .join("preferences.json");
    Application::new()
        .font(FontData::new(&include_bytes!("settings/assets/Geist.ttf")[..])?)
        .font(FontData::new(&include_bytes!("settings/assets/GeistMono.ttf")[..])?)
        .window(WindowOptions {
            title: TITLE.into(),
            width: WIDTH,
            height: HEIGHT,
            transparent: true,
            decorations: cfg!(target_os = "macos"),
            min_size: Some((720., 520.)),
            ..Default::default()
        })
        .run(move |cx| {
            #[cfg(target_os = "macos")]
            {
                let window = cx.window.clone();
                cx.tasks.spawn(async move {
                    zgui::timer::sleep(Duration::from_millis(1)).await;
                    if glass::inset_titlebar(TITLE) {
                        window.set_inner_size(WIDTH, HEIGHT);
                    }
                });
            }
            let host = ScriptHost::new(&cx.ui.runtime, dir.join("app.luau"));
            let window = cx.window.clone();
            host.action("drag_window", move |_| window.drag_window());
            // The script reads what was saved when it loads and writes the
            // whole model back whenever a setting changes.
            let saved = std::fs::read_to_string(&file)
                .ok()
                .and_then(|text| serde_json::from_str(&text).ok())
                .map(|json| from_json(&json))
                .unwrap_or_default();
            let store = host.shared("settings", saved.clone());
            let last = std::cell::RefCell::new(saved);
            let file = Rc::new(file.clone());
            let effect = cx.ui.runtime.effect(move || {
                let value = store.get();
                if value == Value::Nil || *last.borrow() == value {
                    return;
                }
                *last.borrow_mut() = value.clone();
                if let Some(dir) = file.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if let Err(error) = std::fs::write(&*file, to_json(&value).to_string()) {
                    eprintln!("settings: saving {}: {error}", file.display());
                }
            });
            std::mem::forget(effect);
            if let Err(error) = host.reload() {
                eprintln!("settings: {error}");
            }
            cx.tasks.spawn(host.watch(Duration::from_millis(250)));
            cx.render(host.view());
        })
}

fn to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Nil => serde_json::Value::Null,
        Value::Bool(b) => (*b).into(),
        Value::Num(n) => serde_json::Number::from_f64(*n).map_or(serde_json::Value::Null, Into::into),
        Value::Str(s) => s.as_ref().into(),
        Value::List(items) => items.iter().map(to_json).collect(),
        Value::Map(fields) => fields
            .iter()
            .map(|(k, v)| (k.to_string(), to_json(v)))
            .collect::<serde_json::Map<_, _>>()
            .into(),
    }
}

fn from_json(json: &serde_json::Value) -> Value {
    match json {
        serde_json::Value::Null => Value::Nil,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => Value::Num(n.as_f64().unwrap_or(0.)),
        serde_json::Value::String(s) => Value::Str(s.as_str().into()),
        serde_json::Value::Array(items) => Value::List(items.iter().map(from_json).collect()),
        serde_json::Value::Object(fields) => {
            let mut fields: Vec<(Rc<str>, Value)> = fields
                .iter()
                .map(|(k, v)| (Rc::from(k.as_str()), from_json(v)))
                .collect();
            fields.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Map(fields.into())
        }
    }
}
