use std::sync::{LazyLock, Mutex};

use chrono::TimeZone as _;
use chrono_tz::{OffsetComponents, Tz};

use crate::util::java_integer;

const MILLIS_PER_DAY: i64 = 86_400_000;
const EPOCH_OFFSET: i64 = 719_163;
const GREGORIAN_CUTOVER_FIXED_DATE: i64 = 577_736;
const GREGORIAN_CUTOVER_YEAR: i32 = 1582;

struct ZoneRow {
    names: [&'static str; 5],
    raw_offset: i64,
    dst_savings: i64,
}

static ZONE_STRINGS: LazyLock<Vec<ZoneRow>> = LazyLock::new(|| {
    include_str!("java_zone_strings.tsv")
        .lines()
        .filter_map(|line| {
            let mut columns = line.split('\t');
            let names = [columns.next()?, columns.next()?, columns.next()?, columns.next()?, columns.next()?];
            Some(ZoneRow { names, raw_offset: columns.next()?.parse().ok()?, dst_savings: columns.next()?.parse().ok()? })
        })
        .collect()
});

struct ZoneFields {
    zone_offset: Option<i64>,
    dst_offset: Option<i64>,
    zone: Option<&'static ZoneRow>,
}

fn is_leap_julian(year: i32) -> bool {
    year % 4 == 0
}

fn is_leap_gregorian(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn julian_first_of_month(year: i32, month: i32) -> i64 {
    let year = year as i64;
    let mut days = (-2i64).wrapping_add(365i64.wrapping_mul(year - 1));
    days += (year - 1).div_euclid(4);
    days += (367 * month as i64 - 362) / 12;
    if month > 2 {
        days -= if is_leap_julian(year as i32) { 1 } else { 2 };
    }
    days + 1
}

fn gregorian_first_of_month(year: i32, month: i32) -> i64 {
    let previous = year as i64 - 1;
    let mut days = 365i64.wrapping_mul(previous);
    days += previous.div_euclid(4) - previous.div_euclid(100) + previous.div_euclid(400);
    let before = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    days + before[(month - 1) as usize] + (is_leap_gregorian(year) && month > 2) as i64 + 1
}

fn fixed_date_of(gregorian: bool, year: i32, month_index: i32, day: i32) -> i64 {
    let (mut year, mut month) = (year, month_index);
    if month > 11 {
        year = year.wrapping_add(month / 12);
        month %= 12;
    } else if month < 0 {
        year = year.wrapping_add(month.div_euclid(12));
        month = month.rem_euclid(12);
    }
    let first = if gregorian { gregorian_first_of_month(year, month + 1) } else { julian_first_of_month(year, month + 1) };
    first + day as i64 - 1
}

fn wall_millis(year: i32, month_index: i32, day: i32, hour: i32, minute: i32, second: i32) -> i64 {
    let mut time_of_day = hour as i64;
    time_of_day = time_of_day * 60 + minute as i64;
    time_of_day = time_of_day * 60 + second as i64;
    time_of_day *= 1000;
    let mut fixed_date = time_of_day / MILLIS_PER_DAY;
    time_of_day %= MILLIS_PER_DAY;
    while time_of_day < 0 {
        time_of_day += MILLIS_PER_DAY;
        fixed_date -= 1;
    }
    let gregorian_fixed;
    let julian_fixed;
    if year > GREGORIAN_CUTOVER_YEAR {
        gregorian_fixed = fixed_date.wrapping_add(fixed_date_of(true, year, month_index, day));
        if gregorian_fixed >= GREGORIAN_CUTOVER_FIXED_DATE {
            return gregorian_fixed.wrapping_sub(EPOCH_OFFSET).wrapping_mul(MILLIS_PER_DAY).wrapping_add(time_of_day);
        }
        julian_fixed = fixed_date.wrapping_add(fixed_date_of(false, year, month_index, day));
    } else if year < GREGORIAN_CUTOVER_YEAR {
        julian_fixed = fixed_date.wrapping_add(fixed_date_of(false, year, month_index, day));
        if julian_fixed < GREGORIAN_CUTOVER_FIXED_DATE {
            return julian_fixed.wrapping_sub(EPOCH_OFFSET).wrapping_mul(MILLIS_PER_DAY).wrapping_add(time_of_day);
        }
        gregorian_fixed = julian_fixed;
    } else {
        julian_fixed = fixed_date.wrapping_add(fixed_date_of(false, year, month_index, day));
        gregorian_fixed = fixed_date.wrapping_add(fixed_date_of(true, year, month_index, day));
    }
    let fixed = if gregorian_fixed >= GREGORIAN_CUTOVER_FIXED_DATE { gregorian_fixed } else { julian_fixed };
    fixed.wrapping_sub(EPOCH_OFFSET).wrapping_mul(MILLIS_PER_DAY).wrapping_add(time_of_day)
}

fn skip_blanks(chars: &[char], index: &mut usize) {
    while *index < chars.len() && (chars[*index] == ' ' || chars[*index] == '\t') {
        *index += 1;
    }
}

fn parse_number(chars: &[char], index: &mut usize) -> Option<i32> {
    skip_blanks(chars, index);
    let negative = chars.get(*index) == Some(&'-');
    let start = if negative { *index + 1 } else { *index };
    let mut end = start;
    let mut magnitude: u128 = 0;
    while end < chars.len() && java_integer::digit(chars[end]).is_some() {
        magnitude = magnitude.saturating_mul(10).saturating_add(java_integer::digit(chars[end])? as u128);
        end += 1;
    }
    if end == start {
        return None;
    }
    *index = end;
    let value = if negative { -(magnitude.min(i128::MAX as u128) as i128) } else { magnitude.min(i128::MAX as u128) as i128 };
    Some(if value >= i64::MIN as i128 && value <= i64::MAX as i128 {
        value as i64 as i32
    } else if value < 0 {
        i32::MIN
    } else {
        i32::MAX
    })
}

fn expect_literal(chars: &[char], index: &mut usize, literal: char) -> Option<()> {
    if chars.get(*index) == Some(&literal) {
        *index += 1;
        Some(())
    } else {
        None
    }
}

fn parse_numeric_zone(chars: &[char], mut index: usize, sign: i64, colon: bool) -> Option<i64> {
    let digit = |index: usize| chars.get(index).filter(|c| c.is_ascii_digit()).map(|c| *c as i64 - '0' as i64);
    let mut hours = digit(index)?;
    index += 1;
    match digit(index) {
        Some(second) => {
            hours = hours * 10 + second;
            index += 1;
        }
        None if !colon => return None,
        None => {}
    }
    if hours > 23 {
        return None;
    }
    if colon {
        if chars.get(index) != Some(&':') {
            return None;
        }
        index += 1;
    }
    let minutes = digit(index)? * 10 + digit(index + 1)?;
    if minutes > 59 {
        return None;
    }
    Some(sign * (hours * 60 + minutes) * 60_000)
}

fn region_matches_ignore_case(chars: &[char], start: usize, name: &str) -> bool {
    let name: Vec<char> = name.chars().collect();
    if start + name.len() > chars.len() {
        return false;
    }
    chars[start..start + name.len()].iter().zip(&name).all(|(&a, &b)| {
        a == b || {
            let (a, b) = (a.to_uppercase().next().unwrap_or(a), b.to_uppercase().next().unwrap_or(b));
            a == b || a.to_lowercase().next() == b.to_lowercase().next()
        }
    })
}

fn match_zone_string(chars: &[char], start: usize, names: &[&str; 5]) -> Option<usize> {
    (1..=4).find(|&i| region_matches_ignore_case(chars, start, names[i]))
}

fn default_zone_id() -> Option<String> {
    if cfg!(not(windows))
        && let Ok(zone) = std::env::var("TZ")
        && !zone.is_empty()
    {
        return Some(zone);
    }
    iana_time_zone::get_timezone().ok()
}

static DEFAULT_ZONE: LazyLock<Option<&'static ZoneRow>> = LazyLock::new(|| {
    let id = default_zone_id()?;
    ZONE_STRINGS.iter().find(|row| row.names[0] == id)
});

