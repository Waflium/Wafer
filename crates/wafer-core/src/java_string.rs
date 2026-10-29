use std::borrow::Cow;

const LONE_SURROGATE_BASE: u32 = 0x10D800;
const LONE_SURROGATE_COUNT: u32 = 0x800;

pub fn lone_surrogate_char(unit: u16) -> char {
    char::from_u32(LONE_SURROGATE_BASE + (unit as u32 - 0xD800)).expect("a private use character")
}

pub fn lone_surrogate_unit(c: char) -> Option<u16> {
    let code = c as u32;
    (LONE_SURROGATE_BASE..LONE_SURROGATE_BASE + LONE_SURROGATE_COUNT).contains(&code).then(|| (code - LONE_SURROGATE_BASE + 0xD800) as u16)
}

pub fn from_utf16(units: &[u16]) -> String {
    let mut out = String::with_capacity(units.len());
    for decoded in char::decode_utf16(units.iter().copied()) {
        out.push(match decoded {
            Ok(c) => c,
            Err(error) => lone_surrogate_char(error.unpaired_surrogate()),
        });
    }
    out
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
    Cow::Owned(s.chars().map(|c| if lone_surrogate_unit(c).is_some() { '?' } else { c }).collect())
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
    }
}
