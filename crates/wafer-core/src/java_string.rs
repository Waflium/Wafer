use std::borrow::Cow;

const LONE_SURROGATE_BASE: u32 = 0x10D800;
const LONE_SURROGATE_COUNT: u32 = 0x800;
const WRITE_UTF_UNKNOWN: u8 = b'?';

pub fn lone_surrogate_char(unit: u16) -> char {
    char::from_u32(LONE_SURROGATE_BASE + (unit as u32 - 0xD800)).expect("a private use character")
}

pub fn lone_surrogate_unit(c: char) -> Option<u16> {
    let code = c as u32;
    (LONE_SURROGATE_BASE..LONE_SURROGATE_BASE + LONE_SURROGATE_COUNT).contains(&code).then(|| (code - LONE_SURROGATE_BASE + 0xD800) as u16)
}

fn push_code_point(out: &mut String, c: char) {
    if lone_surrogate_unit(c).is_some() {
        for &unit in c.encode_utf16(&mut [0; 2]).iter() {
            out.push(lone_surrogate_char(unit));
        }
    } else {
        out.push(c);
    }
}

pub fn from_utf16(units: &[u16]) -> String {
    let mut out = String::with_capacity(units.len());
    for decoded in char::decode_utf16(units.iter().copied()) {
        match decoded {
            Ok(c) => push_code_point(&mut out, c),
            Err(error) => out.push(lone_surrogate_char(error.unpaired_surrogate())),
        }
    }
    out
}

pub fn from_code_points(s: &str) -> Cow<'_, str> {
    if !contains_lone_surrogate(s) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() + 4);
    for c in s.chars() {
        push_code_point(&mut out, c);
    }
    Cow::Owned(out)
}

pub fn decode_utf8(bytes: &[u8]) -> Cow<'_, str> {
    match std::str::from_utf8(bytes) {
        Ok(s) => from_code_points(s),
        Err(_) => Cow::Owned(decode_utf8_replacing(bytes)),
    }
}

fn decode_utf8_replacing(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let (decoded, length) = next_code_point(&bytes[i..]);
        match decoded {
            Some(c) => push_code_point(&mut out, c),
            None => out.push('\u{FFFD}'),
        }
        i += length;
    }
    out
}

fn is_continuation(b: u8) -> bool {
    b & 0xC0 == 0x80
}

fn is_malformed3_2(b1: u8, b2: u8) -> bool {
    (b1 == 0xE0 && b2 & 0xE0 == 0x80) || !is_continuation(b2)
}

fn is_malformed4_2(b1: u8, b2: u8) -> bool {
    (b1 == 0xF0 && !(0x90..=0xBF).contains(&b2)) || (b1 == 0xF4 && b2 & 0xF0 != 0x80) || !is_continuation(b2)
}

fn next_code_point(b: &[u8]) -> (Option<char>, usize) {
    let b1 = b[0];
    let remaining = b.len();
    match b1 {
        0x00..=0x7F => (Some(b1 as char), 1),
        0xC2..=0xDF => {
            if remaining < 2 {
                return (None, remaining);
            }
            if !is_continuation(b[1]) {
                return (None, 1);
            }
            (char::from_u32(((b1 & 0x1F) as u32) << 6 | (b[1] & 0x3F) as u32), 2)
        }
        0xE0..=0xEF => {
            if remaining < 3 {
                if remaining > 1 && is_malformed3_2(b1, b[1]) {
                    return (None, 1);
                }
                return (None, remaining);
            }
            if is_malformed3_2(b1, b[1]) {
                return (None, 1);
            }
            if !is_continuation(b[2]) {
                return (None, 2);
            }
            let c = ((b1 & 0x0F) as u32) << 12 | ((b[1] & 0x3F) as u32) << 6 | (b[2] & 0x3F) as u32;
            match char::from_u32(c) {
                Some(c) => (Some(c), 3),
                None => (None, 3),
            }
        }
        0xF0..=0xF7 => {
            if remaining < 4 {
                if b1 > 0xF4 || (remaining > 1 && is_malformed4_2(b1, b[1])) {
                    return (None, 1);
                }
                if remaining > 2 && !is_continuation(b[2]) {
                    return (None, 2);
                }
                return (None, remaining);
            }
            if b1 > 0xF4 || is_malformed4_2(b1, b[1]) {
                return (None, 1);
            }
            if !is_continuation(b[2]) {
                return (None, 2);
            }
            if !is_continuation(b[3]) {
                return (None, 3);
            }
            let c = ((b1 & 0x07) as u32) << 18 | ((b[1] & 0x3F) as u32) << 12 | ((b[2] & 0x3F) as u32) << 6 | (b[3] & 0x3F) as u32;
            (char::from_u32(c), 4)
        }
        _ => (None, 1),
    }
}