#[derive(Clone, Copy)]
enum CurrentZone {
    Default,
    Row(&'static ZoneRow),
}

pub struct SimpleDateFormat {
    current: CurrentZone,
    default: Option<&'static ZoneRow>,
}

pub static BAN_LIST_DATE_FORMAT: LazyLock<Mutex<SimpleDateFormat>> = LazyLock::new(|| Mutex::new(SimpleDateFormat::new()));

impl SimpleDateFormat {
    pub fn new() -> SimpleDateFormat {
        SimpleDateFormat { current: CurrentZone::Default, default: *DEFAULT_ZONE }
    }

    #[cfg(test)]
    fn with_default_zone(id: &str) -> SimpleDateFormat {
        SimpleDateFormat { current: CurrentZone::Default, default: ZONE_STRINGS.iter().find(|row| row.names[0] == id) }
    }

    fn current_row(&self) -> Option<&'static ZoneRow> {
        match self.current {
            CurrentZone::Default => self.default,
            CurrentZone::Row(row) => Some(row),
        }
    }

    fn parse_zone_string(&mut self, chars: &[char], start: usize) -> Option<ZoneFields> {
        let try_row = |row: &'static ZoneRow| match_zone_string(chars, start, &row.names).map(|name_index| (row, name_index));
        let (row, name_index) = self
            .current_row()
            .and_then(try_row)
            .or_else(|| self.default.and_then(try_row))
            .or_else(|| ZONE_STRINGS.iter().find_map(try_row))?;
        if self.current_row().is_none_or(|current| current.names[0] != row.names[0]) {
            self.current = CurrentZone::Row(row);
        }
        Some(zone_fields(row, name_index))
    }

