use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{find_field, type_finder};
use crate::datafixers::types::{and, field, optional, remainder_type};
use crate::util::datafix::fixes::abstract_uuid_fix::{AbstractUUIDFix, create_uuid_from_ml, replace_uuid_least_most, replace_uuid_string};
use crate::util::datafix::fixes::references::{LEVEL, TEXT_COMPONENT};

pub struct LevelUUIDFix {
    abstract_uuid_fix: AbstractUUIDFix,
}

impl LevelUUIDFix {
    pub fn new(output_schema: &Arc<Schema>) -> LevelUUIDFix {
        LevelUUIDFix { abstract_uuid_fix: AbstractUUIDFix::new(output_schema, LEVEL) }
    }
}

impl DataFix for LevelUUIDFix {
    fn base(&self) -> &DataFixBase {
        &self.abstract_uuid_fix.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let ty = self.get_input_schema().get_type(self.abstract_uuid_fix.type_reference);
        let custom_boss_events_finder = find_field(&ty, "CustomBossEvents");
        let custom_boss_event_finder = type_finder(and(optional(field("Name", self.get_input_schema().get_type_raw(TEXT_COMPONENT))), remainder_type()));
        self.fix_type_everywhere_typed("LevelUUIDFix", ty, move |input| {
            input.update_remainder(|tag| update_wandering_trader(update_dragon_fight(tag))).update_typed(&custom_boss_events_finder, None, |custom_boss_events| {
                custom_boss_events.update_typed(&custom_boss_event_finder, None, |event| event.update_remainder(update_custom_boss_event))
            })
        })
    }
}

fn update_wandering_trader(tag: Dynamic) -> Dynamic {
    replace_uuid_string(&tag, "WanderingTraderId", "WanderingTraderId").unwrap_or(tag)
}

fn update_dragon_fight(tag: Dynamic) -> Dynamic {
    tag.update("DimensionData", |dimension_data_map| {
        dimension_data_map.update_map_values(|(key, dimension_data)| {
            (key, dimension_data.update("DragonFight", |dragonfight| replace_uuid_least_most(&dragonfight, "DragonUUID", "Dragon").unwrap_or(dragonfight)))
        })
    })
}

fn update_custom_boss_event(tag: Dynamic) -> Dynamic {
    tag.update("Players", |players| {
        players.create_list(players.as_stream().into_iter().map(|player| {
            create_uuid_from_ml(&player).unwrap_or_else(|| {
                warn!("CustomBossEvents contains invalid UUIDs.");
                player
            })
        }))
    })
}