pub fn utf16_units(s: &str) -> impl Iterator<Item = u16> + Clone + '_ {
    s.chars().flat_map(|c| {
        let mut buffer = [0u16; 2];
        let length = match lone_surrogate_unit(c) {
            Some(unit) => {
                buffer[0] = unit;
                1
            }
            None => c.encode_utf16(&mut buffer).len(),
        };
        buffer.into_iter().take(length)
    })
}

pub trait JavaStr {
    fn java_utf16(&self) -> impl Iterator<Item = u16> + Clone + '_;
}

impl JavaStr for str {
    fn java_utf16(&self) -> impl Iterator<Item = u16> + Clone + '_ {
        utf16_units(self)
    }
}

pub fn contains_lone_surrogate(s: &str) -> bool {
    s.as_bytes().windows(3).any(|window| window[0] == 0xF4 && window[1] == 0x8D && window[2] >= 0xA0)
}

pub fn to_utf8_lossy(s: &str) -> Cow<'_, str> {
    if !contains_lone_surrogate(s) {
        return Cow::Borrowed(s);
    }
    Cow::Owned(char::decode_utf16(utf16_units(s)).map(|decoded| decoded.unwrap_or('?')).collect())
}

pub fn to_netty_utf8(s: &str) -> Cow<'_, [u8]> {
    if !contains_lone_surrogate(s) {
        return Cow::Borrowed(s.as_bytes());
    }
    let units: Vec<u16> = utf16_units(s).collect();
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < units.len() {
        let c = units[i];
        if !(0xD800..=0xDFFF).contains(&c) {
            let mut buffer = [0u8; 3];
            out.extend_from_slice(char::from_u32(c as u32).expect("a non-surrogate unit").encode_utf8(&mut buffer).as_bytes());
        } else if c >= 0xDC00 {
            out.push(WRITE_UTF_UNKNOWN);
        } else {
            i += 1;
            if i == units.len() {
                out.push(WRITE_UTF_UNKNOWN);
                break;
            }
            let c2 = units[i];
            if !(0xDC00..=0xDFFF).contains(&c2) {
                out.push(WRITE_UTF_UNKNOWN);
                out.push(if (0xD800..=0xDBFF).contains(&c2) { WRITE_UTF_UNKNOWN } else { c2 as u8 });
            } else {
                let code_point = 0x10000 + (((c as u32) - 0xD800) << 10 | ((c2 as u32) - 0xDC00));
                let mut buffer = [0u8; 4];
                out.extend_from_slice(char::from_u32(code_point).expect("a supplementary character").encode_utf8(&mut buffer).as_bytes());
            }
        }
        i += 1;
    }
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lone_surrogates_round_trip_through_units() {
        let units = [0x61, 0xD800, 0x62, 0xDFFF, 0xD83D, 0xDE00, 0xDC00, 0xD800];
        let text = from_utf16(&units);
        assert_eq!(utf16_units(&text).collect::<Vec<_>>(), units);
        assert!(contains_lone_surrogate(&text));
        assert_eq!(to_utf8_lossy(&text), "a?b?\u{1F600}??");
    }

    #[test]
    fn carried_surrogates_count_and_hash_as_one_unit() {
        let text = from_utf16(&[0x61, 0xD800, 0x62]);
        assert_eq!(text.java_utf16().count(), 3);
        assert_eq!(crate::random::java_string_hash(&text), (0x61 * 31 + 0xD800) * 31 + 0x62);
    }

    #[test]
    fn plain_text_is_untouched() {
        let text = "a\u{E9}\u{20AC}\u{1F600}\u{10D7FF}\u{10E000}";
        assert!(!contains_lone_surrogate(text));
        assert_eq!(utf16_units(text).collect::<Vec<_>>(), text.encode_utf16().collect::<Vec<_>>());
        assert!(matches!(to_utf8_lossy(text), Cow::Borrowed(_)));
        assert!(matches!(from_code_points(text), Cow::Borrowed(_)));
        assert!(matches!(to_netty_utf8(text), Cow::Borrowed(_)));
        assert!(matches!(decode_utf8(text.as_bytes()), Cow::Borrowed(_)));
    }

    #[test]
    fn genuine_carrier_range_characters_stay_distinct_from_lone_surrogates() {
        let genuine = from_utf16(&[0xDBF6, 0xDC00]);
        let lone = from_utf16(&[0xD800]);
        assert_ne!(genuine, lone);
        assert_eq!(genuine.java_utf16().collect::<Vec<_>>(), [0xDBF6, 0xDC00]);
        assert_eq!(lone.java_utf16().collect::<Vec<_>>(), [0xD800]);
        assert_eq!(from_code_points("\u{10D800}"), genuine);
        assert_eq!(decode_utf8(&[0xF4, 0x8D, 0xA0, 0x80]), genuine);
        assert_eq!(to_utf8_lossy(&genuine), "\u{10D800}");
        assert_eq!(to_utf8_lossy(&lone), "?");
        assert_eq!(crate::random::java_string_hash(&genuine), 1801930);
        assert_eq!(crate::random::java_string_hash(&from_utf16(&[0xDBF7, 0xDFFF])), 1802984);
    }

    fn hex_bytes(hex: &str) -> Vec<u8> {
        (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect()
    }

    fn hex_units(text: &str) -> String {
        text.java_utf16().map(|unit| format!("{unit:04x}")).collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn malformed_utf8_is_replaced_like_the_jdk_decoder() {
        let cases = [
            ("f48da080", "dbf6 dc00"),
            ("eda080", "fffd"),
            ("edafb6edb080", "fffd fffd"),
            ("f4908080", "fffd fffd fffd fffd"),
            ("c3", "fffd"),
            ("c080", "fffd fffd"),
            ("61f48da08062", "0061 dbf6 dc00 0062"),
            ("f48daf", "fffd"),
            ("eda0", "fffd"),
            ("ff", "fffd"),
        ];
        for (input, expected) in cases {
            assert_eq!(hex_units(&decode_utf8(&hex_bytes(input))), expected, "{input}");
        }
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn surrogates_are_encoded_like_netty_and_the_jdk() {
        let cases: [(&[u16], &str, &str); 7] = [
            (&[0xDBF6, 0xDC00], "f48da080", "f48da080"),
            (&[0xD800], "3f", "3f"),
            (&[0x61, 0xD800, 0xE9], "613fe9", "613fc3a9"),
            (&[0x61, 0xD800, 0xD83D, 0xDE00, 0x62], "613f3f3f62", "613ff09f988062"),
            (&[0xD800, 0xD800, 0xDC00], "3f3f3f", "3ff0908080"),
            (&[0xDBF6, 0xDC00, 0xDC00], "f48da0803f", "f48da0803f"),
            (&[0xD800, 0x20AC], "3fac", "3fe282ac"),
        ];
        for (units, netty, jdk) in cases {
            let text = from_utf16(units);
            assert_eq!(hex(&to_netty_utf8(&text)), netty, "{units:x?}");
            assert_eq!(hex(to_utf8_lossy(&text).as_bytes()), jdk, "{units:x?}");
        }
    }

    #[test]
    #[ignore]
    fn utf8_encoding_matches_the_netty_and_jdk_oracle() {
        let expected = std::fs::read_to_string(std::env::var("UTF8_ENCODE_ORACLE").expect("UTF8_ENCODE_ORACLE")).unwrap();
        for line in expected.lines().filter(|line| !line.starts_with("frame.")) {
            let mut fields = line.split('\t');
            let units: Vec<u16> = fields.next().unwrap().split(' ').map(|unit| u16::from_str_radix(unit, 16).unwrap()).collect();
            let text = from_utf16(&units);
            assert_eq!(hex(&to_netty_utf8(&text)), fields.next().unwrap(), "{units:x?}");
            assert_eq!(hex(to_utf8_lossy(&text).as_bytes()), fields.next().unwrap(), "{units:x?}");
        }
    }

    #[test]
    #[ignore]
    fn utf8_decoding_matches_the_jdk_oracle() {
        let expected = std::fs::read_to_string(std::env::var("UTF8_DECODE_ORACLE").expect("UTF8_DECODE_ORACLE")).unwrap();
        for line in expected.lines() {
            let (input, units) = line.split_once('\t').unwrap();
            assert_eq!(hex_units(&decode_utf8(&hex_bytes(input))), units, "{input}");
        }
    }
}
