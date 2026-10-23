use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::extra_data_fix_utils;
use crate::util::datafix::fixes::references::SAVED_DATA_RAIDS;

pub struct RaidRenamesDataFix {
    base: DataFixBase,
}

impl RaidRenamesDataFix {
    pub fn new(output_schema: &Arc<Schema>) -> RaidRenamesDataFix {
        RaidRenamesDataFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for RaidRenamesDataFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("RaidRenamesDataFix", self.get_input_schema().get_type(SAVED_DATA_RAIDS), |input| {
            input.update_remainder(|container| container.update("data", fix))
        })
    }
}

fn fix(tag: Dynamic) -> Dynamic {
    tag.rename_and_fix_field("Raids", "raids", |raids| raids.create_list(raids.as_stream().into_iter().map(fix_raid))).rename_field("Tick", "tick").rename_field("NextAvailableID", "next_id")
}

fn fix_raid(raid: Dynamic) -> Dynamic {
    extra_data_fix_utils::fix_inline_block_pos(raid, "CX", "CY", "CZ", "center")
        .rename_field("Id", "id")
        .rename_field("Started", "started")
        .rename_field("Active", "active")
        .rename_field("TicksActive", "ticks_active")
        .rename_field("BadOmenLevel", "raid_omen_level")
        .rename_field("GroupsSpawned", "groups_spawned")
        .rename_field("PreRaidTicks", "cooldown_ticks")
        .rename_field("PostRaidTicks", "post_raid_ticks")
        .rename_field("TotalHealth", "total_health")
        .rename_field("NumGroups", "group_count")
        .rename_field("Status", "status")
        .rename_field("HeroesOfTheVillage", "heroes_of_the_village")
}
