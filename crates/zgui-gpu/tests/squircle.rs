//! Squircle corners (`QuadStyle::corner_smoothing`): fills, borders and
//! rounded clips follow `zgui::decoration::corner_distance`, on the wgpu
//! path and with Metal directly, and the app's default smooths every box
//! that sets none of its own.
use zgui::decoration::{corner_distance, set_default_corner_smoothing};
use zgui::scene::*;
use zgui_gpu::GpuRenderer;

fn gpu_fixture() -> std::sync::MutexGuard<'static, ()> {
    static DEVICE_TEST: std::sync::Mutex<()> = std::sync::Mutex::new(());
    DEVICE_TEST
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
const SIZE: u32 = 200;
const RED: Color = Color(255, 0, 0, 255);
/// The box every test draws: 100 px at (50, 50), corners of 30.
const BOX: (f32, f32, f32) = (50., 50., 100.);
const RADIUS: f32 = 30.;
fn alpha(pixels: &[u8], x: u32, y: u32) -> f32 {
    pixels[((y * SIZE + x) * 4 + 3) as usize] as f32 / 255.
}
fn fixed(width: f32, height: f32) -> Style {
    Style {
        width: Some(width),
        height: Some(height),
        ..Default::default()
    }
}
fn scene() -> Scene {
    let mut scene = Scene::new(SIZE as f32, SIZE as f32);
    scene.set_kind(scene.root(), NodeKind::Container(Layout::Overlay));
    scene
}
fn render(gpu: &mut GpuRenderer, scene: &mut Scene) -> Vec<u8> {
    let damage = scene.flush().damage;
    gpu.render(scene, &damage).unwrap();
    gpu.readback().unwrap()
}
fn renderers() -> impl Iterator<Item = (bool, GpuRenderer)> {
    [true, false].into_iter().map(|direct| {
        let mut gpu = GpuRenderer::new(SIZE, SIZE).unwrap();
        gpu.set_direct_metal(direct);
        (direct, gpu)
    })
}
/// How much of the pixel at (`x`, `y`) the box covers with `smoothing`.
fn coverage(x: u32, y: u32, smoothing: f32) -> f32 {
    let (left, top, size) = BOX;
    let half = size / 2.;
    let q = (
        (x as f32 + 0.5 - left - half).abs(),
        (y as f32 + 0.5 - top - half).abs(),
    );
    (0.5 - corner_distance(q, (half, half), RADIUS, smoothing)).clamp(0., 1.)
}
/// The box's top-left corner in `pixels` against its squircle: each pixel
/// within antialiasing of it, and all of them nearer it than the circle it
/// replaces (the two part by under a pixel, in the curve's ends).
fn assert_squircle(pixels: &[u8], smoothing: f32, what: &str) {
    let (mut worst, mut off_squircle, mut off_circle) = (0_f32, 0., 0.);
    for y in 50..100 {
        for x in 50..100 {
            let shown = alpha(pixels, x, y);
            let error = (shown - coverage(x, y, smoothing)).abs();
            worst = worst.max(error);
            off_squircle += error;
            off_circle += (shown - coverage(x, y, 0.)).abs();
        }
    }
    assert!(worst < 0.3, "{what}: {worst} from the squircle");
    assert!(
        off_squircle * 2. < off_circle,
        "{what}: drawn as a circle ({off_squircle} against {off_circle})"
    );
}
fn panel(scene: &mut Scene, quad: QuadStyle, clip: bool) -> NodeId {
    let root = scene.root();
    let id = scene.append(
        root,
        NodeKind::Panel {
            layout: Layout::Overlay,
            quad,
        },
        Style {
            clip,
            ..fixed(BOX.2, BOX.2)
        },
    );
    scene.set_transform(id, Transform { x: BOX.0, y: BOX.1 });
    id
}

#[test]
fn smoothed_fills_borders_and_clips_follow_the_squircle() {
    let _gpu_fixture = gpu_fixture();
    for (direct, mut gpu) in renderers() {
        // A fill.
        let mut scene = scene();
        panel(
            &mut scene,
            QuadStyle {
                fill: RED,
                radius: RADIUS,
                corner_smoothing: Some(0.6),
                ..Default::default()
            },
            false,
        );
        let p = render(&mut gpu, &mut scene);
        assert_squircle(&p, 0.6, &format!("fill, direct {direct}"));
        // Straight edges stay where they were, and the middle is solid.
        assert_eq!(alpha(&p, 100, 50), 1.);
        assert_eq!(alpha(&p, 100, 49), 0.);
        assert_eq!(alpha(&p, 100, 100), 1.);

        // A border alone (a ring): its outer edge is the squircle's.
        let mut scene = self::scene();
        panel(
            &mut scene,
            QuadStyle {
                radius: RADIUS,
                border_width: 3.,
                border_color: RED,
                corner_smoothing: Some(0.6),
                ..Default::default()
            },
            false,
        );
        let p = render(&mut gpu, &mut scene);
        for (x, y) in [(55, 55), (52, 70), (70, 52)] {
            assert!(
                (alpha(&p, x, y) - coverage(x, y, 0.6)).abs() < 0.3,
                "ring at ({x}, {y}), direct {direct}"
            );
        }
        // Hollow inside.
        assert_eq!(alpha(&p, 100, 100), 0.);

        // A rounded clip cutting a child that fills it.
        let mut scene = self::scene();
        let card = panel(
            &mut scene,
            QuadStyle {
                radius: RADIUS,
                corner_smoothing: Some(0.6),
                ..Default::default()
            },
            true,
        );
        scene.append(card, NodeKind::Rect(RED), fixed(BOX.2, BOX.2));
        let p = render(&mut gpu, &mut scene);
        assert_squircle(&p, 0.6, &format!("clip, direct {direct}"));
    }
}

#[test]
fn the_default_smoothing_applies_to_boxes_without_their_own() {
    let _gpu_fixture = gpu_fixture();
    for (direct, mut gpu) in renderers() {
        let quad = |corner_smoothing| QuadStyle {
            fill: RED,
            radius: RADIUS,
            corner_smoothing,
            ..Default::default()
        };
        set_default_corner_smoothing(1.);
        let mut scene = scene();
        panel(&mut scene, quad(None), false);
        let p = render(&mut gpu, &mut scene);
        // A box's own smoothing wins over the default.
        let mut own = self::scene();
        panel(&mut own, quad(Some(0.)), false);
        let circle = render(&mut gpu, &mut own);
        set_default_corner_smoothing(0.);
        assert_squircle(&p, 1., &format!("default, direct {direct}"));
        let worst = (50..100)
            .flat_map(|y| (50..100).map(move |x| (x, y)))
            .map(|(x, y)| (alpha(&circle, x, y) - coverage(x, y, 0.)).abs())
            .fold(0., f32::max);
        assert!(worst < 0.3, "own smoothing 0: {worst} from the circle");
    }
}
