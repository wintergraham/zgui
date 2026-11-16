//! What Ask's theme sweep (ui/kit/theme_swipe.luau) costs: the CPU work of
//! turning a window's frame into the picture, the first frame (shader
//! compile, picture upload) and the GPU time of each frame of the sweep,
//! offscreen with Metal directly.
//!
//! Run: cargo run --release -p zgx-gpu --example swipe_bench -- <theme_swipe.luau> [width height scale] [sheets]
use std::{sync::Arc, time::Instant};
use zgx::decoration::{Background, Decoration, FillShader, ShaderUniforms};
use zgx::image::ImageData;
use zgx::scene::*;
use zgx_gpu::GpuRenderer;

fn ms(d: std::time::Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

fn eased(curve: [f32; 4], x: f32) -> f32 {
    if x <= 0. {
        return 0.;
    }
    if x >= 1. {
        return 1.;
    }
    let bez = |s: f32, a: f32, b: f32| {
        let r = 1. - s;
        3. * r * r * s * a + 3. * r * s * s * b + s * s * s
    };
    let (mut lo, mut hi) = (0f32, 1f32);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.;
        if bez(mid, curve[0], curve[2]) < x {
            lo = mid
        } else {
            hi = mid
        }
    }
    bez((lo + hi) / 2., curve[1], curve[3])
}

/// The sheet's uniforms at progress `t` (as theme_swipe.luau's `play`).
fn params(t: f32, w: f32, h: f32, page: bool) -> ShaderUniforms {
    let elapsed = t * 2.19;
    let travel = eased([0.22, 1., 0.36, 1.], elapsed / 2.28);
    let rim = eased([0.42, 0., 0.58, 1.], (elapsed + 0.35) / 2.0);
    let hair = eased([0.42, 0., 0.58, 1.], (elapsed + 0.35 - 0.47) / 2.0);
    // The new tab's tray, near the top right; the page card under the bars.
    let (ox, oy, hw, hh) = (w - 180., 120., 80., 18.);
    let radius = if page { -10. } else { 10. };
    ShaderUniforms {
        time: 0.,
        params: [
            [t, travel, rim, hair],
            [ox, oy, hw, hh],
            [0.6, 0.02, -0.05, radius],
            [8., 52., w - 16., h - 60.],
        ],
    }
}

fn sheet(scene: &mut Scene, shader: Arc<FillShader>, image: Arc<ImageData>, w: f32, h: f32) -> NodeId {
    let quad = QuadStyle {
        fill: Color(0, 0, 0, 0),
        decoration: Some(Arc::new(Decoration {
            background: Some(Background::ShadedImage { shader, image }),
            ..Default::default()
        })),
        ..Default::default()
    };
    let root = scene.root();
    scene.append(
        root,
        NodeKind::Panel {
            layout: Layout::Overlay,
            quad,
        },
        Style {
            width: Some(w),
            height: Some(h),
            absolute: true,
            ..Default::default()
        },
    )
}

/// The snapshot's CPU side as zgx-desktop does it (thumbnail::shrink at a
/// factor of 1): premultiplied BGRA rows to straight RGBA, then an image.
fn shrink_like(bytes: &[u8], width: usize, height: usize, stride: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height * 4];
    for y in 0..height {
        let row = &bytes[y * stride..];
        for x in 0..width {
            let px = &row[x * 4..x * 4 + 4];
            let [b, g, r, a] = [px[0], px[1], px[2], px[3]].map(u32::from);
            let straight = |c: u32| (c * 255).checked_div(a).map_or(0, |v| v.min(255) as u8);
            let at = (y * width + x) * 4;
            out[at..at + 4].copy_from_slice(&[straight(r), straight(g), straight(b), a as u8]);
        }
    }
    out
}

