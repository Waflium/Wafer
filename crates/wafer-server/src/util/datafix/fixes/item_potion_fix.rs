use std::sync::Arc;

use wafer_protocol::nbt::Nbt;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, field_finder, find_field};
use crate::datafixers::types::{const_type, named};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::{ITEM_NAME, ITEM_STACK};

const SPLASH: i16 = 16384;
pub const DEFAULT: &str = "minecraft:water";

pub struct ItemPotionFix {
    base: DataFixBase,
}

impl ItemPotionFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> ItemPotionFix {
        ItemPotionFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for ItemPotionFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let item_stack_type = self.get_input_schema().get_type(ITEM_STACK);
        let item_name_type = named(ITEM_NAME.type_name(), const_type(ConstType::NamespacedString));
        let id_finder = field_finder("id", item_name_type.clone());
        let tag_finder = find_field(&item_stack_type, "tag");
        self.fix_type_everywhere_typed("ItemPotionFix", item_stack_type, move |input| {
            let id = input.get_optional(&id_finder);
            if id.as_ref().and_then(Value::as_str) == Some("minecraft:potion") {
                let mut rest = input.get_remainder();
                let tag = input.get_optional_typed(&tag_finder);
                let damage = rest.get("Damage").as_short(0);
                if let Some(tag) = tag {
                    let mut output = input;
                    let tag_rest = tag.get_remainder();
                    let potion = tag_rest.get("Potion").as_string();
                    if !potion.is_success() {
                        let potion_name = POTIONS[(damage & 127) as usize];
                        let potion = tag_rest.create_string(potion_name.unwrap_or(DEFAULT));
                        let new_tag = tag.set_remainder(tag_rest.set("Potion", potion));
                        output = output.set(&tag_finder, new_tag);
                        if damage & SPLASH == SPLASH {
                            output = output.set(&id_finder, Typed::new(item_name_type.clone(), Value::Prim(Nbt::String("minecraft:splash_potion".to_string()))));
                        }
                    }
                    if damage != 0 {
                        let zero = rest.create_short(0);
                        rest = rest.set("Damage", zero);
                    }
                    return output.set_remainder(rest);
                }
                return input;
            }
            input
        })
    }
}

const POTIONS: [Option<&str>; 128] = [
    Some("minecraft:water"),
    Some("minecraft:regeneration"),
    Some("minecraft:swiftness"),
    Some("minecraft:fire_resistance"),
    Some("minecraft:poison"),
    Some("minecraft:healing"),
    Some("minecraft:night_vision"),
    None,
    Some("minecraft:weakness"),
    Some("minecraft:strength"),
    Some("minecraft:slowness"),
    Some("minecraft:leaping"),
    Some("minecraft:harming"),
    Some("minecraft:water_breathing"),
    Some("minecraft:invisibility"),
    None,
    Some("minecraft:awkward"),
    Some("minecraft:regeneration"),
    Some("minecraft:swiftness"),
    Some("minecraft:fire_resistance"),
    Some("minecraft:poison"),
    Some("minecraft:healing"),
    Some("minecraft:night_vision"),
    None,
    Some("minecraft:weakness"),
    Some("minecraft:strength"),
    Some("minecraft:slowness"),
    Some("minecraft:leaping"),
    Some("minecraft:harming"),
    Some("minecraft:water_breathing"),
    Some("minecraft:invisibility"),
    None,
    Some("minecraft:thick"),
    Some("minecraft:strong_regeneration"),
    Some("minecraft:strong_swiftness"),
    Some("minecraft:fire_resistance"),
    Some("minecraft:strong_poison"),
    Some("minecraft:strong_healing"),
    Some("minecraft:night_vision"),
    None,
    Some("minecraft:weakness"),
    Some("minecraft:strong_strength"),
    Some("minecraft:slowness"),
    Some("minecraft:strong_leaping"),
    Some("minecraft:strong_harming"),
    Some("minecraft:water_breathing"),
    Some("minecraft:invisibility"),
    None,
    None,
    Some("minecraft:strong_regeneration"),
    Some("minecraft:strong_swiftness"),
    Some("minecraft:fire_resistance"),
    Some("minecraft:strong_poison"),
    Some("minecraft:strong_healing"),
    Some("minecraft:night_vision"),
    None,
    Some("minecraft:weakness"),
    Some("minecraft:strong_strength"),
    Some("minecraft:slowness"),
    Some("minecraft:strong_leaping"),
    Some("minecraft:strong_harming"),
    Some("minecraft:water_breathing"),
    Some("minecraft:invisibility"),
    None,
    Some("minecraft:mundane"),
    Some("minecraft:long_regeneration"),
    Some("minecraft:long_swiftness"),
    Some("minecraft:long_fire_resistance"),
    Some("minecraft:long_poison"),
    Some("minecraft:healing"),
    Some("minecraft:long_night_vision"),
    None,
    Some("minecraft:long_weakness"),
    Some("minecraft:long_strength"),
    Some("minecraft:long_slowness"),
    Some("minecraft:long_leaping"),
    Some("minecraft:harming"),
    Some("minecraft:long_water_breathing"),
    Some("minecraft:long_invisibility"),
    None,
    Some("minecraft:awkward"),
    Some("minecraft:long_regeneration"),
    Some("minecraft:long_swiftness"),
    Some("minecraft:long_fire_resistance"),
    Some("minecraft:long_poison"),
    Some("minecraft:healing"),
    Some("minecraft:long_night_vision"),
    None,
    Some("minecraft:long_weakness"),
    Some("minecraft:long_strength"),
    Some("minecraft:long_slowness"),
    Some("minecraft:long_leaping"),
    Some("minecraft:harming"),
    Some("minecraft:long_water_breathing"),
    Some("minecraft:long_invisibility"),
    None,
    Some("minecraft:thick"),
    Some("minecraft:regeneration"),
    Some("minecraft:swiftness"),
    Some("minecraft:long_fire_resistance"),
    Some("minecraft:poison"),
    Some("minecraft:strong_healing"),
    Some("minecraft:long_night_vision"),
    None,
    Some("minecraft:long_weakness"),
    Some("minecraft:strength"),
    Some("minecraft:long_slowness"),
    Some("minecraft:leaping"),
    Some("minecraft:strong_harming"),
    Some("minecraft:long_water_breathing"),
    Some("minecraft:long_invisibility"),
    None,
    None,
    Some("minecraft:regeneration"),
    Some("minecraft:swiftness"),
    Some("minecraft:long_fire_resistance"),
    Some("minecraft:poison"),
    Some("minecraft:strong_healing"),
    Some("minecraft:long_night_vision"),
    None,
    Some("minecraft:long_weakness"),
    Some("minecraft:strength"),
    Some("minecraft:long_slowness"),
    Some("minecraft:leaping"),
    Some("minecraft:strong_harming"),
    Some("minecraft:long_water_breathing"),
    Some("minecraft:long_invisibility"),
    None,
];
