//! App commands scripts declare: `ui.command { id, title, menu, key, run }`.
//!
//! Each run declares its commands afresh; after a successful load the host
//! publishes them ([`crate::ScriptHost::commands`]) for the application to
//! show as menu items with key equivalents, and calls them back with
//! [`crate::ScriptHost::run_command`]. Menus reach the app wherever focus is,
//! a terminal or a web page included.
use crate::{Vm, node::env};
use mlua::{Function, Lua, Table};
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};
use zgui::reactive::{Runtime, Signal};

/// A command as the application shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    /// Unique; empty for a separator.
    pub id: String,
    pub title: String,
    /// The menu it goes in ("File", "Edit", "View", "Window", or any other).
    pub menu: String,
    /// Its shortcut: modifiers and a key joined by `+`, as `cmd+shift+d`,
    /// `cmd+alt+arrowleft`, `cmd+1`, `ctrl+tab`.
    pub key: Option<String>,
    /// A separator line, not a command.
    pub separator: bool,
}

type Runner = (Function, Weak<Vm>);
pub(crate) struct Commands {
    pending: RefCell<Vec<(Command, Option<Runner>)>>,
    runners: RefCell<HashMap<String, Runner>>,
    pub list: Signal<Vec<Command>>,
}
impl Commands {
    pub fn new(runtime: &Runtime) -> Self {
        Self {
            pending: RefCell::default(),
            runners: RefCell::default(),
            list: runtime.signal(Vec::new()),
        }
    }
    /// A load begins: what it declares replaces what is published.
    pub fn begin(&self) {
        self.pending.borrow_mut().clear();
    }
    /// The load succeeded: publish what it declared.
    pub fn publish(&self) {
        let pending = std::mem::take(&mut *self.pending.borrow_mut());
        let mut runners = HashMap::new();
        let mut list = Vec::with_capacity(pending.len());
        for (command, runner) in pending {
            if let Some(runner) = runner {
                runners.insert(command.id.clone(), runner);
            }
            list.push(command);
        }
        *self.runners.borrow_mut() = runners;
        if self.list.with_untracked(|old| *old != list) {
            self.list.set(list);
        }
    }
    /// Run the command `id`; whether one ran.
    pub fn run(&self, id: &str) -> bool {
        let runner = self.runners.borrow().get(id).cloned();
        let Some((f, vm)) = runner else {
            return false;
        };
        let Some(vm) = vm.upgrade() else {
            return false;
        };
        if let Err(error) = vm.runtime.clone().batch(|| vm.call::<()>(&f, ())) {
            vm.report(error);
        }
        true
    }
}

/// Installs `ui.command`.
pub(crate) fn install(lua: &Lua, ui: &Table, commands: Rc<Commands>) -> mlua::Result<()> {
    ui.set(
        "command",
        lua.create_function(move |lua, table: Table| {
            let menu = table
                .get::<Option<String>>("menu")?
                .unwrap_or_else(|| "File".into());
            if table.get::<Option<bool>>("separator")?.unwrap_or(false) {
                commands.pending.borrow_mut().push((
                    Command {
                        id: String::new(),
                        title: String::new(),
                        menu,
                        key: None,
                        separator: true,
                    },
                    None,
                ));
                return Ok(());
            }
            let id: String = table
                .get::<Option<String>>("id")?
                .ok_or_else(|| mlua::Error::runtime("ui.command needs an `id`"))?;
            let title = table
                .get::<Option<String>>("title")?
                .unwrap_or_else(|| id.clone());
            let run: Function = table
                .get::<Option<Function>>("run")?
                .ok_or_else(|| mlua::Error::runtime(format!("ui.command `{id}` needs `run`")))?;
            let key = table.get::<Option<String>>("key")?;
            let vm = env(lua)?.vm.clone();
            let mut pending = commands.pending.borrow_mut();
            if pending.iter().any(|(c, _)| c.id == id) {
                return Err(mlua::Error::runtime(format!(
                    "ui.command `{id}` is declared twice"
                )));
            }
            pending.push((
                Command {
                    id,
                    title,
                    menu,
                    key,
                    separator: false,
                },
                Some((run, vm)),
            ));
            Ok(())
        })?,
    )
}
