//! Names people give themselves, their groups, voice rooms and files: what of them shows.
//!
//! Some characters draw as nothing or as blank space: the many kinds of space, fillers (Hangul
//! fillers, the braille blank), marks and joiners with nothing to sit on, variation selectors
//! and tag characters out of place, and format characters that never draw. A name made of them
//! alone looks empty, and one padded with them can pass for another. [`tidy`] keeps what
//! shows. Control characters and marks that hide in text or turn it around are left as they
//! are: whoever checks a name refuses those (see [`hiding_mark`]).

/// A mark that hides in text or turns it around (direction overrides and isolates,
/// zero-width spaces), as in `photo\u{202E}gnp.exe`, which shows as `photoexe.png`.
pub fn hiding_mark(c: char) -> bool {
    matches!(
        c,
        '\u{061C}'
            | '\u{200B}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{2069}'
            | '\u{FEFF}'
    )
}

/// Whether `c` draws as blank space: a space of any width (not a control character such as
/// a line break) or a character made to stand for nothing, like the Hangul fillers and the
/// braille blank.
pub fn blank(c: char) -> bool {
    (c.is_whitespace() && !c.is_control())
        || matches!(
            c,
            '\u{115F}'
                | '\u{1160}'
                | '\u{180E}'
                | '\u{2800}'
                | '\u{3164}'
                | '\u{FFA0}'
                | '\u{1D159}'
        )
}

/// Characters that never draw wherever they are: the soft hyphen, the Khmer inherent vowels,
/// deprecated format characters, interlinear annotations, shorthand and musical format
/// characters, the language tag and code points kept for more of the same.
fn unseen(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{17B4}'
            | '\u{17B5}'
            | '\u{2065}'
            | '\u{206A}'..='\u{206F}'
            | '\u{FFF0}'..='\u{FFFB}'
            | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{1D173}'..='\u{1D17A}'
            | '\u{E0000}'..='\u{E001F}'
            | '\u{E0080}'..='\u{E00FF}'
            | '\u{E01F0}'..='\u{E0FFF}'
    )
}

/// Zero-width joiner and non-joiner: they shape the letters on both sides of them (emoji
/// sequences, Indic and Persian words) and show nothing anywhere else.
fn joiner(c: char) -> bool {
    matches!(c, '\u{200C}' | '\u{200D}')
}

/// Variation selectors: they pick how the character before them looks.
fn selector(c: char) -> bool {
    matches!(c, '\u{FE00}'..='\u{FE0F}' | '\u{E0100}'..='\u{E01EF}')
}

/// Tag characters: they spell out which flag a black flag is (as in the flag of Scotland).
fn tag(c: char) -> bool {
    matches!(c, '\u{E0020}'..='\u{E007F}')
}

const BLACK_FLAG: char = '\u{1F3F4}';

/// Whether `c` is a combining mark (Unicode general category M): it draws on the character
/// before it.
pub fn combining(c: char) -> bool {
    let c = u32::from(c);
    if c < MARKS[0] {
        return false;
    }
    let pairs = MARKS.as_chunks::<2>().0;
    match pairs.binary_search_by(|[start, _]| start.cmp(&c)) {
        Ok(_) => true,
        Err(i) => pairs[i - 1][1] >= c,
    }
}

