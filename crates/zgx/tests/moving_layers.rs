use zgx::scene::{Color, Effects, Layout, NodeId, NodeKind, Rect, Scene, Style, Transform};

fn fixed(width: f32, height: f32) -> Style {
    Style {
        width: Some(width),
        height: Some(height),
        ..Style::default()
    }
}

fn t(x: f32, y: f32) -> Transform {
    Transform { x, y }
}

/// A 100×100 box under the root, holding a 20×20 box at +10,+10.
fn moving_scene() -> (Scene, NodeId, NodeId) {
    let mut scene = Scene::new(400., 300.);
    scene.set_kind(scene.root(), NodeKind::Container(Layout::Overlay));
    let container = scene.append(
        scene.root(),
        NodeKind::Rect(Color(10, 20, 30, 255)),
        fixed(100., 100.),
    );
    let child = scene.append(
        container,
        NodeKind::Rect(Color(200, 10, 10, 255)),
        fixed(20., 20.),
    );
    scene.set_transform(child, t(10., 10.));
    scene.flush();
    (scene, container, child)
}

/// Where a layer's pixels are is its ink — the union of its subtree's
/// paint — so moving one damages its ink where it was and lands, one area
/// where a flat node's subtree damages each descendant's in turn.
#[test]
fn a_moved_layer_damages_where_it_was_and_lands() {
    let (mut scene, container, _child) = moving_scene();
    scene.set_isolated(container, true);
    scene.flush();
    assert!(scene.is_layer(container));
    scene.set_transform(container, t(20., 0.));
    let frame = scene.flush();
    assert_eq!(frame.damage, vec![Rect::new(0., 0., 120., 100.)]);
}

/// Its own move composites the layer's texture elsewhere: the texture
/// stands (`layer_revision`), while a write inside it changes it.
#[test]
fn a_moved_layer_keeps_its_revision() {
    let (mut scene, container, child) = moving_scene();
    scene.set_isolated(container, true);
    scene.flush();
    let revision = scene.layer_revision(container);
    scene.set_transform(container, t(20., 0.));
    scene.flush();
    assert_eq!(scene.layer_revision(container), revision);
    scene.set_effects(
        child,
        Effects {
            opacity: 0.5,
            ..Effects::default()
        },
    );
    scene.flush();
    assert_ne!(scene.layer_revision(container), revision);
}

/// Effects written on a layer damage its ink under the old effects, then
/// the new: a content blur appearing spreads its area.
#[test]
fn a_blur_coming_to_a_layer_damages_its_spread() {
    let (mut scene, container, _child) = moving_scene();
    scene.set_isolated(container, true);
    scene.flush();
    scene.set_effects(
        container,
        Effects {
            filter_blur: 4.,
            ..Effects::default()
        },
    );
    let frame = scene.flush();
    // `filter_spread` past its bounds, the negative side clamped by the
    // viewport: (0, 0, 100 + spread, 100 + spread).
    let spread = zgx::scene::filter_spread(4.);
    assert_eq!(
        frame.damage,
        vec![Rect::new(0., 0., 100. + spread, 100. + spread)]
    );
}
