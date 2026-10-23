use std::sync::Arc;

use wafer_core::mth;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{OpticFinder, Typed, field_finder, find_field, type_finder};
use crate::datafixers::types::{Type, list, named, remainder_type};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::{BLOCK_STATE, CHUNK};

pub struct BitStorageAlignFix {
    base: DataFixBase,
}

impl BitStorageAlignFix {
    pub fn new(schema: &Arc<Schema>) -> BitStorageAlignFix {
        BitStorageAlignFix { base: DataFixBase::new(schema, false) }
    }
}

impl DataFix for BitStorageAlignFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let chunk_type = self.get_input_schema().get_type(CHUNK);
        let level_type = chunk_type.find_field_type("Level").expect("a Level field");
        let level_finder = field_finder("Level", level_type.clone());
        let sections_finder = find_field(&level_type, "Sections");
        let section_type = match &*sections_finder.ty() {
            Type::List(element) => element.clone(),
            other => panic!("Not a list type: {other:?}"),
        };
        let section_finder = type_finder(section_type);
        let block_state_type = named(BLOCK_STATE.type_name(), remainder_type());
        let palette_finder = field_finder("Palette", list(block_state_type));
        self.fix_type_everywhere_typed_to("BitStorageAlignFix", chunk_type, self.get_output_schema().get_type(CHUNK), move |chunk| {
            chunk.update_typed(&level_finder, None, |level| update_heightmaps(update_sections(&sections_finder, &section_finder, &palette_finder, level)))
        })
    }
}

fn update_heightmaps(level: Typed) -> Typed {
    level.update_remainder(|tag| {
        tag.update("Heightmaps", |heightmaps| heightmaps.update_map_values(|(key, heightmap)| (key, update_bit_storage(heightmap, 256, 9))))
    })
}

fn update_sections(sections_finder: &OpticFinder, section_finder: &OpticFinder, palette_finder: &OpticFinder, level: Typed) -> Typed {
    level.update_typed(sections_finder, None, |sections| {
        sections.update_typed(section_finder, None, |section| {
            let bits = match section.get_optional(palette_finder) {
                Some(Value::List(palette)) => 4.max(mth::ceillog2(palette.len() as i32)),
                _ => 0,
            };
            if bits != 0 && !mth::is_power_of_two(bits) {
                section.update_remainder(|tag| tag.update("BlockStates", |states| update_bit_storage(states, 4096, bits)))
            } else {
                section
            }
        })
    })
}

fn update_bit_storage(storage: Dynamic, size: i32, bits: i32) -> Dynamic {
    let input = storage.as_long_stream();
    let output = add_padding(size, bits, &input);
    storage.create_long_list(output)
}

pub fn add_padding(size: i32, bits: i32, data: &[i64]) -> Vec<i64> {
    let data_length = data.len();
    if data_length == 0 {
        return data.to_vec();
    }
    let mask = (1i64 << bits) - 1;
    let values_per_long = 64 / bits;
    let required_length = (size + values_per_long - 1) / values_per_long;
    let mut result = vec![0i64; required_length as usize];
    let mut output_data_index = 0usize;
    let mut output_start = 0;
    let mut output_data = 0i64;
    let mut current_index = 0;
    let mut current = data[0];
    let mut next = if data_length > 1 { data[1] } else { 0 };
    for index in 0..size {
        let position = index * bits;
        let start_data = position >> 6;
        let end_data = (index + 1) * bits - 1 >> 6;
        let start_bit = position ^ start_data << 6;
        if start_data != current_index {
            current = next;
            next = if ((start_data + 1) as usize) < data_length { data[(start_data + 1) as usize] } else { 0 };
            current_index = start_data;
        }
        let value_to_insert = if start_data == end_data {
            ((current as u64) >> start_bit) as i64 & mask
        } else {
            let shift_bits = 64 - start_bit;
            ((((current as u64) >> start_bit) as i64) | next.wrapping_shl(shift_bits as u32)) & mask
        };
        let output_end = output_start + bits;
        if output_end >= 64 {
            result[output_data_index] = output_data;
            output_data_index += 1;
            output_data = value_to_insert;
            output_start = bits;
        } else {
            output_data |= value_to_insert.wrapping_shl(output_start as u32);
            output_start = output_end;
        }
    }
    if output_data != 0 {
        result[output_data_index] = output_data;
    }
    result
}
