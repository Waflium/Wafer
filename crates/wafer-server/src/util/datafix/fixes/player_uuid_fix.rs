use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::find_field;
use crate::util::datafix::fixes::abstract_uuid_fix::{AbstractUUIDFix, replace_uuid_least_most};
use crate::util::datafix::fixes::entity_uuid_fix;
use crate::util::datafix::fixes::references::PLAYER;

pub struct PlayerUUIDFix {
    abstract_uuid_fix: AbstractUUIDFix,
}

impl PlayerUUIDFix {
    pub fn new(output_schema: &Arc<Schema>) -> PlayerUUIDFix {
        PlayerUUIDFix { abstract_uuid_fix: AbstractUUIDFix::new(output_schema, PLAYER) }
    }
}

impl DataFix for PlayerUUIDFix {
    fn base(&self) -> &DataFixBase {
        &self.abstract_uuid_fix.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let player_type = self.get_input_schema().get_type(self.abstract_uuid_fix.type_reference);
        let root_vehicle_finder = find_field(&player_type, "RootVehicle");
        self.fix_type_everywhere_typed("PlayerUUIDFix", player_type, move |input| {
            input
                .update_typed(&root_vehicle_finder, None, |root_vehicle| root_vehicle.update_remainder(|tag| replace_uuid_least_most(&tag, "Attach", "Attach").unwrap_or(tag)))
                .update_remainder(|tag| entity_uuid_fix::update_entity_uuid(entity_uuid_fix::update_living_entity(tag)))
        })
    }
}
