use zgx::{
    compose::prelude::*,
    input::{InputEvent, PointerButton},
    widgets::Ui,
};
#[test]
fn cursor_cascade_changes_without_layout_or_paint_and_disabled_regions_keep_explicit_style() {
    let mut ui = Ui::new(150., 100.);
    let busy = ui.signal(false);
    let view = ui.mount(
        row()
            .cursor(Cursor::Move)
            .child(
                div()
                    .id("child")
                    .size(50., 50.)
                    .disabled(true)
                    .reactive_style({
                        let busy = busy.clone();
                        move || {
                            Styles::new().cursor(if busy.get() {
                                Cursor::NotAllowed
                            } else {
                                Cursor::Grab
                            })
                        }
                    }),
            )
            .child(div().size(50., 50.)),
    );
    ui.prepare_frame();
    ui.scene.borrow_mut().flush();
    let node = view.find("child").unwrap();
    assert_eq!(ui.scene.borrow().cursor_for(node), Some(Cursor::Grab));
    assert_eq!(ui.scene.borrow().cursor_at(20., 20.), Some(Cursor::Grab));
    assert_eq!(ui.scene.borrow().cursor_at(70., 20.), Some(Cursor::Move));
    busy.set(true);
    ui.prepare_frame();
    let report = ui.scene.borrow_mut().flush();
    assert_eq!(report.layout_nodes, 0);
    assert!(report.damage.is_empty());
    assert_eq!(
        ui.scene.borrow().cursor_at(20., 20.),
        Some(Cursor::NotAllowed)
    );
    view.unmount();
    ui.prepare_frame();
    assert_eq!(ui.scene.borrow().cursor_at(20., 20.), None);
}

#[test]
fn a_press_keeps_the_hover_cursor_dragged_off_let_go_and_through_re_renders() {
    let mut ui = Ui::new(300., 200.);
    let tint = ui.signal(false);
    let view = ui.mount(
        column()
            .size(300., 200.)
            .child(
                button()
                    .id("button")
                    .size(100., 40.)
                    .cursor(Cursor::Grab)
                    .active(|s| s.bg(rgb(0xaabbcc)))
                    .reactive_style({
                        let tint = tint.clone();
                        move || Styles::new().opacity(if tint.get() { 0.5 } else { 1. })
                    })
                    .on_click(|| {}),
            )
            // A full-window layer the pointer passes through, as popovers sit in.
            .child(
                div()
                    .absolute()
                    .left(0.)
                    .top(0.)
                    .size(300., 200.)
                    .pointer_events(false),
            ),
    );
    ui.prepare_frame();
    ui.scene.borrow_mut().flush();
    let (inside, outside) = ((50., 20.), (200., 150.));
    let primary = PointerButton::Primary;
    let step = |ui: &mut Ui, event: InputEvent, (x, y): (f32, f32)| {
        ui.dispatch(event);
        ui.prepare_frame();
        ui.scene.borrow_mut().flush();
        ui.input.cursor_at(&ui.scene.borrow(), x, y)
    };
    let (x, y) = inside;
    let hover = step(&mut ui, InputEvent::PointerMove { x, y }, inside);
    assert_eq!(hover, Some(Cursor::Grab), "hovered through the layer");
    let down = InputEvent::PointerDown {
        x,
        y,
        button: primary,
    };
    assert_eq!(step(&mut ui, down, inside), hover, "pressed");
    let (ox, oy) = outside;
    let out = InputEvent::PointerMove { x: ox, y: oy };
    assert_eq!(step(&mut ui, out, outside), hover, "dragged off it");
    tint.set(true);
    ui.prepare_frame();
    ui.scene.borrow_mut().flush();
    assert_eq!(
        ui.input.cursor_at(&ui.scene.borrow(), ox, oy),
        hover,
        "re-styled while held"
    );
    let back = InputEvent::PointerMove { x, y };
    assert_eq!(step(&mut ui, back, inside), hover, "back on it");
    let up = InputEvent::PointerUp {
        x,
        y,
        button: primary,
    };
    assert_eq!(step(&mut ui, up, inside), hover, "let go");
    tint.set(false);
    ui.prepare_frame();
    ui.scene.borrow_mut().flush();
    assert_eq!(
        ui.input.cursor_at(&ui.scene.borrow(), x, y),
        hover,
        "re-styled"
    );

    // Let go off it: the cursor is what is under the pointer again.
    let down = InputEvent::PointerDown {
        x,
        y,
        button: primary,
    };
    step(&mut ui, down, inside);
    step(&mut ui, InputEvent::PointerMove { x: ox, y: oy }, outside);
    let up = InputEvent::PointerUp {
        x: ox,
        y: oy,
        button: primary,
    };
    assert_eq!(step(&mut ui, up, outside), None);
    view.unmount();
}
