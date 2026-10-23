//! Plugins an app manages (`set_plugins`): loaded from the disk with what
//! the app granted them, kept from restricted actions and shared state, and
//! given each slot's context (one panel component, an instance per pane).
mod common;
use common::Harness;
use std::path::PathBuf;
use zgui_script::{Capability, Plugin, ScriptHost, Value};

fn plugin(name: &str, code: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("zgui-plugins-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let entry = dir.join("init.luau");
    std::fs::write(&entry, code).unwrap();
    entry
}
fn reads(name: &str) -> Capability {
    Capability::Named(name.into())
}

const APP: &str = r#"
return ui.column {
    ui.slot("panel:notes", { id = "left", context = { pane = "p1" } }),
    ui.slot("panel:notes", { id = "right", context = { pane = "p2" } }),
    ui.slot("bar", { id = "bar" }),
}
"#;

#[test]
fn a_panel_component_is_an_instance_per_slot_with_its_context() {
    let entry = plugin(
        "notes",
        r#"
local Notes = ui.component(function(props)
    return ui.text { id = `note-{props.slot.pane}`, `notes for {props.slot.pane}` }
end)
ui.contribute("panel:notes", Notes {})
"#,
    );
    let mut h = Harness::new(|runtime| {
        let host = ScriptHost::inline(runtime, "app", APP);
        host.set_plugins(vec![Plugin {
            name: "notes".into(),
            entry,
            grants: vec![],
        }]);
        host
    });
    assert_eq!(h.text("note-p1"), "notes for p1");
    assert_eq!(h.text("note-p2"), "notes for p2");
}

#[test]
fn plugins_are_set_at_runtime_and_load_on_reload() {
    let mut h = Harness::inline(APP);
    assert!(h.find("hello").is_none());
    let entry = plugin(
        "late",
        r#"ui.contribute("bar", ui.text { "hello", id = "hello" })"#,
    );
    h.host.set_plugins(vec![Plugin {
        name: "late".into(),
        entry,
        grants: vec![],
    }]);
    h.host.reload().unwrap();
    h.settle();
    assert_eq!(h.text("hello"), "hello");
    // Taken away again.
    h.host.set_plugins(vec![]);
    h.host.reload().unwrap();
    h.settle();
    assert!(h.find("hello").is_none());
}

#[test]
fn restricted_actions_and_shared_state_need_the_grant() {
    let entry = plugin(
        "peek",
        r#"
local seen = ui.shared("sessions", {})
ui.invoke("send_text", "ls")
ui.contribute("bar", ui.text { "peeked", id = "peeked" })
"#,
    );
    let sent = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let make = |grants: Vec<Capability>| {
        let (entry, sent) = (entry.clone(), sent.clone());
        move |runtime: &zgui::reactive::Runtime| {
            let host = ScriptHost::inline(runtime, "app", APP);
            host.action("send_text", move |text| {
                if let Value::Str(text) = text {
                    sent.borrow_mut().push(text.to_string());
                }
            });
            host.shared("sessions", Value::Nil);
            host.expose_action("send_text", Some(reads("terminals.write")));
            host.expose_shared("sessions", Some(reads("terminals.read")));
            host.set_plugins(vec![Plugin {
                name: "peek".into(),
                entry,
                grants,
            }]);
            host
        }
    };
    // Without the grants: left out, with why.
    let mut h = Harness::new(make(vec![]));
    let error = h.host.error().get().unwrap_or_default();
    assert!(error.contains("ui.shared `sessions`"), "{error}");
    assert!(h.find("peeked").is_none());
    // With only one of them: the action is still out of reach.
    let h = Harness::new(make(vec![reads("terminals.read")]));
    let error = h.host.error().get().unwrap_or_default();
    assert!(error.contains("ui.invoke `send_text`"), "{error}");
    // With both.
    let mut h = Harness::new(make(vec![
        reads("terminals.read"),
        reads("terminals.write"),
    ]));
    assert_eq!(h.host.error().get(), None);
    assert_eq!(h.text("peeked"), "peeked");
    assert_eq!(*sent.borrow(), vec!["ls".to_string()]);
}

#[test]
fn the_app_itself_is_not_restricted() {
    let mut h = Harness::new(|runtime| {
        let host = ScriptHost::inline(
            runtime,
            "app",
            r#"local s = ui.shared("sessions", "x") return ui.text { function() return s:get() end, id = "s" }"#,
        );
        host.shared("sessions", "all of them");
        host.expose_shared("sessions", Some(reads("terminals.read")));
        host
    });
    assert_eq!(h.text("s"), "all of them");
}

#[test]
fn plugins_reach_only_what_the_app_exposes() {
    let try_plugin = |code: &str| {
        let entry = plugin("nosy", code);
        let h = Harness::new(move |runtime| {
            let host = ScriptHost::inline(runtime, "app", APP);
            host.action("close_window", |_| panic!("a plugin closed the window"));
            host.shared("saved_layout", "secret");
            host.set_plugins(vec![Plugin {
                name: "nosy".into(),
                entry,
                grants: vec![],
            }]);
            host
        });
        h.host.error().get().unwrap_or_default()
    };
    let error = try_plugin(r#"ui.invoke("close_window")"#);
    assert!(error.contains("does not expose it to plugins"), "{error}");
    let error = try_plugin(r#"ui.shared("saved_layout", nil)"#);
    assert!(error.contains("does not expose it to plugins"), "{error}");
    // Nor by naming it in a template.
    let error = try_plugin(r#"ui.contribute("bar", ui.text { "{saved_layout}" })"#);
    assert!(error.contains("does not expose it to plugins"), "{error}");
    let error = try_plugin(r#"ui.term("p1")"#);
    assert!(error.contains("Capability::Terminals"), "{error}");
    let error = try_plugin(r#"ui.web({})"#);
    assert!(error.contains("Capability::Web"), "{error}");
}

#[test]
fn a_slot_from_a_plugin_shows_only_what_it_contributes() {
    let mine = plugin(
        "mine",
        r#"ui.contribute("panel:x", ui.text { "mine", id = "mine" })"#,
    );
    let other = plugin(
        "other",
        r#"ui.contribute("panel:x", ui.text { "other", id = "other" })"#,
    );
    let mut h = Harness::new(|runtime| {
        let host = ScriptHost::inline(
            runtime,
            "app",
            r#"return ui.column { ui.slot("panel:x", { from = "mine" }) }"#,
        );
        host.set_plugins(vec![
            Plugin {
                name: "mine".into(),
                entry: mine,
                grants: vec![],
            },
            Plugin {
                name: "other".into(),
                entry: other,
                grants: vec![],
            },
        ]);
        host
    });
    assert_eq!(h.text("mine"), "mine");
    assert!(h.find("other").is_none(), "another plugin's is not shown");
}

#[test]
fn a_plugin_named_as_the_app_does_not_read_its_saved_state() {
    let entry = plugin(
        "app",
        r#"ui.contribute("bar", ui.text { ui.persist("token", "none"):peek(), id = "token" })"#,
    );
    let mut h = Harness::new(|runtime| {
        let host = ScriptHost::inline(
            runtime,
            "app",
            r#"ui.persist("token", "s3cret") return ui.column { ui.slot("bar") }"#,
        );
        host.set_plugins(vec![Plugin {
            name: "app".into(),
            entry,
            grants: vec![],
        }]);
        host
    });
    assert_eq!(h.text("token"), "none");
}

#[test]
fn a_plugin_neither_replaces_nor_breaks_the_apps_commands() {
    let entry = plugin(
        "keys",
        r#"
ui.command { id = "copy", title = "Steal", menu = "Edit", key = "Cmd+C", run = function() end }
ui.command { id = "note", title = "New Note", menu = "Notes", key = "cmd+shift+n", run = function() end }
"#,
    );
    let h = Harness::new(|runtime| {
        let host = ScriptHost::inline(
            runtime,
            "app",
            r#"ui.command { id = "copy", title = "Copy", menu = "Edit", key = "cmd+c", run = function() end }
return ui.column {}"#,
        );
        host.set_plugins(vec![Plugin {
            name: "keys".into(),
            entry,
            grants: vec![],
        }]);
        host
    });
    assert_eq!(h.host.error().get(), None);
    let list = h.host.commands().get();
    let found: Vec<_> = list
        .iter()
        .map(|c| (c.id.as_str(), c.key.as_deref()))
        .collect();
    assert_eq!(
        found,
        [
            ("keys/copy", None),
            ("keys/note", Some("cmd+shift+n")),
            ("copy", Some("cmd+c"))
        ]
    );
}

#[test]
fn a_plugins_pages_are_its_own_not_the_apps() {
    let entry = plugin(
        "pages",
        r#"
local mine = ui.web("web:p1")
ui.contribute("bar", ui.text { function() return "seen: " .. mine.url end, id = "seen" })
"#,
    );
    let mut h = Harness::new(|runtime| {
        let host = ScriptHost::inline(
            runtime,
            "app",
            r#"
local page = ui.web("web:p1")
page:load("https://mail.example.com/inbox")
return ui.column { ui.text { function() return page.url end, id = "app" }, ui.slot("bar") }
"#,
        );
        host.set_plugins(vec![Plugin {
            name: "pages".into(),
            entry,
            grants: vec![Capability::Web],
        }]);
        host
    });
    assert_eq!(h.host.error().get(), None);
    assert_eq!(h.text("app"), "https://mail.example.com/inbox");
    assert_eq!(
        h.text("seen"),
        "seen: ",
        "the plugin's web:p1 is another page"
    );
}
