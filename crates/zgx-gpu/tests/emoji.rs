//! Emoji shape to one glyph of the colour face, with an advance and colours
//! of their own, keep their line where it was, and aren't tinted by the
//! text's colour. Each test passes over a system with no colour emoji face.
use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, SwashCache, SwashContent};
use zgx::{compose::prelude::*, scene::Rect, style::rgb, widgets::Ui};
use zgx_gpu::{GpuRenderer, text::ShapedText};

const FAMILIES: [&str; 3] = ["Apple Color Emoji", "Segoe UI Emoji", "Noto Color Emoji"];
/// A joined family, a skin tone, a flag, a keycap, a heart asked for with
/// U+FE0F (ordinary fonts have the heart), and a plain default emoji.
const CLUSTERS: [&str; 7] = [
    "👨\u{200D}👩\u{200D}👧\u{200D}👦",
    "👍🏽",
    "🇬🇧",
    "1\u{FE0F}\u{20E3}",
    "❤\u{FE0F}",
    "❤\u{FE0F}\u{200D}🔥",
    "😀",
];

fn emoji_face(fonts: &FontSystem) -> Option<cosmic_text::fontdb::ID> {
    FAMILIES.iter().find_map(|name| {
        fonts
            .db()
            .faces()
            .find(|face| face.families.iter().any(|(family, _)| family == name))
            .map(|face| face.id)
    })
}

fn shaped(fonts: &mut FontSystem, text: &str, size: f32) -> Buffer {
    let mut buffer = Buffer::new_empty(Metrics::new(size, size * 1.4));
    buffer.set_text(text, &Attrs::new(), Shaping::Advanced, None);
    buffer.shape_until_scroll(fonts, false);
    buffer
}

#[test]
fn a_cluster_is_one_colour_glyph_with_an_advance() {
    let mut fonts = FontSystem::new();
    let Some(face) = emoji_face(&fonts) else {
        return;
    };
    let mut images = SwashCache::new();
    for text in CLUSTERS {
        let buffer = shaped(&mut fonts, text, 14.);
        let run = buffer.layout_runs().next().unwrap();
        for g in run.glyphs {
            eprintln!("  {text:?}: glyph {} bytes {}..{} advance {}", g.glyph_id, g.start, g.end, g.w);
        }
        // One glyph drawn; a joiner or selector the face hides is a glyph of no width in the same cluster.
        let drawn: Vec<_> = run.glyphs.iter().filter(|g| g.w > 0.).collect();
        assert_eq!(drawn.len(), 1, "{text:?} shapes to one glyph");
        assert!(run.glyphs.iter().all(|g| g.font_id == face && g.start == 0 && g.end == text.len()), "{text:?} is one cluster");
        let glyph = drawn[0];
        assert_eq!(glyph.font_id, face, "{text:?} in the colour face");
        assert!(glyph.w > 10. && glyph.w < 30., "{text:?} advance {}", glyph.w);
        let key = glyph.physical((0., 0.), 2.).cache_key;
        let image = images.get_image_uncached(&mut fonts, key).unwrap();
        assert_eq!(image.content, SwashContent::Color, "{text:?}");
        let p = image.placement;
        assert!(p.width > 0 && p.height > 0, "{text:?}");
        eprintln!(
            "{text}: advance {} placement {}x{} left {} top {}",
            glyph.w, p.width, p.height, p.left, p.top
        );
        // On the baseline at 28 px: it stays within the line, not above it.
        assert!(p.top > 14 && p.top <= 34, "{text:?} top {}", p.top);
    }
}

#[test]
fn emoji_beside_text_and_keycaps_side_by_side_stay_apart() {
    let mut fonts = FontSystem::new();
    let Some(face) = emoji_face(&fonts) else {
        return;
    };
    let buffer = shaped(&mut fonts, "1\u{FE0F}\u{20E3}2\u{FE0F}\u{20E3}3", 14.);
    let run = buffer.layout_runs().next().unwrap();
    let in_face: Vec<_> = run.glyphs.iter().filter(|g| g.w > 0.).map(|g| (g.start, g.font_id == face)).collect();
    assert_eq!(in_face, [(0, true), (7, true), (14, false)]);
    // A plain digit and heart stay text: only U+FE0F asks for the emoji.
    let buffer = shaped(&mut fonts, "1", 14.);
    assert_ne!(buffer.layout_runs().next().unwrap().glyphs[0].font_id, face);
    let buffer = shaped(&mut fonts, "❤\u{FE0E}", 14.);
    assert_ne!(buffer.layout_runs().next().unwrap().glyphs[0].font_id, face);
}

