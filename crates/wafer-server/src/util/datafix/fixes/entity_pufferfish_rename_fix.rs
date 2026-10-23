use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::simplest_entity_rename_fix::SimplestEntityRenameFix;

pub const RENAMED_IDS: &[(&str, &str)] = &[
    ("minecraft:puffer_fish_spawn_egg", "minecraft:pufferfish_spawn_egg"),
];

pub struct EntityPufferfishRenameFix;

impl EntityPufferfishRenameFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> SimplestEntityRenameFix {
        SimplestEntityRenameFix::new("EntityPufferfishRenameFix", output_schema, changes_type, |name| if name == "minecraft:puffer_fish" { "minecraft:pufferfish".to_string() } else { name.to_string() })
    }
}
