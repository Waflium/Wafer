use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, field_finder, find_field};
use crate::datafixers::types::{const_type, named};
use crate::util::datafix::extra_data_fix_utils;
use crate::util::datafix::fixes::references::{ITEM_NAME, ITEM_STACK};

pub struct EntitySpawnerItemVariantComponentFix {
    base: DataFixBase,
}

impl EntitySpawnerItemVariantComponentFix {
    pub fn new(output_schema: &Arc<Schema>) -> EntitySpawnerItemVariantComponentFix {
        EntitySpawnerItemVariantComponentFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for EntitySpawnerItemVariantComponentFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let item_stack_type = self.get_input_schema().get_type(ITEM_STACK);
        let id_finder = field_finder("id", named(ITEM_NAME.type_name(), const_type(ConstType::NamespacedString)));
        let components_finder = find_field(&item_stack_type, "components");
        self.fix_type_everywhere_typed("ItemStack bucket_entity_data variants to separate components", item_stack_type, move |input| {
            let id = input.get_optional(&id_finder).and_then(|id| id.as_str().map(str::to_string)).unwrap_or_default();
            match id.as_str() {
                "minecraft:salmon_bucket" => input.update_typed(&components_finder, None, |components| apply(components, fix_salmon_bucket)),
                "minecraft:axolotl_bucket" => input.update_typed(&components_finder, None, |components| apply(components, fix_axolotl_bucket)),
                "minecraft:tropical_fish_bucket" => input.update_typed(&components_finder, None, |components| apply(components, fix_tropical_fish_bucket)),
                "minecraft:painting" => input.update_typed(&components_finder, None, |components| {
                    let ty = components.ty.clone();
                    crate::util::write_and_read_typed_or_throw(&components, &ty, fix_painting)
                }),
                _ => input,
            }
        })
    }
}

fn get_base_color(packed_variant: i32) -> &'static str {
    extra_data_fix_utils::dye_color_id_to_name(packed_variant >> 16 & 0xFF)
}

fn get_pattern_color(packed_variant: i32) -> &'static str {
    extra_data_fix_utils::dye_color_id_to_name(packed_variant >> 24 & 0xFF)
}

fn get_pattern(packed_variant: i32) -> &'static str {
    match packed_variant & 65535 {
        1 => "flopper",
        256 => "sunstreak",
        257 => "stripey",
        512 => "snooper",
        513 => "glitter",
        768 => "dasher",
        769 => "blockfish",
        1024 => "brinely",
        1025 => "betty",
        1280 => "spotty",
        1281 => "clayfish",
        _ => "kob",
    }
}

fn apply(components: Typed, fixer: fn(Dynamic, Dynamic) -> Dynamic) -> Typed {
    components.update_remainder(|remainder| match remainder.get("minecraft:bucket_entity_data").result() {
        Some(bucket_data) => fixer(remainder, bucket_data),
        None => remainder,
    })
}

fn fix_tropical_fish_bucket(remainder: Dynamic, bucket_data: Dynamic) -> Dynamic {
    let Some(old_variant) = bucket_data.get("BucketVariantTag").as_number().into_result().ok() else { return remainder };
    let packed_variant = old_variant.int_value();
    let pattern = get_pattern(packed_variant);
    let base_color = get_base_color(packed_variant);
    let pattern_color = get_pattern_color(packed_variant);
    let pattern = remainder.create_string(pattern);
    let base_color = remainder.create_string(base_color);
    let pattern_color = remainder.create_string(pattern_color);
    remainder
        .update("minecraft:bucket_entity_data", |b| b.remove("BucketVariantTag"))
        .set("minecraft:tropical_fish/pattern", pattern)
        .set("minecraft:tropical_fish/base_color", base_color)
        .set("minecraft:tropical_fish/pattern_color", pattern_color)
}

fn fix_axolotl_bucket(remainder: Dynamic, bucket_data: Dynamic) -> Dynamic {
    let Some(old_variant) = bucket_data.get("Variant").as_number().into_result().ok() else { return remainder };
    let new_variant = match old_variant.int_value() {
        1 => "wild",
        2 => "gold",
        3 => "cyan",
        4 => "blue",
        _ => "lucy",
    };
    let new_variant = remainder.create_string(new_variant);
    remainder.update("minecraft:bucket_entity_data", |b| b.remove("Variant")).set("minecraft:axolotl/variant", new_variant)
}

fn fix_salmon_bucket(remainder: Dynamic, bucket_data: Dynamic) -> Dynamic {
    match bucket_data.get("type").result() {
        None => remainder,
        Some(ty) => remainder.update("minecraft:bucket_entity_data", |b| b.remove("type")).set("minecraft:salmon/size", ty),
    }
}

fn fix_painting(components: Dynamic) -> Dynamic {
    let Some(entity_data) = components.get("minecraft:entity_data").result() else { return components };
    if entity_data.get("id").as_string().into_result().ok().is_none_or(|id| id != "minecraft:painting") {
        return components;
    }
    let result = entity_data.get("variant").result();
    let entity_data_remainder = entity_data.remove("variant");
    let mut components = components;
    if entity_data_remainder.clone().remove("id") == entity_data_remainder.empty_map() {
        components = components.remove("minecraft:entity_data");
    } else {
        components = components.set("minecraft:entity_data", entity_data_remainder);
    }
    if let Some(result) = result {
        components = components.set("minecraft:painting/variant", result);
    }
    components
}
