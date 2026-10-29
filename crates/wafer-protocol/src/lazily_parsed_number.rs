fn parts(text: &str) -> (bool, Vec<u8>, i64, Option<i32>) {
    let bytes = text.as_bytes();
    let mut index = 0;
    let negative = bytes.first() == Some(&b'-');
    if negative || bytes.first() == Some(&b'+') {
        index += 1;
    }
    let mut digits: Vec<u8> = Vec::new();
    let mut fraction_length: i64 = 0;
    let mut seen_point = false;
    while index < bytes.len() {
        match bytes[index] {
            b'.' => seen_point = true,
            b'e' | b'E' => break,
            digit => {
                digits.push(digit.wrapping_sub(b'0'));
                if seen_point {
                    fraction_length += 1;
                }
            }
        }
        index += 1;
    }
    let mut exponent = Some(0);
    if index < bytes.len() {
        exponent = text[index + 1..].trim_start_matches('+').parse::<i32>().ok();
    }
    (negative, digits, fraction_length, exponent)
}

fn digits_and_scale(text: &str) -> (bool, Vec<u8>, i64) {
    let (negative, digits, fraction_length, exponent) = parts(text);
    (negative, digits, fraction_length - exponent.unwrap_or(0) as i64)
}

pub fn parse_long(text: &str) -> Option<i64> {
    let digits = text.strip_prefix(['-', '+']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.strip_prefix('+').unwrap_or(text).parse::<i64>().ok()
}

pub fn parse_int(text: &str) -> Option<i32> {
    parse_long(text).and_then(|value| i32::try_from(value).ok())
}

fn big_decimal_long_value(text: &str) -> i64 {
    let (negative, digits, scale) = digits_and_scale(text);
    let first_nonzero = digits.iter().position(|&digit| digit != 0).unwrap_or(digits.len());
    let digits = &digits[first_nonzero..];
    let magnitude: u64 = if scale <= 0 {
        if -scale >= 64 {
            0
        } else {
            let mut value: u64 = 0;
            for &digit in digits {
                value = value.wrapping_mul(10).wrapping_add(digit as u64);
            }
            for _ in 0..-scale {
                value = value.wrapping_mul(10);
            }
            value
        }
    } else if scale as u64 >= digits.len() as u64 {
        0
    } else {
        let mut value: u64 = 0;
        for &digit in &digits[..digits.len() - scale as usize] {
            value = value.wrapping_mul(10).wrapping_add(digit as u64);
        }
        value
    };
    if negative { (magnitude as i64).wrapping_neg() } else { magnitude as i64 }
}

pub fn long_value(text: &str) -> i64 {
    parse_long(text).unwrap_or_else(|| big_decimal_long_value(text))
}

pub fn int_value(text: &str) -> i32 {
    parse_int(text).unwrap_or_else(|| long_value(text) as i32)
}

pub fn float_value(text: &str) -> f32 {
    text.parse::<f32>().unwrap_or(0.0)
}

pub fn double_value(text: &str) -> f64 {
    text.parse::<f64>().unwrap_or(0.0)
}

pub fn long_value_exact(text: &str) -> Result<Option<i64>, String> {
    let (negative, digits, fraction_length, exponent) = parts(text);
    let Some(exponent) = exponent else { return Err("NumberFormatException: Exponent overflow.".to_string()) };
    let scale = fraction_length - exponent as i64;
    if scale > i32::MAX as i64 || scale < i32::MIN as i64 {
        return Err("NumberFormatException: Scale out of range.".to_string());
    }
    let Some(first_nonzero) = digits.iter().position(|&digit| digit != 0) else { return Ok(Some(0)) };
    let digits = &digits[first_nonzero..];
    let integer_digits: &[u8] = if scale <= 0 {
        digits
    } else if scale as usize >= digits.len() {
        return Ok(None);
    } else {
        let split = digits.len() - scale as usize;
        if digits[split..].iter().any(|&digit| digit != 0) {
            return Ok(None);
        }
        &digits[..split]
    };
    let trailing_zeros = if scale < 0 { -scale } else { 0 };
    if integer_digits.len() as i64 + trailing_zeros > 19 {
        return Ok(None);
    }
    let mut value: i128 = 0;
    for &digit in integer_digits {
        value = value * 10 + digit as i128;
    }
    for _ in 0..trailing_zeros {
        value *= 10;
    }
    if negative {
        value = -value;
    }
    Ok(i64::try_from(value).ok())
}

pub fn exact_nbt(text: &str) -> crate::nbt::Nbt {
    use crate::nbt::Nbt;
    match long_value_exact(text) {
        Ok(Some(l)) => {
            if l as i8 as i64 == l {
                Nbt::Byte(l as i8)
            } else if l as i16 as i64 == l {
                Nbt::Short(l as i16)
            } else if l as i32 as i64 == l {
                Nbt::Int(l as i32)
            } else {
                Nbt::Long(l)
            }
        }
        _ => {
            let d = double_value(text);
            if d as f32 as f64 == d { Nbt::Float(d as f32) } else { Nbt::Double(d) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_and_long_values_follow_gson() {
        let cases: [(&str, i32, i64); 31] = [
            ("5", 5, 5),
            ("-5", -5, -5),
            ("2147483648", -2147483648, 2147483648),
            ("-2147483649", 2147483647, -2147483649),
            ("4294967297", 1, 4294967297),
            ("9223372036854775807", -1, 9223372036854775807),
            ("9223372036854775808", 0, -9223372036854775808),
            ("18446744073709551615", -1, -1),
            ("18446744073709551616", 0, 0),
            ("100000000000000000000", 1661992960, 7766279631452241920),
            ("123456789012345678901234567890", 1312754386, -4362896299872285998),
            ("1e2", 100, 100),
            ("1E5", 100000, 100000),
            ("1e-9", 0, 0),
            ("0.50", 0, 0),
            ("5.7", 5, 5),
            ("-5.7", -5, -5),
            ("2.9802322e-8", 0, 0),
            ("1e19", -1981284352, -8446744073709551616),
            ("1.2345678901234567E19", -350288040, -6101065172474984616),
            ("-1e19", 1981284352, 8446744073709551616),
            ("1e400", 0, 0),
            ("-0", 0, 0),
            ("0.1e1", 1, 1),
            ("1000000000000e-3", 1000000000, 1000000000),
            ("12345678901234567890123e-2", 792095797, -5670419503621182411),
            ("1e63", 0, -9223372036854775808),
            ("1e64", 0, 0),
            ("3e63", 0, -9223372036854775808),
            ("5e-1", 0, 0),
            ("1.9999999e9", 1999999900, 1999999900),
        ];
        for (text, int, long) in cases {
            assert_eq!(int_value(text), int, "{text}");
            assert_eq!(long_value(text), long, "{text}");
        }
    }

    #[test]
    fn float_values_round_the_literal_directly() {
        assert_eq!(float_value("1.0000000596046448"), 1.0000001);
        assert_eq!(float_value("1.000000059604644775390625"), 1.0);
        assert_eq!(float_value("1e400"), f32::INFINITY);
        assert_eq!(float_value("2.9802322e-8"), 2.9802322e-8);
        assert_eq!(double_value("1.0000000596046448"), 1.0000000596046448);
    }

    #[test]
    fn exact_values_are_the_narrowest_nbt_type() {
        use crate::nbt::Nbt;
        assert_eq!(exact_nbt("5"), Nbt::Byte(5));
        assert_eq!(exact_nbt("300"), Nbt::Short(300));
        assert_eq!(exact_nbt("1e2"), Nbt::Byte(100));
        assert_eq!(exact_nbt("5.0"), Nbt::Byte(5));
        assert_eq!(exact_nbt("0.5"), Nbt::Float(0.5));
        assert_eq!(exact_nbt("0.1"), Nbt::Double(0.1));
        assert_eq!(exact_nbt("100000000000000000000"), Nbt::Double(1e20));
    }
}
