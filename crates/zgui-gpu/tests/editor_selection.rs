use std::{cell::RefCell, rc::Rc};
use zgui::{
    compose::prelude::*,
    input::{InputEvent, Modifiers, PointerButton},
    text_layout::{FontStyle, TextLayout},
    widgets::Ui,
};
use zgui_gpu::text::ShapedText;

#[test]
fn triple_click_selects_full_logical_row_in_mixed_direction_text() {
    let fonts = Rc::new(RefCell::new(cosmic_text::FontSystem::new()));
    for text in ["abc אבג", "אבג abc", "abc אבג xyz דהו", "abc אבג  "] {
        let mut ui = Ui::new(500., 200.);
        let fonts = fonts.clone();
        ui.scene.borrow_mut().set_font_text_shaper(
            move |text: &str, size, width, font: &FontStyle| -> Box<dyn TextLayout> {
                Box::new(ShapedText::with_font(
                    &mut fonts.borrow_mut(),
                    text,
                    size,
                    width,
                    font,
                ))
            },
        );
        let view = ui.mount(
            text_area("Bidi document", ui.signal(format!("{text}\nnext row")))
                .size(440., 120.)
                .p(10.)
                .text_size(16.)
                .line_height(24.),
        );
        ui.prepare_frame();
        ui.input.focus(&ui.scene, Some(view.node()));
        let editor = ui.focused_editor().unwrap();
        editor.editor.borrow_mut().set_selection(0, 0);
        editor.refresh();
        for _ in 0..3 {
            ui.dispatch_with_modifiers(
                InputEvent::PointerDown {
                    x: 40.,
                    y: 22.,
                    button: PointerButton::Primary,
                },
                Modifiers::default(),
            );
            ui.dispatch_with_modifiers(
                InputEvent::PointerUp {
                    x: 40.,
                    y: 22.,
                    button: PointerButton::Primary,
                },
                Modifiers::default(),
            );
        }
        assert_eq!(
            editor.copy(),
            text,
            "complete bidi row including trailing spaces"
        );
        assert_eq!(editor.editor.borrow().selection().anchor, 0);
        assert_eq!(editor.editor.borrow().selection().focus, text.len());
    }
}
