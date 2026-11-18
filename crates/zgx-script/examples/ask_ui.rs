//! A generic runner for Ask's Luau UI: opens the script given as the first
//! argument in a glass window, reloading it whenever it (or what it
//! requires) changes, with its `ui.command`s as menu items. How the Luau UI
//! is iterated on before the Ask app hosts it. `ui.fetch` reaches this
//! machine's loopback only (`http://127.0.0.1:…`, `http://localhost:…`): a
//! local service standing in for the app's backend, which a workpad talks to.
//! Run: cargo run --release -p zgx-script --example ask_ui -- path/to/ui/gallery.luau
use std::{path::PathBuf, sync::Arc, time::Duration};
use zgx::input::{Key, Modifiers};
use zgx_desktop::{AppMenu, AppMenuEntry, Application, FontData, MenuAccelerator, WindowOptions};
use zgx_script::{Command, Request, Response, ScriptHost};

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
        &include_bytes!("../../zgx-desktop/examples/support/fonts/GeistVariable.ttf")[..],
        include_bytes!("../../zgx-desktop/examples/support/fonts/GeistMonoVariable.ttf"),
    ] {
        app = app.font(FontData::new(Arc::<[u8]>::from(bytes))?);
    }
    app.window(WindowOptions {
        title: TITLE.into(),
        width: WIDTH,
        height: HEIGHT,
        transparent: true,
        titlebar: zgx_desktop::Titlebar::Inset,
        min_size: Some((760., 560.)),
        ..Default::default()
    })
    .run(move |cx| {
        let host = ScriptHost::new(&cx.ui.runtime, script.clone());
        let window = cx.window.clone();
        host.action("drag_window", move |_| window.drag_window());
        // `ui.invoke("window_buttons", { visible, dx, dy })`: the traffic lights shown or hidden and moved from home
        // (a sidebar carrying them as it slides).
        let window = cx.window.clone();
        host.action("window_buttons", move |arg| {
            let num = |k: &str| arg.field(k).and_then(|v| v.as_f64()).unwrap_or(0.);
            let visible = !matches!(arg.field("visible"), Some(zgx_script::Value::Bool(false)));
            window.set_window_buttons(visible, num("dx"), num("dy"));
        });
        let window = cx.window.clone();
        host.fetcher(move |request| loopback(request, window.clone()));
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

/// `ui.fetch` on this machine's loopback, read on a thread of its own so a
/// slow answer (a model's) never holds the window: plain HTTP/1.1, the whole
/// body once the service closes the connection.
async fn loopback(request: Request, window: zgx_desktop::WindowHandle) -> Result<Response, String> {
    use std::{
        future::poll_fn,
        sync::Mutex,
        task::{Poll, Waker},
    };
    let rest = request
        .url
        .strip_prefix("http://127.0.0.1:")
        .or_else(|| request.url.strip_prefix("http://localhost:"))
        .ok_or_else(|| format!("ask_ui fetches only this machine's loopback, not {}", request.url))?;
    let (port, path) = rest.split_once('/').map(|(p, r)| (p, format!("/{r}"))).unwrap_or((rest, "/".into()));
    let address = format!("127.0.0.1:{port}");
    type Slot = Mutex<(Option<Result<(u16, Vec<u8>), String>>, Option<Waker>)>;
    let slot: Arc<Slot> = Arc::new(Mutex::new((None, None)));
    let done = slot.clone();
    std::thread::spawn(move || {
        let answer = (|| -> Result<(u16, Vec<u8>), String> {
            use std::io::{Read, Write};
            let mut stream = std::net::TcpStream::connect(&address).map_err(|e| e.to_string())?;
            let body = request.body.unwrap_or_default();
            let mut head = format!(
                "{} {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\nContent-Length: {}\r\n",
                request.method,
                body.len()
            );
            for (name, value) in &request.headers {
                head.push_str(&format!("{name}: {value}\r\n"));
            }
            head.push_str("\r\n");
            stream.write_all(head.as_bytes()).and_then(|_| stream.write_all(&body)).map_err(|e| e.to_string())?;
            let mut raw = Vec::new();
            stream.read_to_end(&mut raw).map_err(|e| e.to_string())?;
            let split = raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or("No HTTP head in the answer")?;
            let status = String::from_utf8_lossy(&raw[..split])
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .ok_or("No status in the answer")?;
            Ok((status, raw[split + 4..].to_vec()))
        })();
        let waker = {
            let mut slot = done.lock().unwrap();
            slot.0 = Some(answer);
            slot.1.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
        window.wake();
    });
    let (status, body) = poll_fn(|cx| {
        let mut slot = slot.lock().unwrap();
        match slot.0.take() {
            Some(answer) => Poll::Ready(answer),
            None => {
                slot.1 = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    })
    .await?;
    Ok(Response {
        status,
        headers: vec![("Content-Type".into(), "application/json".into())],
        body,
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