/// What shows of `name`: blank characters become one space between words and none at either
/// end; marks, joiners, variation selectors and tag characters with nothing to sit on, and
/// characters that never draw, are left out. Empty when nothing shows.
pub fn tidy(name: &str) -> String {
    let mut out: Vec<char> = Vec::with_capacity(name.len());
    // A space is written only once something shows after it.
    let mut space = false;
    // The last character written is one a variation selector may follow.
    let mut selectable = false;
    // Within a black flag's tag characters.
    let mut tags = false;
    let drop_joiners = |out: &mut Vec<char>| {
        while out.last().copied().is_some_and(joiner) {
            out.pop();
        }
    };
    for c in name.chars() {
        let sits = !space && !out.is_empty();
        if blank(c) {
            space = !out.is_empty();
            selectable = false;
            tags = false;
            drop_joiners(&mut out);
            continue;
        }
        if unseen(c) {
            continue;
        }
        if tag(c) {
            if tags && sits {
                out.push(c);
                tags = c != '\u{E007F}';
            }
            selectable = false;
            continue;
        }
        tags = false;
        if selector(c) {
            if selectable && sits {
                out.push(c);
            }
            selectable = false;
            continue;
        }
        if joiner(c) {
            if sits && !out.last().copied().is_some_and(joiner) {
                out.push(c);
            }
            selectable = false;
            continue;
        }
        if combining(c) {
            if sits {
                out.push(c);
            }
            selectable = false;
            continue;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(c);
        // Text characters of plain ASCII have no variations; keycaps (`1\u{FE0F}\u{20E3}`)
        // start with a digit, `#` or `*`.
        selectable = !c.is_ascii() || c.is_ascii_digit() || matches!(c, '#' | '*');
        tags = c == BLACK_FLAG;
    }
    drop_joiners(&mut out);
    out.into_iter().collect()
}

/// Starts and ends of the combining marks (general category M) of Unicode 16.0, in order.
#[rustfmt::skip]
const MARKS: [u32; 642] = [
    0x0300, 0x036F, 0x0483, 0x0489, 0x0591, 0x05BD, 0x05BF, 0x05BF, 0x05C1, 0x05C2,
    0x05C4, 0x05C5, 0x05C7, 0x05C7, 0x0610, 0x061A, 0x064B, 0x065F, 0x0670, 0x0670,
    0x06D6, 0x06DC, 0x06DF, 0x06E4, 0x06E7, 0x06E8, 0x06EA, 0x06ED, 0x0711, 0x0711,
    0x0730, 0x074A, 0x07A6, 0x07B0, 0x07EB, 0x07F3, 0x07FD, 0x07FD, 0x0816, 0x0819,
    0x081B, 0x0823, 0x0825, 0x0827, 0x0829, 0x082D, 0x0859, 0x085B, 0x0897, 0x089F,
    0x08CA, 0x08E1, 0x08E3, 0x0903, 0x093A, 0x093C, 0x093E, 0x094F, 0x0951, 0x0957,
    0x0962, 0x0963, 0x0981, 0x0983, 0x09BC, 0x09BC, 0x09BE, 0x09C4, 0x09C7, 0x09C8,
    0x09CB, 0x09CD, 0x09D7, 0x09D7, 0x09E2, 0x09E3, 0x09FE, 0x09FE, 0x0A01, 0x0A03,
    0x0A3C, 0x0A3C, 0x0A3E, 0x0A42, 0x0A47, 0x0A48, 0x0A4B, 0x0A4D, 0x0A51, 0x0A51,
    0x0A70, 0x0A71, 0x0A75, 0x0A75, 0x0A81, 0x0A83, 0x0ABC, 0x0ABC, 0x0ABE, 0x0AC5,
    0x0AC7, 0x0AC9, 0x0ACB, 0x0ACD, 0x0AE2, 0x0AE3, 0x0AFA, 0x0AFF, 0x0B01, 0x0B03,
    0x0B3C, 0x0B3C, 0x0B3E, 0x0B44, 0x0B47, 0x0B48, 0x0B4B, 0x0B4D, 0x0B55, 0x0B57,
    0x0B62, 0x0B63, 0x0B82, 0x0B82, 0x0BBE, 0x0BC2, 0x0BC6, 0x0BC8, 0x0BCA, 0x0BCD,
    0x0BD7, 0x0BD7, 0x0C00, 0x0C04, 0x0C3C, 0x0C3C, 0x0C3E, 0x0C44, 0x0C46, 0x0C48,
    0x0C4A, 0x0C4D, 0x0C55, 0x0C56, 0x0C62, 0x0C63, 0x0C81, 0x0C83, 0x0CBC, 0x0CBC,
    0x0CBE, 0x0CC4, 0x0CC6, 0x0CC8, 0x0CCA, 0x0CCD, 0x0CD5, 0x0CD6, 0x0CE2, 0x0CE3,
    0x0CF3, 0x0CF3, 0x0D00, 0x0D03, 0x0D3B, 0x0D3C, 0x0D3E, 0x0D44, 0x0D46, 0x0D48,
    0x0D4A, 0x0D4D, 0x0D57, 0x0D57, 0x0D62, 0x0D63, 0x0D81, 0x0D83, 0x0DCA, 0x0DCA,
    0x0DCF, 0x0DD4, 0x0DD6, 0x0DD6, 0x0DD8, 0x0DDF, 0x0DF2, 0x0DF3, 0x0E31, 0x0E31,
    0x0E34, 0x0E3A, 0x0E47, 0x0E4E, 0x0EB1, 0x0EB1, 0x0EB4, 0x0EBC, 0x0EC8, 0x0ECE,
    0x0F18, 0x0F19, 0x0F35, 0x0F35, 0x0F37, 0x0F37, 0x0F39, 0x0F39, 0x0F3E, 0x0F3F,
    0x0F71, 0x0F84, 0x0F86, 0x0F87, 0x0F8D, 0x0F97, 0x0F99, 0x0FBC, 0x0FC6, 0x0FC6,
    0x102B, 0x103E, 0x1056, 0x1059, 0x105E, 0x1060, 0x1062, 0x1064, 0x1067, 0x106D,
    0x1071, 0x1074, 0x1082, 0x108D, 0x108F, 0x108F, 0x109A, 0x109D, 0x135D, 0x135F,
    0x1712, 0x1715, 0x1732, 0x1734, 0x1752, 0x1753, 0x1772, 0x1773, 0x17B4, 0x17D3,
    0x17DD, 0x17DD, 0x180B, 0x180D, 0x180F, 0x180F, 0x1885, 0x1886, 0x18A9, 0x18A9,
    0x1920, 0x192B, 0x1930, 0x193B, 0x1A17, 0x1A1B, 0x1A55, 0x1A5E, 0x1A60, 0x1A7C,
    0x1A7F, 0x1A7F, 0x1AB0, 0x1ACE, 0x1B00, 0x1B04, 0x1B34, 0x1B44, 0x1B6B, 0x1B73,
    0x1B80, 0x1B82, 0x1BA1, 0x1BAD, 0x1BE6, 0x1BF3, 0x1C24, 0x1C37, 0x1CD0, 0x1CD2,
    0x1CD4, 0x1CE8, 0x1CED, 0x1CED, 0x1CF4, 0x1CF4, 0x1CF7, 0x1CF9, 0x1DC0, 0x1DFF,
    0x20D0, 0x20F0, 0x2CEF, 0x2CF1, 0x2D7F, 0x2D7F, 0x2DE0, 0x2DFF, 0x302A, 0x302F,
    0x3099, 0x309A, 0xA66F, 0xA672, 0xA674, 0xA67D, 0xA69E, 0xA69F, 0xA6F0, 0xA6F1,
    0xA802, 0xA802, 0xA806, 0xA806, 0xA80B, 0xA80B, 0xA823, 0xA827, 0xA82C, 0xA82C,
    0xA880, 0xA881, 0xA8B4, 0xA8C5, 0xA8E0, 0xA8F1, 0xA8FF, 0xA8FF, 0xA926, 0xA92D,
    0xA947, 0xA953, 0xA980, 0xA983, 0xA9B3, 0xA9C0, 0xA9E5, 0xA9E5, 0xAA29, 0xAA36,
    0xAA43, 0xAA43, 0xAA4C, 0xAA4D, 0xAA7B, 0xAA7D, 0xAAB0, 0xAAB0, 0xAAB2, 0xAAB4,
    0xAAB7, 0xAAB8, 0xAABE, 0xAABF, 0xAAC1, 0xAAC1, 0xAAEB, 0xAAEF, 0xAAF5, 0xAAF6,
    0xABE3, 0xABEA, 0xABEC, 0xABED, 0xFB1E, 0xFB1E, 0xFE00, 0xFE0F, 0xFE20, 0xFE2F,
    0x101FD, 0x101FD, 0x102E0, 0x102E0, 0x10376, 0x1037A, 0x10A01, 0x10A03, 0x10A05, 0x10A06,
    0x10A0C, 0x10A0F, 0x10A38, 0x10A3A, 0x10A3F, 0x10A3F, 0x10AE5, 0x10AE6, 0x10D24, 0x10D27,
    0x10D69, 0x10D6D, 0x10EAB, 0x10EAC, 0x10EFC, 0x10EFF, 0x10F46, 0x10F50, 0x10F82, 0x10F85,
    0x11000, 0x11002, 0x11038, 0x11046, 0x11070, 0x11070, 0x11073, 0x11074, 0x1107F, 0x11082,
    0x110B0, 0x110BA, 0x110C2, 0x110C2, 0x11100, 0x11102, 0x11127, 0x11134, 0x11145, 0x11146,
    0x11173, 0x11173, 0x11180, 0x11182, 0x111B3, 0x111C0, 0x111C9, 0x111CC, 0x111CE, 0x111CF,
    0x1122C, 0x11237, 0x1123E, 0x1123E, 0x11241, 0x11241, 0x112DF, 0x112EA, 0x11300, 0x11303,
    0x1133B, 0x1133C, 0x1133E, 0x11344, 0x11347, 0x11348, 0x1134B, 0x1134D, 0x11357, 0x11357,
    0x11362, 0x11363, 0x11366, 0x1136C, 0x11370, 0x11374, 0x113B8, 0x113C0, 0x113C2, 0x113C2,
    0x113C5, 0x113C5, 0x113C7, 0x113CA, 0x113CC, 0x113D0, 0x113D2, 0x113D2, 0x113E1, 0x113E2,
    0x11435, 0x11446, 0x1145E, 0x1145E, 0x114B0, 0x114C3, 0x115AF, 0x115B5, 0x115B8, 0x115C0,
    0x115DC, 0x115DD, 0x11630, 0x11640, 0x116AB, 0x116B7, 0x1171D, 0x1172B, 0x1182C, 0x1183A,
    0x11930, 0x11935, 0x11937, 0x11938, 0x1193B, 0x1193E, 0x11940, 0x11940, 0x11942, 0x11943,
    0x119D1, 0x119D7, 0x119DA, 0x119E0, 0x119E4, 0x119E4, 0x11A01, 0x11A0A, 0x11A33, 0x11A39,
    0x11A3B, 0x11A3E, 0x11A47, 0x11A47, 0x11A51, 0x11A5B, 0x11A8A, 0x11A99, 0x11C2F, 0x11C36,
    0x11C38, 0x11C3F, 0x11C92, 0x11CA7, 0x11CA9, 0x11CB6, 0x11D31, 0x11D36, 0x11D3A, 0x11D3A,
    0x11D3C, 0x11D3D, 0x11D3F, 0x11D45, 0x11D47, 0x11D47, 0x11D8A, 0x11D8E, 0x11D90, 0x11D91,
    0x11D93, 0x11D97, 0x11EF3, 0x11EF6, 0x11F00, 0x11F01, 0x11F03, 0x11F03, 0x11F34, 0x11F3A,
    0x11F3E, 0x11F42, 0x11F5A, 0x11F5A, 0x13440, 0x13440, 0x13447, 0x13455, 0x1611E, 0x1612F,
    0x16AF0, 0x16AF4, 0x16B30, 0x16B36, 0x16F4F, 0x16F4F, 0x16F51, 0x16F87, 0x16F8F, 0x16F92,
    0x16FE4, 0x16FE4, 0x16FF0, 0x16FF1, 0x1BC9D, 0x1BC9E, 0x1CF00, 0x1CF2D, 0x1CF30, 0x1CF46,
    0x1D165, 0x1D169, 0x1D16D, 0x1D172, 0x1D17B, 0x1D182, 0x1D185, 0x1D18B, 0x1D1AA, 0x1D1AD,
    0x1D242, 0x1D244, 0x1DA00, 0x1DA36, 0x1DA3B, 0x1DA6C, 0x1DA75, 0x1DA75, 0x1DA84, 0x1DA84,
    0x1DA9B, 0x1DA9F, 0x1DAA1, 0x1DAAF, 0x1E000, 0x1E006, 0x1E008, 0x1E018, 0x1E01B, 0x1E021,
    0x1E023, 0x1E024, 0x1E026, 0x1E02A, 0x1E08F, 0x1E08F, 0x1E130, 0x1E136, 0x1E2AE, 0x1E2AE,
    0x1E2EC, 0x1E2EF, 0x1E4EC, 0x1E4EF, 0x1E5EE, 0x1E5EF, 0x1E8D0, 0x1E8D6, 0x1E944, 0x1E94A,
    0xE0100, 0xE01EF,
];

/// Chat text as it shows: control characters other than line breaks and tabs, and marks
/// that hide in text or turn it around, left out (they draw as boxes, or pad words so that a
/// search misses them); line and paragraph separators become line breaks.
pub fn shown_text(text: &str) -> String {
    text.chars()
        .filter_map(|c| match c {
            '\n' | '\t' => Some(c),
            '\u{2028}' | '\u{2029}' => Some('\n'),
            c if c.is_control() || hiding_mark(c) => None,
            c => Some(c),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fillers_and_spaces_alone_leave_nothing() {
        for blank in [
            "\u{3164}",
            "\u{115F}\u{1160}",
            "\u{FFA0}\u{FFA0}",
            "\u{2800}",
            "\u{180E}",
            "\u{1D159}",
            "\u{00A0}",
            "\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200A}",
            "\u{202F}\u{205F}\u{3000}",
            "  \u{3164} \u{2800} ",
        ] {
            assert_eq!(tidy(blank), "", "{blank:?}");
        }
    }

    #[test]
    fn padding_is_trimmed_and_runs_collapse() {
        assert_eq!(tidy("Ana\u{3164}\u{3164}\u{3164}"), "Ana");
        assert_eq!(tidy("\u{2800}\u{2800}Ana"), "Ana");
        assert_eq!(tidy("  Ana \u{3000}\u{00A0} Bea  "), "Ana Bea");
        assert_eq!(tidy("Ana\u{115F}Bea"), "Ana Bea");
        assert_eq!(tidy("Ana\u{180E}\u{FFA0}Bea"), "Ana Bea");
    }

    #[test]
    fn marks_with_nothing_to_sit_on_are_left_out() {
        assert_eq!(tidy("\u{0301}\u{0301}\u{0301}"), "");
        assert_eq!(tidy("\u{20E3}"), "");
        assert_eq!(tidy("\u{0301}Ana"), "Ana");
        assert_eq!(tidy("Ana \u{0301}\u{0308}Bea"), "Ana Bea");
        assert_eq!(tidy("Jose\u{0301}"), "Jose\u{0301}");
        assert_eq!(tidy("\u{034F}"), "");
    }

    #[test]
    fn selectors_and_tags_out_of_place_are_left_out() {
        assert_eq!(tidy("\u{FE0F}\u{FE0E}"), "");
        assert_eq!(tidy("\u{E0100}"), "");
        assert_eq!(tidy("Ana\u{FE0F}"), "Ana");
        assert_eq!(tidy("Ana\u{FE0F}\u{FE0F}"), "Ana");
        assert_eq!(tidy("\u{E0041}\u{E0042}\u{E007F}"), "");
        assert_eq!(tidy("Ana\u{E0041}\u{E0042}"), "Ana");
        assert_eq!(tidy("\u{2764}\u{FE0F}\u{FE0F}"), "\u{2764}\u{FE0F}");
    }

    #[test]
    fn characters_that_never_draw_are_left_out() {
        assert_eq!(
            tidy("\u{00AD}\u{17B4}\u{17B5}\u{206A}\u{FFF9}\u{1D173}\u{E0001}"),
            ""
        );
        assert_eq!(tidy("A\u{00AD}na"), "Ana");
        assert_eq!(tidy("\u{1BCA0}Ana\u{E0001}"), "Ana");
    }

    #[test]
    fn joiners_stay_only_between_what_shows() {
        assert_eq!(tidy("\u{200D}\u{200C}"), "");
        assert_eq!(tidy("Ana\u{200D}"), "Ana");
        assert_eq!(tidy("\u{200C}Ana"), "Ana");
        assert_eq!(tidy("Ana\u{200D} Bea"), "Ana Bea");
        assert_eq!(tidy("a\u{200D}\u{200D}\u{200D}b"), "a\u{200D}b");
    }

    #[test]
    fn real_names_in_any_script_are_kept() {
        for name in [
            "Marta",
            "Sofía",
            "김민지",
            "이 서연",
            "山田 太郎",
            "مريم",
            "می\u{200C}خواهم",
            "नमस्ते",
            "क्\u{200D}षमा",
            "สวัสดี",
            "Zoë",
            "e\u{0301}",
            "👨\u{200D}👩\u{200D}👧",
            "👩🏽\u{200D}💻",
            "🏳\u{FE0F}\u{200D}🌈",
            "❤\u{FE0F}\u{200D}🔥",
            "1\u{FE0F}\u{20E3}",
            "#\u{FE0F}\u{20E3}",
            "\u{1F3F4}\u{E0067}\u{E0062}\u{E0073}\u{E0063}\u{E0074}\u{E007F}",
            "葛\u{E0100}",
            "ᄀ\u{1161}\u{11A8}",
        ] {
            assert_eq!(tidy(name), name, "{name:?}");
        }
        assert_eq!(tidy("山田\u{3000}太郎"), "山田 太郎");
    }

    #[test]
    fn controls_and_hiding_marks_are_left_for_the_checks() {
        assert_eq!(tidy("Ana\nBea"), "Ana\nBea");
        assert_eq!(tidy("Ana\u{202E}"), "Ana\u{202E}");
        for mark in ['\u{FEFF}', '\u{200B}', '\u{202E}', '\u{2066}', '\u{061C}'] {
            assert!(hiding_mark(mark));
        }
        assert!(!hiding_mark('\u{200D}'));
    }

    #[test]
    fn combining_marks_are_told_apart() {
        for c in [
            '\u{0300}',
            '\u{036F}',
            '\u{0903}',
            '\u{20E3}',
            '\u{E01EF}',
            '\u{1E94A}',
        ] {
            assert!(combining(c), "{c:?}");
        }
        for c in [
            '\0',
            'a',
            '\u{02FF}',
            '\u{0370}',
            '\u{E01F0}',
            '\u{10FFFF}',
            '😀',
        ] {
            assert!(!combining(c), "{c:?}");
        }
    }

    #[test]
    fn chat_text_drops_what_draws_as_boxes_or_hides() {
        assert_eq!(
            shown_text("pass\u{200B}word\r\n\u{0007}ok\u{2028}\tbye\u{202E}"),
            "password\nok\n\tbye"
        );
        assert_eq!(
            shown_text("hi 👋🏽 \u{1F468}\u{200D}\u{1F469}"),
            "hi 👋🏽 \u{1F468}\u{200D}\u{1F469}"
        );
    }
}
