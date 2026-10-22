//! Luau scripts that describe zgui interfaces, animated with anime.js's API.
//!
//! A script runs once per (re)load and returns a description of components,
//! state bindings and animations. Rust lowers that description to ordinary
//! retained views: layout, drawing, state and animation all stay in Rust,
//! and the script is called again only for event handlers, components as
//! they mount, and properties the script explicitly computes with a
//! function. Text templates such as `"Count: {count}"` and every animation
//! run without calling the script.
//!
//! State is Rust-owned and keyed, so a hot reload keeps it. Each script gets
//! its own sandboxed VM with a memory limit and a time budget per call, which
//! makes plugins safe to load next to the application script.
//!
//! ```lua
//! local count = ui.state("count", 0)
//! anime.animate(".dot", { y = { 0, -8, 0 }, delay = anime.stagger(40), loop = true })
//! return ui.column {
//!     p = 24, gap = 12, bg = "#172030",
//!     ui.text { "Count: {count}", text_size = 18 },
//!     ui.button { on_click = ui.add(count), px = 16, py = 10, "Increment" },
//! }
//! ```
//!
//! The `ui` and `anime` libraries are documented in `docs/scripting.md`.
mod anim;
mod anime;
mod canvas;
mod command;
mod element;
mod lower;
mod node;
mod term;
mod transform;
mod value;

pub use command::Command;
pub use value::{Store, Value};

use mlua::{Lua, VmState};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant, SystemTime},
};
use zgui::{
    compose::{View, prelude::*, switch},
    reactive::{Runtime, Signal},
};

/// Resource limits for each script.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Heap limit per script VM.
    pub memory: usize,
    /// Budget for evaluating a script on load.
    pub load: Duration,
    /// Budget for one handler or computed property call.
    pub call: Duration,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            memory: 32 << 20,
            load: Duration::from_millis(500),
            call: Duration::from_millis(50),
        }
    }
}

pub(crate) type Actions = Rc<RefCell<HashMap<String, Rc<dyn Fn(Value)>>>>;

/// Native views the host provides by key (`ScriptHost::native`), placed by
/// `ui.native { key = … }`. They outlive reloads.
pub(crate) struct Natives {
    views: RefCell<HashMap<String, zgui::native_view::NativeContent>>,
    /// Bumped on every change: `ui.native` nodes look their key up again.
    revision: Signal<u64>,
}
impl Natives {
    fn new(runtime: &Runtime) -> Self {
        Self {
            views: RefCell::default(),
            revision: runtime.signal(0),
        }
    }
    /// `key`'s content, tracking changes to what is provided.
    pub(crate) fn get(&self, key: &str) -> Option<zgui::native_view::NativeContent> {
        self.revision.get();
        self.views.borrow().get(key).cloned()
    }
}

/// One script's VM.
/// `.luaurc`'s `aliases` in `dir`, resolved against it.
fn read_aliases(dir: &std::path::Path) -> HashMap<String, PathBuf> {
    let Ok(text) = std::fs::read_to_string(dir.join(".luaurc")) else {
        return HashMap::new();
    };
    let Ok(config) = serde_json::from_str::<serde_json::Value>(&text) else {
        eprintln!("{}: .luaurc is not JSON", dir.display());
        return HashMap::new();
    };
    config
        .get("aliases")
        .and_then(|a| a.as_object())
        .map(|aliases| {
            aliases
                .iter()
                .filter_map(|(name, path)| {
                    Some((name.to_lowercase(), normalize(&dir.join(path.as_str()?))))
                })
                .collect()
        })
        .unwrap_or_default()
}
/// `path` with `.` and `..` resolved in place, without touching the disk.
fn normalize(path: &std::path::Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// A script call's time budget. It counts the CPU time the thread spends,
/// so a busy machine does not fail a script that is not looping; the cheap
/// wall clock decides when the CPU time is worth reading.
#[derive(Clone, Copy)]
struct Deadline {
    wall: Instant,
    cpu: Duration,
    budget: Duration,
}
impl Deadline {
    fn start(budget: Duration) -> Self {
        Self {
            wall: Instant::now(),
            cpu: thread_cpu(),
            budget,
        }
    }
    /// The same budget with the time `other` has run since it started
    /// taken off the clock.
    fn shifted(self, other: &Deadline) -> Self {
        Self {
            wall: self.wall + other.wall.elapsed(),
            cpu: self.cpu + thread_cpu().saturating_sub(other.cpu),
            budget: self.budget,
        }
    }
    fn exceeded(&self) -> bool {
        self.wall.elapsed() > self.budget && thread_cpu().saturating_sub(self.cpu) > self.budget
    }
}
/// CPU time this thread has used.
#[cfg(unix)]
fn thread_cpu() -> Duration {
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `t` is a valid out-pointer for the duration of the call.
    let ok = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) } == 0;
    if ok {
        Duration::new(t.tv_sec as u64, t.tv_nsec as u32)
    } else {
        Duration::ZERO
    }
}
/// Without a thread CPU clock, wall time stands in.
#[cfg(not(unix))]
fn thread_cpu() -> Duration {
    thread_local! { static START: Instant = Instant::now(); }
    START.with(|s| s.elapsed())
}