    pub fn parse(&mut self, text: &str) -> Option<i64> {
        let chars: Vec<char> = text.chars().collect();
        let mut index = 0;
        let year = parse_number(&chars, &mut index)?;
        expect_literal(&chars, &mut index, '-')?;
        let month = parse_number(&chars, &mut index)?;
        expect_literal(&chars, &mut index, '-')?;
        let day = parse_number(&chars, &mut index)?;
        expect_literal(&chars, &mut index, ' ')?;
        let hour = parse_number(&chars, &mut index)?;
        expect_literal(&chars, &mut index, ':')?;
        let minute = parse_number(&chars, &mut index)?;
        expect_literal(&chars, &mut index, ':')?;
        let second = parse_number(&chars, &mut index)?;
        expect_literal(&chars, &mut index, ' ')?;
        let zone = self.parse_zone(&chars, &mut index)?;
        let wall = wall_millis(year, month.wrapping_sub(1), day, hour, minute, second);
        let (zone_offset, dst_offset) = match zone.zone {
            Some(row) => {
                let (raw, dst) = zone_offsets_at(row, wall);
                (zone.zone_offset.unwrap_or(raw), zone.dst_offset.unwrap_or(dst))
            }
            None => (zone.zone_offset.unwrap_or(0), zone.dst_offset.unwrap_or(0)),
        };
        Some(wall.wrapping_sub(zone_offset + dst_offset))
    }

    pub fn format(&self, millis: i64) -> String {
        let offset_millis = match self.current_row() {
            Some(row) => zone_offset_at_instant(row, millis),
            None => chrono::Local.timestamp_millis_opt(millis).single().map_or(0, |date| chrono::Offset::fix(date.offset()).local_minus_utc() as i64 * 1000),
        };
        let local = chrono::DateTime::from_timestamp_millis(millis.wrapping_add(offset_millis)).map_or_else(String::new, |date| date.naive_utc().format("%Y-%m-%d %H:%M:%S").to_string());
        let minutes = offset_millis.abs() / 60_000;
        format!("{local} {}{:02}{:02}", if offset_millis < 0 { '-' } else { '+' }, minutes / 60, minutes % 60)
    }
}

impl Default for SimpleDateFormat {
    fn default() -> SimpleDateFormat {
        SimpleDateFormat::new()
    }
}

fn zone_fields(row: &'static ZoneRow, name_index: usize) -> ZoneFields {
    let use_same_name = name_index <= 2 && row.names[name_index].eq_ignore_ascii_case(row.names[name_index + 2]);
    let dst_amount = if name_index >= 3 { row.dst_savings } else { 0 };
    let fixed = !(use_same_name || (name_index >= 3 && dst_amount == 0));
    ZoneFields { zone_offset: None, dst_offset: fixed.then_some(dst_amount), zone: Some(row) }
}

impl SimpleDateFormat {
    fn parse_zone(&mut self, chars: &[char], index: &mut usize) -> Option<ZoneFields> {
        skip_blanks(chars, index);
        let start = *index;
        let fixed = |offset: i64| ZoneFields { zone_offset: Some(offset), dst_offset: Some(0), zone: None };
        match chars.get(start)? {
            '+' => parse_numeric_zone(chars, start + 1, 1, false).map(fixed),
            '-' => parse_numeric_zone(chars, start + 1, -1, false).map(fixed),
            'G' | 'g' if region_matches_ignore_case(chars, start, "GMT") => match chars.get(start + 3) {
                Some('+') => parse_numeric_zone(chars, start + 4, 1, true).map(fixed),
                Some('-') => parse_numeric_zone(chars, start + 4, -1, true).map(fixed),
                _ => Some(fixed(0)),
            },
            _ => self.parse_zone_string(chars, start),
        }
    }
}

const UTC1900_MILLIS: i64 = -2_208_988_800_000;
const NEGATIVE_DST_ZONES: [&str; 6] = ["Africa/Casablanca", "Africa/El_Aaiun", "Africa/Windhoek", "Europe/Prague", "Europe/Dublin", "Eire"];

fn java_zone(zone: &ZoneRow) -> Option<Tz> {
    match zone.names[0] {
        "IET" => "America/Indiana/Indianapolis".parse().ok(),
        id => id.parse().ok(),
    }
}

