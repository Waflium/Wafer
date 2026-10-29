use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::LazyLock;

use wafer_protocol::lazily_parsed_number;
use wafer_protocol::nbt::Nbt;

use crate::network::chat::Component;
use crate::network::chat::component_serialization;
use crate::util::codec::{self, DecodeResult, Record};
use crate::util::gson::JsonElement;
use crate::util::java_throwable::JavaThrowable;
use crate::world::flag::FeatureFlagSet;
use crate::world::item::component::element_to_nbt;

const LAST_PRE_MINOR_VERSION: i32 = 81;

const GET_OR_THROW: &str = "com.mojang.serialization.DataResult$Error.getOrThrow(DataResult.java:287)";
const GET_SECTION: &str = "net.minecraft.server.packs.resources.ResourceMetadata$2.getSection(ResourceMetadata.java:40)";
const GET_METADATA_SECTION_DECODE: &str = "net.minecraft.server.packs.AbstractPackMetadataResources.getMetadataSection(AbstractPackMetadataResources.java:26)";
const PARSE_BIG_DECIMAL: &str = "com.google.gson.internal.NumberLimits.parseBigDecimal(NumberLimits.java:27)";
const AS_BIG_DECIMAL: &str = "com.google.gson.internal.LazilyParsedNumber.asBigDecimal(LazilyParsedNumber.java:41)";
const INT_VALUE: &str = "com.google.gson.internal.LazilyParsedNumber.intValue(LazilyParsedNumber.java:52)";

static NUMBER_FORMAT_TRACES: LazyLock<HashMap<&'static str, Vec<&'static str>>> = LazyLock::new(|| {
    include_str!("number_format_traces.tsv")
        .lines()
        .map(|line| {
            let mut columns = line.split('\t');
            (columns.next().expect("a trace key"), columns.collect())
        })
        .collect()
});

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct PackFormat {
    pub major: i32,
    pub minor: i32,
}

impl PackFormat {
    pub const CURRENT: PackFormat = PackFormat { major: 121, minor: 0 };

    pub const fn of(major: i32, minor: i32) -> PackFormat {
        PackFormat { major, minor }
    }

    pub fn minor_range(self) -> FormatRange {
        FormatRange { min: self, max: PackFormat::of(self.major, i32::MAX) }
    }
}

impl std::fmt::Display for PackFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.minor == i32::MAX { write!(f, "{}.*", self.major) } else { write!(f, "{}.{}", self.major, self.minor) }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FormatRange {
    pub min: PackFormat,
    pub max: PackFormat,
}

impl FormatRange {
    pub fn contains(&self, format: PackFormat) -> bool {
        format >= self.min && format <= self.max
    }
}

#[derive(Clone, Debug)]
pub struct PackMetadataSection {
    pub description: Component,
    pub supported_formats: FormatRange,
}

struct Context {
    section: &'static str,
    thrown: RefCell<Option<JavaThrowable>>,
}

impl Context {
    fn new(section: &'static str) -> Context {
        Context { section, thrown: RefCell::new(None) }
    }

    fn int(&self, tag: &Nbt, location: &str) -> DecodeResult<i32> {
        if let Nbt::LazilyParsedNumber(text) = tag
            && let Some(message) = lazily_parsed_number::number_limits_error(text)
        {
            let mut thrown = self.thrown.borrow_mut();
            if thrown.is_none() {
                let mut frames = vec![PARSE_BIG_DECIMAL, AS_BIG_DECIMAL, INT_VALUE];
                frames.extend(NUMBER_FORMAT_TRACES.get(format!("{}.{location}", self.section).as_str()).map_or(&[][..], Vec::as_slice));
                frames.extend([GET_SECTION, GET_METADATA_SECTION_DECODE]);
                *thrown = Some(JavaThrowable::new(format!("java.lang.NumberFormatException: {message}"), frames));
            }
            return DecodeResult::error(message);
        }
        codec::int(tag).into()
    }

    fn non_negative_int(&self, tag: &Nbt, location: &str) -> DecodeResult<i32> {
        let value = self.int(tag, location);
        match value.value {
            Some(number) if value.error.is_none() && number < 0 => DecodeResult::error(format!("Value must be non-negative: {number}")),
            _ => value,
        }
    }

