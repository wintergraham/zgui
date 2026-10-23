//! The quick_ask example, driven headless through its main paths.
mod common;
use common::Harness;
use std::path::Path;
use zgui::input::{InputEvent, Key, Modifiers};
use zgui_script::ScriptHost;

fn quick_ask() -> Harness {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/quick_ask/app.luau");
    let h = Harness::new(|runtime| {
        let host = ScriptHost::new(runtime, path);
        host.action("drag_window", |_| {});
        host
    });
    h.ui.scene.borrow_mut().resize(1280., 860.);
    h
}

/// Writes a GPU capture when QUICK_ASK_SNAPSHOT names a file prefix.
fn snapshot(h: &Harness, suffix: &str) {
    let Some(path) = std::env::var_os("QUICK_ASK_SNAPSHOT") else {
        return;
    };
    let path = format!("{}{suffix}.ppm", path.to_string_lossy());
    let mut gpu = zgui_gpu::GpuRenderer::new(1280, 860).expect("a GPU");
    for bytes in [
        &include_bytes!("../../zgui-desktop/examples/support/fonts/GeistVariable.ttf")[..],
        &include_bytes!("../../zgui-desktop/examples/support/fonts/GeistMonoVariable.ttf")[..],
    ] {
        zgui_gpu::text::FontData::new(bytes)
            .unwrap()
            .install(&mut gpu.text_system().borrow_mut());
    }
    gpu.install_text(&mut h.ui.scene.borrow_mut());
    h.ui.refresh_text_geometry();
    h.frame();
    h.ui.prepare_frame();
    gpu.render(
        &h.ui.scene.borrow(),
        &[zgui::scene::Rect::new(0., 0., 1280., 860.)],
    )
    .unwrap();
    let pixels = gpu.readback().unwrap();
    let mut out = "P6\n1280 860\n255\n".to_string().into_bytes();
    for rgba in pixels.chunks(4) {
        out.extend_from_slice(&rgba[..3]);
    }
    std::fs::write(&path, out).unwrap();
}
fn ok(h: &Harness) {
    if let Some(e) = h.host.error().with_untracked(|e| e.clone()) {
        panic!("script error: {e}");
    }
}
fn frames(h: &Harness, n: usize) {
    for _ in 0..n {
        h.frame();
        ok(h);
    }
}
fn key(h: &mut Harness, key: Key, modifiers: Modifiers) {
    h.ui.dispatch(InputEvent::KeyDown {
        key,
        modifiers,
        repeat: false,
    });
    h.settle();
    ok(h);
}
fn meta() -> Modifiers {
    Modifiers {
        meta: true,
        ..Default::default()
    }
}

#[test]
fn asks_answers_and_pastes_back() {
    let mut h = quick_ask();
    frames(&h, 30);
    assert!(
        h.find("quick_ask").is_some(),
        "summoned over Chrome on launch"
    );
    snapshot(&h, "1-chrome");
    assert_eq!(h.text("row_1"), "Summarize this page");

    // Running a suggestion thinks, then answers with the hand-off actions.
    h.click("row_1");
    frames(&h, 20);
    snapshot(&h, "2-thinking");
    frames(&h, 60 * 2);
    snapshot(&h, "3-answer");
    assert!(
        h.find("paste").is_some(),
        "the answer offers Paste into Chrome"
    );
    h.click("paste");
    frames(&h, 30);
    assert!(h.find("quick_ask").is_none(), "pasting closes the panel");

    // ⌥Space brings it back, empty.
    key(
        &mut h,
        Key::Space,
        Modifiers {
            alt: true,
            ..Default::default()
        },
    );
    frames(&h, 30);
    assert!(h.find("quick_ask").is_some(), "⌥Space summons it again");
    assert!(h.find("row_1").is_some(), "back to suggestions");
}

