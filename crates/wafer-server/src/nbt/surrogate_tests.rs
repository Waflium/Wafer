use wafer_core::java_string::{JavaStr, from_utf16};
use wafer_protocol::nbt::Nbt;
use wafer_protocol::{Reader, Writer};

use crate::network::chat::component_serialization;
use crate::world::item::component::json_to_nbt;

const STRINGS: [&[u16]; 13] = [
    &[0xDBF6, 0xDC00],
    &[0xDBF7, 0xDFFF],
    &[0xD800],
    &[0xDC00],
    &[0x61, 0xD800, 0xE9],
    &[0x61, 0xD800, 0xD83D, 0xDE00, 0x62],
    &[0xDC00, 0xD800],
    &[0xD800, 0xD800, 0xDC00],
    &[0x78, 0xDBFF],
    &[0xDBF6, 0xDBF6, 0xDC00],
    &[0xDBF6, 0xDC00, 0xDC00],
    &[0xD800, 0x20AC],
    &[0xD800, 0x101],
];

const NETWORK_WRITES: [&str; 13] = [
    "04f48da080", "04f48dbfbf", "013f", "013f", "03613fe9", "05613f3f3f62", "023f3f", "033f3f3f", "02783f", "033f3f3f", "05f48da0803f", "023fac", "023f01",
];

const NBT_WRITES: [&str; 13] = [
    "0006edafb6edb080",
    "0006edafb7edbfbf",
    "0003eda080",
    "0003edb080",
    "000661eda080c3a9",
    "000b61eda080eda0bdedb88062",
    "0006edb080eda080",
    "0009eda080eda080edb080",
    "000478edafbf",
    "0009edafb6edafb6edb080",
    "0009edafb6edb080edb080",
    "0006eda080e282ac",
    "0005eda080c481",
];

const HASHES: [i32; 13] = [1801930, 1802984, 55296, 56320, 1807626, 1791864640, 1801216, 54909952, 60039, 55915840, 55916150, 1722540, 1714433];

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn bytes(hex: &str) -> Vec<u8> {
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect()
}

fn units(text: &str) -> Vec<u16> {
    text.java_utf16().collect()
}

#[test]
fn strings_are_written_to_the_network_like_netty() {
    for (i, input) in STRINGS.iter().enumerate() {
        let mut w = Writer::new();
        w.string(&from_utf16(input));
        assert_eq!(hex(w.as_slice()), NETWORK_WRITES[i], "{input:x?}");
    }
}

#[test]
fn strings_are_read_from_the_network_like_netty() {
    let cases: [(&str, &[u16]); 10] = [
        ("f48da080", &[0xDBF6, 0xDC00]),
        ("eda080", &[0xFFFD]),
        ("edafb6edb080", &[0xFFFD, 0xFFFD]),
        ("f4908080", &[0xFFFD, 0xFFFD, 0xFFFD, 0xFFFD]),
        ("c3", &[0xFFFD]),
        ("c080", &[0xFFFD, 0xFFFD]),
        ("61f48da08062", &[0x61, 0xDBF6, 0xDC00, 0x62]),
        ("f48daf", &[0xFFFD]),
        ("eda0", &[0xFFFD]),
        ("ff", &[0xFFFD]),
    ];
    for (input, expected) in cases {
        let mut data = vec![(input.len() / 2) as u8];
        data.extend(bytes(input));
        assert_eq!(units(&Reader::new(&data).string(32767).unwrap()), expected, "{input}");
    }
}

#[test]
fn a_genuine_carrier_range_character_round_trips_through_the_network() {
    let name = from_utf16(&[0x41, 0xDBF6, 0xDC00, 0x42]);
    let mut w = Writer::new();
    w.string(&name);
    assert_eq!(hex(w.as_slice()), "0641f48da08042");
    assert_eq!(Reader::new(w.as_slice()).string(16).unwrap(), name);
}

