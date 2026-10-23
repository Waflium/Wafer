use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::simplest_entity_rename_fix::SimplestEntityRenameFix;

pub const RENAMED_IDS: &[(&str, &str)] = &[
    ("minecraft:illager_beast_spawn_egg", "minecraft:ravager_spawn_egg"),
];

pub struct EntityRavagerRenameFix;

impl EntityRavagerRenameFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> SimplestEntityRenameFix {
        SimplestEntityRenameFix::new("EntityRavagerRenameFix", output_schema, changes_type, |name| if name == "minecraft:illager_beast" { "minecraft:ravager".to_string() } else { name.to_string() })
    }
}
