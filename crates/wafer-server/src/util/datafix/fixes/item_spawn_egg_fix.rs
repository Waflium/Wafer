use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{field_finder, find_field, type_finder};
use crate::datafixers::types::{const_type, named};
use crate::datafixers::value::Value;
use crate::util::datafix::extra_data_fix_utils;
use crate::util::datafix::fixes::references::{ENTITY, ITEM_NAME, ITEM_STACK};

pub struct ItemSpawnEggFix {
    base: DataFixBase,
}

impl ItemSpawnEggFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> ItemSpawnEggFix {
        ItemSpawnEggFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for ItemSpawnEggFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let input_schema = self.get_input_schema();
        let item_stack_type = input_schema.get_type(ITEM_STACK);
        let id_finder = field_finder("id", named(ITEM_NAME.type_name(), const_type(ConstType::NamespacedString)));
        let entity_id_finder = field_finder("id", const_type(ConstType::String));
        let tag_finder = find_field(&item_stack_type, "tag");
        let entity_tag_finder = find_field(&tag_finder.ty(), "EntityTag");
        let entity_finder = type_finder(input_schema.get_type_raw(ENTITY));
        self.fix_type_everywhere_typed("ItemSpawnEggFix", item_stack_type, move |input| {
            let id = input.get_optional(&id_finder);
            if id.as_ref().and_then(Value::as_str) != Some("minecraft:spawn_egg") {
                return input;
            }
            let mut rest = input.get_remainder();
            let damage = rest.get("Damage").as_short(0);
            let tag_optional = input.get_optional_typed(&tag_finder);
            let entity_tree_optional = tag_optional.and_then(|value| value.get_optional_typed(&entity_tag_finder));
            let entity_optional = entity_tree_optional.and_then(|value| value.get_optional_typed(&entity_finder));
            let old_id = entity_optional.and_then(|value| value.get_optional(&entity_id_finder)).and_then(|id| id.as_str().map(str::to_string));
            let mut output = input.clone();
            let entity_name = ID_TO_ENTITY[(damage & 0xFF) as usize];
            if let Some(entity_name) = entity_name
                && old_id.as_deref() != Some(entity_name)
            {
                let tag = input.get_or_create_typed(&tag_finder);
                let entity_tag = match tag.get_optional_typed(&entity_tag_finder) {
                    Some(entity_tree) => entity_tree.write().into_result().unwrap_or_else(|message| panic!("{message}")),
                    None => rest.empty_map(),
                };
                let id = entity_tag.create_string(entity_name);
                let entity_tag: Dynamic = entity_tag.set("id", id);
                output = output.set(&tag_finder, extra_data_fix_utils::read_and_set(tag, &entity_tag_finder, entity_tag));
            }
            if damage != 0 {
                let zero = rest.create_short(0);
                rest = rest.set("Damage", zero);
                output = output.set_remainder(rest);
            }
            output
        })
    }
}

const ID_TO_ENTITY: [Option<&str>; 256] = [
    None,
    Some("Item"),
    Some("XPOrb"),
    None,
    None,
    None,
    None,
    Some("ThrownEgg"),
    Some("LeashKnot"),
    Some("Painting"),
    Some("Arrow"),
    Some("Snowball"),
    Some("Fireball"),
    Some("SmallFireball"),
    Some("ThrownEnderpearl"),
    Some("EyeOfEnderSignal"),
    Some("ThrownPotion"),
    Some("ThrownExpBottle"),
    Some("ItemFrame"),
    Some("WitherSkull"),
    Some("PrimedTnt"),
    Some("FallingSand"),
    Some("FireworksRocketEntity"),
    Some("TippedArrow"),
    Some("SpectralArrow"),
    Some("ShulkerBullet"),
    Some("DragonFireball"),
    None,
    None,
    None,
    Some("ArmorStand"),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some("MinecartCommandBlock"),
    Some("Boat"),
    Some("MinecartRideable"),
    Some("MinecartChest"),
    Some("MinecartFurnace"),
    Some("MinecartTNT"),
    Some("MinecartHopper"),
    Some("MinecartSpawner"),
    None,
    None,
    Some("Creeper"),
    Some("Skeleton"),
    Some("Spider"),
    Some("Giant"),
    Some("Zombie"),
    Some("Slime"),
    Some("Ghast"),
    Some("PigZombie"),
    Some("Enderman"),
    Some("CaveSpider"),
    Some("Silverfish"),
    Some("Blaze"),
    Some("LavaSlime"),
    Some("EnderDragon"),
    Some("WitherBoss"),
    Some("Bat"),
    Some("Witch"),
    Some("Endermite"),
    Some("Guardian"),
    Some("Shulker"),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some("Pig"),
    Some("Sheep"),
    Some("Cow"),
    Some("Chicken"),
    Some("Squid"),
    Some("Wolf"),
    Some("MushroomCow"),
    Some("SnowMan"),
    Some("Ozelot"),
    Some("VillagerGolem"),
    Some("EntityHorse"),
    Some("Rabbit"),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some("Villager"),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some("EnderCrystal"),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
];