fn split_offsets(zone: &ZoneRow, base: i64, dst: i64) -> (i64, i64) {
    let total = base + dst;
    if NEGATIVE_DST_ZONES.contains(&zone.names[0]) && matches!(total - zone.raw_offset, 0 | 3_600_000) {
        return (zone.raw_offset, total - zone.raw_offset);
    }
    (base, dst)
}

const MOROCCO_LAST_TRANSITION_END_MILLIS: i64 = 3_723_753_600_000;
const MOROCCO_ZONES: [&str; 2] = ["Africa/Casablanca", "Africa/El_Aaiun"];

fn zone_offsets_at(zone: &ZoneRow, wall_millis: i64) -> (i64, i64) {
    if wall_millis < UTC1900_MILLIS || (wall_millis >= MOROCCO_LAST_TRANSITION_END_MILLIS && MOROCCO_ZONES.contains(&zone.names[0])) {
        return (zone.raw_offset, 0);
    }
    let Some(tz) = java_zone(zone) else { return (zone.raw_offset, 0) };
    let Some(local) = chrono::DateTime::from_timestamp_millis(wall_millis).map(|instant| instant.naive_utc()) else { return (zone.raw_offset, 0) };
    match tz.offset_from_local_datetime(&local).earliest() {
        Some(offset) => split_offsets(zone, offset.base_utc_offset().num_milliseconds(), offset.dst_offset().num_milliseconds()),
        None => (zone.raw_offset, 0),
    }
}

fn zone_offset_at_instant(zone: &ZoneRow, instant_millis: i64) -> i64 {
    if instant_millis < UTC1900_MILLIS {
        return zone.raw_offset;
    }
    let Some(tz) = java_zone(zone) else { return zone.raw_offset };
    let Some(instant) = chrono::DateTime::from_timestamp_millis(instant_millis) else { return zone.raw_offset };
    let offset = tz.offset_from_utc_datetime(&instant.naive_utc());
    offset.base_utc_offset().num_milliseconds() + offset.dst_offset().num_milliseconds()
}

pub fn parse(text: &str) -> Option<i64> {
    SimpleDateFormat::new().parse(text)
}

pub fn parse_ban_list_date_time(text: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    let millis = BAN_LIST_DATE_FORMAT.lock().unwrap_or_else(std::sync::PoisonError::into_inner).parse(text)?;
    chrono::DateTime::from_timestamp_millis(millis).map(|date| date.fixed_offset())
}

pub fn parse_date_time(text: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    chrono::DateTime::from_timestamp_millis(parse(text)?).map(|date| date.fixed_offset())
}

#[cfg(test)]
mod tests {
    use super::SimpleDateFormat;

    #[test]
    fn shared_formats_remember_the_last_parsed_zone_and_try_the_default_zone_first() {
        let mut failures = Vec::new();
        let mut format: Option<(String, SimpleDateFormat)> = None;
        for line in include_str!("java_simple_date_format_sequences.tsv").lines() {
            let columns: Vec<&str> = line.split('\t').collect();
            let (default, text, expected, formatted_first, formatted_second) = (columns[0], columns[1], columns[2], columns[3], columns[4]);
            if format.as_ref().is_none_or(|(id, _)| id != default) {
                format = Some((default.to_string(), SimpleDateFormat::with_default_zone(default)));
            }
            let format = &mut format.as_mut().unwrap().1;
            let expected = if expected == "ERR" { None } else { Some(expected.parse::<i64>().unwrap()) };
            let actual = format.parse(text);
            let actual_formats = (format.format(1_700_000_000_000), format.format(1_720_000_000_000));
            if actual != expected || actual_formats.0 != formatted_first || actual_formats.1 != formatted_second {
                failures.push(format!("{default} {text}: expected {expected:?} {formatted_first} {formatted_second}, got {actual:?} {} {}", actual_formats.0, actual_formats.1));
            }
        }
        assert!(failures.is_empty(), "{} mismatches, first: {:#?}", failures.len(), &failures[..failures.len().min(20)]);
    }

    #[test]
    fn parsing_matches_simple_date_format() {
        let mut failures = Vec::new();
        for line in include_str!("java_simple_date_format_cases.tsv").lines() {
            let (text, expected) = line.rsplit_once('\t').unwrap();
            let text = text.replace("\\t", "\t");
            let expected = if expected == "ERR" { None } else { Some(expected.parse::<i64>().unwrap()) };
            let actual = SimpleDateFormat::with_default_zone("Asia/Seoul").parse(&text);
            if actual != expected {
                failures.push(format!("{text:?}: expected {expected:?}, got {actual:?}"));
            }
        }
        assert!(failures.is_empty(), "{} mismatches, first: {:#?}", failures.len(), &failures[..failures.len().min(20)]);
    }
}
