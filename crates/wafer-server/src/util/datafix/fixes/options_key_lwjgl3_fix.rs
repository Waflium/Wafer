use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::OPTIONS;

pub const KEY_UNKNOWN: &str = "key.unknown";

fn map_get_or_default(key: i32) -> &'static str {
    match key {
        0 => "key.unknown",
        11 => "key.0",
        2 => "key.1",
        3 => "key.2",
        4 => "key.3",
        5 => "key.4",
        6 => "key.5",
        7 => "key.6",
        8 => "key.7",
        9 => "key.8",
        10 => "key.9",
        30 => "key.a",
        40 => "key.apostrophe",
        48 => "key.b",
        43 => "key.backslash",
        14 => "key.backspace",
        46 => "key.c",
        58 => "key.caps.lock",
        51 => "key.comma",
        32 => "key.d",
        211 => "key.delete",
        208 => "key.down",
        18 => "key.e",
        207 => "key.end",
        28 => "key.enter",
        13 => "key.equal",
        1 => "key.escape",
        33 => "key.f",
        59 => "key.f1",
        68 => "key.f10",
        87 => "key.f11",
        88 => "key.f12",
        100 => "key.f13",
        101 => "key.f14",
        102 => "key.f15",
        103 => "key.f16",
        104 => "key.f17",
        105 => "key.f18",
        113 => "key.f19",
        60 => "key.f2",
        61 => "key.f3",
        62 => "key.f4",
        63 => "key.f5",
        64 => "key.f6",
        65 => "key.f7",
        66 => "key.f8",
        67 => "key.f9",
        34 => "key.g",
        41 => "key.grave.accent",
        35 => "key.h",
        199 => "key.home",
        23 => "key.i",
        210 => "key.insert",
        36 => "key.j",
        37 => "key.k",
        82 => "key.keypad.0",
        79 => "key.keypad.1",
        80 => "key.keypad.2",
        81 => "key.keypad.3",
        75 => "key.keypad.4",
        76 => "key.keypad.5",
        77 => "key.keypad.6",
        71 => "key.keypad.7",
        72 => "key.keypad.8",
        73 => "key.keypad.9",
        78 => "key.keypad.add",
        83 => "key.keypad.decimal",
        181 => "key.keypad.divide",
        156 => "key.keypad.enter",
        141 => "key.keypad.equal",
        55 => "key.keypad.multiply",
        74 => "key.keypad.subtract",
        38 => "key.l",
        203 => "key.left",
        56 => "key.left.alt",
        26 => "key.left.bracket",
        29 => "key.left.control",
        42 => "key.left.shift",
        219 => "key.left.win",
        50 => "key.m",
        12 => "key.minus",
        49 => "key.n",
        69 => "key.num.lock",
        24 => "key.o",
        25 => "key.p",
        209 => "key.page.down",
        201 => "key.page.up",
        197 => "key.pause",
        52 => "key.period",
        183 => "key.print.screen",
        16 => "key.q",
        19 => "key.r",
        205 => "key.right",
        184 => "key.right.alt",
        27 => "key.right.bracket",
        157 => "key.right.control",
        54 => "key.right.shift",
        220 => "key.right.win",
        31 => "key.s",
        70 => "key.scroll.lock",
        39 => "key.semicolon",
        53 => "key.slash",
        57 => "key.space",
        20 => "key.t",
        15 => "key.tab",
        22 => "key.u",
        200 => "key.up",
        47 => "key.v",
        17 => "key.w",
        45 => "key.x",
        21 => "key.y",
        44 => "key.z",
        _ => KEY_UNKNOWN,
    }
}

fn parse_int(input: &str) -> i32 {
    let digits = input.strip_prefix(['-', '+']).unwrap_or(input);
    if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
        panic!("For input string: \"{input}\"");
    }
    let magnitude = digits.bytes().try_fold(0i64, |value, c| value.checked_mul(10)?.checked_add((c - b'0') as i64).filter(|&value| value <= 1 << 31));
    match magnitude.map(|magnitude| if input.starts_with('-') { -magnitude } else { magnitude }).and_then(|value| i32::try_from(value).ok()) {
        Some(value) => value,
        None => panic!("For input string: \"{input}\""),
    }
}

pub struct OptionsKeyLwjgl3Fix {
    base: DataFixBase,
}

impl OptionsKeyLwjgl3Fix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> OptionsKeyLwjgl3Fix {
        OptionsKeyLwjgl3Fix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for OptionsKeyLwjgl3Fix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("OptionsKeyLwjgl3Fix", self.get_input_schema().get_type(OPTIONS), |input| {
            input.update_remainder(|tag| match tag.get_map_values().into_result() {
                Ok(map) => {
                    let entries: Vec<_> = map
                        .into_iter()
                        .map(|(key, value)| {
                            if key.as_string_or("").starts_with("key_") {
                                let old_value = parse_int(&value.as_string_or(""));
                                if old_value < 0 {
                                    let button = old_value + 100;
                                    let name = match button {
                                        0 => "key.mouse.left".to_string(),
                                        1 => "key.mouse.right".to_string(),
                                        2 => "key.mouse.middle".to_string(),
                                        _ => format!("key.mouse.{}", button + 1),
                                    };
                                    let name = value.create_string(name);
                                    (key, name)
                                } else {
                                    let name = value.create_string(map_get_or_default(old_value));
                                    (key, name)
                                }
                            } else {
                                (key, value)
                            }
                        })
                        .collect();
                    tag.create_map(entries)
                }
                Err(_) => tag,
            })
        })
    }
}