    fn pack_format(&self, tag: &Nbt, default_minor: i32, location: &str) -> DecodeResult<PackFormat> {
        let index = Cell::new(0);
        let list = codec::list_between_partial(tag, 1, 256, |element| {
            let position = if index.replace(1) == 0 { format!("{location}[0]") } else { format!("{location}[1]") };
            self.non_negative_int(element, &position)
        });
        codec::either_result(list, || self.non_negative_int(tag, location)).map(|format| match format {
            Ok(list) => PackFormat::of(list[0], list.get(1).copied().unwrap_or(default_minor)),
            Err(major) => PackFormat::of(major, default_minor),
        })
    }

    fn int_range(&self, tag: &Nbt, location: &str) -> DecodeResult<(i32, i32)> {
        let create = |min: i32, max: i32| {
            if min > max { DecodeResult::error("min_inclusive must be less than or equal to max_inclusive") } else { DecodeResult::success((min, max)) }
        };
        let array = || {
            let index = Cell::new(0);
            let list = codec::list_partial(tag, |element| {
                let position = if index.replace(1) == 0 { format!("{location}[0]") } else { format!("{location}[1]") };
                self.int(element, &position)
            });
            list.flat_map(|mut elements| {
                if elements.len() != 2 {
                    let value = (elements.len() >= 2).then(|| {
                        elements.truncate(2);
                        (elements[0], elements[1])
                    });
                    return DecodeResult { value, error: Some("Input is not a list of 2 elements".to_string()) };
                }
                create(elements[0], elements[1])
            })
        };
        let object = || {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = Record::default();
            let min = record.required(tag, "min_inclusive", |value| self.int(value, &format!("{location}.min_inclusive")));
            let max = record.required(tag, "max_inclusive", |value| self.int(value, &format!("{location}.max_inclusive")));
            record.build(|| Some((min?, max?))).flat_map(|(min, max)| create(min, max))
        };
        let array_or_object = || codec::either_result(array(), object).map(|range| match range {
            Ok(range) | Err(range) => range,
        });
        codec::either_result(self.int(tag, location), array_or_object).flat_map(|range| match range {
            Ok(min) => DecodeResult::success((min, min)),
            Err(range) => DecodeResult::success(range),
        })
    }
}

#[derive(Debug, Default)]
struct IntermediaryFormat {
    min: Option<PackFormat>,
    max: Option<PackFormat>,
    format: Option<i32>,
    supported: Option<(i32, i32)>,
}

impl IntermediaryFormat {
    fn pack(tag: &Nbt, context: &Context) -> DecodeResult<IntermediaryFormat> {
        let mut record = Record::default();
        let min = record.optional(tag, "min_format", |value| context.pack_format(value, 0, "min_format"));
        let max = record.optional(tag, "max_format", |value| context.pack_format(value, i32::MAX, "max_format"));
        let format = record.optional(tag, "pack_format", |value| context.int(value, "pack_format"));
        let supported = record.optional(tag, "supported_formats", |value| context.int_range(value, "supported_formats"));
        record.build(|| Some(IntermediaryFormat { min, max, format, supported }))
    }

    fn overlay(tag: &Nbt, context: &Context) -> DecodeResult<IntermediaryFormat> {
        let mut record = Record::default();
        let min = record.optional(tag, "min_format", |value| context.pack_format(value, 0, "min_format"));
        let max = record.optional(tag, "max_format", |value| context.pack_format(value, i32::MAX, "max_format"));
        let supported = record.optional(tag, "formats", |value| context.int_range(value, "formats"));
        record.build(|| Some(IntermediaryFormat { min, max, format: min.map(|min| min.major), supported }))
    }

    fn effective_min_major_version(&self) -> i32 {
        match (self.min, self.supported) {
            (Some(min), Some(supported)) => min.major.min(supported.0),
            (Some(min), None) => min.major,
            (None, Some(supported)) => supported.0,
            (None, None) => i32::MAX,
        }
    }

