use serde_json::Value;

use super::dimension_type;
use super::dimension_type_string::direct_holder_to_string;
use crate::world::item::component::element_to_nbt;
use crate::world::item::component::types;

const BASE: &str = r##"{"has_skylight":true,"has_ceiling":false,"has_ender_dragon_fight":false,"coordinate_scale":1,"min_y":-64,"height":384,"logical_height":384,"infiniburn":"#minecraft:infiniburn_overworld","ambient_light":0,"monster_spawn_light_level":{"type":"minecraft:uniform","min_inclusive":0,"max_inclusive":7},"monster_spawn_block_light_limit":0}"##;

fn without_identity_hashes(text: &str) -> String {
    let hex_digits = |text: &str, limit: usize| text.bytes().take(limit).take_while(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')).count();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < text.len() {
        let rest = &text[index..];
        if let Some(after) = rest.strip_prefix('@') {
            let digits = hex_digits(after, 8);
            if digits > 0 {
                out.push_str("@HASH");
                index += 1 + digits;
                continue;
            }
        }
        if let Some(after) = rest.strip_prefix("Lambda/0x") {
            let digits = hex_digits(after, usize::MAX);
            if digits > 0 {
                out.push_str("Lambda/0xADDR");
                index += "Lambda/0x".len() + digits;
                continue;
            }
        }
        let character = rest.chars().next().unwrap();
        out.push(character);
        index += character.len_utf8();
    }
    out
}

fn dimension_type_json(extra: &str) -> Value {
    let mut json: Value = serde_json::from_str(BASE).unwrap();
    let extra: Value = serde_json::from_str(extra).unwrap();
    for (key, value) in extra.as_object().unwrap() {
        if value.is_null() {
            json.as_object_mut().unwrap().shift_remove(key);
        } else {
            json.as_object_mut().unwrap().insert(key.clone(), value.clone());
        }
    }
    json
}

fn describe(extra: &str) -> String {
    let text = serde_json::to_string(&dimension_type_json(extra)).unwrap();
    let element = crate::util::gson::parse_strict(&text).unwrap();
    let source = element_to_nbt(&element);
    let tag = crate::nbt::with_json_source(|| types::holder("minecraft:dimension_type", dimension_type::direct_codec()).0.decode(&source));
    let tag = match tag {
        Ok(_) => source,
        Err(error) => return format!("DECODE_ERROR {error}"),
    };
    without_identity_hashes(&direct_holder_to_string(&tag))
}

#[test]
fn direct_dimension_types_print_like_java_records() {
    let cases = include_str!("dimension_type_string_cases.txt");
    let expected = include_str!("dimension_type_string_expected.txt");
    let mut count = 0;
    let mut mismatches = Vec::new();
    for (case, expected) in cases.lines().zip(expected.lines()) {
        let (name, extra) = case.split_once('|').unwrap();
        let (expected_name, expected) = expected.split_once('|').unwrap();
        assert_eq!(name, expected_name);
        let actual = describe(extra);
        if actual != expected {
            let common = actual.chars().zip(expected.chars()).take_while(|(a, b)| a == b).count();
            mismatches.push(format!("{name}\n  actual:   ...{}\n  expected: ...{}", actual.chars().skip(common.saturating_sub(60)).take(220).collect::<String>(), expected.chars().skip(common.saturating_sub(60)).take(220).collect::<String>()));
        }
        count += 1;
    }
    assert_eq!(count, 139);
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
