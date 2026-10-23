use std::sync::Arc;

use wafer_protocol::nbt::Nbt;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{field_finder, remainder_finder};
use crate::datafixers::types::const_type;
use crate::datafixers::value::Value;
use crate::util::datafix::extra_data_fix_utils;
use crate::util::datafix::fixes::references::ENTITY;

pub struct BoatSplitFix {
    base: DataFixBase,
}

impl BoatSplitFix {
    pub fn new(output_schema: &Arc<Schema>) -> BoatSplitFix {
        BoatSplitFix { base: DataFixBase::new(output_schema, true) }
    }
}

fn is_normal_boat(id: &str) -> bool {
    id == "minecraft:boat"
}

fn is_chest_boat(id: &str) -> bool {
    id == "minecraft:chest_boat"
}

fn is_any_boat(id: &str) -> bool {
    is_normal_boat(id) || is_chest_boat(id)
}

fn map_variant_to_normal_boat(id: &str) -> &'static str {
    match id {
        "spruce" => "minecraft:spruce_boat",
        "birch" => "minecraft:birch_boat",
        "jungle" => "minecraft:jungle_boat",
        "acacia" => "minecraft:acacia_boat",
        "cherry" => "minecraft:cherry_boat",
        "dark_oak" => "minecraft:dark_oak_boat",
        "mangrove" => "minecraft:mangrove_boat",
        "bamboo" => "minecraft:bamboo_raft",
        _ => "minecraft:oak_boat",
    }
}

fn map_variant_to_chest_boat(id: &str) -> &'static str {
    match id {
        "spruce" => "minecraft:spruce_chest_boat",
        "birch" => "minecraft:birch_chest_boat",
        "jungle" => "minecraft:jungle_chest_boat",
        "acacia" => "minecraft:acacia_chest_boat",
        "cherry" => "minecraft:cherry_chest_boat",
        "dark_oak" => "minecraft:dark_oak_chest_boat",
        "mangrove" => "minecraft:mangrove_chest_boat",
        "bamboo" => "minecraft:bamboo_chest_raft",
        _ => "minecraft:oak_chest_boat",
    }
}

impl DataFix for BoatSplitFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let id_f = field_finder("id", const_type(ConstType::NamespacedString));
        let old_type = self.get_input_schema().get_type(ENTITY);
        let new_type = self.get_output_schema().get_type(ENTITY);
        let output_type = new_type.clone();
        self.fix_type_everywhere_typed_to("BoatSplitFix", old_type, new_type, move |input| {
            let id = input.get_optional(&id_f).and_then(|id| id.as_str().map(str::to_string));
            match id {
                Some(id) if is_any_boat(&id) => {
                    let tag = match input.get_or_create_typed(&remainder_finder()).value {
                        Value::Dynamic(tag) => tag,
                        _ => Dynamic::empty(),
                    };
                    let maybe_boat_id = tag.get("Type").as_string().into_result().ok();
                    let new_id = if is_chest_boat(&id) {
                        maybe_boat_id.as_deref().map_or("minecraft:oak_chest_boat", map_variant_to_chest_boat)
                    } else {
                        maybe_boat_id.as_deref().map_or("minecraft:oak_boat", map_variant_to_normal_boat)
                    };
                    extra_data_fix_utils::cast(&output_type, input).update_remainder(|remainder| remainder.remove("Type")).update(&id_f, |_| Value::Prim(Nbt::String(new_id.to_string())))
                }
                _ => extra_data_fix_utils::cast(&output_type, input),
            }
        })
    }
}