    fn validate(&self, has_pack_format_field: bool, require_old_field: bool, context: &str, old_field_name: &str) -> Result<FormatRange, String> {
        let last = LAST_PRE_MINOR_VERSION;
        if self.min.is_some() != self.max.is_some() {
            return Err(format!("{context} missing field, must declare both min_format and max_format"));
        }
        if require_old_field && self.supported.is_none() {
            return Err(format!(
                "{context} missing required field {old_field_name}, must be present in all overlays for any overlays to work across game versions"
            ));
        }
        if let (Some(min), Some(max)) = (self.min, self.max) {
            return self.validate_new_format(min, max, has_pack_format_field, require_old_field, context, old_field_name);
        }
        if let Some((min, max)) = self.supported {
            if max > last {
                return Err(format!("{context} declares support for version newer than {last}, but is missing mandatory fields min_format and max_format"));
            }
            if has_pack_format_field {
                let Some(format) = self.format else {
                    return Err(format!(
                        "{context} declares support for formats up to {last}, but game versions supporting formats 15 to {last} require a pack_format field. Add \"pack_format\": {min} or require a version greater or equal to {}.0.",
                        last + 1
                    ));
                };
                validate_pack_format_for_range(format, min, max)?;
            }
            return Ok(FormatRange { min: PackFormat::of(min, 0), max: PackFormat::of(max, 0) });
        }
        if has_pack_format_field && let Some(format) = self.format {
            if format > last {
                return Err(format!("{context} declares support for version newer than {last}, but is missing mandatory fields min_format and max_format"));
            }
            return Ok(FormatRange { min: PackFormat::of(format, 0), max: PackFormat::of(format, 0) });
        }
        Err(format!("{context} could not be parsed, missing format version information"))
    }

    fn validate_new_format(
        &self,
        min: PackFormat,
        max: PackFormat,
        has_pack_format_field: bool,
        require_old_field: bool,
        context: &str,
        old_field_name: &str,
    ) -> Result<FormatRange, String> {
        let last = LAST_PRE_MINOR_VERSION;
        let (major_min, major_max) = (min.major, max.major);
        if min > max {
            return Err(format!("{context} min_format ({min}) is greater than max_format ({max})"));
        }
        if major_min > last && !require_old_field {
            if self.supported.is_some() {
                return Err(format!(
                    "{context} key {old_field_name} is deprecated starting from pack format {}. Remove {old_field_name} from your pack.mcmeta.",
                    last + 1
                ));
            }
            if has_pack_format_field && let Some(format) = self.format {
                validate_pack_format_for_range(format, major_min, major_max)?;
            }
        } else {
            let Some(supported) = self.supported else {
                return Err(format!(
                    "{context} declares support for format {major_min}, but game versions supporting formats 15 to {last} require a {old_field_name} field. Add \"{old_field_name}\": [{major_min}, {last}] or require a version greater or equal to {}.0.",
                    last + 1
                ));
            };
            if supported.0 != major_min {
                return Err(format!("{context} version declaration mismatch between {old_field_name} (from {}) and min_format ({min})", supported.0));
            }
            if supported.1 != major_max && supported.1 != last {
                return Err(format!("{context} version declaration mismatch between {old_field_name} (up to {}) and max_format ({max})", supported.1));
            }
            if has_pack_format_field {
                let Some(format) = self.format else {
                    return Err(format!(
                        "{context} declares support for formats up to {last}, but game versions supporting formats 15 to {last} require a pack_format field. Add \"pack_format\": {major_min} or require a version greater or equal to {}.0.",
                        last + 1
                    ));
                };
                validate_pack_format_for_range(format, major_min, major_max)?;
            }
        }
        Ok(FormatRange { min, max })
    }
}

fn validate_pack_format_for_range(format: i32, min: i32, max: i32) -> Result<(), String> {
    if format < min || format > max {
        return Err(format!("Pack declared support for versions {min} to {max} but declared main format is {format}"));
    }
    if format < 15 {
        return Err("Multi-version packs cannot support minimum version of less than 15, since this will leave versions in range unable to load pack.".into());
    }
    Ok(())
}

fn flat_validate<T>(result: DecodeResult<T>, validate: impl FnOnce(&T) -> Result<FormatRange, String>) -> DecodeResult<FormatRange> {
    result.flat_map(|value| match validate(&value) {
        Ok(range) => DecodeResult::success(range),
        Err(error) => DecodeResult::error(error),
    })
}

fn describe(tag: &Nbt) -> DecodeResult<Component> {
    component_serialization::decode_partial(tag)
}

