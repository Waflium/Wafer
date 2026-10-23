use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::{Dynamic, OptionalDynamic};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{OpticFinder, Typed, find_field, remainder_finder, type_finder};
use crate::datafixers::types::{Type, and_all, equals, field, optional, remainder_type};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::CHUNK;
use crate::util::datafix::schemas::namespaced_schema::ensure_namespaced;

const OLD_INTERMEDIATE_TERRAIN_STATUSES: [&str; 2] = ["minecraft:noise", "minecraft:surface"];

pub struct MergeTerrainChunkStatusFix {
    base: DataFixBase,
}

impl MergeTerrainChunkStatusFix {
    pub fn new(schema: &Arc<Schema>) -> MergeTerrainChunkStatusFix {
        MergeTerrainChunkStatusFix { base: DataFixBase::new(schema, false) }
    }
}

struct FixedAndStatuses {
    fixed: Typed,
    old_status: String,
    old_target_status: Option<String>,
}

impl DataFix for MergeTerrainChunkStatusFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let chunk_type = self.get_input_schema().get_type(CHUNK);
        let sections_finder = find_field(&chunk_type, "sections");
        let section_type = match &*sections_finder.ty() {
            Type::List(element) => element.clone(),
            other => panic!("Not a list type: {other:?}"),
        };
        let biomes_type = section_type.find_field_type("biomes").expect("a biomes field");
        let block_states_type = section_type.find_field_type("block_states").expect("a block_states field");
        let expected_section_type = and_all(vec![optional(field("biomes", biomes_type)), optional(field("block_states", block_states_type)), remainder_type()]);
        if !equals(&section_type, &expected_section_type, false) {
            panic!("{section_type:?} did not match {expected_section_type:?}");
        }
        let section_finder = type_finder(expected_section_type);
        self.fix_type_everywhere_typed("MergeTerrainChunkStatusFix", chunk_type, move |chunk| {
            let statuses = fix_statuses(chunk);
            let mut chunk = statuses.fixed;
            let status_intermediate = OLD_INTERMEDIATE_TERRAIN_STATUSES.contains(&statuses.old_status.as_str());
            let target_status_intermediate = statuses.old_target_status.as_deref().is_some_and(|status| OLD_INTERMEDIATE_TERRAIN_STATUSES.contains(&status));
            if status_intermediate || target_status_intermediate {
                let only_remove_blocks_below_zero = statuses.old_target_status.is_some() && !target_status_intermediate;
                chunk = remove_block_states(chunk, &sections_finder, &section_finder, only_remove_blocks_below_zero);
            }
            chunk
        })
    }
}

fn remove_block_states(chunk: Typed, sections_finder: &OpticFinder, section_finder: &OpticFinder, only_below_zero: bool) -> Typed {
    chunk.update_remainder(|remainder| remainder.remove("Heightmaps").remove("blending_data")).update_typed(sections_finder, None, |sections| {
        sections.update(section_finder, |section| {
            let Value::Pair(biomes, block_states_and_remainder) = section else { return section };
            let Value::Pair(block_states, section_remainder) = *block_states_and_remainder else {
                return Value::Pair(biomes, block_states_and_remainder);
            };
            let Value::Dynamic(section_remainder) = *section_remainder else {
                return Value::Pair(biomes, Box::new(Value::Pair(block_states, section_remainder)));
            };
            let y = section_remainder.get("Y").as_byte(0);
            if y >= 0 && only_below_zero {
                Value::Pair(biomes, Box::new(Value::Pair(block_states, Box::new(Value::Dynamic(section_remainder)))))
            } else {
                Value::Pair(biomes, Box::new(Value::Pair(Box::new(Value::Right(Box::new(Value::Unit))), Box::new(Value::Dynamic(section_remainder.remove("block_states"))))))
            }
        })
    })
}

fn fix_statuses(chunk: Typed) -> FixedAndStatuses {
    let mut remainder = match chunk.get_or_create_typed(&remainder_finder()).value {
        Value::Dynamic(dynamic) => dynamic,
        _ => Dynamic::empty(),
    };
    let old_status = unwrap_status(remainder.get("Status"));
    let status = remainder.create_string(fix_status(&old_status));
    remainder = remainder.set("Status", status);
    let old_target_status = match remainder.get("below_zero_retrogen").result() {
        Some(below_zero_retrogen) => {
            let old_target_status = unwrap_status(below_zero_retrogen.get("target_status"));
            let target_status = remainder.create_string(fix_status(&old_target_status));
            let below_zero_retrogen = below_zero_retrogen.set("target_status", target_status);
            remainder = remainder.set("below_zero_retrogen", below_zero_retrogen);
            Some(old_target_status)
        }
        None => None,
    };
    FixedAndStatuses { fixed: chunk.set_remainder(remainder), old_status, old_target_status }
}

fn unwrap_status(status: OptionalDynamic) -> String {
    ensure_namespaced(&status.as_string_or("minecraft:empty"))
}

fn fix_status(status: &str) -> String {
    if status == "minecraft:carvers" {
        "minecraft:terrain".to_string()
    } else if OLD_INTERMEDIATE_TERRAIN_STATUSES.contains(&status) {
        "minecraft:biomes".to_string()
    } else {
        status.to_string()
    }
}
