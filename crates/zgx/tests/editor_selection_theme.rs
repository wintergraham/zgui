//! An editor's selection takes the theme's selection colour as it changes
//! (a theme switch, the system's accent), not only the one it was drawn in.
use zgx::{
    compose::prelude::*,
    scene::{Color, NodeId, NodeKind},
    theme::tok,
    widgets::Ui,
};

/// The colours of the rects drawn inside `node`.
fn rects(ui: &Ui, node: NodeId) -> Vec<Color> {
    let scene = ui.scene.borrow();
    let mut stack = vec![node];
    let mut colors = Vec::new();
    while let Some(id) = stack.pop() {
        if let NodeKind::Rect(c) = scene.kind(id) {
            colors.push(*c);
        }
        stack.extend(scene.children(id).iter().copied());
    }
    colors
}

/// The corner radii of the marks drawn inside `node` in `color`: 0 for a
/// square rect.
fn radii(ui: &Ui, node: NodeId, color: Color) -> Vec<f32> {
    let scene = ui.scene.borrow();
    let mut stack = vec![node];
    let mut radii = Vec::new();
    while let Some(id) = stack.pop() {
        match scene.kind(id) {
            NodeKind::Rect(c) if *c == color => radii.push(0.),
            NodeKind::Quad(q) if q.fill == color => radii.push(q.radius),
            _ => {}
        }
        stack.extend(scene.children(id).iter().copied());
    }
    radii
}

#[test]
fn a_selection_rounds_each_line_by_the_theme_radius() {
    let mut ui = Ui::new(400., 200.);
    let value = ui.signal("first\nsecond".to_string());
    let view = ui.mount(column().child(text_area("Notes", value).id("notes").w(300.).h(80.)));
    let field = view.find("notes").unwrap();
    ui.prepare_frame();
    let editor = ui.editor(field).unwrap();
    editor.set_selection(0, 5);
    ui.input.focus(&ui.scene, Some(field));
    ui.prepare_frame();
    let color = ui.theme().root().color_untracked(tok::SELECTION);
    assert_eq!(radii(&ui, field, color), [0.], "square by default");
    // A showing selection takes a new radius, and lines drawn after it too,
    // each rounded apart.
    ui.theme()
        .set_system_tokens([(tok::SELECTION_RADIUS.0, 3f32.into())]);
    ui.prepare_frame();
    assert_eq!(radii(&ui, field, color), [3.]);
    editor.set_selection(0, 12);
    ui.prepare_frame();
    assert_eq!(radii(&ui, field, color), [3., 3.]);
}

#[test]
fn a_showing_selection_takes_a_new_selection_colour() {
    let mut ui = Ui::new(400., 200.);
    let value = ui.signal("select me".to_string());
    let view = ui.mount(column().child(text_input("Field", value).id("field").w(300.).h(30.)));
    let field = view.find("field").unwrap();
    ui.prepare_frame();
    let editor = ui.editor(field).unwrap();
    editor.set_selection(0, 9);
    ui.input.focus(&ui.scene, Some(field));
    ui.prepare_frame();
    let before = ui.theme().root().color_untracked(tok::SELECTION);
    assert!(
        rects(&ui, field).contains(&before),
        "the selection is drawn"
    );
    let accent = Color(255, 0, 0, 0x66);
    ui.theme()
        .set_system_tokens([(tok::SELECTION.0, accent.into())]);
    ui.prepare_frame();
    let colors = rects(&ui, field);
    assert!(
        colors.contains(&accent),
        "the selection took the new colour: {colors:?}"
    );
    assert!(
        !colors.contains(&before),
        "and kept none of the old: {colors:?}"
    );
}
