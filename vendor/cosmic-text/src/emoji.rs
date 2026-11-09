// SPDX-License-Identifier: MIT OR Apache-2.0

//! zgx patch: which graphemes draw as emoji, so they take the platform's
//! colour face first, whatever the text's own font covers ("1️⃣" and "❤️"
//! have digits and hearts in ordinary fonts).

/// Colour emoji faces, the platform's first. The first one installed is used.
pub(crate) const EMOJI_FAMILIES: [&str; 4] = [
    "Apple Color Emoji",
    "Segoe UI Emoji",
    "Noto Color Emoji",
    "Twemoji Mozilla",
];

/// Characters at or above U+1F000 with the Emoji property (Unicode 16),
/// which every platform draws in colour with or without U+FE0F.
const SUPPLEMENTARY: [(u32, u32); 51] = [
    (0x1F004, 0x1F004),
    (0x1F0CF, 0x1F0CF),
    (0x1F170, 0x1F171),
    (0x1F17E, 0x1F17F),
    (0x1F18E, 0x1F18E),
    (0x1F191, 0x1F19A),
    (0x1F1E6, 0x1F1FF),
    (0x1F201, 0x1F202),
    (0x1F21A, 0x1F21A),
    (0x1F22F, 0x1F22F),
    (0x1F232, 0x1F23A),
    (0x1F250, 0x1F251),
    (0x1F300, 0x1F321),
    (0x1F324, 0x1F393),
    (0x1F396, 0x1F397),
    (0x1F399, 0x1F39B),
    (0x1F39E, 0x1F3F0),
    (0x1F3F3, 0x1F3F5),
    (0x1F3F7, 0x1F4FD),
    (0x1F4FF, 0x1F53D),
    (0x1F549, 0x1F54E),
    (0x1F550, 0x1F567),
    (0x1F56F, 0x1F570),
    (0x1F573, 0x1F57A),
    (0x1F587, 0x1F587),
    (0x1F58A, 0x1F58D),
    (0x1F590, 0x1F590),
    (0x1F595, 0x1F596),
    (0x1F5A4, 0x1F5A5),
    (0x1F5A8, 0x1F5A8),
    (0x1F5B1, 0x1F5B2),
    (0x1F5BC, 0x1F5BC),
    (0x1F5C2, 0x1F5C4),
    (0x1F5D1, 0x1F5D3),
    (0x1F5DC, 0x1F5DE),
    (0x1F5E1, 0x1F5E1),
    (0x1F5E3, 0x1F5E3),
    (0x1F5E8, 0x1F5E8),
    (0x1F5EF, 0x1F5EF),
    (0x1F5F3, 0x1F5F3),
    (0x1F5FA, 0x1F64F),
    (0x1F680, 0x1F6C5),
    (0x1F6CB, 0x1F6D2),
    (0x1F6D5, 0x1F6D7),
    (0x1F6DC, 0x1F6E5),
    (0x1F6E9, 0x1F6E9),
    (0x1F6EB, 0x1F6F0),
    (0x1F6F3, 0x1F6FC),
    (0x1F7E0, 0x1F7F0),
    (0x1F90C, 0x1F9FF),
    (0x1FA70, 0x1FAFF),
];

/// Characters below U+1F000 that are emoji with nothing after them
/// (Emoji_Presentation). The others need U+FE0F.
const BASIC_PRESENTATION: [(u32, u32); 32] = [
    (0x231A, 0x231B),
    (0x23E9, 0x23EC),
    (0x23F0, 0x23F0),
    (0x23F3, 0x23F3),
    (0x25FD, 0x25FE),
    (0x2614, 0x2615),
    (0x2648, 0x2653),
    (0x267F, 0x267F),
    (0x2693, 0x2693),
    (0x26A1, 0x26A1),
    (0x26AA, 0x26AB),
    (0x26BD, 0x26BE),
    (0x26C4, 0x26C5),
    (0x26CE, 0x26CE),
    (0x26D4, 0x26D4),
    (0x26EA, 0x26EA),
    (0x26F2, 0x26F3),
    (0x26F5, 0x26F5),
    (0x26FA, 0x26FA),
    (0x26FD, 0x26FD),
    (0x2705, 0x2705),
    (0x270A, 0x270B),
    (0x2728, 0x2728),
    (0x274C, 0x274C),
    (0x274E, 0x274E),
    (0x2753, 0x2755),
    (0x2757, 0x2757),
    (0x2795, 0x2797),
    (0x27B0, 0x27B0),
    (0x27BF, 0x27BF),
    (0x2B1B, 0x2B1C),
    (0x2B50, 0x2B55),
];

fn in_ranges(ranges: &[(u32, u32)], c: u32) -> bool {
    let at = ranges.partition_point(|&(_, end)| end < c);
    ranges.get(at).is_some_and(|&(start, _)| start <= c)
}

/// Whether a grapheme cluster draws as an emoji: a default emoji, or a
/// character with a sequence after it that asks for one (U+FE0F, a skin tone,
/// a joiner, a keycap's U+20E3). U+FE0E asks for text. Most text leaves at the
/// first character's range.
pub(crate) fn presents_as_emoji(grapheme: &str) -> bool {
    let mut chars = grapheme.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let c = first as u32;
    // Below the first emoji only keycaps, © and ® are: all need U+FE0F.
    if c < 0x203C {
        return matches!(first, '0'..='9' | '#' | '*' | '©' | '®')
            && chars.any(|c| matches!(c, '\u{FE0F}' | '\u{20E3}'));
    }
    // CJK and the rest, but for U+3030, U+303D, U+3297 and U+3299.
    if (0x3300..0x1F000).contains(&c) {
        return false;
    }
    let mut asks = false;
    for c in chars {
        match c {
            '\u{FE0E}' => return false,
            '\u{FE0F}' | '\u{200D}' | '\u{20E3}' | '\u{1F3FB}'..='\u{1F3FF}' => asks = true,
            _ => (),
        }
    }
    asks || if c >= 0x1F000 {
        in_ranges(&SUPPLEMENTARY, c)
    } else {
        in_ranges(&BASIC_PRESENTATION, c)
    }
}

#[cfg(test)]
mod tests {
    use super::presents_as_emoji as emoji;

    #[test]
    fn sequences_and_defaults_are_emoji() {
        for text in [
            "😀", "👍🏽", "👨\u{200D}👩\u{200D}👧\u{200D}👦", "🇬🇧", "1\u{FE0F}\u{20E3}", "#\u{FE0F}\u{20E3}",
            "❤\u{FE0F}", "❤\u{FE0F}\u{200D}🔥", "✌🏽", "⭐", "✅", "©\u{FE0F}",
            "🏴\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}",
        ] {
            assert!(emoji(text), "{text}");
        }
    }

    #[test]
    fn text_is_not() {
        for text in [
            "a", "1", "#", "é", "e\u{301}", "日", "한", "ש", "م", "→", "❤", "©", "™", "❤\u{FE0E}",
            "⭐\u{FE0E}", "\u{1F100}", "\u{1F700}", "क\u{94D}\u{200D}", "",
        ] {
            assert!(!emoji(text), "{text}");
        }
    }

    #[test]
    fn the_tables_are_in_order() {
        for ranges in [&super::SUPPLEMENTARY[..], &super::BASIC_PRESENTATION[..]] {
            assert!(ranges.iter().all(|(start, end)| start <= end));
            assert!(ranges.windows(2).all(|pair| pair[0].1 < pair[1].0));
        }
    }
}