#[test]
fn every_scene_opens_and_takes_keys() {
    let mut h = quick_ask();
    frames(&h, 10);
    for scene in ["code", "mail", "finder", "chrome"] {
        h.click(&format!("scene_{scene}"));
        frames(&h, 30);
        assert!(h.find("quick_ask").is_some(), "{scene}: the panel is up");
        snapshot(&h, &format!("4-{scene}"));
    }
    // Context keys, destination, menus and the keyboard map.
    key(
        &mut h,
        Key::Character("2".into()),
        Modifiers {
            alt: true,
            ..Default::default()
        },
    );
    key(&mut h, Key::Tab, Modifiers::default());
    key(&mut h, Key::Character("k".into()), meta());
    frames(&h, 10);
    assert!(h.find("action_run").is_some(), "⌘K opens the actions menu");
    snapshot(&h, "5-actions");
    key(&mut h, Key::Escape, Modifiers::default());
    key(&mut h, Key::Character("/".into()), meta());
    frames(&h, 20);
    assert!(h.find("keymap").is_some(), "⌘/ shows the keyboard map");
    snapshot(&h, "6-keymap");
    key(&mut h, Key::Escape, Modifiers::default());
    key(&mut h, Key::Escape, Modifiers::default());
    frames(&h, 30);
    assert!(h.find("quick_ask").is_none(), "esc closes it");
    key(
        &mut h,
        Key::Space,
        Modifiers {
            alt: true,
            ..Default::default()
        },
    );
    frames(&h, 30);
    assert!(h.find("quick_ask").is_some(), "⌥Space reopens it after esc");
}

#[test]
fn plus_attaches_and_uploads() {
    let mut h = quick_ask();
    frames(&h, 10);
    h.click("add");
    frames(&h, 60 * 2);
    assert!(
        h.find("remove_file:Invoice INV-2291.pdf").is_some(),
        "the picked file is a chip"
    );
}

#[test]
fn finder_files_drop_onto_the_panel() {
    let mut h = quick_ask();
    frames(&h, 10);
    h.click("scene_finder");
    frames(&h, 30);
    // Move-in notes isn't picked, so dropping it attaches one more file.
    let from = h.find("finder_file_4").unwrap();
    let to = h.find("qa_input").unwrap();
    let (a, b) = {
        let scene = h.ui.scene.borrow();
        (scene.bounds(from), scene.bounds(to))
    };
    let (x0, y0) = (a.x + a.width / 2., a.y + 20.);
    let (x1, y1) = (b.x + b.width / 2., b.y + b.height / 2.);
    h.ui.dispatch(InputEvent::PointerDown {
        x: x0,
        y: y0,
        button: zgui::input::PointerButton::Primary,
    });
    for i in 1..=12 {
        let t = i as f32 / 12.;
        h.ui.dispatch(InputEvent::PointerMove {
            x: x0 + (x1 - x0) * t,
            y: y0 + (y1 - y0) * t,
        });
        h.frame();
    }
    ok(&h);
    snapshot(&h, "7-dropping");
    h.ui.dispatch(InputEvent::PointerUp {
        x: x1,
        y: y1,
        button: zgui::input::PointerButton::Primary,
    });
    frames(&h, 60 * 2);
    snapshot(&h, "8-dropped");
    assert!(
        h.find("remove_file:Move-in notes.md").is_some(),
        "the dropped file is a chip"
    );
}

