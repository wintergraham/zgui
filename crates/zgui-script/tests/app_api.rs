//! What scripts ask of their application: commands for its menus, host
//! actions called directly, terminal sessions that outlive reloads, and
//! drag and drop between their nodes.
mod common;

use common::Harness;
use zgui::{
    input::{InputEvent, PointerButton},
    native_view::NativeContent,
};
use zgui_script::{ScriptHost, Value};

#[test]
fn commands_are_published_after_a_load_and_run_by_id() {
    let mut h = Harness::inline(
        r##"local count = ui.state("count", 0)
        ui.command { id = "bump", title = "Bump", menu = "Shell", key = "cmd+b", run = function() count:add(1) end }
        ui.command { menu = "Shell", separator = true }
        ui.command { id = "reset", title = "Reset", menu = "Edit", run = function() count:set(0) end }
        return ui.text { "{count}", id = "count" }"##,
    );
    let commands = h.host.commands().get();
    let shown: Vec<_> = commands
        .iter()
        .map(|c| {
            (
                c.id.as_str(),
                c.menu.as_str(),
                c.key.as_deref(),
                c.separator,
            )
        })
        .collect();
    assert_eq!(
        shown,
        vec![
            ("bump", "Shell", Some("cmd+b"), false),
            ("", "Shell", None, true),
            ("reset", "Edit", None, false),
        ]
    );
    assert!(h.host.run_command("bump"));
    assert!(h.host.run_command("bump"));
    assert!(!h.host.run_command("missing"));
    h.settle();
    assert_eq!(h.text("count"), "2");
}

