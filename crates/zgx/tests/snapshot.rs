//! A node's picture as the window last showed it: the host takes it
//! (`Ui::on_snapshot`), given the node's box; without one there is none.
use std::{cell::RefCell, rc::Rc, sync::Arc};
use zgx::{compose::prelude::*, image::ImageData, scene::Rect, widgets::Ui};

#[test]
fn a_snapshot_is_the_hosts_picture_of_the_nodes_box() {
    let mut ui = Ui::new(400., 300.);
    let view = ui.mount(
        column()
            .p(10.)
            .child(div().id("card").w(120.).h(60.).bg(rgb(0x334455)))
            .child(div().id("away").w(120.).h(60.).hidden()),
    );
    ui.prepare_frame();
    let card = view.find("card").unwrap();
    // No host taking pictures: none.
    assert!(ui.snapshot(card, 100).is_none());
    let asked: Rc<RefCell<Vec<(Rect, u32)>>> = Rc::default();
    let log = asked.clone();
    ui.on_snapshot(move |rect, max_width| {
        log.borrow_mut().push((rect, max_width));
        Some(Arc::new(ImageData::new(2, 1, vec![255u8; 8]).unwrap()))
    });
    let picture = ui.snapshot(card, 100).expect("a picture");
    assert_eq!((picture.width(), picture.height()), (2, 1));
    assert_eq!(
        *asked.borrow(),
        vec![(Rect::new(10., 10., 120., 60.), 100)],
        "the node's box, in the window"
    );
    // Hidden (a tab not shown): its box is not what shows, so none.
    let away = view.find("away").unwrap();
    assert!(ui.snapshot(away, 100).is_none());
    assert_eq!(asked.borrow().len(), 1, "the host is not asked");
    // Gone from the scene: none.
    view.unmount();
    ui.prepare_frame();
    assert!(ui.snapshot(card, 100).is_none());
}