#[test]
fn strings_round_trip_through_nbt_like_data_output_streams() {
    for (i, input) in STRINGS.iter().enumerate() {
        let text = from_utf16(input);
        let mut w = Writer::new();
        Nbt::String(text.clone()).write(&mut w);
        assert_eq!(hex(&w.as_slice()[1..]), NBT_WRITES[i], "{input:x?}");
        let Nbt::String(read) = Nbt::read(&mut Reader::new(w.as_slice())).unwrap() else { panic!("not a string") };
        assert_eq!(units(&read), *input);
        assert_eq!(wafer_core::random::java_string_hash(&text), HASHES[i], "{input:x?}");
    }
}

#[test]
fn modified_utf8_rejects_what_data_input_streams_reject() {
    for input in ["f48da080", "c3", "eda0", "ff", "c341", "e08041"] {
        let mut data = vec![8, 0, (input.len() / 2) as u8];
        data.extend(bytes(input));
        assert!(Nbt::read(&mut Reader::new(&data)).is_err(), "{input}");
    }
    let mut data = vec![8, 0, 2, 0xC0, 0x80];
    assert_eq!(Nbt::read(&mut Reader::new(&data)).unwrap(), Nbt::String("\0".into()));
    data = vec![8, 0, 3, 0xED, 0xA0, 0x80];
    assert_eq!(Nbt::read(&mut Reader::new(&data)).unwrap(), Nbt::String(from_utf16(&[0xD800])));
}

#[test]
fn snbt_escapes_and_raw_characters_parse_like_vanilla() {
    let cases: [(&str, &[u16], &str); 5] = [
        ("{a:\"\\udbf6\\udc00\"}", &[0xDBF6, 0xDC00], "0a0000080001610006edafb6edb08000"),
        ("{a:\"\\ud800\"}", &[0xD800], "0a0000080001610003eda08000"),
        ("{a:\"\\U0010d800\"}", &[0xDBF6, 0xDC00], "0a0000080001610006edafb6edb08000"),
        ("{a:\"\u{10D800}\"}", &[0xDBF6, 0xDC00], "0a0000080001610006edafb6edb08000"),
        ("{a:'x\\ud800y'}", &[0x78, 0xD800, 0x79], "0a000008000161000578eda0807900"),
    ];
    for (input, expected, nbt) in cases {
        let input = wafer_core::java_string::from_code_points(input);
        let tag = crate::nbt::parse_fully(&input).unwrap();
        assert_eq!(units(tag.get("a").and_then(Nbt::as_str).unwrap()), expected, "{input}");
        let mut w = Writer::new();
        tag.write_named("", &mut w);
        assert_eq!(hex(w.as_slice()), nbt, "{input}");
    }
}

#[test]
fn json_text_components_round_trip_like_vanilla() {
    let json = from_utf16(&"{\"text\":\"\\udbf6\\udc00|\\ud800\"}".encode_utf16().collect::<Vec<_>>());
    let value = crate::util::gson::parse_strict_value(&json).unwrap();
    let component = component_serialization::decode(&json_to_nbt(&value)).unwrap();
    assert_eq!(units(&component.get_string()), [0xDBF6, 0xDC00, 0x7C, 0xD800]);
    let mut w = Writer::new();
    component.to_nbt().write(&mut w);
    assert_eq!(hex(w.as_slice()), "08000aedafb6edb0807ceda080");
    let raw = crate::util::gson::parse_strict(&from_utf16(&[0x22, 0xDBF6, 0xDC00, 0x22])).unwrap();
    assert_eq!(units(&crate::util::gson::to_json_string(&raw)), [0x22, 0xDBF6, 0xDC00, 0x22]);
    let lone = crate::util::gson::parse_strict("\"\\ud800\"").unwrap();
    assert_eq!(units(&crate::util::gson::to_json_string(&lone)), [0x22, 0xD800, 0x22]);
}

#[test]
fn text_is_logged_and_saved_like_the_jdk_encoder() {
    let text = from_utf16(&[0x61, 0xDBF6, 0xDC00, 0xD800, 0xE9]);
    assert_eq!(hex(wafer_core::java_string::to_utf8_lossy(&text).as_bytes()), "61f48da0803fc3a9");
}