pub(crate) struct Vm {
    name: String,
    /// Keeps the state alive: script values only hold weak references to it.
    pub(crate) lua: Lua,
    /// Where relative image and SVG paths resolve from.
    pub(crate) base: Option<PathBuf>,
    deadline: Rc<Cell<Option<Deadline>>>,
    budget: Duration,
    pub(crate) runtime: Runtime,
    errors: Signal<Option<String>>,
}
impl Vm {
    /// Runs `f` with its own budget. A nested call (a component built
    /// while a handler runs) stops the outer call's clock meanwhile.
    fn guard<R>(&self, budget: Duration, f: impl FnOnce() -> mlua::Result<R>) -> mlua::Result<R> {
        let started = Deadline::start(budget);
        let outer = self.deadline.replace(Some(started));
        let result = f();
        self.deadline.set(outer.map(|d| d.shifted(&started)));
        result
    }
    /// Runs Rust work a script asked for (a state write and the interface
    /// it rebuilds) off the script's clock: the budget stops runaway
    /// scripts, not the updates they cause.
    pub(crate) fn off_clock<R>(&self, f: impl FnOnce() -> R) -> R {
        let started = Deadline::start(Duration::ZERO);
        let result = f();
        if let Some(d) = self.deadline.get() {
            self.deadline.set(Some(d.shifted(&started)));
        }
        result
    }
    pub(crate) fn call<R: mlua::FromLuaMulti>(
        &self,
        f: &mlua::Function,
        args: impl mlua::IntoLuaMulti,
    ) -> mlua::Result<R> {
        self.guard(self.budget, || f.call(args))
    }
    pub(crate) fn report(&self, error: mlua::Error) {
        let message = describe(&self.name, &error);
        eprintln!("{message}");
        self.errors.set(Some(message));
    }
}

/// An error with Rust-side frames removed from its traceback.
fn describe(name: &str, error: &mlua::Error) -> String {
    let text = error.to_string();
    let lines: Vec<_> = text
        .lines()
        .filter(|line| line.trim() != "[C]: in ?")
        .collect();
    let lines = match lines.as_slice() {
        [message @ .., last] if *last == "stack traceback:" => message,
        all => all,
    };
    format!("{name}: {}", lines.join("\n"))
}

enum Script {
    File(PathBuf),
    Inline { name: String, code: String },
}
impl Script {
    /// Shown in errors and tracebacks.
    fn name(&self) -> String {
        match self {
            Script::File(path) => path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned(),
            Script::Inline { name, .. } => name.clone(),
        }
    }
    /// The directory a file script is in.
    fn dir(&self) -> Option<PathBuf> {
        match self {
            Script::File(path) => path.parent().map(PathBuf::from),
            Script::Inline { .. } => None,
        }
    }
    /// Keys the script's state across reloads.
    fn key(&self) -> String {
        match self {
            Script::File(path) => path.display().to_string(),
            Script::Inline { name, .. } => format!("inline:{name}"),
        }
    }
    fn code(&self) -> Result<String, String> {
        match self {
            Script::File(path) => {
                std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
            }
            Script::Inline { code, .. } => Ok(code.clone()),
        }
    }
}

struct Loaded {
    _vms: Vec<Rc<Vm>>,
    root: Rc<node::Node>,
    lowering: Rc<lower::Lowering>,
}

