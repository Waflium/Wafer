use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_write_read_fix::NamedEntityWriteReadFix;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

pub struct TrialSpawnerConfigFix;

impl TrialSpawnerConfigFix {
    pub fn new(output_schema: &Arc<Schema>) -> NamedEntityWriteReadFix {
        NamedEntityWriteReadFix::new(output_schema, true, "Trial Spawner config tag fixer", BLOCK_ENTITY, "minecraft:trial_spawner", fix)
    }
}

fn move_to_config_tag(mut input: Dynamic) -> Dynamic {
    let keys_to_move = [
        "spawn_range",
        "total_mobs",
        "simultaneous_mobs",
        "total_mobs_added_per_player",
        "simultaneous_mobs_added_per_player",
        "ticks_between_spawn",
        "spawn_potentials",
        "loot_tables_to_eject",
        "items_to_drop_when_ominous",
    ];
    let mut map: Vec<(Dynamic, Dynamic)> = Vec::with_capacity(keys_to_move.len());
    for key in keys_to_move {
        if let Some(value_for_key) = input.get(key).result() {
            map.push((input.create_string(key), value_for_key));
            input = input.remove(key);
        }
    }
    if map.is_empty() {
        input
    } else {
        let normal_config = input.create_map(map);
        input.set("normal_config", normal_config)
    }
}

fn fix(input: Dynamic) -> Dynamic {
    move_to_config_tag(input)
}
