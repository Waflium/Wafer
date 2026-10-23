use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::simplest_entity_rename_fix::SimplestEntityRenameFix;

pub const RENAMED_IDS: &[(&str, &str)] = &[("minecraft:zombie_pigman_spawn_egg", "minecraft:zombified_piglin_spawn_egg")];

pub struct EntityZombifiedPiglinRenameFix;

impl EntityZombifiedPiglinRenameFix {
    pub fn new(output_schema: &Arc<Schema>) -> SimplestEntityRenameFix {
        SimplestEntityRenameFix::new("EntityZombifiedPiglinRenameFix", output_schema, true, rename)
    }
}

fn rename(name: &str) -> String {
    if name == "minecraft:zombie_pigman" { "minecraft:zombified_piglin".to_string() } else { name.to_string() }
}
