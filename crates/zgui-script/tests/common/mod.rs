//! The test harness: a headless Ui with a task executor and a frame clock
//! that tests deliver frames to.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};
use zgui::{
    compose::{TaskRunner, ViewHandle, prelude::*},
    frame::{Frame, FrameClock},
    input::{InputEvent, PointerButton},
    reactive::Runtime,
    scene::NodeKind,
    task::LocalExecutor,
    widgets::Ui,
};
use zgui_script::ScriptHost;

#[allow(dead_code)]
pub struct Harness {
    pub ui: Ui,
    pub host: ScriptHost,
    pub handle: ViewHandle,
    pub executor: Rc<RefCell<LocalExecutor>>,
    pub clock: FrameClock,
    pub frame: Cell<u64>,
    pub start: Instant,
    /// Native views (web pages) the script placed; unsupported, so they
    /// show placeholders.
    pub natives: zgui::native_view::NativeViews,
}
#[allow(dead_code)]
impl Harness {
    pub fn new(host: impl FnOnce(&Runtime) -> ScriptHost) -> Self {
        let mut ui = Ui::new(400., 300.);
        let host = host(&ui.runtime);
        host.reload().unwrap();
        let executor = Rc::new(RefCell::new(LocalExecutor::new()));
        let clock = FrameClock::new();
        let natives = zgui::native_view::NativeViews::new();
        let handle = ui.mount(provide(
            natives.clone(),
            provide(
                TaskRunner::from_executor(executor.clone()),
                provide(clock.clone(), host.view()),
            ),
        ));
        let harness = Self {
            ui,
            host,
            handle,
            executor,
            clock,
            frame: Cell::new(0),
            start: Instant::now(),
            natives,
        };
        harness.settle();
        // A window lays out before its first frame.
        harness.ui.prepare_frame();
        harness.settle();
        harness
    }
    pub fn inline(code: &str) -> Self {
        Self::new(|runtime| ScriptHost::inline(runtime, "app", code))
    }
    pub fn settle(&self) {
        while self.executor.borrow_mut().tick() > 0 {}
    }
    pub fn find(&mut self, id: &str) -> Option<zgui::scene::NodeId> {
        self.ui.prepare_frame();
        self.handle.find(id)
    }
    /// The text of node `id`, or of the label inside it (styled text has a
    /// layout box around its label).
    pub fn text(&mut self, id: &str) -> String {
        let node = self.find(id).unwrap_or_else(|| panic!("no node `{id}`"));
        let scene = self.ui.scene.borrow();
        let mut stack = vec![node];
        while let Some(n) = stack.pop() {
            if let NodeKind::Text { text, .. } = scene.kind(n) {
                return text.to_string();
            }
            stack.extend(scene.children(n).iter().rev());
        }
        panic!("no text in `{id}`: {:?}", scene.kind(node))
    }
    pub fn click(&mut self, id: &str) {
        let node = self.find(id).unwrap_or_else(|| panic!("no node `{id}`"));
        let b = self.ui.scene.borrow().bounds(node);
        let (x, y) = (b.x + b.width / 2., b.y + b.height / 2.);
        self.ui.dispatch(InputEvent::PointerDown {
            x,
            y,
            button: PointerButton::Primary,
        });
        self.ui.dispatch(InputEvent::PointerUp {
            x,
            y,
            button: PointerButton::Primary,
        });
        self.settle();
    }
    /// Delivers one 60 Hz display frame and lays out, as a window does.
    pub fn frame(&self) {
        let index = self.frame.get() + 1;
        self.frame.set(index);
        let interval = Duration::from_secs_f64(1. / 60.);
        self.clock.deliver(Frame {
            index,
            time: self.start + interval * index as u32,
            interval,
        });
        self.settle();
        self.ui.prepare_frame();
        self.settle();
    }
    pub fn opacity(&mut self, id: &str) -> f32 {
        let node = self.find(id).unwrap();
        self.ui.scene.borrow().effects(node).opacity
    }
}