struct Inner {
    runtime: Runtime,
    app: Script,
    plugin_dir: Option<PathBuf>,
    plugins: Vec<Script>,
    limits: Limits,
    stores: RefCell<HashMap<String, Rc<Store>>>,
    shared: Rc<Store>,
    elements: Rc<element::Elements>,
    engine: Rc<anime::Engine>,
    assets: Rc<lower::Assets>,
    actions: Actions,
    current: RefCell<Option<Rc<Loaded>>>,
    generation: Signal<u64>,
    error: Signal<Option<String>>,
    stamps: RefCell<Vec<(PathBuf, Option<SystemTime>)>>,
    /// Files scripts `require`d in the last load: watched with the scripts.
    required: RefCell<Vec<PathBuf>>,
    /// Terminal sessions (`ui.term`): they outlive reloads.
    terms: Rc<term::Terms>,
    /// App commands (`ui.command`) the last successful load declared.
    commands: Rc<command::Commands>,
    /// Host-provided native views (`ui.native`): they outlive reloads.
    natives: Rc<Natives>,
}

/// Loads an application script and its plugins and mounts what they describe.
#[derive(Clone)]
pub struct ScriptHost(Rc<Inner>);
/// A script run: its VM, the view it returned and its slot contributions.
type Ran = (
    Rc<Vm>,
    Option<Rc<node::Node>>,
    Vec<(String, Rc<node::Node>)>,
);

impl ScriptHost {
    /// A host for the application script at `path`.
    pub fn new(runtime: &Runtime, path: impl Into<PathBuf>) -> Self {
        Self::with(runtime, Script::File(path.into()))
    }
    /// A host for script source held in memory.
    pub fn inline(runtime: &Runtime, name: impl Into<String>, code: impl Into<String>) -> Self {
        Self::with(
            runtime,
            Script::Inline {
                name: name.into(),
                code: code.into(),
            },
        )
    }
    fn with(runtime: &Runtime, app: Script) -> Self {
        let elements = Rc::new(element::Elements::default());
        Self(Rc::new(Inner {
            runtime: runtime.clone(),
            app,
            plugin_dir: None,
            plugins: Vec::new(),
            limits: Limits::default(),
            stores: RefCell::default(),
            elements: elements.clone(),
            engine: {
                let engine = anime::Engine::new(runtime, elements.clone());
                let weak = Rc::downgrade(&engine);
                elements.set_wake(Rc::new(move || {
                    if let Some(engine) = weak.upgrade() {
                        engine.wake();
                    }
                }));
                engine
            },
            assets: Rc::default(),
            shared: Rc::new(Store::new(runtime)),
            actions: Actions::default(),
            current: RefCell::default(),
            generation: runtime.signal(0),
            error: runtime.signal(None),
            stamps: RefCell::default(),
            required: RefCell::default(),
            terms: Rc::default(),
            commands: Rc::new(command::Commands::new(runtime)),
            natives: Rc::new(Natives::new(runtime)),
        }))
    }
    fn configure(self, f: impl FnOnce(&mut Inner)) -> Self {
        let mut inner = Rc::try_unwrap(self.0)
            .unwrap_or_else(|_| panic!("configure a ScriptHost before cloning it"));
        f(&mut inner);
        Self(Rc::new(inner))
    }
    /// Loads every `.luau` file in `dir` as a plugin.
    pub fn plugins(self, dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        self.configure(|inner| inner.plugin_dir = Some(dir))
    }
    /// Adds a plugin from source held in memory.
    pub fn plugin(self, name: impl Into<String>, code: impl Into<String>) -> Self {
        let script = Script::Inline {
            name: name.into(),
            code: code.into(),
        };
        self.configure(|inner| inner.plugins.push(script))
    }
    pub fn limits(self, limits: Limits) -> Self {
        self.configure(|inner| inner.limits = limits)
    }

