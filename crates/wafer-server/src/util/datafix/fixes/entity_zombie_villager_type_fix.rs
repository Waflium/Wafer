use std::sync::Arc;

use wafer_core::random::{LegacyRandomSource, RandomSource};

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;

const PROFESSION_MAX: i32 = 6;

pub struct EntityZombieVillagerTypeFix;

impl EntityZombieVillagerTypeFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, changes_type, "EntityZombieVillagerTypeFix", ENTITY, "Zombie", |entity| entity.update_remainder(fix_tag))
    }
}

pub fn fix_tag(mut input: Dynamic) -> Dynamic {
    if input.get("IsVillager").as_boolean_or(false) {
        if input.get("ZombieType").result().is_none() {
            let mut ty = get_villager_profession(input.get("VillagerProfession").as_int(-1));
            if ty == -1 {
                ty = get_villager_profession(LegacyRandomSource::new(rand::random()).next_int_bounded(PROFESSION_MAX));
            }
            let zombie_type = input.create_int(ty);
            input = input.set("ZombieType", zombie_type);
        }
        input = input.remove("IsVillager");
    }
    input
}

fn get_villager_profession(profession: i32) -> i32 {
    if (0..PROFESSION_MAX).contains(&profession) { profession } else { -1 }
}
