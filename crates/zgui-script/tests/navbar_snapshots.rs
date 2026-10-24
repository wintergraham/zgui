//! GPU snapshots of the Navbar lab's states, for comparison with the Ask motion lab.
//! Run: NAVBAR_LAB_SNAPSHOTS=/tmp/out cargo test --release -p zgui-script --test navbar_snapshots -- --nocapture
//! Optional NAVBAR_LAB_ONLY=open,ask limits the states.
mod common;
use common::Harness;
use std::path::{Path, PathBuf};
use zgui::input::{InputEvent, Key, Modifiers, PointerButton};
use zgui_gpu::GpuRenderer;
use zgui_script::ScriptHost;

const W: f32 = 1180.;
const H: f32 = 1000.;
const SCALE: f32 = 2.;

struct Shot {
    h: Harness,
    gpu: GpuRenderer,
}
impl Shot {
    fn new() -> Self {
        let mut gpu =
            GpuRenderer::new((W * SCALE) as u32, (H * SCALE) as u32).expect("GPU adapter");
        gpu.set_scale_factor(SCALE);
        for bytes in [
            &include_bytes!("../examples/controls_lab/assets/Geist.ttf")[..],
            &include_bytes!("../examples/controls_lab/assets/GeistMono.ttf")[..],
        ] {
            zgui_gpu::text::FontData::new(bytes)
                .unwrap()
                .install(&mut gpu.text_system().borrow_mut());
        }
        let h = Harness::new(|runtime| {
            ScriptHost::new(
                runtime,
                Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/navbar_lab/app.luau"),
            )
        });
        gpu.install_text(&mut h.ui.scene.borrow_mut());
        h.ui.refresh_text_geometry();
        h.ui.scene.borrow_mut().resize(W, H);
        h.ui.prepare_frame();
        h.settle();
        let shot = Self { h, gpu };
        shot.frames(30);
        shot
    }
    fn frames(&self, n: usize) {
        for _ in 0..n {
            self.h.frame();
        }
    }
    fn ms(&self, ms: u32) {
        self.frames((ms as usize * 60).div_ceil(1000));
    }
    fn center(&mut self, id: &str) -> (f32, f32) {
        let node = self.h.find(id).unwrap_or_else(|| panic!("no node `{id}`"));
        let b = self.h.ui.scene.borrow().bounds(node);
        (b.x + b.width / 2., b.y + b.height / 2.)
    }
    fn click(&mut self, id: &str) {
        let (x, y) = self.center(id);
        self.h.ui.dispatch(InputEvent::PointerMove { x, y });
        self.h.click(id);
        self.frames(1);
    }
    fn press(&mut self, id: &str) {
        let (x, y) = self.center(id);
        self.h.ui.dispatch(InputEvent::PointerMove { x, y });
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
    fn hover(&mut self, id: &str) {
        let (x, y) = self.center(id);
        self.h.ui.dispatch(InputEvent::PointerMove { x, y });
        self.h.settle();
    }
    fn key(&mut self, key: Key, meta: bool) {
        let modifiers = Modifiers {
            meta,
            ..Default::default()
        };
        self.h.ui.dispatch(InputEvent::KeyDown {
            key: key.clone(),
            modifiers,
            repeat: false,
        });
        self.h.ui.dispatch(InputEvent::KeyUp { key, modifiers });
        self.h.settle();
        self.frames(1);
    }
    fn typing(&mut self, text: &str) {
        for c in text.chars() {
            self.key(Key::Character(c.to_string()), false);
            self.h.ui.dispatch(InputEvent::Text(c.to_string()));
            self.h.settle();
            self.frames(1);
        }
    }
    fn command(&mut self, id: &str) {
        self.h.host.run_command(id);
        self.h.settle();
        self.frames(1);
    }
    fn wheel(&mut self, id: &str, dy: f32) {
        let (x, y) = self.center(id);
        self.h.ui.dispatch(InputEvent::Scroll {
            x,
            y: y + 200.,
            delta_x: 0.,
            delta_y: dy,
        });
        self.h.settle();
        self.frames(1);
    }
    fn save(&mut self, dir: &Path, name: &str) {
        assert_eq!(self.h.host.error().get(), None, "{name}");
        let frame = self.h.find("nb_win_frame").unwrap();
        let b = self.h.ui.scene.borrow().bounds(frame);
        let damage = self.h.ui.scene.borrow_mut().flush().damage;
        let _ = damage;
        self.gpu
            .render(
                &self.h.ui.scene.borrow(),
                &[zgui::scene::Rect::new(0., 0., W, H)],
            )
            .unwrap();
        let pixels = self.gpu.readback().unwrap();
        let (x0, y0) = ((b.x * SCALE) as usize, (b.y * SCALE) as usize);
        let (w, h) = ((b.width * SCALE) as usize, (b.height * SCALE) as usize);
        let stride = (W * SCALE) as usize;
        let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                let i = (y * stride + x) * 4;
                out.extend_from_slice(&pixels[i..i + 3]);
            }
        }
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
        println!("{name}: {}", self.h.text("nb_state"));
    }
}