#[test]
fn an_emoji_moves_neither_baseline_nor_line_height() {
    let mut fonts = FontSystem::new();
    if emoji_face(&fonts).is_none() {
        return;
    }
    let line = |fonts: &mut FontSystem, text: &str| {
        let buffer = shaped(fonts, text, 14.);
        let run = buffer.layout_runs().next().unwrap();
        (run.line_y, run.line_height)
    };
    let plain = line(&mut fonts, "Hello");
    assert_eq!(line(&mut fonts, "Hello 😀"), plain);
    assert_eq!(line(&mut fonts, "😀"), plain, "alone on its line too");
}

#[test]
fn carets_and_selections_take_a_cluster_whole() {
    let mut fonts = FontSystem::new();
    if emoji_face(&fonts).is_none() {
        return;
    }
    let family = "👨\u{200D}👩\u{200D}👧\u{200D}👦";
    let text = format!("a{family}b");
    let shaped = ShapedText::new(&mut fonts, text.as_str(), 14., None);
    let (_, height) = shaped.size();
    let (width, _) = shaped.size();
    for step in 0..=40 {
        let x = width * step as f32 / 40.;
        let at = shaped.hit_test(x, height / 2.).unwrap().byte_index;
        assert!(
            [0, 1, 1 + family.len(), text.len()].contains(&at),
            "a hit lands on a cluster's edge, not at byte {at}"
        );
    }
    let highlight = shaped.selection(1..1 + family.len());
    let width: f32 = highlight.iter().map(|rect| rect.width).sum();
    assert!(width > 10., "its selection has its width, {width}");

    let mut editor = zgx::text_edit::TextEditor::new(format!("{family}🇬🇧1\u{FE0F}\u{20E3}👍🏽"));
    for left in ["👨\u{200D}👩\u{200D}👧\u{200D}👦🇬🇧1\u{FE0F}\u{20E3}", "👨\u{200D}👩\u{200D}👧\u{200D}👦🇬🇧", family, ""] {
        assert!(editor.backspace());
        assert_eq!(editor.text(), left, "backspace takes one cluster");
    }
}

/// How many pixels are far from grey: colours of an emoji's own.
fn colourful(pixels: &[u8]) -> usize {
    pixels
        .chunks_exact(4)
        .filter(|p| {
            let (lo, hi) = (p[0].min(p[1]).min(p[2]), p[0].max(p[1]).max(p[2]));
            hi - lo > 80
        })
        .count()
}

fn draw(gpu: &mut GpuRenderer, ink: u32, label: &str) -> Vec<u8> {
    let mut ui = Ui::new(160., 60.);
    gpu.install_text(&mut ui.scene.borrow_mut());
    ui.refresh_text_geometry();
    ui.mount(
        column()
            .size(160., 60.)
            .p(8.)
            .bg(rgb(0xffffff))
            .text_color(rgb(ink))
            .text_size(24.)
            .child(text(label.to_owned())),
    );
    ui.prepare_frame();
    let mut scene = ui.scene.borrow_mut();
    scene.flush();
    gpu.render(&scene, &[Rect::new(0., 0., 160., 60.)]).unwrap();
    gpu.readback().unwrap()
}

#[test]
fn emoji_keep_their_colours_whatever_the_text_colour() {
    if emoji_face(&FontSystem::new()).is_none() {
        return;
    }
    let mut gpu = GpuRenderer::new(160, 60).unwrap();
    let black = draw(&mut gpu, 0x000000, "😀");
    let blue = draw(&mut gpu, 0x0000ff, "😀");
    assert!(colourful(&black) > 50, "black text doesn't blacken an emoji");
    assert_eq!(black, blue, "nor does the text's colour change it");
    // Beside it, ordinary glyphs in one text still take the text's colour.
    let mut gpu = GpuRenderer::new(160, 60).unwrap();
    let black_letters = draw(&mut gpu, 0x000000, "iiii");
    assert_eq!(colourful(&black_letters), 0);
    let blue_letters = draw(&mut gpu, 0x0000ff, "iiii");
    assert!(colourful(&blue_letters) > 0);
    let mixed_black = draw(&mut gpu, 0x000000, "ii😀");
    let mixed_blue = draw(&mut gpu, 0x0000ff, "ii😀");
    assert!(colourful(&mixed_black) > 50);
    assert_ne!(mixed_black, mixed_blue, "the letters follow the colour");
}
