//! Windows: frames drawn through the repository's wgpu-hal raise no error in
//! the Direct3D 12 debug layer: every resource is in the state each command
//! needs, the vertex ring's buffers included. Test builds ask for the debug
//! layer (wgpu's default flags in debug builds); where it isn't installed
//! (Windows' Graphics Tools), the test says so and checks nothing, unless
//! `ZGX_REQUIRE_DEBUG_LAYER` is set, as in the manual `d3d12-debug-layer`
//! workflow, which installs it.
#![cfg(all(windows, zgx_wgpu_hal_patched))]
use windows::{
    Win32::System::LibraryLoader::{GetModuleHandleW, LoadLibraryW},
    core::w,
};
use zgx::scene::*;
use zgx_gpu::GpuRenderer;

fn fixed(w: f32, h: f32) -> Style {
    Style {
        width: Some(w),
        height: Some(h),
        ..Default::default()
    }
}

#[test]
fn frames_raise_no_debug_layer_error() {
    let required = std::env::var_os("ZGX_REQUIRE_DEBUG_LAYER").is_some();
    // SAFETY: loads a system library by name.
    if unsafe { LoadLibraryW(w!("d3d12SDKLayers.dll")) }.is_err() {
        assert!(!required, "no Direct3D 12 debug layer (Graphics Tools)");
        eprintln!("skipped: no Direct3D 12 debug layer (Windows' Graphics Tools) on this machine");
        return;
    }
    let _ = wgpu_hal::VALIDATION_CANARY.get_and_reset();
    let mut gpu = GpuRenderer::new(160, 80).expect("an adapter");
    if required {
        // SAFETY: looks a loaded module up by name.
        assert!(
            unsafe { GetModuleHandleW(w!("d3d12SDKLayers.dll")) }.is_ok(),
            "the debug layer is on"
        );
    }
    let mut scene = Scene::new(160., 80.);
    scene.set_kind(scene.root(), NodeKind::Container(Layout::Overlay));
    scene.append(
        scene.root(),
        NodeKind::Rect(Color(20, 40, 60, 255)),
        fixed(160., 80.),
    );
    let moving = scene.append(
        scene.root(),
        NodeKind::Rect(Color(200, 0, 0, 128)),
        fixed(20., 20.),
    );
    let label = scene.append(
        scene.root(),
        NodeKind::Text {
            text: "barriers".into(),
            font_size: 14.,
            color: Color(255, 255, 255, 255),
        },
        fixed(120., 24.),
    );
    // Frames back to back go round the vertex ring while earlier ones are
    // in flight; some redraw whole, some are read back.
    for frame in 0..40 {
        scene.set_transform(
            moving,
            Transform {
                x: (frame * 3) as f32,
                y: 10.,
            },
        );
        if frame % 5 == 0 {
            scene.set_kind(
                label,
                NodeKind::Text {
                    text: format!("frame {frame}").into(),
                    font_size: 14.,
                    color: Color(255, 255, 255, 255),
                },
            );
        }
        let damage = scene.flush().damage;
        gpu.render(&scene, &damage).unwrap();
        if frame % 10 == 9 {
            gpu.render(&scene, &[Rect::new(0., 0., 160., 80.)]).unwrap();
            gpu.readback().unwrap();
        }
        gpu.present().unwrap();
    }
    gpu.readback().unwrap();
    let errors = wgpu_hal::VALIDATION_CANARY.get_and_reset();
    assert!(errors.is_empty(), "the debug layer reported: {errors:#?}");
}
