use std::sync::Arc;

use wafer_protocol::nbt::Nbt;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, field_finder, find_field};
use crate::datafixers::types::{const_type, named};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::{ITEM_NAME, ITEM_STACK};

const MAP: [(&str, &str); 49] = [
    ("minecraft:bat", "minecraft:bat_spawn_egg"),
    ("minecraft:blaze", "minecraft:blaze_spawn_egg"),
    ("minecraft:cave_spider", "minecraft:cave_spider_spawn_egg"),
    ("minecraft:chicken", "minecraft:chicken_spawn_egg"),
    ("minecraft:cow", "minecraft:cow_spawn_egg"),
    ("minecraft:creeper", "minecraft:creeper_spawn_egg"),
    ("minecraft:donkey", "minecraft:donkey_spawn_egg"),
    ("minecraft:elder_guardian", "minecraft:elder_guardian_spawn_egg"),
    ("minecraft:ender_dragon", "minecraft:ender_dragon_spawn_egg"),
    ("minecraft:enderman", "minecraft:enderman_spawn_egg"),
    ("minecraft:endermite", "minecraft:endermite_spawn_egg"),
    ("minecraft:evocation_illager", "minecraft:evocation_illager_spawn_egg"),
    ("minecraft:ghast", "minecraft:ghast_spawn_egg"),
    ("minecraft:guardian", "minecraft:guardian_spawn_egg"),
    ("minecraft:horse", "minecraft:horse_spawn_egg"),
    ("minecraft:husk", "minecraft:husk_spawn_egg"),
    ("minecraft:iron_golem", "minecraft:iron_golem_spawn_egg"),
    ("minecraft:llama", "minecraft:llama_spawn_egg"),
    ("minecraft:magma_cube", "minecraft:magma_cube_spawn_egg"),
    ("minecraft:mooshroom", "minecraft:mooshroom_spawn_egg"),
    ("minecraft:mule", "minecraft:mule_spawn_egg"),
    ("minecraft:ocelot", "minecraft:ocelot_spawn_egg"),
    ("minecraft:pufferfish", "minecraft:pufferfish_spawn_egg"),
    ("minecraft:parrot", "minecraft:parrot_spawn_egg"),
    ("minecraft:pig", "minecraft:pig_spawn_egg"),
    ("minecraft:polar_bear", "minecraft:polar_bear_spawn_egg"),
    ("minecraft:rabbit", "minecraft:rabbit_spawn_egg"),
    ("minecraft:sheep", "minecraft:sheep_spawn_egg"),
    ("minecraft:shulker", "minecraft:shulker_spawn_egg"),
    ("minecraft:silverfish", "minecraft:silverfish_spawn_egg"),
    ("minecraft:skeleton", "minecraft:skeleton_spawn_egg"),
    ("minecraft:skeleton_horse", "minecraft:skeleton_horse_spawn_egg"),
    ("minecraft:slime", "minecraft:slime_spawn_egg"),
    ("minecraft:snow_golem", "minecraft:snow_golem_spawn_egg"),
    ("minecraft:spider", "minecraft:spider_spawn_egg"),
    ("minecraft:squid", "minecraft:squid_spawn_egg"),
    ("minecraft:stray", "minecraft:stray_spawn_egg"),
    ("minecraft:turtle", "minecraft:turtle_spawn_egg"),
    ("minecraft:vex", "minecraft:vex_spawn_egg"),
    ("minecraft:villager", "minecraft:villager_spawn_egg"),
    ("minecraft:vindication_illager", "minecraft:vindication_illager_spawn_egg"),
    ("minecraft:witch", "minecraft:witch_spawn_egg"),
    ("minecraft:wither", "minecraft:wither_spawn_egg"),
    ("minecraft:wither_skeleton", "minecraft:wither_skeleton_spawn_egg"),
    ("minecraft:wolf", "minecraft:wolf_spawn_egg"),
    ("minecraft:zombie", "minecraft:zombie_spawn_egg"),
    ("minecraft:zombie_horse", "minecraft:zombie_horse_spawn_egg"),
    ("minecraft:zombie_pigman", "minecraft:zombie_pigman_spawn_egg"),
    ("minecraft:zombie_villager", "minecraft:zombie_villager_spawn_egg"),
];

pub struct ItemStackSpawnEggFix {
    base: DataFixBase,
    item_type: String,
}

impl ItemStackSpawnEggFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool, item_type: &str) -> ItemStackSpawnEggFix {
        ItemStackSpawnEggFix { base: DataFixBase::new(output_schema, changes_type), item_type: item_type.to_string() }
    }
}

impl DataFix for ItemStackSpawnEggFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let item_stack_type = self.get_input_schema().get_type(ITEM_STACK);
        let id_type = named(ITEM_NAME.type_name(), const_type(ConstType::NamespacedString));
        let id_f = field_finder("id", id_type.clone());
        let entity_id_f = field_finder("id", const_type(ConstType::NamespacedString));
        let tag_f = find_field(&item_stack_type, "tag");
        let entity_f = find_field(&tag_f.ty(), "EntityTag");
        let item_type = self.item_type.clone();
        self.fix_type_everywhere_typed(&format!("ItemInstanceSpawnEggFix{}", self.get_output_schema().version_key), item_stack_type, move |input: Typed| {
            let id = input.get_optional(&id_f);
            if id.as_ref().and_then(Value::as_str) == Some(item_type.as_str()) {
                let tag = input.get_or_create_typed(&tag_f);
                let entity = tag.get_or_create_typed(&entity_f);
                if let Some(entity_id) = entity.get_optional(&entity_id_f) {
                    let egg = entity_id.as_str().and_then(|entity_id| MAP.iter().find(|(entity, _)| *entity == entity_id)).map_or("minecraft:pig_spawn_egg", |(_, egg)| *egg);
                    return input.set(&id_f, Typed::new(id_type.clone(), Value::Prim(Nbt::String(egg.to_string()))));
                }
            }
            input
        })
    }
}
