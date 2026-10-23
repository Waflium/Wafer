use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::{Dynamic, OptionalDynamic};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::ENTITY;

const ARMOR_SLOT_NAMES: [&str; 4] = ["feet", "legs", "chest", "head"];
const HAND_SLOT_NAMES: [&str; 2] = ["mainhand", "offhand"];
const DEFAULT_CHANCE: f32 = 0.085;

pub struct DropChancesFormatFix {
    base: DataFixBase,
}

impl DropChancesFormatFix {
    pub fn new(output_schema: &Arc<Schema>) -> DropChancesFormatFix {
        DropChancesFormatFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for DropChancesFormatFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("DropChancesFormatFix", self.get_input_schema().get_type(ENTITY), |input| {
            input.update_remainder(|remainder| {
                let armor_drop_chances = parse_drop_chances(remainder.get("ArmorDropChances"));
                let hand_drop_chances = parse_drop_chances(remainder.get("HandDropChances"));
                let body_armor_drop_chance = remainder.get("body_armor_drop_chance").as_number().into_result().ok().map_or(DEFAULT_CHANCE, |number| number.float_value());
                let remainder = remainder.remove("ArmorDropChances").remove("HandDropChances").remove("body_armor_drop_chance");
                let mut slot_chances = remainder.empty_map();
                slot_chances = add_slot_chances(slot_chances, &armor_drop_chances, &ARMOR_SLOT_NAMES);
                slot_chances = add_slot_chances(slot_chances, &hand_drop_chances, &HAND_SLOT_NAMES);
                if body_armor_drop_chance != DEFAULT_CHANCE {
                    let body = remainder.create_float(body_armor_drop_chance);
                    slot_chances = slot_chances.set("body", body);
                }
                if slot_chances != remainder.empty_map() { remainder.set("drop_chances", slot_chances) } else { remainder }
            })
        })
    }
}

fn add_slot_chances(mut output: Dynamic, chances: &[f32], slot_names: &[&str]) -> Dynamic {
    let mut i = 0;
    while i < slot_names.len() && i < chances.len() {
        let slot = slot_names[i];
        let chance = chances[i];
        if chance != DEFAULT_CHANCE {
            let chance = output.create_float(chance);
            output = output.set(slot, chance);
        }
        i += 1;
    }
    output
}

fn parse_drop_chances(value: OptionalDynamic) -> Vec<f32> {
    value.as_stream().into_iter().map(|dynamic| dynamic.as_float(DEFAULT_CHANCE)).collect()
}
