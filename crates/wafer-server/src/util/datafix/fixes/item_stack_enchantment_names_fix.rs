use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::find_field;
use crate::util::datafix::fixes::references::ITEM_STACK;

fn map(id: i32) -> &'static str {
    match id {
        0 => "minecraft:protection",
        1 => "minecraft:fire_protection",
        2 => "minecraft:feather_falling",
        3 => "minecraft:blast_protection",
        4 => "minecraft:projectile_protection",
        5 => "minecraft:respiration",
        6 => "minecraft:aqua_affinity",
        7 => "minecraft:thorns",
        8 => "minecraft:depth_strider",
        9 => "minecraft:frost_walker",
        10 => "minecraft:binding_curse",
        16 => "minecraft:sharpness",
        17 => "minecraft:smite",
        18 => "minecraft:bane_of_arthropods",
        19 => "minecraft:knockback",
        20 => "minecraft:fire_aspect",
        21 => "minecraft:looting",
        22 => "minecraft:sweeping",
        32 => "minecraft:efficiency",
        33 => "minecraft:silk_touch",
        34 => "minecraft:unbreaking",
        35 => "minecraft:fortune",
        48 => "minecraft:power",
        49 => "minecraft:punch",
        50 => "minecraft:flame",
        51 => "minecraft:infinity",
        61 => "minecraft:luck_of_the_sea",
        62 => "minecraft:lure",
        65 => "minecraft:loyalty",
        66 => "minecraft:impaling",
        67 => "minecraft:riptide",
        68 => "minecraft:channeling",
        70 => "minecraft:mending",
        71 => "minecraft:vanishing_curse",
        _ => "null",
    }
}

pub struct ItemStackEnchantmentNamesFix {
    base: DataFixBase,
}

impl ItemStackEnchantmentNamesFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> ItemStackEnchantmentNamesFix {
        ItemStackEnchantmentNamesFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for ItemStackEnchantmentNamesFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let item = self.get_input_schema().get_type(ITEM_STACK);
        let tag_finder = find_field(&item, "tag");
        self.fix_type_everywhere_typed("ItemStackEnchantmentFix", item, move |input| input.update_typed(&tag_finder, None, |tag| tag.update_remainder(fix_tag)))
    }
}

fn fix_enchantment(element: Dynamic) -> Dynamic {
    let id = element.create_string(map(element.get("id").as_int(0)));
    element.set("id", id)
}

fn fix_tag(mut tag: Dynamic) -> Dynamic {
    if let Ok(enchantments) = tag.get("ench").as_stream_opt().into_result() {
        let new_ench = tag.create_list(enchantments.into_iter().map(fix_enchantment));
        tag = tag.remove("ench").set("Enchantments", new_ench);
    }
    tag.update("StoredEnchantments", |list| match list.as_stream_opt().into_result() {
        Ok(enchantments) => list.create_list(enchantments.into_iter().map(fix_enchantment)),
        Err(_) => list,
    })
}
