//! The focused editor's caret gliding to where an edit put it
//! (`EditorHandle::set_caret_glide`), rather than jumping there: words
//! streaming into a field (dictation) carry the caret along with them.
//!
//! Only an edit glides it; a click, an arrow key or a scroll puts it in
//! place at once. Along a line it slides; onto another line it goes at
//! once and slides along that line from its start (or, going up, from its
//! end), never across the text between. Each step moves the caret's
//! rectangle alone, a step a refresh, as the caret's blink does.
use crate::scene::{NodeId, Transform};
use crate::widgets::Ui;
use std::cell::Cell;
use std::time::{Duration, Instant};

/// The document's one gliding caret: the focused editor's.
#[derive(Default)]
pub(crate) struct Glide {
    caret: Cell<Option<NodeId>>,
    from: Cell<(f32, f32)>,
    /// Where it is going (or, still, where it is).
    to: Cell<(f32, f32)>,
    start: Cell<Option<Instant>>,
    duration: Cell<Duration>,
    next: Cell<Option<Instant>>,
}

/// Ease-out cubic.
fn ease(t: f32) -> f32 {
    1. - (1. - t.clamp(0., 1.)).powi(3)
}

impl Glide {
    /// Where the caret shows at `now`.
    fn at(&self, now: Instant) -> (f32, f32) {
        let (to, from) = (self.to.get(), self.from.get());
        let Some(start) = self.start.get() else {
            return to;
        };
        let span = self.duration.get().as_secs_f32();
        let t = if span > 0. {
            ease(now.saturating_duration_since(start).as_secs_f32() / span)
        } else {
            1.
        };
        (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t)
    }

    /// Where `caret` shows as its editor draws it at `to`: gliding there
    /// over `duration` when an edit `moved` it, from where it showed (onto
    /// another line, from that line's start, or its `end` going up); still
    /// on its way when it is drawn again going to the same place; else
    /// there at once.
    pub(crate) fn place(
        &self,
        caret: NodeId,
        to: (f32, f32),
        duration: Option<Duration>,
        moved: bool,
        end: f32,
        now: Instant,
    ) -> (f32, f32) {
        let ours = self.caret.get() == Some(caret);
        // Another editor's caret, not gliding: the one gliding keeps on.
        if duration.is_none() && !ours {
            return to;
        }
        let shown = ours.then(|| self.at(now));
        if let (Some(duration), true, Some(shown)) =
            (duration.filter(|d| !d.is_zero()), moved, shown)
            && shown != to
        {
            let from = if (shown.1 - to.1).abs() < 0.5 {
                shown
            } else if to.1 > shown.1 {
                (0f32.min(to.0), to.1)
            } else {
                (end.max(to.0), to.1)
            };
            self.from.set(from);
            self.to.set(to);
            self.start.set(Some(now));
            self.duration.set(duration);
            self.next.set(Some(now));
            return from;
        }
        if ours && self.start.get().is_some() && self.to.get() == to && duration.is_some() {
            return self.at(now);
        }
        self.caret.set(Some(caret));
        self.to.set(to);
        self.start.set(None);
        self.next.set(None);
        to
    }
}

/// When the caret next moves, while one glides in a presented window.
pub(crate) fn next(ui: &Ui) -> Option<Instant> {
    let glide = ui.caret_glide();
    let caret = glide.caret.get()?;
    if !ui.is_presented() || !ui.scene.borrow().contains(caret) {
        return None;
    }
    glide.next.get()
}

/// Move the gliding caret a step if one is due; whether it moved.
pub(crate) fn advance(ui: &Ui, now: Instant) -> bool {
    let glide = ui.caret_glide();
    let (Some(caret), Some(start)) = (glide.caret.get(), glide.start.get()) else {
        return false;
    };
    if glide.next.get().is_none_or(|due| due > now) {
        return false;
    }
    if !ui.scene.borrow().contains(caret) {
        glide.start.set(None);
        glide.next.set(None);
        return false;
    }
    let (x, y) = glide.at(now);
    if now.saturating_duration_since(start) >= glide.duration.get() {
        glide.start.set(None);
        glide.next.set(None);
    } else {
        glide.next.set(Some(now + ui.refresh_interval()));
    }
    ui.scene
        .borrow_mut()
        .set_transform(caret, Transform { x, y });
    true
}
