//! GPU snapshots of any Ask Luau UI script (usually `<ask>/ui/gallery.luau`), driven through named steps, for
//! checking screens without a window.
//! Run: ASK_UI_SCRIPT=<ask>/ui/gallery.luau ASK_UI_SNAPSHOTS=/tmp/out \
//!      ASK_UI_SHOTS='history=cmd:route_history,ms:600;menu=cmd:route_history,click:hist_more_3,ms:400' \
//!      cargo test --release -p zgui-script --test ask_ui_snapshots -- --nocapture
//! Steps: `cmd:<command id>`, `click:<node id>`, `hover:<node id>`, `wheel:<node id>:<dy>`, `ms:<n>`, `type:<text>`, `key:<Enter|Escape|Tab|…>`,
//! `down:<node id>`, `to:<node id>:<dy>` (glides the held pointer to that node's centre plus `dy`), `up`,
//! `size:<w>x<h>` (before anything else). Each shot starts from a fresh load and writes `<name>.ppm` (2x).
mod common;
use common::Harness;
use std::path::{Path, PathBuf};
use zgui::input::{InputEvent, Key, Modifiers, PointerButton};
use zgui_gpu::GpuRenderer;
use zgui_script::ScriptHost;

const SCALE: f32 = 2.;

struct Shot {
    h: Harness,
    gpu: GpuRenderer,
    w: f32,
    hgt: f32,
    at: (f32, f32),
}
impl Shot {
    fn new(script: &Path, w: f32, hgt: f32) -> Self {
        let mut gpu =
            GpuRenderer::new((w * SCALE) as u32, (hgt * SCALE) as u32).expect("GPU adapter");
        gpu.set_scale_factor(SCALE);
        for bytes in [
            &include_bytes!("../../zgui-desktop/examples/support/fonts/GeistVariable.ttf")[..],
            &include_bytes!("../../zgui-desktop/examples/support/fonts/GeistMonoVariable.ttf")[..],
        ] {
            zgui_gpu::text::FontData::new(bytes)
                .unwrap()
                .install(&mut gpu.text_system().borrow_mut());
        }
        let script = script.to_path_buf();
        let h = Harness::new(|runtime| ScriptHost::new(runtime, script));
        gpu.install_text(&mut h.ui.scene.borrow_mut());
        h.ui.refresh_text_geometry();
        h.ui.scene.borrow_mut().resize(w, hgt);
        h.ui.prepare_frame();
        h.settle();
        let shot = Self {
            h,
            gpu,
            w,
            hgt,
            at: (0., 0.),
        };
        shot.frames(30);
        shot
    }
    fn frames(&self, n: usize) {
        for _ in 0..n {
            self.h.frame();
        }
    }
    fn center(&mut self, id: &str) -> (f32, f32) {
        let node = self.h.find(id).unwrap_or_else(|| panic!("no node `{id}`"));
        let b = self.h.ui.scene.borrow().bounds(node);
        (b.x + b.width / 2., b.y + b.height / 2.)
    }
    fn step(&mut self, step: &str) {
        let (kind, arg) = step.split_once(':').unwrap_or((step, ""));
        match kind {
            "cmd" => {
                self.h.host.run_command(arg);
                self.h.settle();
                self.frames(1);
            }
            "click" => {
                let (x, y) = self.center(arg);
                self.h.ui.dispatch(InputEvent::PointerMove { x, y });
                self.h.settle();
                self.frames(1);
                self.h.ui.dispatch(InputEvent::PointerDown {
                    x,
                    y,
                    button: PointerButton::Primary,
                });
                self.h.ui.dispatch(InputEvent::PointerUp {
                    x,
                    y,
                    button: PointerButton::Primary,
                });
                self.h.settle();
                self.frames(1);
            }
            "hover" => {
                let (x, y) = self.center(arg);
                self.h.ui.dispatch(InputEvent::PointerMove { x, y });
                self.h.settle();
                self.frames(1);
            }
            "wheel" => {
                // wheel:<node id>:<dy>
                let (id, dy) = arg.rsplit_once(':').unwrap();
                let (x, y) = self.center(id);
                self.h.ui.dispatch(InputEvent::PointerMove { x, y });
                self.h.ui.dispatch(InputEvent::Scroll {
                    x,
                    y,
                    delta_x: 0.,
                    delta_y: dy.parse().unwrap(),
                });
                self.h.settle();
                self.frames(1);
            }
            "down" => {
                let (x, y) = self.center(arg);
                self.at = (x, y);
                self.h.ui.dispatch(InputEvent::PointerMove { x, y });
                self.h.ui.dispatch(InputEvent::PointerDown {
                    x,
                    y,
                    button: PointerButton::Primary,
                });
                self.h.settle();
                self.frames(1);
            }
            "to" => {
                // to:<node id>:<dy>, in eight moves so drags pass their threshold and targets see each step.
                let (id, dy) = arg.rsplit_once(':').unwrap_or((arg, "0"));
                let (x, y) = self.center(id);
                let (x, y) = (x, y + dy.parse::<f32>().unwrap());
                let (x0, y0) = self.at;
                for i in 1..=8 {
                    let t = i as f32 / 8.;
                    self.h.ui.dispatch(InputEvent::PointerMove {
                        x: x0 + (x - x0) * t,
                        y: y0 + (y - y0) * t,
                    });
                    self.h.settle();
                    self.frames(1);
                }
                self.at = (x, y);
            }
            "up" => {
                let (x, y) = self.at;
                self.h.ui.dispatch(InputEvent::PointerUp {
                    x,
                    y,
                    button: PointerButton::Primary,
                });
                self.h.settle();
                self.frames(1);
            }
            "ms" => {
                let ms: usize = arg.parse().unwrap();
                self.frames((ms * 60).div_ceil(1000));
            }
            "type" => {
                for c in arg.chars() {
                    let key = Key::Character(c.to_string());
                    self.h.ui.dispatch(InputEvent::KeyDown {
                        key: key.clone(),
                        modifiers: Modifiers::default(),
                        repeat: false,
                    });
                    self.h.ui.dispatch(InputEvent::Text(c.to_string()));
                    self.h.ui.dispatch(InputEvent::KeyUp {
                        key,
                        modifiers: Modifiers::default(),
                    });
                    self.h.settle();
                    self.frames(1);
                }
            }
            "key" => {
                let key = match arg {
                    "Enter" => Key::Enter,
                    "Escape" => Key::Escape,
                    "Tab" => Key::Tab,
                    "ArrowDown" => Key::ArrowDown,
                    "ArrowUp" => Key::ArrowUp,
                    "Backspace" => Key::Backspace,
                    other => Key::Character(other.to_string()),
                };
                self.h.ui.dispatch(InputEvent::KeyDown {
                    key: key.clone(),
                    modifiers: Modifiers::default(),
                    repeat: false,
                });
                self.h.ui.dispatch(InputEvent::KeyUp {
                    key,
                    modifiers: Modifiers::default(),
                });
                self.h.settle();
                self.frames(1);
            }
            "bounds" => {
                // bounds:<node id>: the node's box and its descendants', three deep, for layout checks.
                self.h.ui.prepare_frame();
                let id = self
                    .h
                    .find(arg)
                    .unwrap_or_else(|| panic!("no node `{arg}`"));
                let scene = self.h.ui.scene.borrow();
                fn walk(scene: &zgui::scene::Scene, id: zgui::scene::NodeId, depth: usize) {
                    let b = scene.bounds(id);
                    println!(
                        "{}{:?} {} {:.1},{:.1} {:.1}x{:.1}",
                        "  ".repeat(depth),
                        id,
                        match scene.kind(id) {
                            zgui::scene::NodeKind::Container(l) => format!("{l:?}"),
                            zgui::scene::NodeKind::Panel { layout, .. } =>
                                format!("Panel {layout:?}"),
                            zgui::scene::NodeKind::RichText { .. }
                            | zgui::scene::NodeKind::Text { .. } => "Text".into(),
                            _ => "Leaf".into(),
                        },
                        b.x,
                        b.y,
                        b.width,
                        b.height
                    );
                    if depth < 7 {
                        for c in scene.children(id) {
                            walk(scene, *c, depth + 1);
                        }
                    }
                }
                walk(&scene, id, 0);
            }
            other => panic!("unknown step `{other}`"),
        }
        if let Some(e) = self.h.host.error().get() {
            panic!("script error after `{step}`: {e}");
        }
    }
    fn save(&mut self, path: &Path) {
        self.h.ui.prepare_frame();
        let _ = self.h.ui.scene.borrow_mut().flush();
        self.gpu
            .render(
                &self.h.ui.scene.borrow(),
                &[zgui::scene::Rect::new(0., 0., self.w, self.hgt)],
            )
            .unwrap();
        let pixels = self.gpu.readback().unwrap();
        let (w, h) = ((self.w * SCALE) as usize, (self.hgt * SCALE) as usize);
        let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
        for rgba in pixels.chunks(4).take(w * h) {
            out.extend_from_slice(&rgba[..3]);
        }
        std::fs::write(path, out).unwrap();
    }
}

