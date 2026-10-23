use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct VillagerDataFix;

impl VillagerDataFix {
    pub fn new(schema: &Arc<Schema>, entity_type: &str) -> NamedEntityFix {
        NamedEntityFix::new(schema, false, &format!("Villager profession data fix ({entity_type})"), ENTITY, entity_type, |entity| entity.update_remainder(fix_remainder))
    }
}

fn fix_remainder(remainder: Dynamic) -> Dynamic {
    let villager_data = remainder.create_map([
        (remainder.create_string("type"), remainder.create_string("minecraft:plains")),
        (
            remainder.create_string("profession"),
            remainder.create_string(upgrade_data(remainder.get("Profession").as_int(0), remainder.get("Career").as_int(0))),
        ),
        (remainder.create_string("level"), remainder.get("CareerLevel").result().unwrap_or_else(|| remainder.create_int(1))),
    ]);
    remainder.remove("Profession").remove("Career").remove("CareerLevel").set("VillagerData", villager_data)
}

fn upgrade_data(profession: i32, career: i32) -> &'static str {
    if profession == 0 {
        if career == 2 {
            "minecraft:fisherman"
        } else if career == 3 {
            "minecraft:shepherd"
        } else if career == 4 {
            "minecraft:fletcher"
        } else {
            "minecraft:farmer"
        }
    } else if profession == 1 {
        if career == 2 { "minecraft:cartographer" } else { "minecraft:librarian" }
    } else if profession == 2 {
        "minecraft:cleric"
    } else if profession == 3 {
        if career == 2 {
            "minecraft:weaponsmith"
        } else if career == 3 {
            "minecraft:toolsmith"
        } else {
            "minecraft:armorer"
        }
    } else if profession == 4 {
        if career == 2 { "minecraft:leatherworker" } else { "minecraft:butcher" }
    } else if profession == 5 {
        "minecraft:nitwit"
    } else {
        "minecraft:none"
    }
}
