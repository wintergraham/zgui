//! Unicode grapheme-aware text editing with bounded undo history and IME preedit.
//! All offsets are UTF-8 bytes, clamped to extended grapheme boundaries.
use std::{collections::VecDeque, ops::Range};
use unicode_segmentation::UnicodeSegmentation;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub focus: usize,
}
impl Selection {
    pub fn range(self) -> Range<usize> {
        self.anchor.min(self.focus)..self.anchor.max(self.focus)
    }
    pub fn is_empty(self) -> bool {
        self.anchor == self.focus
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preedit {
    pub text: String,
    pub cursor: Option<(usize, usize)>,
}
#[derive(Clone)]
struct Edit {
    start: usize,
    deleted: String,
    inserted: String,
    before: Selection,
    after: Selection,
}
pub struct TextEditor {
    text: String,
    selection: Selection,
    preedit: Option<Preedit>,
    composition_cancel_revision: u64,
    undo: VecDeque<Edit>,
    redo: VecDeque<Edit>,
    undo_bytes: usize,
    redo_bytes: usize,
    history_limit: usize,
    history_bytes: usize,
}
impl Default for TextEditor {
    fn default() -> Self {
        Self::new("")
    }
}
impl TextEditor {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let end = text.len();
        Self {
            text,
            selection: Selection {
                anchor: end,
                focus: end,
            },
            preedit: None,
            composition_cancel_revision: 0,
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            undo_bytes: 0,
            redo_bytes: 0,
            history_limit: 100,
            history_bytes: 4 * 1024 * 1024,
        }
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn selection(&self) -> Selection {
        self.selection
    }
    pub fn selected_text(&self) -> &str {
        &self.text[self.selection.range()]
    }
    pub fn preedit(&self) -> Option<&Preedit> {
        self.preedit.as_ref()
    }
    /// Changes only when a local editor operation cancels nonempty preedit.
    /// This is not a model revision or native session epoch; it wraps at u64::MAX.
    /// Native empty-preedit updates and commits do not increment this revision.
    /// Hosts can use it to reset the native input context after external edits.
    pub fn composition_cancel_revision(&self) -> u64 {
        self.composition_cancel_revision
    }
    /// Bound each undo/redo stack independently by entry count and retained
    /// inserted/deleted UTF-8 payload bytes. The byte budget excludes edit and
    /// allocator metadata. Lowering either limit also releases spare stack capacity.
    pub fn set_history_limits(&mut self, entries: usize, bytes: usize) {
        let lowered = entries < self.history_limit || bytes < self.history_bytes;
        self.history_limit = entries;
        self.history_bytes = bytes;
        Self::trim(&mut self.undo, &mut self.undo_bytes, entries, bytes);
        Self::trim(&mut self.redo, &mut self.redo_bytes, entries, bytes);
        if lowered {
            self.undo.shrink_to_fit();
            self.redo.shrink_to_fit();
        }
    }
    fn trim(history: &mut VecDeque<Edit>, payload: &mut usize, entries: usize, bytes: usize) {
        while history.len() > entries || *payload > bytes {
            if let Some(edit) = history.pop_front() {
                *payload -= edit.deleted.len() + edit.inserted.len();
            } else {
                break;
            }
        }
    }
    fn boundary(&self, offset: usize) -> usize {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        let mut cursor = unicode_segmentation::GraphemeCursor::new(offset, self.text.len(), true);
        if cursor.is_boundary(&self.text, 0).unwrap_or(false) {
            offset
        } else {
            cursor
                .prev_boundary(&self.text, 0)
                .ok()
                .flatten()
                .unwrap_or(0)
        }
    }
    pub fn set_selection(&mut self, anchor: usize, focus: usize) {
        self.cancel_preedit();
        self.selection = Selection {
            anchor: self.boundary(anchor),
            focus: self.boundary(focus),
        };
    }
    pub fn select_all(&mut self) {
        self.set_selection(0, self.text.len());
    }
    pub fn insert(&mut self, value: &str) -> bool {
        let range = self.selection.range();
        if self.text[range.clone()] == *value {
            self.cancel_preedit();
            self.set_selection(range.start + value.len(), range.start + value.len());
            return false;
        }
        let before = self.selection;
        let deleted = self.text[range.clone()].to_owned();
        self.cancel_preedit();
        self.redo.clear();
        self.redo_bytes = 0;
        self.text.replace_range(range.clone(), value);
        // Insertion can join an adjacent combining mark/ZWJ sequence. Keep the caret valid.
        let desired = range.start + value.len();
        let end = if desired == self.text.len() {
            desired
        } else {
            let mut cursor =
                unicode_segmentation::GraphemeCursor::new(desired, self.text.len(), true);
            if cursor.is_boundary(&self.text, 0).unwrap_or(false) {
                desired
            } else {
                cursor
                    .next_boundary(&self.text, 0)
                    .ok()
                    .flatten()
                    .unwrap_or(self.text.len())
            }
        };
        self.selection = Selection {
            anchor: end,
            focus: end,
        };
        self.undo_bytes += deleted.len() + value.len();
        self.undo.push_back(Edit {
            start: range.start,
            deleted,
            inserted: value.to_owned(),
            before,
            after: self.selection,
        });
        Self::trim(
            &mut self.undo,
            &mut self.undo_bytes,
            self.history_limit,
            self.history_bytes,
        );
        true
    }
    pub fn set_text(&mut self, value: impl Into<String>) -> bool {
        let value = value.into();
        if self.text == value {
            return false;
        }
        let before = self.selection;
        self.select_all();
        let changed = self.insert(&value);
        if let Some(edit) = self.undo.back_mut() {
            edit.before = before;
        }
        changed
    }
    fn previous(&self, word: bool) -> usize {
        let pos = self.selection.focus;
        if word {
            self.text[..pos]
                .unicode_word_indices()
                .map(|(i, _)| i)
                .next_back()
                .unwrap_or(0)
        } else {
            self.text[..pos]
                .grapheme_indices(true)
                .map(|(i, _)| i)
                .next_back()
                .unwrap_or(0)
        }
    }
    fn next(&self, word: bool) -> usize {
        let pos = self.selection.focus;
        if word {
            self.text[pos..]
                .unicode_word_indices()
                .next()
                .map(|(i, w)| pos + i + w.len())
                .unwrap_or(self.text.len())
        } else {
            self.text[pos..]
                .graphemes(true)
                .next()
                .map(|g| pos + g.len())
                .unwrap_or(pos)
        }
    }
    fn move_to(&mut self, offset: usize, extend: bool) {
        self.cancel_preedit();
        self.selection.focus = self.boundary(offset);
        if !extend {
            self.selection.anchor = self.selection.focus;
        }
    }
    pub fn move_left(&mut self, word: bool, extend: bool) {
        let pos = if !extend && !self.selection.is_empty() {
            self.selection.range().start
        } else {
            self.previous(word)
        };
        self.move_to(pos, extend);
    }
    pub fn move_right(&mut self, word: bool, extend: bool) {
        let pos = if !extend && !self.selection.is_empty() {
            self.selection.range().end
        } else {
            self.next(word)
        };
        self.move_to(pos, extend);
    }
    pub fn move_home(&mut self, extend: bool) {
        let pos = self.text[..self.selection.focus]
            .rfind('\n')
            .map_or(0, |i| i + 1);
        self.move_to(pos, extend);
    }
    pub fn move_end(&mut self, extend: bool) {
        let focus = self.selection.focus;
        let pos = self.text[focus..]
            .find('\n')
            .map_or(self.text.len(), |i| focus + i);
        self.move_to(pos, extend);
    }
    pub fn move_document_start(&mut self, extend: bool) {
        self.move_to(0, extend);
    }
    pub fn move_document_end(&mut self, extend: bool) {
        self.move_to(self.text.len(), extend);
    }
    pub fn backspace(&mut self) -> bool {
        self.delete_backward(false)
    }
    pub fn delete_backward(&mut self, word: bool) -> bool {
        if self.selection.is_empty() {
            let pos = self.previous(word);
            if pos == self.selection.focus {
                return false;
            }
            let old = self.selection;
            self.selection.anchor = pos;
            let changed = self.insert("");
            if let Some(s) = self.undo.back_mut() {
                s.before = old;
            }
            changed
        } else {
            self.insert("")
        }
    }
    pub fn delete_forward(&mut self) -> bool {
        self.delete_next(false)
    }
    pub fn delete_next(&mut self, word: bool) -> bool {
        if self.selection.is_empty() {
            let pos = self.next(word);
            if pos == self.selection.focus {
                return false;
            }
            let old = self.selection;
            self.selection.anchor = pos;
            let changed = self.insert("");
            if let Some(s) = self.undo.back_mut() {
                s.before = old;
            }
            changed
        } else {
            self.insert("")
        }
    }
    /// Return selected text and remove it in one undoable operation. The host owns the OS clipboard.
    pub fn cut(&mut self) -> String {
        let value = self.selected_text().to_owned();
        self.insert("");
        value
    }
    pub fn undo(&mut self) -> bool {
        let Some(edit) = self.undo.pop_back() else {
            return false;
        };
        self.text
            .replace_range(edit.start..edit.start + edit.inserted.len(), &edit.deleted);
        self.selection = edit.before;
        self.cancel_preedit();
        let bytes = edit.deleted.len() + edit.inserted.len();
        self.undo_bytes -= bytes;
        self.redo_bytes += bytes;
        self.redo.push_back(edit);
        Self::trim(
            &mut self.redo,
            &mut self.redo_bytes,
            self.history_limit,
            self.history_bytes,
        );
        true
    }
    pub fn redo(&mut self) -> bool {
        let Some(edit) = self.redo.pop_back() else {
            return false;
        };
        self.text
            .replace_range(edit.start..edit.start + edit.deleted.len(), &edit.inserted);
        self.selection = edit.after;
        self.cancel_preedit();
        let bytes = edit.deleted.len() + edit.inserted.len();
        self.redo_bytes -= bytes;
        self.undo_bytes += bytes;
        self.undo.push_back(edit);
        Self::trim(
            &mut self.undo,
            &mut self.undo_bytes,
            self.history_limit,
            self.history_bytes,
        );
        true
    }
    /// Preedit is ephemeral: it never changes committed text or enters undo history.
    pub fn set_preedit(&mut self, text: impl Into<String>, cursor: Option<(usize, usize)>) {
        let text = text.into();
        if text.is_empty() {
            self.preedit = None;
            return;
        }

        let clamp = |n: usize| {
            let mut n = n.min(text.len());
            while !text.is_char_boundary(n) {
                n -= 1;
            }
            let mut cursor = unicode_segmentation::GraphemeCursor::new(n, text.len(), true);
            if cursor.is_boundary(&text, 0).unwrap_or(false) {
                n
            } else {
                cursor.prev_boundary(&text, 0).ok().flatten().unwrap_or(0)
            }
        };
        let cursor = cursor.map(|(a, b)| (clamp(a), clamp(b)));
        self.preedit = Some(Preedit { text, cursor });
    }
    pub fn commit_preedit(&mut self, text: &str) -> bool {
        self.preedit = None;
        self.insert(text)
    }
    pub fn cancel_preedit(&mut self) {
        if self.preedit.take().is_some() {
            self.composition_cancel_revision = self.composition_cancel_revision.wrapping_add(1);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_document_keeps_only_edit_deltas() {
        let mut editor = TextEditor::new("x".repeat(1_000_000));
        editor.set_history_limits(10, 8);
        editor.insert("a");
        editor.insert("b");
        assert_eq!(
            editor
                .undo
                .iter()
                .map(|e| e.inserted.len() + e.deleted.len())
                .sum::<usize>(),
            2
        );
        assert!(editor.undo());
        assert!(editor.undo());
        assert_eq!(editor.text().len(), 1_000_000);
    }
    #[test]
    fn graphemes_are_atomic() {
        let mut e = TextEditor::new("a👩‍💻e\u{301}");
        assert!(e.backspace());
        assert_eq!(e.text(), "a👩‍💻");
        e.move_left(false, false);
        assert_eq!(e.selection().focus, 1);
        e.delete_forward();
        assert_eq!(e.text(), "a");
        e.undo();
        assert_eq!(e.text(), "a👩‍💻");
        assert_eq!(e.selection().focus, 1);
    }
    #[test]
    fn replacement_undo_and_redo() {
        let mut e = TextEditor::new("hello world");
        e.set_selection(6, 11);
        e.insert("🌎");
        assert_eq!(e.text(), "hello 🌎");
        e.undo();
        assert_eq!(e.selected_text(), "world");
        e.redo();
        assert_eq!(e.text(), "hello 🌎");
        e.insert("!");
        assert!(!e.redo());
    }
    #[test]
    fn ime_does_not_mutate_until_commit() {
        let mut e = TextEditor::new("a");
        e.set_preedit("に", Some((1, 99)));
        assert_eq!(e.text(), "a");
        assert_eq!(e.preedit().unwrap().cursor, Some((0, 3)));
        assert!(!e.undo());
        e.commit_preedit("日本");
        assert_eq!(e.text(), "a日本");
        e.undo();
        assert_eq!(e.text(), "a");
    }
    #[test]
    fn lowered_history_limits_remain_bounded_when_transferring_between_stacks() {
        let mut editor = TextEditor::new("");
        for text in ["a", "b", "c", "d", "e", "f"] {
            editor.insert(text);
        }
        for _ in 0..3 {
            assert!(editor.undo());
        }
        editor.set_history_limits(2, 2);
        let bounded = |editor: &TextEditor| {
            for history in [&editor.undo, &editor.redo] {
                assert!(history.len() <= 2, "history exceeded entry bound");
                assert!(
                    history
                        .iter()
                        .map(|edit| edit.inserted.len() + edit.deleted.len())
                        .sum::<usize>()
                        <= 2,
                    "history exceeded byte bound"
                );
            }
        };
        bounded(&editor);
        assert!(editor.undo());
        assert_eq!(editor.text(), "ab");
        bounded(&editor);
        assert!(editor.redo());
        assert_eq!(editor.text(), "abc");
        bounded(&editor);
        assert!(editor.redo());
        assert_eq!(editor.text(), "abcd");
        bounded(&editor);
        assert!(!editor.redo());
        editor.set_history_limits(0, 0);
        assert!(!editor.undo());
        assert!(!editor.redo());
    }

    #[test]
    fn payload_accounting_tracks_eviction_transfers_and_new_edit_redo_clear() {
        let mut editor = TextEditor::new("old");
        let check = |editor: &TextEditor| {
            assert_eq!(
                editor.undo_bytes,
                editor
                    .undo
                    .iter()
                    .map(|e| e.deleted.len() + e.inserted.len())
                    .sum::<usize>()
            );
            assert_eq!(
                editor.redo_bytes,
                editor
                    .redo
                    .iter()
                    .map(|e| e.deleted.len() + e.inserted.len())
                    .sum::<usize>()
            );
        };
        editor.set_history_limits(1000, 10000);
        for _ in 0..200 {
            editor.insert("é");
            check(&editor);
        }
        for _ in 0..100 {
            editor.undo();
            check(&editor);
        }
        let capacities = (editor.undo.capacity(), editor.redo.capacity());
        editor.set_history_limits(3, 5);
        check(&editor);
        assert!(editor.undo.capacity() < capacities.0);
        assert!(editor.redo.capacity() < capacities.1);
        editor.undo();
        check(&editor);
        editor.redo();
        check(&editor);
        editor.insert("!");
        check(&editor);
        assert!(editor.redo.is_empty());
        editor.select_all();
        editor.insert("oversize");
        check(&editor);
        assert_eq!(editor.undo_bytes, 0);
        assert!(!editor.undo());
        editor.set_history_limits(0, 0);
        assert_eq!(editor.undo.capacity(), 0);
        assert_eq!(editor.redo.capacity(), 0);
    }

    #[test]
    fn external_composition_cancellation_has_one_revision_per_existing_preedit() {
        let mut editor = TextEditor::new("abc");
        let compose = |editor: &mut TextEditor| editor.set_preedit("中", Some((3, 3)));
        compose(&mut editor);
        editor.set_selection(0, 0);
        assert_eq!(editor.composition_cancel_revision(), 1);
        editor.set_selection(0, 0);
        editor.cancel_preedit();
        assert_eq!(editor.composition_cancel_revision(), 1);
        compose(&mut editor);
        editor.insert(""); // Equal replacement still cancels composition.
        assert_eq!(editor.composition_cancel_revision(), 2);
        compose(&mut editor);
        editor.insert("!");
        assert_eq!(editor.composition_cancel_revision(), 3);
        compose(&mut editor);
        editor.set_text("changed");
        assert_eq!(editor.composition_cancel_revision(), 4);
        compose(&mut editor);
        assert!(editor.undo());
        assert_eq!(editor.composition_cancel_revision(), 5);
        compose(&mut editor);
        assert!(editor.redo());
        assert_eq!(editor.composition_cancel_revision(), 6);
        compose(&mut editor);
        editor.move_left(false, false);
        assert_eq!(editor.composition_cancel_revision(), 7);
        compose(&mut editor);
        editor.cancel_preedit();
        assert_eq!(editor.composition_cancel_revision(), 8);
        editor.composition_cancel_revision = u64::MAX;
        compose(&mut editor);
        editor.cancel_preedit();
        assert_eq!(editor.composition_cancel_revision(), 0);
    }

    #[test]
    fn native_preedit_clear_commit_and_noop_model_update_do_not_signal_external_cancel() {
        let mut editor = TextEditor::new("abc");
        editor.set_preedit("中", None);
        editor.set_preedit("文", Some((3, 3)));
        assert!(!editor.set_text("abc"));
        assert!(editor.preedit().is_some());
        assert!(!editor.undo());
        assert!(!editor.redo());
        assert!(editor.preedit().is_some());
        assert_eq!(editor.composition_cancel_revision(), 0);
        editor.set_preedit("", None);
        assert_eq!(editor.composition_cancel_revision(), 0);
        assert!(editor.preedit().is_none());
        editor.set_preedit("中", None);
        editor.commit_preedit("中");
        assert_eq!(editor.text(), "abc中");
        assert_eq!(editor.composition_cancel_revision(), 0);
        assert!(editor.preedit().is_none());
        editor.set_preedit("ignored", None);
        editor.commit_preedit("");
        assert_eq!(editor.composition_cancel_revision(), 0);
    }

    #[test]
    fn history_is_bounded() {
        let mut e = TextEditor::new("");
        e.set_history_limits(2, 100);
        e.insert("a");
        e.insert("b");
        e.insert("c");
        assert!(e.undo());
        assert!(e.undo());
        assert!(!e.undo());
        assert_eq!(e.text(), "a");
    }
    #[test]
    fn byte_offsets_clamp_and_combining_insert_stays_valid() {
        let mut e = TextEditor::new("é");
        e.set_selection(1, 1);
        assert_eq!(e.selection().focus, 0);
        e.move_right(false, false);
        e.insert("\u{301}");
        assert_eq!(e.selection().focus, e.text().len());
        e.move_left(false, false);
        assert_eq!(e.selection().focus, 0);
    }
}
