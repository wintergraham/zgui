//! Renders each onboarding step offscreen on the GPU (shader fills too) to
//! `$ONBOARDING_SHOTS/<step>.ppm`, for looking at without a window:
//! `ONBOARDING_SHOTS=/tmp/shots cargo test --release -p zgui-script --test onboarding_render -- --ignored`
mod common;
use common::Harness;
use std::{io::Write, path::Path, sync::Arc};
use zgui::scene::Rect;
use zgui_gpu::{GpuRenderer, text::FontData};
use zgui_script::ScriptHost;

const STEPS: &[&str] = &[
    "splash",
    "welcome",
    "browsers",
    "data",
    "importing",
    "appearance",
    "agent",
    "per_action",
    "ready",
    "coach",
];

#[test]
#[ignore]
fn render_steps() {
    let Ok(out) = std::env::var("ONBOARDING_SHOTS") else {
        return;
    };
    let out = Path::new(&out);
    std::fs::create_dir_all(out).unwrap();
    let only = std::env::var("ONBOARDING_ONLY")
        .ok()
        .filter(|o| !o.is_empty());
    for step in STEPS.iter().filter(|s| {
        only.as_deref()
            .is_none_or(|o| o.split(',').any(|x| x == **s))
    }) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/onboarding/app.luau");
        let h = Harness::new(|runtime| {
            let host = ScriptHost::new(runtime, path);
            host.shared("onboarding_step", *step);
            host.action("close", |_| {});
            host
        });
        let scale = 2.;
        let mut gpu = GpuRenderer::new(1760, 1160).expect("a GPU adapter");
        gpu.set_scale_factor(scale);
        for bytes in [
            &include_bytes!("../../zgui-desktop/examples/support/fonts/GeistVariable.ttf")[..],
            include_bytes!("../../zgui-desktop/examples/support/fonts/GeistMonoVariable.ttf"),
        ] {
            FontData::new(Arc::<[u8]>::from(bytes))
                .unwrap()
                .install(&mut gpu.text_system().borrow_mut());
        }
        {
            let mut scene = h.ui.scene.borrow_mut();
            gpu.install_text(&mut scene);
            scene.resize(880., 580.);
        }
        h.ui.refresh_text_geometry();
        // Long enough for the entrance, the import's first progress and a
        // few dither frames.
        h.frames(90);
        if let Some(e) = h.error() {
            panic!("{step}: {e}");
        }
        let mut scene = h.ui.scene.borrow_mut();
        scene.flush();
        gpu.render(&scene, &[Rect::new(0., 0., 880., 580.)])
            .unwrap();
        let pixels = gpu.readback().unwrap();
        let mut file = std::fs::File::create(out.join(format!("{step}.ppm"))).unwrap();
        write!(file, "P6\n1760 1160\n255\n").unwrap();
        let rgb: Vec<u8> = pixels.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
        file.write_all(&rgb).unwrap();
    }
}
