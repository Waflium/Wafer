use std::sync::Arc;

use crate::datafixers::dsl::TypeTemplates;
use crate::datafixers::schemas::schema::{Schema, SchemaClass, put, remove};

pub static V1510: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: Some(register_entities),
    register_block_entities: None,
};

fn register_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_entities();
    let removed = remove(&mut map, "minecraft:commandblock_minecart").expect("minecraft:commandblock_minecart");
    put(&mut map, "minecraft:command_block_minecart", removed);
    let removed = remove(&mut map, "minecraft:ender_crystal").expect("minecraft:ender_crystal");
    put(&mut map, "minecraft:end_crystal", removed);
    let removed = remove(&mut map, "minecraft:snowman").expect("minecraft:snowman");
    put(&mut map, "minecraft:snow_golem", removed);
    let removed = remove(&mut map, "minecraft:evocation_illager").expect("minecraft:evocation_illager");
    put(&mut map, "minecraft:evoker", removed);
    let removed = remove(&mut map, "minecraft:evocation_fangs").expect("minecraft:evocation_fangs");
    put(&mut map, "minecraft:evoker_fangs", removed);
    let removed = remove(&mut map, "minecraft:illusion_illager").expect("minecraft:illusion_illager");
    put(&mut map, "minecraft:illusioner", removed);
    let removed = remove(&mut map, "minecraft:vindication_illager").expect("minecraft:vindication_illager");
    put(&mut map, "minecraft:vindicator", removed);
    let removed = remove(&mut map, "minecraft:villager_golem").expect("minecraft:villager_golem");
    put(&mut map, "minecraft:iron_golem", removed);
    let removed = remove(&mut map, "minecraft:xp_orb").expect("minecraft:xp_orb");
    put(&mut map, "minecraft:experience_orb", removed);
    let removed = remove(&mut map, "minecraft:xp_bottle").expect("minecraft:xp_bottle");
    put(&mut map, "minecraft:experience_bottle", removed);
    let removed = remove(&mut map, "minecraft:eye_of_ender_signal").expect("minecraft:eye_of_ender_signal");
    put(&mut map, "minecraft:eye_of_ender", removed);
    let removed = remove(&mut map, "minecraft:fireworks_rocket").expect("minecraft:fireworks_rocket");
    put(&mut map, "minecraft:firework_rocket", removed);
    map
}