#[test]
fn navbar_lab_snapshots() {
    let Some(dir) = std::env::var_os("NAVBAR_LAB_SNAPSHOTS").map(PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let only: Vec<String> = std::env::var("NAVBAR_LAB_ONLY")
        .map(|s| s.split(',').map(str::to_owned).collect())
        .unwrap_or_default();
    type Step = fn(&mut Shot);
    let states: Vec<(&str, Step)> = vec![
        ("rest", |_| {}),
        ("hover", |s| {
            s.hover("nb_identity");
            s.ms(700);
        }),
        ("open", |s| {
            s.press("nb_identity");
            s.ms(900);
        }),
        ("typed", |s| {
            s.command("focus_address");
            s.ms(500);
            s.typing("gi");
            s.ms(700);
        }),
        ("ask", |s| {
            s.command("focus_address");
            s.ms(500);
            s.key(Key::Tab, false);
            s.ms(800);
        }),
        ("tabs", |s| {
            s.command("focus_address");
            s.ms(400);
            s.typing("@tabs");
            s.key(Key::Tab, false);
            s.ms(800);
        }),
        ("site", |s| {
            s.click("nb_logo");
            s.ms(600);
        }),
        ("sitechoice", |s| {
            s.click("nb_logo");
            s.ms(400);
            s.click("nb_choice_Camera");
            s.ms(400);
        }),
        ("ext", |s| {
            s.click("nb_ext");
            s.ms(600);
        }),
        ("bookmark", |s| {
            s.click("nb_star");
            s.ms(700);
        }),
        ("pill", |s| {
            s.wheel("nb_win_frame", 600.);
            s.ms(100);
            s.wheel("nb_win_frame", 100.);
            s.ms(900);
        }),
        ("loading", |s| {
            s.click("nb_slow");
            s.click("nb_site_3");
            s.ms(2500);
        }),
        ("newtab", |s| {
            s.click("nb_site_1");
            s.ms(1800);
        }),
        ("http", |s| {
            s.click("nb_site_4");
            s.ms(1800);
        }),
        ("internal", |s| {
            s.click("nb_site_5");
            s.ms(1800);
        }),
        ("acting", |s| {
            s.click("nb_acting");
            s.ms(800);
        }),
        ("perm", |s| {
            s.click("nb_perm");
            s.ms(300);
            s.click("nb_perm_btn");
            s.ms(600);
        }),
        ("offline", |s| {
            s.click("nb_offline");
            s.ms(600);
        }),
        ("data", |s| {
            s.click("nb_logo");
            s.ms(300);
            s.click("nb_site_data");
            s.ms(1200);
        }),
    ];
    for (name, step) in states {
        if !only.is_empty() && !only.iter().any(|o| o == name) {
            continue;
        }
        let mut shot = Shot::new();
        step(&mut shot);
        shot.save(&dir, name);
    }
}
