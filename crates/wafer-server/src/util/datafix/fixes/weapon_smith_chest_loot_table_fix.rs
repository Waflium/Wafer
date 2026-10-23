use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

pub struct WeaponSmithChestLootTableFix;

impl WeaponSmithChestLootTableFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, changes_type, "WeaponSmithChestLootTableFix", BLOCK_ENTITY, "minecraft:chest", |entity| {
            entity.update_remainder(|tag| {
                let loot_table = tag.get("LootTable").as_string_or("");
                if loot_table == "minecraft:chests/village_blacksmith" {
                    let fixed = tag.create_string("minecraft:chests/village/village_weaponsmith");
                    tag.set("LootTable", fixed)
                } else {
                    tag
                }
            })
        })
    }
}