fn decode_pack(tag: &Nbt, context: &Context) -> DecodeResult<PackMetadataSection> {
    if let Err(error) = codec::map(tag) {
        return DecodeResult::error(error);
    }
    let mut record = Record::default();
    let description = record.required(tag, "description", describe);
    let formats = flat_validate(IntermediaryFormat::pack(tag, context), |format| format.validate(true, false, "Pack", "supported_formats"));
    let supported_formats = record.absorb(formats);
    record.build(|| Some(PackMetadataSection { description: description?, supported_formats: supported_formats? }))
}

fn decode_fallback_pack(tag: &Nbt) -> DecodeResult<PackMetadataSection> {
    if let Err(error) = codec::map(tag) {
        return DecodeResult::error(error);
    }
    let mut record = Record::default();
    let description = record.required(tag, "description", describe);
    let unknown = PackFormat::of(i32::MAX, 0);
    record.build(|| Some(PackMetadataSection { description: description?, supported_formats: FormatRange { min: unknown, max: unknown } }))
}

fn decode_features(tag: &Nbt) -> DecodeResult<FeatureFlagSet> {
    if let Err(error) = codec::map(tag) {
        return DecodeResult::error(error);
    }
    let mut record = Record::default();
    let flags = record.required(tag, "enabled", |enabled| {
        codec::list_partial(enabled, |name| DecodeResult::from(codec::identifier(name))).flat_map(|names| match FeatureFlagSet::from_names(names.iter().map(String::as_str)) {
            Ok(flags) => DecodeResult::success(flags),
            Err(error) => DecodeResult::error(error),
        })
    });
    record.build(|| flags)
}

fn decode_overlays(tag: &Nbt, context: &Context) -> DecodeResult<Vec<String>> {
    if let Err(error) = codec::map(tag) {
        return DecodeResult::error(error);
    }
    let mut record = Record::default();
    let overlays = record.required(tag, "entries", |entries| {
        let list = codec::list_partial(entries, |entry| decode_overlay_entry(entry, context));
        list.flat_map(|entries| validate_overlays(entries, context))
    });
    record.build(|| overlays)
}

fn decode_overlay_entry(tag: &Nbt, context: &Context) -> DecodeResult<(IntermediaryFormat, String)> {
    if let Err(error) = codec::map(tag) {
        return DecodeResult::error(error);
    }
    let mut record = Record::default();
    let format = record.absorb(IntermediaryFormat::overlay(tag, context));
    let directory = record.required(tag, "directory", |directory| {
        DecodeResult::from(codec::string(directory).map(str::to_string)).flat_map(|directory| {
            if !directory.is_empty() && directory.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.')) {
                DecodeResult::success(directory)
            } else {
                DecodeResult::error(format!("{directory} is not accepted directory name"))
            }
        })
    });
    record.build(|| Some((format?, directory?)))
}

fn validate_overlays(entries: Vec<(IntermediaryFormat, String)>, context: &Context) -> DecodeResult<Vec<String>> {
    let min_version = entries.iter().map(|(format, _)| format.effective_min_major_version()).min().unwrap_or(i32::MAX);
    let mut overlays = Vec::new();
    for (format, directory) in entries {
        if format.min.is_none() && format.max.is_none() && format.supported.is_none() {
            if context.thrown.borrow().is_none() {
                warn!("Unknown or broken overlay entry {directory}");
            }
            continue;
        }
        match format.validate(false, min_version <= LAST_PRE_MINOR_VERSION, &format!("Overlay \"{directory}\""), "formats") {
            Ok(range) => {
                if range.contains(PackFormat::CURRENT) {
                    overlays.push(directory);
                }
            }
            Err(error) => return DecodeResult::error(error),
        }
    }
    DecodeResult::success(overlays)
}

fn read_section<T>(root: &JsonElement, name: &'static str, context: &Context, decode: impl FnOnce(&Nbt, &Context) -> DecodeResult<T>) -> Result<Option<T>, JavaThrowable> {
    let Some(raw) = root.get(name) else { return Ok(None) };
    let result = crate::nbt::with_json_source(|| decode(&element_to_nbt(raw), context));
    if let Some(thrown) = context.thrown.borrow_mut().take() {
        return Err(thrown);
    }
    match result.error {
        None => Ok(Some(result.value.expect("a successful value"))),
        Some(message) => Err(JavaThrowable::new(format!("com.google.gson.JsonParseException: {message}"), vec![GET_OR_THROW, GET_SECTION, GET_METADATA_SECTION_DECODE])),
    }
}

