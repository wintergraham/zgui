//! Checks the zgx patch to cosmic-text (see ../ZGX_PATCH.md) against
//! upstream 0.19.0. `cargo run --release` lays out thousands of random lines
//! with both at several widths and wrap modes and requires identical glyphs,
//! upstream shaping every run afresh (no word cache: the patched one, kept
//! however long, must not change what is drawn);
//! `cargo run --release -- bench` times shaping a streaming transcript, as
//! zgx's lab workload does, with each.
//!
//!     cargo run --release --manifest-path vendor/cosmic-text/check/Cargo.toml
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    time::{Duration, Instant},
};

/// Every laid-out glyph of `text`, as comparable lines.
macro_rules! layout {
    ($ct:ident, $fonts:expr, $text:expr, $width:expr, $wrap:expr) => {{
        use $ct::*;
        let wrap = [Wrap::None, Wrap::Word, Wrap::Glyph, Wrap::WordOrGlyph][$wrap];
        let mut buffer = Buffer::new($fonts, Metrics::new(14., 18.));
        buffer.set_wrap(wrap);
        buffer.set_size(Some($width), None);
        let attrs = Attrs::new().family(Family::SansSerif);
        buffer.set_text($text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll($fonts, false);
        let mut out = Vec::new();
        for run in buffer.layout_runs() {
            out.push(format!("line {} rtl {} y {} w {}", run.line_i, run.rtl, run.line_y, run.line_w));
            for g in run.glyphs {
                out.push(format!(
                    "{} {}..{} x {} y {} w {} level {} font {:?}",
                    g.glyph_id, g.start, g.end, g.x, g.y, g.w, g.level.number(), g.font_id
                ));
            }
        }
        out
    }};
}

/// `layout!` for `text` in up to four spans of different attributes (a
/// weight, a family, metadata, a size), cut at `cuts` (character counts),
/// as a rich text's runs are.
macro_rules! layout_rich {
    ($ct:ident, $fonts:expr, $text:expr, $cuts:expr, $width:expr, $wrap:expr) => {{
        use $ct::*;
        let wrap = [Wrap::None, Wrap::Word, Wrap::Glyph, Wrap::WordOrGlyph][$wrap];
        let mut buffer = Buffer::new($fonts, Metrics::new(14., 18.));
        buffer.set_wrap(wrap);
        buffer.set_size(Some($width), None);
        let base = Attrs::new().family(Family::SansSerif);
        let kinds = [
            base.clone().metadata(1),
            base.clone().weight(Weight::BOLD).metadata(2),
            base.clone().family(Family::Monospace).metadata(3),
            base.clone().metrics(Metrics::new(17., 21.)).metadata(4),
        ];
        let text: &str = $text;
        let bounds: Vec<usize> = text.char_indices().map(|(i, _)| i).chain([text.len()]).collect();
        let mut spans = Vec::new();
        let mut at = 0;
        for (k, cut) in $cuts.iter().enumerate() {
            let end = bounds[(*cut).min(bounds.len() - 1)].max(at);
            if end > at {
                spans.push((&text[at..end], kinds[k % kinds.len()].clone()));
            }
            at = end;
        }
        if at < text.len() {
            spans.push((&text[at..], base.clone()));
        }
        buffer.set_rich_text(spans, &base, Shaping::Advanced, None);
        buffer.shape_until_scroll($fonts, false);
        let mut out = Vec::new();
        for run in buffer.layout_runs() {
            out.push(format!("line {} rtl {} y {} w {}", run.line_i, run.rtl, run.line_y, run.line_w));
            for g in run.glyphs {
                out.push(format!(
                    "{} {}..{} x {} y {} w {} level {} font {:?} meta {}",
                    g.glyph_id, g.start, g.end, g.x, g.y, g.w, g.level.number(), g.font_id, g.metadata
                ));
            }
        }
        out
    }};
}

/// Shaping time for a transcript growing by a token a tick, its last seven
/// 105-byte lines reshaped every tick.
macro_rules! bench {
    ($ct:ident, $ticks:expr $(, $cache:ident)?) => {{
        use $ct::*;
        const TOKEN: &str = "The quick brown fox streams a token. ";
        let mut fonts = FontSystem::new();
        let attrs = Attrs::new().family(Family::SansSerif);
        let mut text = String::new();
        let mut shaping = Duration::ZERO;
        for tick in 0..$ticks {
            text.push_str(&format!("{tick:06} {TOKEN}"));
            let bytes = text.as_bytes();
            let window = &bytes[bytes.len().saturating_sub(735)..];
            let lines: Vec<&str> = window
                .chunks(105)
                .take(7)
                .map(|c| std::str::from_utf8(c).unwrap())
                .collect();
            let started = Instant::now();
            for line in lines {
                let attrs = AttrsList::new(&attrs);
                let mut line = BufferLine::new(line, LineEnding::None, attrs, Shaping::Advanced);
                line.shape(&mut fonts, 8);
            }
            shaping += started.elapsed();
            $(fonts.$cache.trim(2);)?
        }
        shaping.as_secs_f64() * 1e6 / $ticks as f64
    }};
}

/// Shaping time for a line that needs font fallback (CJK, emoji and
/// codepoints no installed font holds) — every line unique so the patched
/// word cache cannot serve it and the fallback iterator's scan runs.
macro_rules! fbench {
    ($ct:ident) => {{
        use $ct::*;
        let mut fonts = FontSystem::new();
        let attrs = Attrs::new().family(Family::SansSerif);
        let mut shaping = Duration::ZERO;
        for tick in 0..20_000u64 {
            let line = format!(
                "{tick:05} 漢{tick:05}字 ひら{tick:03}な カタ{tick:04}ナ 😀한{tick:03}어 \u{10fffd}{tick:04}\u{e0001}"
            );
            let started = Instant::now();
            let mut line = BufferLine::new(
                &line,
                LineEnding::None,
                AttrsList::new(&attrs),
                Shaping::Advanced,
            );
            line.shape(&mut fonts, 8);
            shaping += started.elapsed();
        }
        shaping.as_secs_f64() * 1e6 / 20_000.
    }};
}

fn next(seed: &mut u64) -> u64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("bench") {
        // `bench patched`: only the patch, long enough to profile.
        if std::env::args().nth(2).as_deref() == Some("patched") {
            println!(
                "shaping per tick: patched {:.2} µs",
                bench!(patched, 400_000, shape_run_cache)
            );
            return;
        }
        for _ in 0..3 {
            let upstream = bench!(upstream, 50_000);
            let patched = bench!(patched, 50_000, shape_run_cache);
            println!("shaping per tick: upstream (no word cache) {upstream:.2} µs, patched {patched:.2} µs");
        }
        return;
    }
    if std::env::args().nth(1).as_deref() == Some("scan") {
        // The fallback iterator alone: a word of many scripts, most of
        // whose fallback families no installed font holds, so their
        // membership tests scan every match key — the test the zgx family
        // index replaces.
        use patched::*;
        use unicode_script::Script::*;
        let mut fonts = FontSystem::new();
        let attrs = Attrs::new().family(Family::SansSerif);
        let keys = fonts.get_font_matches(&attrs);
        let default_families = [&attrs.family];
        let scripts = [
            Adlam, Armenian, Buhid, Canadian_Aboriginal, Chakma, Devanagari, Ethiopic, Gothic,
            Grantha, Hanunoo, Javanese, Kannada, Khmer, Malayalam, Mongolian, Myanmar, Oriya,
            Sinhala, Syriac, Tagalog, Tagbanwa, Tai_Le, Tai_Tham, Tai_Viet, Tamil, Telugu, Thaana,
            Tifinagh, Vai, Yi,
        ];
        let mut scan = Duration::ZERO;
        for _ in 0..2000 {
            let mut iter = patched::fallback::FontFallbackIter::new(
                &mut fonts,
                &keys,
                &default_families,
                &scripts,
                "x",
                attrs.weight,
                0,
            );
            let started = Instant::now();
            // A handful of `next`s: the first walks the default family,
            // the next few walk every script and common family — where
            // missing families mean a pass over every match key each.
            for _ in 0..8 {
                iter.next();
            }
            scan += started.elapsed();
        }
        println!(
            "fallback iteration, first 8 next()s: {:.2} µs",
            scan.as_secs_f64() * 1e6 / 2000.
        );
        return;
    }
    if std::env::args().nth(1).as_deref() == Some("fallback") {
        for _ in 0..3 {
            let upstream = fbench!(upstream);
            let patched = fbench!(patched);
            println!("fallback shaping per line: upstream {upstream:.2} µs, patched {patched:.2} µs");
        }
        return;
    }
    // ASCII including controls and paragraph separators, and lines mixing in
    // accents, CJK, right-to-left scripts and bidi controls.
    let ascii: Vec<char> = (0u8..128).map(char::from).filter(|c| *c != '\n').collect();
    let other = ["é", "ü", "日本", "שלום", "مرحبا", "\u{200f}", "\u{202e}", "\u{2029}", "ﬁ", "\u{85}"];
    let mut seed = 0x9e37_79b9_7f4a_7c15;
    let mut texts: Vec<String> = ["", " ", "\t", "\n", "a\n", "\r\n", "12 34", "  lead", "trail  ", "x\u{1c}y", "fi ffl"]
        .map(String::from)
        .into();
    for _ in 0..4000 {
        let mixed = next(&mut seed) % 4 == 0;
        let text = (0..next(&mut seed) % 200)
            .map(|_| match mixed && next(&mut seed) % 10 == 0 {
                true => other[(next(&mut seed) % other.len() as u64) as usize].to_string(),
                false => ascii[(next(&mut seed) % ascii.len() as u64) as usize].to_string(),
            })
            .collect();
        texts.push(text);
    }
    let mut up = upstream::FontSystem::new();
    let mut pa = patched::FontSystem::new();
    std::panic::set_hook(Box::new(|_| {}));
    let (mut identical, mut rich, mut both_panicked) = (0, 0, 0);
    for (i, text) in texts.iter().enumerate() {
        for (width, wrap) in [(1e6, 0), (120., 1), (60., 2), (240., 3)] {
            let a = catch_unwind(AssertUnwindSafe(|| layout!(upstream, &mut up, text, width, wrap)));
            let b = catch_unwind(AssertUnwindSafe(|| layout!(patched, &mut pa, text, width, wrap)));
            match (a, b) {
                (Ok(a), Ok(b)) if a == b => identical += 1,
                // Upstream panics on paragraphs of mixed direction in one line.
                (Err(_), Err(_)) => {
                    both_panicked += 1;
                    up = upstream::FontSystem::new();
                    pa = patched::FontSystem::new();
                }
                (a, b) => {
                    let _ = std::panic::take_hook();
                    let first = match (&a, &b) {
                        (Ok(a), Ok(b)) => a.iter().zip(b).find(|(x, y)| x != y).map(|(x, y)| format!("{x}\n  {y}")),
                        _ => None,
                    };
                    panic!("text {i} {text:?} (wrap {wrap}) differs: {:?}", first.unwrap_or_else(|| format!("upstream panicked {}, patched panicked {}", a.is_err(), b.is_err())));
                }
            }
        }
        // The same text in spans of different attributes (a rich text's
        // runs), cut at random places.
        let chars = text.chars().count().max(1) as u64;
        let cuts: Vec<usize> = {
            let mut cuts: Vec<usize> = (0..3).map(|_| (next(&mut seed) % (chars + 1)) as usize).collect();
            cuts.sort();
            cuts
        };
        for (width, wrap) in [(1e6, 0), (120., 1), (240., 3)] {
            let a = catch_unwind(AssertUnwindSafe(|| layout_rich!(upstream, &mut up, text, cuts, width, wrap)));
            let b = catch_unwind(AssertUnwindSafe(|| layout_rich!(patched, &mut pa, text, cuts, width, wrap)));
            match (a, b) {
                (Ok(a), Ok(b)) if a == b => rich += 1,
                (Err(_), Err(_)) => {
                    both_panicked += 1;
                    up = upstream::FontSystem::new();
                    pa = patched::FontSystem::new();
                }
                (a, b) => {
                    let _ = std::panic::take_hook();
                    let first = match (&a, &b) {
                        (Ok(a), Ok(b)) => a.iter().zip(b).find(|(x, y)| x != y).map(|(x, y)| format!("{x}\n  {y}")),
                        _ => None,
                    };
                    panic!("rich text {i} {text:?} cut {cuts:?} (wrap {wrap}) differs: {:?}", first.unwrap_or_else(|| format!("upstream panicked {}, patched panicked {}", a.is_err(), b.is_err())));
                }
            }
        }
        if i % 4 == 3 {
            pa.shape_run_cache.trim(1);
        }
    }
    println!("{identical} identical layouts, {rich} of rich text; both panicked on {both_panicked}");
}
