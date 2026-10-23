use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::simplest_entity_rename_fix::SimplestEntityRenameFix;

pub const RENAMED_IDS: &[(&str, &str)] = &[
    ("minecraft:salmon_mob", "minecraft:salmon"),
    ("minecraft:cod_mob", "minecraft:cod"),
];

pub const RENAMED_EGG_IDS: &[(&str, &str)] = &[
    ("minecraft:salmon_mob_spawn_egg", "minecraft:salmon_spawn_egg"),
    ("minecraft:cod_mob_spawn_egg", "minecraft:cod_spawn_egg"),
];

pub struct EntityCodSalmonFix;

impl EntityCodSalmonFix {
    pub fn new(schema: &Arc<Schema>, changes_type: bool) -> SimplestEntityRenameFix {
        SimplestEntityRenameFix::new("EntityCodSalmonFix", schema, changes_type, |name| RENAMED_IDS.iter().find(|(from, _)| *from == name).map_or(name, |(_, to)| to).to_string())
    }
}
