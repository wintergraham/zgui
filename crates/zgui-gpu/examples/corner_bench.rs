//! GPU cost of rounded corners, circular against squircle
//! (`corner_smoothing`): scenes where corners are most of the pixels drawn
//! (translucent pills, wide shadows, rounded clips) and a window of cards,
//! repainted in full at 2x. The two smoothings render on alternate frames,
//! so load on the machine falls on both alike; times are the GPU's, from
//! direct Metal frames.
//!
//! Run: cargo run --release -p zgui-gpu --example corner_bench [frames]
use std::time::Duration;
use zgui::scene::*;
use zgui_gpu::GpuRenderer;

const WIDTH: f32 = 1180.;
const HEIGHT: f32 = 780.;
const SMOOTHINGS: [f32; 2] = [0., 0.6];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let frames: usize = std::env::args()
        .nth(1)
        .map_or(300, |v| v.parse().expect("frame count"));
    for (name, scenario) in [
        ("pills", Scenario::Pills),
        ("shadows", Scenario::Shadows),
        ("rounded clips", Scenario::Clips),
        ("cards", Scenario::Cards),
    ] {
        let times = run(scenario, frames)?;
        if let [circle, squircle] = &times[..] {
            // Adjacent frames share the machine's load: their ratio holds
            // when the times themselves swing.
            let mut ratios: Vec<f64> = circle
                .iter()
                .zip(squircle)
                .map(|(c, s)| s.as_secs_f64() / c.as_secs_f64())
                .collect();
            ratios.sort_by(f64::total_cmp);
            println!(
                "{name:14} squircle / circle, frame by frame: median {:.3}",
                ratios[ratios.len() / 2]
            );
        }
        for (smoothing, mut times) in SMOOTHINGS.into_iter().zip(times) {
            times.sort();
            println!(
                "{name:14} smoothing {smoothing:.1}: GPU min {:>6.3} ms  median {:>6.3} ms",
                ms(times[0]),
                ms(times[times.len() / 2]),
            );
        }
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq)]
enum Scenario {
    /// 600 overlapping translucent 80 pt circles: every pixel a corner's.
    Pills,
    /// 200 boxes with 24 pt corners and 30 pt shadows.
    Shadows,
    /// 300 clips with 40 pt corners around a rect that fills each.
    Clips,
    /// 12 by 10 cards: fill, border, shadow and a clipped child each.
    Cards,
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.
}

fn fixed(width: f32, height: f32) -> Style {
    Style {
        width: Some(width),
        height: Some(height),
        ..Default::default()
    }
}

/// A scatter of `n` positions over the window, the same every run.
fn spots(n: usize, size: f32) -> impl Iterator<Item = (f32, f32)> {
    (0..n).map(move |i| {
        let t = i as f32 * 0.618_034;
        (
            (t.fract() * (WIDTH - size)).floor(),
            ((i as f32 * 0.414_213).fract() * (HEIGHT - size)).floor(),
        )
    })
}

fn scene(scenario: Scenario, smoothing: f32) -> Scene {
    let mut scene = Scene::new(WIDTH, HEIGHT);
    scene.set_kind(scene.root(), NodeKind::Container(Layout::Overlay));
    let root = scene.root();
    let quad = |quad: QuadStyle| QuadStyle {
        corner_smoothing: Some(smoothing),
        ..quad
    };
    let place = |scene: &mut Scene, kind, style, (x, y)| {
        let id = scene.append(root, kind, style);
        scene.set_transform(id, Transform { x, y });
        id
    };
    match scenario {
        Scenario::Pills => {
            for at in spots(600, 80.) {
                let kind = NodeKind::Quad(quad(QuadStyle {
                    fill: Color(60, 110, 220, 40),
                    radius: 40.,
                    ..Default::default()
                }));
                place(&mut scene, kind, fixed(80., 80.), at);
            }
        }
        Scenario::Shadows => {
            for at in spots(200, 120.) {
                let kind = NodeKind::Quad(quad(QuadStyle {
                    fill: Color(250, 250, 252, 255),
                    radius: 24.,
                    shadow: Some(BoxShadow {
                        color: Color(0, 0, 0, 40),
                        offset: Transform { x: 0., y: 6. },
                        blur_radius: 15.,
                        spread: 0.,
                    }),
                    ..Default::default()
                }));
                place(&mut scene, kind, fixed(120., 90.), at);
            }
        }
        Scenario::Clips => {
            for at in spots(300, 100.) {
                let kind = NodeKind::Panel {
                    layout: Layout::Overlay,
                    quad: quad(QuadStyle {
                        radius: 40.,
                        ..Default::default()
                    }),
                };
                let style = Style {
                    clip: true,
                    ..fixed(100., 100.)
                };
                let card = place(&mut scene, kind, style, at);
                scene.append(
                    card,
                    NodeKind::Rect(Color(60, 110, 220, 60)),
                    fixed(100., 100.),
                );
            }
        }
        Scenario::Cards => {
            for row in 0..10 {
                for col in 0..12 {
                    let at = (20. + col as f32 * 96., 20. + row as f32 * 76.);
                    let kind = NodeKind::Panel {
                        layout: Layout::Overlay,
                        quad: quad(QuadStyle {
                            fill: Color(250, 250, 252, 255),
                            radius: 18.,
                            border_width: 1.,
                            border_color: Color(0, 0, 0, 40),
                            shadow: Some(BoxShadow {
                                color: Color(0, 0, 0, 60),
                                offset: Transform { x: 0., y: 4. },
                                blur_radius: 10.,
                                spread: 0.,
                            }),
                            ..Default::default()
                        }),
                    };
                    let style = Style {
                        clip: true,
                        ..fixed(84., 64.)
                    };
                    let card = place(&mut scene, kind, style, at);
                    scene.append(
                        card,
                        NodeKind::Rect(Color(60, 110, 220, 255)),
                        fixed(84., 40.),
                    );
                }
            }
        }
    }
    scene.flush();
    scene
}

/// Each smoothing's GPU times, rendered on alternate frames.
fn run(
    scenario: Scenario,
    frames: usize,
) -> Result<Vec<Vec<Duration>>, Box<dyn std::error::Error>> {
    let mut setups = Vec::new();
    for smoothing in SMOOTHINGS {
        let mut renderer = GpuRenderer::new((WIDTH * 2.) as u32, (HEIGHT * 2.) as u32)?;
        renderer.set_scale_factor(2.);
        renderer.set_background(Color(236, 238, 242, 255));
        renderer.set_direct_metal(true);
        setups.push((renderer, scene(scenario, smoothing), Vec::new()));
    }
    let full = [Rect::new(0., 0., WIDTH, HEIGHT)];
    for frame in 0..frames + 20 {
        for (renderer, scene, times) in &mut setups {
            renderer.render(scene, &full)?;
            // Waits for the frame, and its time.
            renderer.readback()?;
            if let Some(time) = renderer.take_gpu_time()
                && frame >= 20
            {
                times.push(time);
            }
        }
    }
    let times: Vec<_> = setups.into_iter().map(|(_, _, times)| times).collect();
    if times.iter().any(Vec::is_empty) {
        return Err("no GPU times (direct Metal frames only)".into());
    }
    Ok(times)
}