pub fn pack_section(root: &JsonElement) -> Result<Option<PackMetadataSection>, JavaThrowable> {
    read_section(root, "pack", &Context::new("pack"), decode_pack)
}

pub fn fallback_pack_section(root: &JsonElement) -> Result<Option<PackMetadataSection>, JavaThrowable> {
    read_section(root, "pack", &Context::new("pack"), |tag, _| decode_fallback_pack(tag))
}

pub fn features_section(root: &JsonElement) -> Result<Option<FeatureFlagSet>, JavaThrowable> {
    read_section(root, "features", &Context::new("features"), |tag, _| decode_features(tag))
}

pub fn overlays_section(root: &JsonElement) -> Result<Option<Vec<String>>, JavaThrowable> {
    read_section(root, "overlays", &Context::new("overlay"), decode_overlays)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::gson::gson_helper_parse;

    fn root(json: &str) -> JsonElement {
        gson_helper_parse(json).expect("a JSON object")
    }

    fn pack_error(json: &str) -> JavaThrowable {
        pack_section(&root(json)).expect_err("a pack metadata error")
    }

    #[test]
    fn a_boolean_format_reports_the_either_chain_and_the_missing_field() {
        let error = pack_error(r#"{"pack":{"description":"x","min_format":true,"max_format":121}}"#);
        assert_eq!(
            error.description,
            "com.google.gson.JsonParseException: Failed to parse either. First: Not a json array: true; Second: Not a number: true; Pack missing field, must declare both min_format and max_format"
        );
        assert_eq!(error.stack_trace[0], GET_OR_THROW);
    }

    #[test]
    fn a_broken_overlay_format_reports_the_either_chain_of_the_range() {
        let error = overlays_section(&root(r#"{"overlays":{"entries":[{"formats":[1,"x"],"directory":"o"}]}}"#)).expect_err("an overlay error");
        assert_eq!(
            error.description,
            r#"com.google.gson.JsonParseException: Failed to parse either. First: Not a number: [1,"x"]; Second: Failed to parse either. First: Not a number: "x"; Input is not a list of 2 elements; Second: Not a JSON object: [1,"x"]"#
        );
    }

    #[test]
    fn a_huge_exponent_throws_a_number_format_exception_with_the_codec_frames() {
        let error = pack_error(r#"{"pack":{"description":"x","min_format":1E+10000,"max_format":121}}"#);
        assert_eq!(error.description, "java.lang.NumberFormatException: Number has unsupported scale: 1E+10000");
        assert_eq!(error.stack_trace[..3], [PARSE_BIG_DECIMAL, AS_BIG_DECIMAL, INT_VALUE]);
        assert!(error.stack_trace.iter().any(|frame| frame.contains("EitherCodec.decode")));
        assert_eq!(error.stack_trace.last(), Some(&GET_METADATA_SECTION_DECODE));
    }

    #[test]
    fn an_empty_document_ends_the_input() {
        let error = gson_helper_parse("").expect_err("an empty document");
        assert_eq!(error.description, "com.google.gson.JsonParseException: java.io.EOFException: End of input at line 1 column 1 path $");
        assert_eq!(error.cause.as_deref().map(|cause| cause.description.as_str()), Some("java.io.EOFException: End of input at line 1 column 1 path $"));
    }

    #[test]
    fn a_document_that_is_not_an_object_is_rejected() {
        let error = gson_helper_parse("[]").expect_err("an array document");
        assert_eq!(error.description, "com.google.gson.JsonSyntaxException: Expected a com.google.gson.JsonObject but was com.google.gson.JsonArray; at path $");
    }

    #[test]
    fn valid_sections_decode() {
        let section = pack_section(&root(r#"{"pack":{"description":"x","min_format":121,"max_format":[121,5]}}"#)).expect("a pack section").expect("present");
        assert_eq!(section.supported_formats, FormatRange { min: PackFormat::of(121, 0), max: PackFormat::of(121, 5) });
        let overlays = overlays_section(&root(r#"{"overlays":{"entries":[{"directory":"o","min_format":121,"max_format":121},{"directory":"p","min_format":122,"max_format":122}]}}"#))
            .expect("an overlays section")
            .expect("present");
        assert_eq!(overlays, vec!["o".to_string()]);
    }
}
