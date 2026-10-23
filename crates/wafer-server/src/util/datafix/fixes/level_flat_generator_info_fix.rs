use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::block_state_data;
use crate::util::datafix::fixes::entity_block_state_fix::EntityBlockStateFix;
use crate::util::datafix::fixes::references::LEVEL;

const GENERATOR_OPTIONS: &str = "generatorOptions";
const DEFAULT: &str = "minecraft:bedrock,2*minecraft:dirt,minecraft:grass_block;1;village";

pub struct LevelFlatGeneratorInfoFix {
    base: DataFixBase,
}

impl LevelFlatGeneratorInfoFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> LevelFlatGeneratorInfoFix {
        LevelFlatGeneratorInfoFix { base: DataFixBase::new(output_schema, changes_type) }
    }

    fn fix(input: Dynamic) -> Dynamic {
        if input.get("generatorName").as_string_or("").eq_ignore_ascii_case("flat") {
            input.update(GENERATOR_OPTIONS, |options| match options.as_string().into_result() {
                Ok(options_string) => options.create_string(LevelFlatGeneratorInfoFix::fix_string(&options_string)),
                Err(_) => options,
            })
        } else {
            input
        }
    }

    pub fn fix_string(generator_options: &str) -> String {
        if generator_options.is_empty() {
            return DEFAULT.to_string();
        }
        let mut parts = generator_options.splitn(5, ';');
        let first_part = parts.next().expect("a first part");
        let version;
        let layer_info;
        let mut rest = parts.peekable();
        if rest.peek().is_some() {
            version = to_int(first_part, 0);
            layer_info = rest.next().expect("layer info");
        } else {
            version = 0;
            layer_info = first_part;
        }
        if (0..=3).contains(&version) {
            let mut result = String::new();
            let separator = if version < 3 { 'x' } else { '*' };
            let layers: Vec<String> = layer_info
                .split(',')
                .map(|layer_string| {
                    let list: Vec<&str> = layer_string.splitn(2, separator).collect();
                    let height;
                    let layer_type;
                    if list.len() == 2 {
                        height = to_int(list[0], 0);
                        layer_type = list[1];
                    } else {
                        height = 1;
                        layer_type = list[0];
                    }
                    let layer_parts: Vec<&str> = layer_type.splitn(3, ':').collect();
                    let name_index = if layer_parts[0] == "minecraft" { 1 } else { 0 };
                    let block_string = layer_parts.get(name_index).copied().unwrap_or_else(|| panic!("Index {} out of bounds for length {}", name_index, layer_parts.len()));
                    let block_id = if version == 3 { EntityBlockStateFix::get_block_id(&format!("minecraft:{block_string}")) } else { to_int(block_string, 0) };
                    let data_index = name_index + 1;
                    let data = if layer_parts.len() > data_index { to_int(layer_parts[data_index], 0) } else { 0 };
                    let name = block_state_data::get_tag(block_id << 4 | data).get("Name").as_string_or("");
                    format!("{}{}", if height == 1 { String::new() } else { format!("{height}*") }, name)
                })
                .collect();
            result.push_str(&layers.join(","));
            for part in rest {
                result.push(';');
                result.push_str(part);
            }
            result
        } else {
            DEFAULT.to_string()
        }
    }
}

fn to_int(input: &str, default_value: i32) -> i32 {
    let digits = input.strip_prefix(['-', '+']).unwrap_or(input);
    if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return default_value;
    }
    let magnitude = digits.bytes().try_fold(0i64, |value, c| value.checked_mul(10)?.checked_add((c - b'0') as i64).filter(|&value| value <= 1 << 31));
    magnitude.map(|magnitude| if input.starts_with('-') { -magnitude } else { magnitude }).and_then(|value| i32::try_from(value).ok()).unwrap_or(default_value)
}

impl DataFix for LevelFlatGeneratorInfoFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("LevelFlatGeneratorInfoFix", self.get_input_schema().get_type(LEVEL), |input| input.update_remainder(LevelFlatGeneratorInfoFix::fix))
    }
}