/// The same with zgx-desktop's own-size path (`ask/swipe-perf`): opaque pixels without dividing.
fn shrink_fast(bytes: &[u8], width: usize, height: usize, stride: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height * 4];
    for y in 0..height {
        let row = &bytes[y * stride..][..width * 4];
        for (px, at) in row.as_chunks::<4>().0.iter().zip(out[y * width * 4..][..width * 4].as_chunks_mut::<4>().0) {
            let [b, g, r, a] = *px;
            *at = match a {
                255 => [r, g, b, 255],
                0 => [0; 4],
                _ => {
                    let straight = |c: u8| ((u32::from(c) * 255) / u32::from(a)).min(255) as u8;
                    [straight(r), straight(g), straight(b), a]
                }
            };
        }
    }
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let luau = std::fs::read_to_string(&args[0])?;
    let w: f32 = args.get(1).map_or(1512., |v| v.parse().unwrap());
    let h: f32 = args.get(2).map_or(945., |v| v.parse().unwrap());
    let scale: f32 = args.get(3).map_or(2., |v| v.parse().unwrap());
    let sheets: usize = args.get(4).map_or(2, |v| v.parse().unwrap());
    let start = luau.find("ui.shader([=[").expect("shader") + "ui.shader([=[".len();
    let end = luau[start..].find("]=]").expect("end") + start;
    let source = luau[start..end].to_string();
    let (pw, ph) = ((w * scale) as usize, (h * scale) as usize);
    println!("window {w}x{h} @{scale}x = {pw}x{ph} px ({:.1} MB a picture), {sheets} sheet(s)", (pw * ph * 4) as f64 / 1e6);

    // A frame's pixels: translucent sidebar, opaque card, some texture.
    let stride = (pw * 4).div_ceil(64) * 64;
    let mut frame = vec![0u8; stride * ph];
    for y in 0..ph {
        for x in 0..pw {
            let a: u8 = if x < pw / 5 { 200 } else { 255 };
            let v = ((x ^ y) & 0x3f) as u8 + 100;
            let c = (u16::from(v) * u16::from(a) / 255) as u8;
            frame[y * stride + x * 4..][..4].copy_from_slice(&[c, c / 2 + 20, c, a]);
        }
    }
    let t0 = Instant::now();
    let straight = shrink_like(&frame, pw, ph, stride);
    let t_shrink = t0.elapsed();
    let t0 = Instant::now();
    let fast = shrink_fast(&frame, pw, ph, stride);
    let t_fast = t0.elapsed();
    assert_eq!(fast, straight, "fast path differs");
    println!("straight-alpha convert, opaque fast path: {:.1} ms (same bytes)", ms(t_fast));
    let t0 = Instant::now();
    let image = Arc::new(ImageData::new(pw as u32, ph as u32, straight)?);
    let t_image = t0.elapsed();
    let page_image = Arc::new(ImageData::new(pw as u32, ph as u32, image.pixels().to_vec())?);
    println!("snapshot CPU: straight-alpha convert {:.1} ms, into ImageData {:.1} ms", ms(t_shrink), ms(t_image));

    let mut gpu = GpuRenderer::new(pw as u32, ph as u32)?;
    gpu.set_scale_factor(scale);
    gpu.set_direct_metal(std::env::var("DIRECT").map_or(true, |v| v != "0"));
    let mut scene = Scene::new(w, h);
    scene.set_kind(scene.root(), NodeKind::Container(Layout::Overlay));
    // A plain first frame, so the renderer's own setup is not counted below.
    scene.append(scene.root(), NodeKind::Rect(Color(30, 30, 30, 255)), Style { width: Some(w), height: Some(h), ..Default::default() });
    let full = [Rect::new(0., 0., w, h)];
    let f = scene.flush();
    gpu.render(&scene, &f.damage)?;
    gpu.readback()?;

    let t0 = Instant::now();
    let shader = FillShader::new(source.clone(), None);
    let ok = zgx_gpu::check_fill_shader(&source);
    let t_check = t0.elapsed();
    println!("ui.shader check (naga parse+validate): {:.1} ms {:?}", ms(t_check), ok.err());

    // Pipeline: a 1x1 box filled by the shader alone.
    let tiny = {
        let quad = QuadStyle {
            fill: Color(0, 0, 0, 0),
            decoration: Some(Arc::new(Decoration {
                background: Some(Background::Shader(shader.clone())),
                ..Default::default()
            })),
            ..Default::default()
        };
        let root = scene.root();
        scene.append(root, NodeKind::Panel { layout: Layout::Overlay, quad }, Style { width: Some(1.), height: Some(1.), absolute: true, ..Default::default() })
    };
    scene.set_shader_uniforms(tiny, params(0.5, w, h, false));
    let f = scene.flush();
    let t0 = Instant::now();
    gpu.render(&scene, &f.damage)?;
    let t_compile = t0.elapsed();
    gpu.readback()?;
    let t_compile_wait = t0.elapsed();
    println!("first draw with the shader (pipeline compile): render {:.1} ms, to GPU done {:.1} ms", ms(t_compile), ms(t_compile_wait));
    scene.remove(tiny);

    // The sheets, at progress 0: the pictures upload.
    let mut ids = vec![];
    if sheets > 1 {
        ids.push((sheet(&mut scene, shader.clone(), page_image.clone(), w, h), true));
    }
    ids.push((sheet(&mut scene, shader.clone(), image.clone(), w, h), false));
    for &(id, page) in &ids {
        scene.set_shader_uniforms(id, params(0., w, h, page));
    }
    let f = scene.flush();
    let t0 = Instant::now();
    let stats = gpu.render(&scene, &f.damage)?;
    let t_upload = t0.elapsed();
    gpu.readback()?;
    let t_upload_wait = t0.elapsed();
    println!("first frame with the pictures ({} uploads): render {:.1} ms, to GPU done {:.1} ms", stats.image_uploads, ms(t_upload), ms(t_upload_wait));

    // The sweep: GPU time per frame (Metal's own start/end), full damage; each frame's least over REPS passes
    // (the GPU's clock wanders between frames read back one at a time).
    let steps = 44;
    let reps: usize = std::env::var("REPS").map_or(5, |v| v.parse().unwrap());
    let mut best = vec![f64::INFINITY; steps + 1];
    for _ in 0..reps {
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            for &(id, page) in &ids {
                scene.set_shader_uniforms(id, params(t, w, h, page));
            }
            let _ = scene.flush();
            gpu.render(&scene, &full)?;
            gpu.readback()?;
            let g = gpu.take_gpu_time().map_or(f64::NAN, ms);
            best[i] = best[i].min(g);
        }
    }
    let mut worst = (0., 0.);
    for (i, g) in best.iter().enumerate() {
        let t = i as f32 / steps as f32;
        if *g > worst.1 {
            worst = (t, *g);
        }
        if i % 11 == 0 || std::env::var("ALL").is_ok() {
            println!("  t={:>4.0}%  gpu {:>6.2} ms", t * 100., g);
        }
    }
    let mean = best.iter().sum::<f64>() / best.len() as f64;
    println!("GPU per frame: mean {:.2} ms, worst {:.2} ms at {:.0}%", mean, worst.1, worst.0 * 100.);
    if let Ok(dir) = std::env::var("DUMP") {
        std::fs::create_dir_all(&dir)?;
        for pct in [0u32, 10, 25, 50, 75, 100] {
            let t = pct as f32 / 100.;
            for &(id, page) in &ids {
                scene.set_shader_uniforms(id, params(t, w, h, page));
            }
            let _ = scene.flush();
            gpu.render(&scene, &full)?;
            let px = gpu.readback()?;
            let stride = px.len() / ph;
            let mut out = Vec::with_capacity(pw * ph * 4);
            for y in 0..ph {
                out.extend_from_slice(&px[y * stride..y * stride + pw * 4]);
            }
            std::fs::write(format!("{dir}/f{pct:03}.rgba"), &out)?;
        }
        std::fs::write(format!("{dir}/size"), format!("{pw} {ph}"))?;
    }
    // Baseline: the same frame drawn with an empty shader on the picture.
    let plain = FillShader::new("fn fill(input: FillInput) -> vec4<f32> { return zgx_image(input.p / input.size); }", None);
    for &(id, _) in &ids {
        scene.remove(id);
    }
    let mut b = vec![];
    if sheets > 1 {
        b.push(sheet(&mut scene, plain.clone(), page_image.clone(), w, h));
    }
    b.push(sheet(&mut scene, plain.clone(), image.clone(), w, h));
    let _ = scene.flush();
    let mut base = f64::INFINITY;
    for _ in 0..20 {
        gpu.render(&scene, &full)?;
        gpu.readback()?;
        base = base.min(gpu.take_gpu_time().map_or(f64::NAN, ms));
    }
    println!("baseline (same sheets, trivial shader): {:.2} ms", base);
    Ok(())
}
