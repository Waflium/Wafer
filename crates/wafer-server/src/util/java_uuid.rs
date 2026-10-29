use wafer_core::java_string::JavaStr;

const DIGIT_RUNS: [u16; 37] = [
    0x0030, 0x0660, 0x06F0, 0x07C0, 0x0966, 0x09E6, 0x0A66, 0x0AE6, 0x0B66, 0x0BE6, 0x0C66, 0x0CE6, 0x0D66, 0x0DE6, 0x0E50, 0x0ED0, 0x0F20, 0x1040, 0x1090, 0x17E0, 0x1810, 0x1946, 0x19D0, 0x1A80,
    0x1A90, 0x1B50, 0x1BB0, 0x1C40, 0x1C50, 0xA620, 0xA8D0, 0xA900, 0xA9D0, 0xA9F0, 0xAA50, 0xABF0, 0xFF10,
];

fn character_digit(unit: u16, radix: i64) -> i64 {
    let value = match unit {
        0x41..=0x5A => i64::from(unit) - 0x41 + 10,
        0x61..=0x7A => i64::from(unit) - 0x61 + 10,
        0xFF21..=0xFF3A => i64::from(unit) - 0xFF21 + 10,
        0xFF41..=0xFF5A => i64::from(unit) - 0xFF41 + 10,
        _ => match DIGIT_RUNS.iter().find(|&&start| unit >= start && unit < start + 10) {
            Some(&start) => i64::from(unit - start),
            None => return -1,
        },
    };
    if value < radix { value } else { -1 }
}

fn number_format_for_char_sequence(units: &[u16], begin: usize, end: usize, error_index: usize) -> String {
    format!("Error at index {} in: \"{}\"", error_index - begin, wafer_core::java_string::from_utf16(&units[begin..end]))
}

fn parse_long(units: &[u16], begin: usize, end: usize, radix: i64) -> Result<i64, String> {
    if begin >= end {
        return Err(format!("For input string: \"\" under radix {radix}"));
    }
    let mut negative = false;
    let mut i = begin;
    let mut limit = -i64::MAX;
    let first = units[i];
    if first < u16::from(b'0') {
        if first == u16::from(b'-') {
            negative = true;
            limit = i64::MIN;
        } else if first != u16::from(b'+') {
            return Err(number_format_for_char_sequence(units, begin, end, i));
        }
        i += 1;
    }
    if i >= end {
        return Err(number_format_for_char_sequence(units, begin, end, i));
    }
    let multmin = limit / radix;
    let mut result: i64 = 0;
    while i < end {
        let digit = character_digit(units[i], radix);
        if digit < 0 || result < multmin {
            return Err(number_format_for_char_sequence(units, begin, end, i));
        }
        result *= radix;
        if result < limit + digit {
            return Err(number_format_for_char_sequence(units, begin, end, i));
        }
        result -= digit;
        i += 1;
    }
    Ok(if negative { result } else { -result })
}

fn index_of_dash(units: &[u16], from: usize) -> Option<usize> {
    units.get(from..)?.iter().position(|&unit| unit == u16::from(b'-')).map(|index| index + from)
}

pub fn from_string(name: &str) -> Result<u128, String> {
    let units: Vec<u16> = name.java_utf16().collect();
    let len = units.len();
    if len > 36 {
        return Err("UUID string too large".to_string());
    }
    let dash1 = index_of_dash(&units, 0);
    let dash2 = dash1.and_then(|dash| index_of_dash(&units, dash + 1));
    let dash3 = dash2.and_then(|dash| index_of_dash(&units, dash + 1));
    let dash4 = dash3.and_then(|dash| index_of_dash(&units, dash + 1));
    let dash5 = dash4.and_then(|dash| index_of_dash(&units, dash + 1));
    let (Some(dash1), Some(dash2), Some(dash3), Some(dash4), None) = (dash1, dash2, dash3, dash4, dash5) else {
        return Err(format!("Invalid UUID string: {name}"));
    };
    let mut most = parse_long(&units, 0, dash1, 16)? & 0xffff_ffff;
    most <<= 16;
    most |= parse_long(&units, dash1 + 1, dash2, 16)? & 0xffff;
    most <<= 16;
    most |= parse_long(&units, dash2 + 1, dash3, 16)? & 0xffff;
    let mut least = parse_long(&units, dash3 + 1, dash4, 16)? & 0xffff;
    least <<= 48;
    least |= parse_long(&units, dash4 + 1, len, 16)? & 0xffff_ffff_ffff;
    Ok(u128::from(most as u64) << 64 | u128::from(least as u64))
}

#[cfg(test)]
mod tests {
    use super::from_string;

    fn unhex(value: &str) -> String {
        let units: Vec<u16> = (0..value.len()).step_by(4).map(|i| u16::from_str_radix(&value[i..i + 4], 16).unwrap()).collect();
        String::from_utf16(&units).unwrap()
    }

    #[test]
    fn matches_the_vanilla_cases() {
        let mut failures = 0;
        for line in include_str!("java_uuid_cases.tsv").lines() {
            let fields: Vec<&str> = line.split('\t').collect();
            let input = unhex(fields[0]);
            let expected = match fields[1] {
                "OK" => Ok((u64::from_str_radix(fields[2], 16).unwrap(), u64::from_str_radix(fields[3], 16).unwrap())),
                _ => Err(unhex(fields[2])),
            };
            let actual = from_string(&input).map(|uuid| ((uuid >> 64) as u64, uuid as u64));
            if actual != expected {
                failures += 1;
                eprintln!("{input:?}: expected {expected:?}, got {actual:?}");
            }
        }
        assert_eq!(failures, 0);
    }
}