    /// Registers a Rust function scripts can bind with `ui.action(name, arg)`.
    pub fn action(&self, name: impl Into<String>, action: impl Fn(Value) + 'static) {
        self.0
            .actions
            .borrow_mut()
            .insert(name.into(), Rc::new(action));
    }
    /// Provides a native view under `key`, for scripts to place with
    /// `ui.native { key = key, … }`: laid out, clipped, rounded and faded as
    /// any node, with zgui content drawn over it on the overlay plane. On
    /// macOS the desktop host mounts `NativeContent::Platform(Rc::new(view))`
    /// for a `Retained<NSView>`, and sets the view's frame to the node's box
    /// in window points. Providing other content for a key swaps the view in
    /// place; it is kept across reloads until `remove_native`.
    pub fn native(&self, key: impl Into<String>, content: zgui::native_view::NativeContent) {
        self.0.natives.views.borrow_mut().insert(key.into(), content);
        self.0.natives.revision.update(|r| *r += 1);
    }
    /// Stops providing `key`'s view (its nodes show a placeholder); what it was.
    pub fn remove_native(&self, key: &str) -> Option<zgui::native_view::NativeContent> {
        let removed = self.0.natives.views.borrow_mut().remove(key);
        if removed.is_some() {
            self.0.natives.revision.update(|r| *r += 1);
        }
        removed
    }
    /// State shared by the application and every script (`ui.shared`). Rust
    /// can publish model data here and observe what scripts write.
    pub fn shared(&self, key: &str, initial: impl Into<Value>) -> Signal<Value> {
        self.0.shared.state(key, initial.into())
    }
    /// The app commands scripts declare (`ui.command`), in declaration
    /// order, as of the last successful load: show them as menu items.
    pub fn commands(&self) -> Signal<Vec<Command>> {
        self.0.commands.list.clone()
    }
    /// Run the command `id` (a menu item was chosen); whether one ran.
    pub fn run_command(&self, id: &str) -> bool {
        self.0.commands.run(id)
    }
    /// The latest load or script error, shown over the interface until fixed.
    pub fn error(&self) -> Signal<Option<String>> {
        self.0.error.clone()
    }

