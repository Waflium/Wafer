const DECIMAL_DIGIT_ZEROS: [u32; 37] = [
    0x0030, 0x0660, 0x06F0, 0x07C0, 0x0966, 0x09E6, 0x0A66, 0x0AE6, 0x0B66, 0x0BE6, 0x0C66, 0x0CE6, 0x0D66, 0x0DE6, 0x0E50, 0x0ED0, 0x0F20, 0x1040, 0x1090, 0x17E0, 0x1810, 0x1946, 0x19D0, 0x1A80, 0x1A90, 0x1B50,
    0x1BB0, 0x1C40, 0x1C50, 0xA620, 0xA8D0, 0xA900, 0xA9D0, 0xA9F0, 0xAA50, 0xABF0, 0xFF10,
];

pub fn digit(c: char) -> Option<u32> {
    let code = c as u32;
    if code < 0x80 {
        return c.to_digit(10);
    }
    DECIMAL_DIGIT_ZEROS.iter().find(|&&zero| code >= zero && code < zero + 10).map(|&zero| code - zero)
}

fn parse_signed(input: &str, limit: i128) -> Option<i128> {
    let (negative, digits) = match input.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, input.strip_prefix('+').unwrap_or(input)),
    };
    if digits.is_empty() {
        return None;
    }
    let mut magnitude: i128 = 0;
    for unit in digits.chars() {
        magnitude = magnitude * 10 + digit(unit)? as i128;
        if magnitude > limit {
            return None;
        }
    }
    Some(if negative { -magnitude } else { magnitude })
}

pub fn parse_int(input: &str) -> Option<i32> {
    i32::try_from(parse_signed(input, 1 << 31)?).ok()
}

pub fn parse_long(input: &str) -> Option<i64> {
    i64::try_from(parse_signed(input, 1 << 63)?).ok()
}

pub fn number_format_exception(input: &str) -> String {
    format!("For input string: \"{input}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_int_matches_java() {
        let cases: &[(&str, Option<i32>)] = &[
            ("\u{663}", Some(3)),
            ("-\u{663}\u{664}", Some(-34)),
            ("+\u{663}", Some(3)),
            ("\u{FF11}\u{FF12}", Some(12)),
            ("\u{663}a", None),
            ("", None),
            ("-", None),
            ("+", None),
            ("2147483647", Some(2147483647)),
            ("2147483648", None),
            ("-2147483648", Some(-2147483648)),
            ("-2147483649", None),
            ("0000000000000000000005", Some(5)),
            ("\u{967}\u{968}\u{969}", Some(123)),
            ("\u{1D7D3}", None),
            (" 5", None),
            ("5 ", None),
            ("+-5", None),
            ("--5", None),
            ("5\u{0}", None),
        ];
        for (input, expected) in cases {
            assert_eq!(parse_int(input), *expected, "{input:?}");
        }
    }

    #[test]
    fn parse_long_matches_java() {
        let cases: &[(&str, Option<i64>)] = &[
            ("\u{663}", Some(3)),
            ("-\u{663}\u{664}", Some(-34)),
            ("+\u{FF11}\u{FF12}", Some(12)),
            ("9223372036854775807", Some(i64::MAX)),
            ("9223372036854775808", None),
            ("-9223372036854775808", Some(i64::MIN)),
            ("-9223372036854775809", None),
            ("99999999999999999999999999999999999999999", None),
            ("0000000000000000000000000000000000005", Some(5)),
            ("2147483648", Some(2147483648)),
            ("", None),
            ("-", None),
            ("+", None),
            (" 5", None),
            ("5 ", None),
            ("1L", None),
            ("\u{1D7D3}", None),
        ];
        for (input, expected) in cases {
            assert_eq!(parse_long(input), *expected, "{input:?}");
        }
    }

    #[test]
    fn digit_covers_every_decimal_digit_of_the_basic_multilingual_plane() {
        let count = (0..=0xFFFFu32).filter_map(char::from_u32).filter(|&unit| digit(unit).is_some()).count();
        assert_eq!(count, 370);
        assert_eq!(digit('\u{1C59}'), Some(9));
        assert_eq!(digit('\u{1C5A}'), None);
        assert_eq!(digit('a'), None);
    }
}