/// Frame costs per interaction: script and layout, then encoding and GPU
/// work (render plus a readback, which waits for the GPU and adds a few ms;
/// QUICK_ASK_NO_READBACK=1 times encoding alone).
/// `cargo test --release -p zgui-script --test quick_ask profile -- --ignored --nocapture`
#[test]
#[ignore]
fn profile_interactions() {
    use std::time::Instant;
    use zgui::input::PointerButton::Primary;
    let mut h = quick_ask();
    // QUICK_ASK_SCALE=2 renders as a Retina window does.
    let scale: f32 = std::env::var("QUICK_ASK_SCALE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.);
    let mut gpu =
        zgui_gpu::GpuRenderer::new((1280. * scale) as u32, (860. * scale) as u32).expect("a GPU");
    gpu.set_scale_factor(scale);
    for bytes in [
        &include_bytes!("../../zgui-desktop/examples/support/fonts/GeistVariable.ttf")[..],
        &include_bytes!("../../zgui-desktop/examples/support/fonts/GeistMonoVariable.ttf")[..],
    ] {
        zgui_gpu::text::FontData::new(bytes)
            .unwrap()
            .install(&mut gpu.text_system().borrow_mut());
    }
    gpu.install_text(&mut h.ui.scene.borrow_mut());
    h.ui.refresh_text_geometry();
    let mut run = |h: &mut Harness,
                   name: &str,
                   n: usize,
                   step: &mut dyn FnMut(&mut Harness, usize)| {
        let (mut cpu, mut draw, mut worst, mut blur, mut layers, mut px, mut drawn) =
            (0., 0., 0f64, 0, 0, 0u64, 0);
        for i in 0..n {
            let t = Instant::now();
            step(h, i);
            h.frame();
            ok(h);
            let c = t.elapsed().as_secs_f64() * 1e3;
            let t = Instant::now();
            let report = h.ui.scene.borrow_mut().flush();
            if !report.damage.is_empty() {
                let stats = gpu.render(&h.ui.scene.borrow(), &report.damage).unwrap();
                if std::env::var_os("QUICK_ASK_NO_READBACK").is_none() {
                    gpu.readback().unwrap();
                }
                blur += stats.blur_passes;
                layers += stats.layer_repaints;
                px += stats.damaged_pixels;
                drawn += 1;
            }
            let d = t.elapsed().as_secs_f64() * 1e3;
            cpu += c;
            draw += d;
            worst = worst.max(c + d);
        }
        println!(
            "{name:<12} cpu {:6.2} ms  draw {:6.2} ms  worst {:6.2} ms  drawn {drawn:3}/{n}  blur {blur:4}  layers {layers:4}  px/frame {:>8}",
            cpu / n as f64,
            draw / n as f64,
            worst,
            px / drawn.max(1) as u64
        );
    };
    run(&mut h, "settle", 60, &mut |_, _| {});
    run(&mut h, "idle", 60, &mut |_, _| {});
    let phrase = "how long should it rest";
    run(&mut h, "typing", phrase.len(), &mut |h, i| {
        h.ui.dispatch(InputEvent::Text(phrase[i..i + 1].into()));
    });
    run(&mut h, "after-type", 40, &mut |_, _| {});
    let row = {
        let n = h.find("row_1").unwrap();
        h.ui.scene.borrow().bounds(n)
    };
    run(&mut h, "hover", 60, &mut |h, i| {
        let y = row.y + 17. + (i % 30) as f32 * 6.;
        h.ui.dispatch(InputEvent::PointerMove { x: row.x + 200., y });
    });
    run(&mut h, "clear", 30, &mut |h, i| {
        if i == 0 {
            for _ in 0..phrase.len() {
                h.ui.dispatch(InputEvent::KeyDown {
                    key: Key::Backspace,
                    modifiers: Modifiers::default(),
                    repeat: false,
                });
            }
        }
    });
    run(&mut h, "dictate", 90, &mut |h, i| {
        if i == 0 {
            h.click("mic")
        }
    });
    run(&mut h, "upload", 120, &mut |h, i| {
        if i == 0 {
            h.click("add")
        }
    });
    run(&mut h, "ask", 150, &mut |h, i| {
        if i == 0 {
            key(h, Key::Enter, Modifiers::default())
        }
    });
    run(&mut h, "close", 40, &mut |h, i| {
        if i == 0 {
            key(h, Key::Escape, Modifiers::default())
        }
    });
    run(&mut h, "open", 40, &mut |h, i| {
        if i == 0 {
            key(
                h,
                Key::Space,
                Modifiers {
                    alt: true,
                    ..Default::default()
                },
            )
        }
    });
    for scene in ["code", "mail", "finder", "code", "chrome"] {
        run(&mut h, scene, 60, &mut |h, i| {
            if i == 0 {
                h.click(&format!("scene_{scene}"))
            }
        });
    }
    let (from, to) = {
        let scene = h.ui.scene.borrow();
        let a = scene.bounds(h.handle.find("finder_file_4").unwrap());
        let b = scene.bounds(h.handle.find("qa_input").unwrap());
        (
            (a.x + a.width / 2., a.y + 20.),
            (b.x + b.width / 2., b.y + b.height / 2.),
        )
    };
    run(&mut h, "drag", 40, &mut |h, i| {
        let t = (i as f32 / 20.).min(1.);
        let (x, y) = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
        if i == 0 {
            h.ui.dispatch(InputEvent::PointerDown {
                x,
                y,
                button: Primary,
            });
        } else if i == 39 {
            h.ui.dispatch(InputEvent::PointerUp {
                x,
                y,
                button: Primary,
            });
        } else {
            h.ui.dispatch(InputEvent::PointerMove { x, y });
        }
    });
    run(&mut h, "keymap", 120, &mut |h, i| {
        if i == 0 {
            key(h, Key::Character("/".into()), meta())
        }
    });
    run(&mut h, "keymap-off", 40, &mut |h, i| {
        if i == 0 {
            key(h, Key::Escape, Modifiers::default())
        }
    });
    for round in 0..2 {
        run(&mut h, &format!("close{round}"), 40, &mut |h, i| {
            if i == 0 {
                h.click("status_item")
            }
        });
        run(&mut h, &format!("open{round}"), 40, &mut |h, i| {
            if i == 0 {
                h.click("status_item")
            }
        });
    }
}