    /// Evaluates every script again and remounts the interface. State is kept.
    /// On failure the previous interface stays mounted and the error is shown.
    pub fn reload(&self) -> Result<(), String> {
        self.0.commands.begin();
        let loaded = self.load();
        // After loading: the files it required are watched too.
        *self.0.stamps.borrow_mut() = self.stamps();
        match loaded {
            Ok(loaded) => {
                *self.0.current.borrow_mut() = Some(Rc::new(loaded));
                self.0.commands.publish();
                self.0.runtime.batch(|| {
                    self.0.error.set(None);
                    self.0.generation.update(|g| *g += 1);
                });
                // The replaced scripts' animations end with them.
                self.0.engine.drop_orphans();
                Ok(())
            }
            Err(error) => {
                eprintln!("{error}");
                self.0.error.set(Some(error.clone()));
                Err(error)
            }
        }
    }
    /// Reloads if any script file changed since the last load.
    pub fn reload_if_changed(&self) -> bool {
        if self.stamps() == *self.0.stamps.borrow() {
            return false;
        }
        let _ = self.reload();
        true
    }
    /// Polls script files and reloads when they change. Spawn this on the
    /// window's tasks during development; it only stats files.
    pub fn watch(&self, every: Duration) -> impl std::future::Future<Output = ()> + 'static {
        let host = self.clone();
        async move {
            loop {
                zgui::timer::sleep(every).await;
                host.reload_if_changed();
            }
        }
    }

    /// The mounted interface, with an error banner above it.
    pub fn view(&self) -> View {
        let host = self.0.clone();
        let generation = self.0.generation.clone();
        let app = switch(
            move || generation.get(),
            move |_, _| {
                let current = host.current.borrow().clone();
                current.map_or_else(div, |loaded| lower::lower(&loaded.root, &loaded.lowering))
            },
        );
        let error = self.0.error.clone();
        let banner = switch(
            move || error.get(),
            |message, _| match message {
                None => div(),
                Some(message) => div()
                    .p(12.)
                    .rounded(8.)
                    .bg(rgba(0x4a1820f0))
                    .text_color(rgb(0xffd7d7))
                    .child(
                        zgui::compose::text(message)
                            .id("script-error")
                            .font_family(FontFamily::Monospace)
                            .text_size(12.)
                            .text_wrap(true),
                    ),
            },
        );
        // The region is what is positioned: insets resolve against the window.
        let banner = banner.absolute().left(12.).right(12.).bottom(12.);
        let engine = self.0.engine.clone();
        component(move |cx| {
            // One frame loop ticks every animation; it idles at rest.
            if let Some(frames) = cx.try_service::<zgui::frame::FrameClock>() {
                cx.tasks().spawn(engine.run((*frames).clone()));
            }
            overlay()
                .w_full()
                .h_full()
                .child(app.w_full().h_full())
                .child(banner)
        })
    }

    fn scripts(&self) -> Vec<Script> {
        let mut files: Vec<_> = self
            .0
            .plugin_dir
            .as_ref()
            .and_then(|dir| std::fs::read_dir(dir).ok())
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "luau"))
            .collect();
        files.sort();
        files.into_iter().map(Script::File).collect()
    }
    fn stamps(&self) -> Vec<(PathBuf, Option<SystemTime>)> {
        let mut paths: Vec<_> = self
            .scripts()
            .into_iter()
            .filter_map(|s| match s {
                Script::File(path) => Some(path),
                Script::Inline { .. } => None,
            })
            .collect();
        if let Script::File(path) = &self.0.app {
            paths.push(path.clone());
        }
        paths.extend(self.0.required.borrow().iter().cloned());
        paths
            .into_iter()
            .map(|path| {
                let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
                (path, modified)
            })
            .collect()
    }

    fn load(&self) -> Result<Loaded, String> {
        self.0.required.borrow_mut().clear();
        let mut vms = Vec::new();
        let mut slots: HashMap<String, Vec<Rc<node::Node>>> = HashMap::new();
        let files = self.scripts();
        for script in self.0.plugins.iter().chain(&files) {
            let (vm, root, contributions) = self.run(script)?;
            if root.is_some() {
                return Err(format!(
                    "{}: a plugin contributes with ui.contribute; it returns nothing",
                    script.name()
                ));
            }
            for (slot, node) in contributions {
                slots.entry(slot).or_default().push(node);
            }
            vms.push(vm);
        }
        let (vm, root, contributions) = self.run(&self.0.app)?;
        for (slot, node) in contributions {
            slots.entry(slot).or_default().push(node);
        }
        vms.push(vm);
        let root = root.ok_or_else(|| {
            format!(
                "{}: the application script must return a ui node",
                self.0.app.name()
            )
        })?;
        Ok(Loaded {
            _vms: vms,
            root,
            lowering: Rc::new(lower::Lowering {
                slots,
                actions: self.0.actions.clone(),
                elements: self.0.elements.clone(),
                runtime: self.0.runtime.clone(),
                assets: self.0.assets.clone(),
                natives: self.0.natives.clone(),
            }),
        })
    }

    #[allow(clippy::type_complexity)]
    /// `require(path)`, as Luau resolves it: `"./card"` and `"../ui/card"`
    /// from the requiring file, `"@alias/card"` from the `.luaurc` aliases
    /// beside the script, `"@self/card"` from the requiring file's own
    /// directory, a directory as its `init.luau`; a bare `"card"` from the
    /// script's directory. Modules stay inside the script's directory, run
    /// once and return what they return; a change to one reloads the scripts.
    fn install_require(&self, lua: &Lua, dir: Option<PathBuf>) -> mlua::Result<()> {
        let host = Rc::downgrade(&self.0);
        let modules: Rc<RefCell<HashMap<PathBuf, Option<mlua::Value>>>> = Rc::default();
        let root = dir.map(|d| d.canonicalize().unwrap_or(d));
        let aliases = root.as_deref().map(read_aliases).unwrap_or_default();
        let require = lua.create_function(move |lua, name: String| {
            let Some(root) = &root else {
                return Err(mlua::Error::runtime(
                    "require needs a script loaded from a file",
                ));
            };
            let fail = |why: String| mlua::Error::runtime(format!("require(\"{name}\"): {why}"));
            // The requiring file, from its chunk name (its path in the root).
            let caller = lua
                .inspect_stack(1, |d| d.source().source.map(|s| s.into_owned()))
                .flatten()
                .and_then(|s| s.strip_prefix('@').map(|rel| root.join(rel)));
            let here = caller
                .as_deref()
                .and_then(std::path::Path::parent)
                .unwrap_or(root)
                .to_path_buf();
            let joined = if let Some(rest) = name.strip_prefix("@self") {
                here.join(rest.trim_start_matches('/'))
            } else if let Some(rest) = name.strip_prefix('@') {
                let (alias, rest) = rest.split_once('/').unwrap_or((rest, ""));
                let base = aliases
                    .get(alias)
                    .ok_or_else(|| fail(format!("no alias `@{alias}` in .luaurc")))?;
                base.join(rest)
            } else if name.starts_with("./") || name.starts_with("../") {
                here.join(&name)
            } else {
                root.join(&name)
            };
            let mut path = normalize(&joined);
            if !path.starts_with(root) {
                return Err(fail(
                    "modules come from the script's directory or below".into(),
                ));
            }
            if path.is_dir() {
                path = path.join("init.luau");
            } else if path.extension().is_none() {
                path.set_extension("luau");
            }
            match modules.borrow().get(&path) {
                Some(Some(value)) => return Ok(value.clone()),
                Some(None) => return Err(fail("a require cycle".into())),
                None => {}
            }
            let code = std::fs::read_to_string(&path)
                .map_err(|e| fail(format!("{}: {e}", path.display())))?;
            if let Some(host) = host.upgrade() {
                host.required.borrow_mut().push(path.clone());
            }
            modules.borrow_mut().insert(path.clone(), None);
            // Named by its path in the root: errors show it, and its own
            // requires resolve from it.
            let chunk = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            let value = match lua
                .load(code)
                .set_name(format!("@{chunk}"))
                .eval::<mlua::Value>()
            {
                Ok(mlua::Value::Nil) => mlua::Value::Boolean(true),
                Ok(value) => value,
                Err(e) => {
                    modules.borrow_mut().remove(&path);
                    return Err(e);
                }
            };
            modules.borrow_mut().insert(path, Some(value.clone()));
            Ok(value)
        })?;
        lua.globals().set("require", require)
    }

    fn run(&self, script: &Script) -> Result<Ran, String> {
        let name = script.name();
        let fail = |error: mlua::Error| describe(&name, &error);
        let code = script.code()?;
        let lua = Lua::new();
        let limits = self.0.limits;
        lua.set_memory_limit(limits.memory).map_err(fail)?;
        let deadline: Rc<Cell<Option<Deadline>>> = Rc::default();
        {
            let deadline = deadline.clone();
            lua.set_interrupt(move |_| match deadline.get() {
                Some(d) if d.exceeded() => {
                    Err(mlua::Error::runtime("script exceeded its time budget"))
                }
                _ => Ok(VmState::Continue),
            });
        }
        let vm = Rc::new(Vm {
            name: name.clone(),
            lua: lua.clone(),
            base: script.dir(),
            deadline,
            budget: limits.call,
            runtime: self.0.runtime.clone(),
            errors: self.0.error.clone(),
        });
        let store = self
            .0
            .stores
            .borrow_mut()
            .entry(script.key())
            .or_insert_with(|| Rc::new(Store::new(&self.0.runtime)))
            .clone();
        lua.set_app_data(node::Env {
            vm: Rc::downgrade(&vm),
            store,
            shared: self.0.shared.clone(),
            contributions: RefCell::default(),
            engine: self.0.engine.clone(),
            runtime: self.0.runtime.clone(),
            scope: RefCell::default(),
            retained: RefCell::default(),
            actions: self.0.actions.clone(),
        });
        node::install(&lua).map_err(fail)?;
        let ui: mlua::Table = lua.globals().get("ui").map_err(fail)?;
        term::install(&lua, &ui, self.0.terms.clone()).map_err(fail)?;
        command::install(&lua, &ui, self.0.commands.clone()).map_err(fail)?;
        ui.set_readonly(true);
        anime::api::install(&lua).map_err(fail)?;
        self.install_require(&lua, script.dir()).map_err(fail)?;
        lua.sandbox(true).map_err(fail)?;
        // Building runs untracked: reading state while describing the tree
        // must not subscribe whatever is currently mounting.
        let result = self.0.runtime.untracked(|| {
            vm.guard(limits.load, || {
                lua.load(code)
                    .set_name(format!("@{name}"))
                    .eval::<mlua::Value>()
            })
        });
        let root = match result.map_err(fail)? {
            mlua::Value::Nil => None,
            mlua::Value::UserData(ud) if ud.is::<node::NodeUd>() => {
                Some(ud.borrow::<node::NodeUd>().map_err(fail)?.0.clone())
            }
            other => {
                return Err(format!(
                    "{name}: a script returns a ui node or nothing, not {}",
                    other.type_name()
                ));
            }
        };
        let contributions = lua
            .app_data_ref::<node::Env>()
            .map(|env| env.contributions.take())
            .unwrap_or_default();
        Ok((vm, root, contributions))
    }
}