#[test]
fn a_failed_load_keeps_the_published_commands() {
    let runtime = zgui::reactive::Runtime::new();
    let dir = std::env::temp_dir().join(format!("zgui-commands-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("app.luau");
    std::fs::write(
        &path,
        r#"ui.command { id = "a", run = function() end }
        return ui.div {}"#,
    )
    .unwrap();
    let host = ScriptHost::new(&runtime, &path);
    host.reload().unwrap();
    std::fs::write(
        &path,
        "ui.command { id = \"b\", run = function() end }\nerror(\"broken\")",
    )
    .unwrap();
    assert!(host.reload().is_err());
    let ids: Vec<String> = host.commands().get().into_iter().map(|c| c.id).collect();
    assert_eq!(ids, vec!["a".to_string()]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn scripts_invoke_host_actions() {
    let runtime = zgui::reactive::Runtime::new();
    let host = ScriptHost::inline(
        &runtime,
        "app",
        r#"ui.invoke("set_title", "Hello")
        return ui.div {}"#,
    );
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Value::Nil));
    let sink = seen.clone();
    host.action("set_title", move |value| *sink.borrow_mut() = value);
    host.reload().unwrap();
    assert_eq!(*seen.borrow(), Value::Str("Hello".into()));
    let missing = ScriptHost::inline(&runtime, "app", r#"ui.invoke("nope") return ui.div {}"#);
    assert!(
        missing
            .reload()
            .unwrap_err()
            .contains("no host action named `nope`")
    );
}

/// The terminal a mounted `ui.terminal` shows.
fn terminal(h: &mut Harness) -> zgui::terminal::Terminal {
    h.ui.prepare_frame();
    let node = h.natives.nodes()[0];
    match h.natives.content(node) {
        Some(NativeContent::Terminal(term)) => term,
        _ => panic!("no terminal mounted"),
    }
}

#[test]
fn terminal_sessions_outlive_reloads_and_end_when_closed() {
    let mut h = Harness::inline(
        r##"local term = ui.term("p1", { cwd = "/tmp", command = "zsh", env = { A = "1" } })
        return ui.column {
            ui.terminal { term = term, w = 200, h = 100 },
            ui.button { id = "close", w = 20, h = 20, on_click = function() term:close() end },
        }"##,
    );
    let first = terminal(&mut h);
    let options = first.options();
    assert_eq!(options.command.as_deref(), Some("zsh"));
    assert_eq!(
        options.working_directory.as_deref(),
        Some(std::path::Path::new("/tmp"))
    );
    assert_eq!(options.env, vec![("A".to_string(), "1".to_string())]);
    // A reload asks for "p1" again: the same session.
    h.host.reload().unwrap();
    h.settle();
    let again = terminal(&mut h);
    assert!(first == again, "a reload keeps the session");
    h.click("close");
    assert!(first.is_closed());
}

#[test]
fn terminal_handlers_follow_the_latest_run() {
    let mut h = Harness::inline(
        r##"local log = ui.state("log", "")
        local term = ui.term("p1", { on_title = function(t) log:set(log:peek() .. t .. ";") end })
        -- A lookup without options keeps them.
        local same = ui.term("p1")
        return ui.column {
            ui.terminal { term = same, w = 200, h = 100 },
            ui.text { "{log}", id = "log" },
            ui.text { function() return term.title end, id = "title" },
        }"##,
    );
    let term = terminal(&mut h);
    term.update_state(|s| s.title = "vim".into());
    term.emit(zgui::terminal::TerminalEvent::TitleChanged {
        title: "vim".into(),
    });
    h.settle();
    assert_eq!(h.text("log"), "vim;");
    assert_eq!(h.text("title"), "vim");
}

#[test]
fn nodes_drag_and_drop_script_values() {
    let mut h = Harness::inline(
        r##"local log = ui.state("log", "")
        local function note(s) log:set(log:peek() .. s .. ";") end
        return ui.column {
            ui.div { id = "src", w = 100, h = 40,
                on_drag = function() return { kind = "pane", id = "p7" } end,
                on_drag_end = function(p, accepted) note("end " .. p.id .. " " .. tostring(accepted)) end },
            ui.div { id = "dst", w = 100, h = 100,
                drop_when = function(p) return p.kind == "pane" end,
                on_drag_over = function(p, e) note("over " .. math.floor(e.local_y) .. "/" .. e.h) end,
                on_drop = function(p, e) note("drop " .. p.id) end },
            ui.text { "{log}", id = "log" },
        }"##,
    );
    h.ui.prepare_frame();
    let bounds = |h: &mut Harness, id: &str| {
        let node = h.find(id).unwrap();
        h.ui.scene.borrow().bounds(node)
    };
    let (src, dst) = (bounds(&mut h, "src"), bounds(&mut h, "dst"));
    h.ui.dispatch(InputEvent::PointerDown {
        x: src.x + 10.,
        y: src.y + 10.,
        button: PointerButton::Primary,
    });
    h.ui.dispatch(InputEvent::PointerMove {
        x: src.x + 30.,
        y: src.y + 20.,
    });
    h.ui.dispatch(InputEvent::PointerMove {
        x: dst.x + 50.,
        y: dst.y + 25.,
    });
    h.ui.dispatch(InputEvent::PointerUp {
        x: dst.x + 50.,
        y: dst.y + 25.,
        button: PointerButton::Primary,
    });
    h.settle();
    let log = h.text("log");
    assert!(log.contains("over 25/100;"), "{log}");
    assert!(log.contains("drop p7;"), "{log}");
    assert!(log.ends_with("end p7 true;"), "{log}");
}

#[test]
fn a_press_that_follows_the_pointer_keeps_it() {
    let mut h = Harness::inline(
        r##"local x = ui.state("x", 0)
        local dragging = false
        return ui.row {
            ui.div { id = "divider", w = 6, h = 100,
                on_press = function() dragging = true end,
                on_pointer_move = function(e) if dragging then x:set(math.floor(e.x)) end end,
                on_release = function() dragging = false end },
            ui.div { w = 200, h = 100 },
            ui.text { "{x}", id = "x" },
        }"##,
    );
    h.ui.prepare_frame();
    let node = h.find("divider").unwrap();
    let b = h.ui.scene.borrow().bounds(node);
    h.ui.dispatch(InputEvent::PointerDown {
        x: b.x + 3.,
        y: b.y + 50.,
        button: PointerButton::Primary,
    });
    // Far outside the divider: it still hears the pointer.
    h.ui.dispatch(InputEvent::PointerMove { x: 150., y: 60. });
    h.ui.dispatch(InputEvent::PointerUp {
        x: 150.,
        y: 60.,
        button: PointerButton::Primary,
    });
    h.settle();
    assert_eq!(h.text("x"), "150");
}

#[test]
fn any_container_without_an_id_takes_drops_and_drags() {
    for area in ["ui.div", "ui.overlay", "ui.column", "ui.row"] {
        let code = format!(
            r##"local log = ui.state("log", "")
            return ui.column {{
                {area} {{ w = 40, h = 40, on_drag = function() return {{ k = 1 }} end, ui.div {{ id = "src", w = 40, h = 40 }} }},
                {area} {{ w = 200, h = 200,
                    on_drag_over = function(p, e) log:set("over " .. math.floor(e.local_x)) end,
                    on_drop = function(p, e) log:set(log:peek() .. " drop " .. p.k) end,
                    ui.div {{ id = "child", w = 100, h = 100 }},
                }},
                ui.text {{ "{{log}}", id = "log" }},
            }}"##
        );
        let mut h = Harness::inline(&code);
        h.ui.prepare_frame();
        let bounds = |h: &mut Harness, id: &str| {
            let node = h.find(id).unwrap();
            h.ui.scene.borrow().bounds(node)
        };
        let (src, dst) = (bounds(&mut h, "src"), bounds(&mut h, "child"));
        h.ui.dispatch(InputEvent::PointerDown {
            x: src.x + 5.,
            y: src.y + 5.,
            button: PointerButton::Primary,
        });
        h.ui.dispatch(InputEvent::PointerMove {
            x: src.x + 15.,
            y: src.y + 15.,
        });
        h.ui.dispatch(InputEvent::PointerMove {
            x: dst.x + 20.,
            y: dst.y + 20.,
        });
        h.ui.dispatch(InputEvent::PointerUp {
            x: dst.x + 20.,
            y: dst.y + 20.,
            button: PointerButton::Primary,
        });
        h.settle();
        assert_eq!(h.text("log"), "over 20 drop 1", "{area}");
    }
}

#[test]
fn context_menus_show_what_scripts_return_at_the_pointer() {
    let mut h = Harness::inline(
        r##"local log = ui.state("log", "")
        return ui.column {
            ui.div { id = "target", w = 200, h = 100,
                context_menu = function(e)
                    return {
                        ui.menu_item { "Copy", id = "copy", on_click = function() log:set("copy at " .. e.local_x) end },
                        ui.menu_item { "Paste", id = "paste", on_click = function() log:set("paste") end },
                    }
                end },
            ui.text { "{log}", id = "log" },
        }"##,
    );
    h.ui.prepare_frame();
    let target = h.find("target").unwrap();
    let b = h.ui.scene.borrow().bounds(target);
    let secondary = |h: &mut Harness, x: f32, y: f32| {
        h.ui.dispatch(InputEvent::PointerDown {
            x,
            y,
            button: PointerButton::Secondary,
        });
        h.ui.dispatch(InputEvent::PointerUp {
            x,
            y,
            button: PointerButton::Secondary,
        });
        h.settle();
        h.ui.prepare_frame();
    };
    secondary(&mut h, b.x + 25., b.y + 10.);
    let copy = h.find("copy").expect("the menu shows");
    let c = h.ui.scene.borrow().bounds(copy);
    assert!(
        c.x >= b.x + 25. && c.y >= b.y + 10.,
        "at the pointer: {c:?}"
    );
    h.click("copy");
    assert_eq!(h.text("log"), "copy at 25");
    assert_eq!(h.host.error().get(), None);
}
