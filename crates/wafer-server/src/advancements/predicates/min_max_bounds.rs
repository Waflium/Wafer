use wafer_protocol::nbt::Nbt;

use crate::commands::brigadier::{
    CommandSyntaxException, DynamicCommandExceptionType, READER_INVALID_DOUBLE, READER_INVALID_FLOAT, READER_INVALID_INT, SimpleCommandExceptionType,
    StringReader,
};
use crate::util::codec::{self, DataResult, DecodeResult, Record};
use crate::world::item::component::codec::Wrap;
use crate::world::item::component::custom;
use crate::world::item::component::types::Pair;

pub fn ints_codec() -> Pair {
    custom::ints_bounds()
}

pub fn doubles_codec() -> Pair {
    custom::doubles_bounds()
}

pub static ERROR_EMPTY: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.range.empty");
pub static ERROR_SWAPPED: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.range.swapped");

pub trait JavaToString {
    fn java_to_string(&self) -> String;
}

impl JavaToString for i32 {
    fn java_to_string(&self) -> String {
        self.to_string()
    }
}

impl JavaToString for f64 {
    fn java_to_string(&self) -> String {
        crate::nbt::java_double_to_string(*self)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Bounds<T> {
    pub min: Option<T>,
    pub max: Option<T>,
}

impl<T: Copy + PartialOrd> Bounds<T> {
    pub fn are_swapped(&self) -> bool {
        matches!((self.min, self.max), (Some(min), Some(max)) if min > max)
    }

    fn map<U>(&self, f: impl Fn(T) -> U) -> Bounds<U> {
        Bounds { min: self.min.map(&f), max: self.max.map(&f) }
    }

    pub fn is_any(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }

    pub fn as_range_string(&self) -> String
    where
        T: JavaToString,
    {
        match (self.min, self.max) {
            (Some(min), Some(max)) => format!("[{}..{}]", min.java_to_string(), max.java_to_string()),
            (Some(min), None) => format!("[{}..+\u{221e})", min.java_to_string()),
            (None, Some(max)) => format!("(-\u{221e}..{}]", max.java_to_string()),
            (None, None) => "(-\u{221e}..+\u{221e})".to_string(),
        }
    }

    fn decode_partial(tag: &Nbt, number: fn(&Nbt) -> DataResult<T>) -> DecodeResult<Bounds<T>>
    where
        T: JavaToString,
    {
        let range = |tag: &Nbt| {
            codec::as_codec(|| {
                if let Err(error) = codec::map(tag) {
                    return DecodeResult::error(error);
                }
                let mut record = Record::default();
                let min = record.optional(tag, "min", number);
                let max = record.optional(tag, "max", number);
                record.build(|| Some(Bounds { min, max }))
            })
        };
        codec::wrapped(Wrap::FlatXmap, || {
            codec::wrapped(Wrap::Xmap, || codec::either_result(codec::either_first(|| range(tag)), || codec::either_second(|| DecodeResult::from(number(tag)))))
                .map(|either| match either {
                    Ok(bounds) => bounds,
                    Err(value) => Bounds { min: Some(value), max: Some(value) },
                })
                .validate(|bounds| match (bounds.min, bounds.max) {
                    (Some(min), Some(max)) if min > max => Err(format!("Swapped bounds in range: Optional[{}] is higher than Optional[{}]", min.java_to_string(), max.java_to_string())),
                    _ => Ok(()),
                })
        })
    }

    fn from_reader(
        reader: &mut StringReader,
        converter: fn(&str) -> Option<T>,
        parse_exc: &DynamicCommandExceptionType,
    ) -> Result<Bounds<T>, CommandSyntaxException> {
        if !reader.can_read() {
            return Err(ERROR_EMPTY.create_with_context(reader));
        }
        let start = reader.cursor;
        let result = (|| {
            let min = read_number(reader, converter, parse_exc)?;
            let max = if reader.can_read_n(2) && reader.peek() == b'.' as u16 && reader.peek_at(1) == b'.' as u16 {
                reader.skip();
                reader.skip();
                read_number(reader, converter, parse_exc)?
            } else {
                min
            };
            if min.is_none() && max.is_none() {
                return Err(ERROR_EMPTY.create_with_context(reader));
            }
            Ok(Bounds { min, max })
        })();
        result.map_err(|mut e| {
            reader.cursor = start;
            e.cursor = start as i32;
            e
        })
    }
}

fn read_number<T>(reader: &mut StringReader, converter: fn(&str) -> Option<T>, parse_exc: &DynamicCommandExceptionType) -> Result<Option<T>, CommandSyntaxException> {
    let start = reader.cursor;
    while reader.can_read() && is_allowed_input_char(reader) {
        reader.skip();
    }
    let number = reader.substring(start, reader.cursor);
    if number.is_empty() {
        return Ok(None);
    }
    match converter(&number) {
        Some(value) => Ok(Some(value)),
        None => Err(parse_exc.create_with_context(reader, number)),
    }
}

fn is_allowed_input_char(reader: &StringReader) -> bool {
    let c = reader.peek();
    if (b'0' as u16..=b'9' as u16).contains(&c) || c == b'-' as u16 {
        true
    } else if c != b'.' as u16 {
        false
    } else {
        !reader.can_read_n(2) || reader.peek_at(1) != b'.' as u16
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Ints {
    pub bounds: Bounds<i32>,
}

impl Ints {
    pub const ANY: Ints = Ints { bounds: Bounds { min: None, max: None } };

    pub fn decode(tag: &Nbt) -> DataResult<Ints> {
        Ints::decode_partial(tag).into_result()
    }

    pub fn decode_partial(tag: &Nbt) -> DecodeResult<Ints> {
        codec::wrapped(Wrap::Xmap, || Bounds::decode_partial(tag, codec::int)).map(|bounds| Ints { bounds })
    }

    pub fn is_any(&self) -> bool {
        self.bounds.is_any()
    }

    pub fn matches_sqr(&self, value_sqr: i64) -> bool {
        let sqr = self.bounds.map(|v| v as i64 * v as i64);
        if sqr.min.is_some_and(|min| min > value_sqr) {
            return false;
        }
        sqr.max.is_none_or(|max| max >= value_sqr)
    }

    pub fn matches(&self, value: i32) -> bool {
        if self.bounds.min.is_some_and(|min| min > value) {
            return false;
        }
        self.bounds.max.is_none_or(|max| max >= value)
    }

    pub fn from_reader(reader: &mut StringReader) -> Result<Ints, CommandSyntaxException> {
        let start = reader.cursor;
        let bounds = Bounds::from_reader(reader, |s| s.parse().ok(), &READER_INVALID_INT)?;
        if bounds.are_swapped() {
            reader.cursor = start;
            return Err(ERROR_SWAPPED.create_with_context(reader));
        }
        Ok(Ints { bounds })
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Doubles {
    pub bounds: Bounds<f64>,
    bounds_sqr: Bounds<f64>,
}

impl Doubles {
    pub const ANY: Doubles = Doubles { bounds: Bounds { min: None, max: None }, bounds_sqr: Bounds { min: None, max: None } };

    fn new(bounds: Bounds<f64>) -> Doubles {
        Doubles { bounds, bounds_sqr: bounds.map(|v| v * v) }
    }

    pub fn decode(tag: &Nbt) -> DataResult<Doubles> {
        Doubles::decode_partial(tag).into_result()
    }

    pub fn decode_partial(tag: &Nbt) -> DecodeResult<Doubles> {
        codec::wrapped(Wrap::Xmap, || Bounds::decode_partial(tag, codec::double)).map(Doubles::new)
    }

    pub fn is_any(&self) -> bool {
        self.bounds.is_any()
    }

    pub fn matches(&self, value: f64) -> bool {
        if self.bounds.min.is_some_and(|min| min > value) {
            return false;
        }
        self.bounds.max.is_none_or(|max| !(max < value))
    }

    pub fn matches_sqr(&self, value_sqr: f64) -> bool {
        if self.bounds_sqr.min.is_some_and(|min| min > value_sqr) {
            return false;
        }
        self.bounds_sqr.max.is_none_or(|max| !(max < value_sqr))
    }

    pub fn from_reader(reader: &mut StringReader) -> Result<Doubles, CommandSyntaxException> {
        let start = reader.cursor;
        let bounds = Bounds::from_reader(reader, |s| s.parse().ok(), &READER_INVALID_DOUBLE)?;
        if bounds.are_swapped() {
            reader.cursor = start;
            return Err(ERROR_SWAPPED.create_with_context(reader));
        }
        Ok(Doubles::new(bounds))
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FloatDegrees {
    pub bounds: Bounds<f32>,
}

impl FloatDegrees {
    pub fn from_reader(reader: &mut StringReader) -> Result<FloatDegrees, CommandSyntaxException> {
        Ok(FloatDegrees { bounds: Bounds::from_reader(reader, |s| s.parse().ok(), &READER_INVALID_FLOAT)? })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ints(input: &str) -> Result<(Ints, usize), String> {
        let units: Vec<u16> = input.encode_utf16().collect();
        let mut reader = StringReader::new(&units);
        Ints::from_reader(&mut reader).map(|r| (r, reader.cursor)).map_err(|e| e.get_message())
    }

    fn doubles(input: &str) -> Result<Doubles, String> {
        let units: Vec<u16> = input.encode_utf16().collect();
        Doubles::from_reader(&mut StringReader::new(&units)).map_err(|e| e.get_message())
    }

    #[test]
    fn range_text_follows_guava() {
        assert_eq!(Bounds { min: Some(-1), max: Some(-1) }.as_range_string(), "[-1..-1]");
        assert_eq!(Bounds { min: Some(1), max: None }.as_range_string(), "[1..+\u{221e})");
        assert_eq!(Bounds { min: None, max: Some(8) }.as_range_string(), "(-\u{221e}..8]");
        assert_eq!(Bounds::<i32> { min: None, max: None }.as_range_string(), "(-\u{221e}..+\u{221e})");
    }

    #[test]
    fn a_failed_bound_keeps_the_other_one() {
        let tag = crate::nbt::parse_fully(r#"{min:1,max:"x"}"#).unwrap();
        let result = codec::with_nbt_ops(|| Ints::decode_partial(&tag));
        assert_eq!(result.value.map(|ints| ints.bounds), Some(Bounds { min: Some(1), max: None }));
        assert_eq!(result.error.as_deref(), Some("Not a number"));
    }

    #[test]
    fn reads_ranges() {
        let (range, cursor) = ints("1..5]").unwrap();
        assert_eq!((range.bounds, cursor), (Bounds { min: Some(1), max: Some(5) }, 4));
        assert_eq!(ints("..-2").unwrap().0.bounds, Bounds { min: None, max: Some(-2) });
        assert_eq!(ints("3").unwrap().0.bounds, Bounds { min: Some(3), max: Some(3) });
        assert_eq!(doubles("1.5..").unwrap().bounds, Bounds { min: Some(1.5), max: None });
        assert!(doubles("..2").unwrap().matches_sqr(4.0) && !doubles("..2").unwrap().matches_sqr(4.01));
    }

    #[test]
    fn errors_point_at_the_range() {
        assert_eq!(ints("").unwrap_err(), "Expected value or range of values at position 0: <--[HERE]");
        assert_eq!(ints("..").unwrap_err(), "Expected value or range of values at position 0: <--[HERE]");
        assert_eq!(ints("1-2").unwrap_err(), "Invalid integer '1-2' at position 0: <--[HERE]");
        assert_eq!(ints("5..1").unwrap_err(), "Min cannot be bigger than max at position 0: <--[HERE]");
        assert_eq!(doubles("1.2.3").unwrap_err(), "Invalid double '1.2.3' at position 0: <--[HERE]");
    }
}
