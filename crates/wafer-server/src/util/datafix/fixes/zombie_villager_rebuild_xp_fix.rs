use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;
use crate::util::datafix::fixes::villager_rebuild_level_and_xp_fix;

pub struct ZombieVillagerRebuildXpFix;

impl ZombieVillagerRebuildXpFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, changes_type, "Zombie Villager XP rebuild", ENTITY, "minecraft:zombie_villager", |entity| {
            entity.update_remainder(|remainder| {
                if remainder.get("Xp").as_number().into_result().is_err() {
                    let level = remainder.get("VillagerData").get("level").as_int(1);
                    let xp = remainder.create_int(villager_rebuild_level_and_xp_fix::get_min_xp_per_level(level));
                    remainder.set("Xp", xp)
                } else {
                    remainder
                }
            })
        })
    }
}
