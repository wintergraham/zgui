//! An app's own steps in an editor's undo history (an attachment added
//! beside a message): undone and redone in order with the typing around
//! them, cleared by a new edit, trimmed by the history's limits, and told
//! to the app by the editor's undo keys.
use std::{cell::RefCell, rc::Rc};
use zgx::{
    compose::prelude::*,
    input::{InputEvent, Key, Modifiers},
    text_edit::{HistoryStep, TextEditor},
    widgets::{HistoryDirection, Ui},
};

fn typed(editor: &mut TextEditor, text: &str) {
    for c in text.chars() {
        editor.insert(&c.to_string());
    }
}

#[test]
fn app_steps_interleave_with_typing_in_undo_and_redo() {
    let mut editor = TextEditor::new("");
    typed(&mut editor, "ab");
    editor.push_app_step(7);
    typed(&mut editor, "c");
    assert_eq!(editor.undo_step(), Some(HistoryStep::Text));
    assert_eq!(
        editor.text(),
        "ab",
        "typing after the step is a run of its own"
    );
    assert_eq!(editor.undo_step(), Some(HistoryStep::App(7)));
    assert_eq!(editor.text(), "ab", "an app's step changes no text");
    assert_eq!(editor.undo_step(), Some(HistoryStep::Text));
    assert_eq!(editor.text(), "");
    assert_eq!(editor.undo_step(), None);
    assert_eq!(editor.redo_step(), Some(HistoryStep::Text));
    assert_eq!(editor.text(), "ab");
    assert_eq!(editor.redo_step(), Some(HistoryStep::App(7)));
    assert_eq!(editor.text(), "ab");
    assert_eq!(editor.redo_step(), Some(HistoryStep::Text));
    assert_eq!(editor.text(), "abc");
    assert_eq!(editor.redo_step(), None);
}

#[test]
fn a_new_edit_clears_app_steps_that_could_be_redone() {
    let mut editor = TextEditor::new("");
    typed(&mut editor, "a");
    editor.push_app_step(1);
    assert_eq!(editor.undo_step(), Some(HistoryStep::App(1)));
    typed(&mut editor, "b");
    assert_eq!(editor.redo_step(), None);
    // And a step of the app's own clears what could be redone, as an edit.
    assert_eq!(editor.undo_step(), Some(HistoryStep::Text));
    editor.push_app_step(2);
    assert_eq!(editor.redo_step(), None);
    assert_eq!(editor.undo_step(), Some(HistoryStep::App(2)));
}

#[test]
fn the_history_limit_trims_app_steps() {
    let mut editor = TextEditor::new("");
    editor.set_history_limits(2, 1024);
    editor.push_app_step(1);
    typed(&mut editor, "x");
    editor.push_app_step(3);
    assert_eq!(editor.undo_step(), Some(HistoryStep::App(3)));
    assert_eq!(editor.undo_step(), Some(HistoryStep::Text));
    assert_eq!(
        editor.undo_step(),
        None,
        "the oldest, the app's first, went"
    );
    // Lowering the limit trims what can be redone too.
    editor.set_history_limits(1, 1024);
    assert_eq!(editor.redo_step(), Some(HistoryStep::Text));
    assert_eq!(editor.redo_step(), None);
}

fn primary(shift: bool) -> Modifiers {
    Modifiers {
        meta: cfg!(target_os = "macos"),
        control: !cfg!(target_os = "macos"),
        shift,
        ..Modifiers::default()
    }
}

/// ⌘Z with nothing but an app's step to undo is the app's, not swallowed:
/// the field tells it, and the text stays as it is.
#[test]
fn undo_keys_tell_the_app_of_its_steps() {
    let mut ui = Ui::new(400., 200.);
    let value = ui.signal(String::new());
    let told = Rc::new(RefCell::new(Vec::new()));
    let log = told.clone();
    let view = ui.mount(
        text_area("Message", value.clone())
            .size(300., 120.)
            .on_history(move |token, how| log.borrow_mut().push((token, how))),
    );
    ui.prepare_frame();
    ui.input.focus(&ui.scene, Some(view.node()));
    let editor = ui.focused_editor().unwrap();
    editor.push_history(4);
    ui.dispatch(InputEvent::Text("a".into()));
    let z = |ui: &mut Ui, shift| {
        ui.dispatch(InputEvent::KeyDown {
            key: Key::Character("z".into()),
            modifiers: primary(shift),
            repeat: false,
        })
        .default_prevented
    };
    assert!(z(&mut ui, false));
    assert_eq!(value.get(), "");
    assert!(told.borrow().is_empty(), "the typing went first");
    assert!(z(&mut ui, false), "the app's step is handled");
    assert_eq!(*told.borrow(), [(4, HistoryDirection::Undo)]);
    assert_eq!(value.get(), "");
    assert!(z(&mut ui, true));
    assert_eq!(
        *told.borrow(),
        [(4, HistoryDirection::Undo), (4, HistoryDirection::Redo)]
    );
    assert!(z(&mut ui, true));
    assert_eq!(value.get(), "a");
    assert_eq!(told.borrow().len(), 2);
}