#[test]
fn ask_ui_snapshots() {
    let (Some(script), Some(dir)) = (
        std::env::var_os("ASK_UI_SCRIPT").map(PathBuf::from),
        std::env::var_os("ASK_UI_SNAPSHOTS").map(PathBuf::from),
    ) else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let shots = std::env::var("ASK_UI_SHOTS").unwrap_or_else(|_| "rest=ms:300".into());
    for shot in shots.split(';').filter(|s| !s.trim().is_empty()) {
        let (name, steps) = shot.split_once('=').unwrap_or((shot, ""));
        let steps: Vec<&str> = steps.split(',').filter(|s| !s.is_empty()).collect();
        let (mut w, mut h) = (1440., 900.);
        for s in &steps {
            if let Some(size) = s.strip_prefix("size:") {
                let (a, b) = size.split_once('x').unwrap();
                (w, h) = (a.parse().unwrap(), b.parse().unwrap());
            }
        }
        let mut s = Shot::new(&script, w, h);
        if let Some(e) = s.h.host.error().get() {
            panic!("script error on load: {e}");
        }
        let t = std::time::Instant::now();
        for step in steps.iter().filter(|s| !s.starts_with("size:")) {
            let at = std::time::Instant::now();
            s.step(step);
            if std::env::var_os("ASK_UI_TIMES").is_some() {
                println!("  {step}: {:.1} ms", at.elapsed().as_secs_f64() * 1000.);
            }
        }
        s.save(&dir.join(format!("{}.ppm", name.trim())));
        println!("{name}: {:?}", t.elapsed());
    }
}
