//! `ui.every(ms, fn)` and `ui.after(ms, fn)`: timers on zgx's own clock
//! that wake the app only when they fire (no frames between), owned like
//! `ui.effect`: by the component or builder that made them, else the script,
//! so a reload or an unmount stops them.
use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    rc::Rc,
    task::Poll,
    time::Duration,
};

type Pending = Pin<Box<dyn Future<Output = ()>>>;

/// Pause and resume for every timer of a host: a timer that comes due while
/// paused waits for the resume, which wakes it at once.
#[derive(Default)]
struct Gate {
    paused: Cell<bool>,
    /// A waker per timer waiting out the pause (`wait` polls each in turn).
    wakers: RefCell<Vec<std::task::Waker>>,
}
impl Gate {
    fn set(&self, paused: bool) {
        if paused == self.paused.replace(paused) {
            return;
        }
        if !paused {
            for waker in self.wakers.borrow_mut().drain(..) {
                waker.wake();
            }
        }
    }
    /// Resolves once unpaused (at once while unpaused already).
    fn wait(&self) -> impl Future<Output = ()> + '_ {
        std::future::poll_fn(|cx| {
            if !self.paused.get() {
                return Poll::Ready(());
            }
            self.wakers.borrow_mut().push(cx.waker().clone());
            Poll::Pending
        })
    }
}
/// Where timers run: the host view's tasks, once it mounts (a script makes
/// its timers while it loads, before that; they wait here).
#[derive(Default)]
pub(crate) struct Timers {
    tasks: RefCell<Option<zgx::compose::Tasks>>,
    pending: RefCell<Vec<Pending>>,
    /// `set_paused`'s hold on every timer: `fire`s wait it out.
    gate: Rc<Gate>,
}
impl Timers {
    pub fn set_tasks(&self, tasks: zgx::compose::Tasks) {
        for task in self.pending.take() {
            tasks.spawn(task);
        }
        *self.tasks.borrow_mut() = Some(tasks);
    }
    /// Hold every timer's `fire` until resumed (`ScriptHost::pause_timers`): a
    /// host whose window is out of sight has no use for the callbacks. Waits
    /// already made wait out the pause.
    pub fn set_paused(&self, paused: bool) {
        self.gate.set(paused);
    }
    pub(crate) fn spawn(&self, task: impl Future<Output = ()> + 'static) {
        match &*self.tasks.borrow() {
            Some(tasks) => tasks.spawn(task),
            None => self.pending.borrow_mut().push(Box::pin(task)),
        }
    }
    /// Call `fire` after `period`, then every `period` while `repeat`, until
    /// the returned guard drops or `fire` says stop (returns false).
    pub fn start(
        &self,
        period: Duration,
        repeat: bool,
        mut fire: impl FnMut() -> bool + 'static,
    ) -> Stop {
        let alive = Rc::new(Cell::new(true));
        let running = alive.clone();
        let gate = self.gate.clone();
        self.spawn(async move {
            loop {
                zgx::timer::sleep(period).await;
                if !running.get() {
                    return;
                }
                gate.wait().await;
                if !running.get() || !fire() || !repeat {
                    return;
                }
            }
        });
        Stop(alive)
    }
}
/// Stops its timer when dropped (or `stop`ped).
pub(crate) struct Stop(Rc<Cell<bool>>);
impl Stop {
    pub fn handle(&self) -> Rc<Cell<bool>> {
        self.0.clone()
    }
}
impl Drop for Stop {
    fn drop(&mut self) {
        self.0.set(false);
    }
}
