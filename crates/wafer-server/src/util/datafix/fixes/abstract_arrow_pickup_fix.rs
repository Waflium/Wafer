use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::abstract_uuid_fix::NamedChoiceUpdate;
use crate::util::datafix::fixes::references::ENTITY;

pub struct AbstractArrowPickupFix {
    base: DataFixBase,
}

impl AbstractArrowPickupFix {
    pub fn new(output_schema: &Arc<Schema>) -> AbstractArrowPickupFix {
        AbstractArrowPickupFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for AbstractArrowPickupFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let input_schema = self.get_input_schema();
        let updates: Vec<NamedChoiceUpdate> =
            ["minecraft:arrow", "minecraft:spectral_arrow", "minecraft:trident"].iter().map(|name| NamedChoiceUpdate::new(input_schema, self.get_output_schema(), ENTITY, name)).collect();
        self.fix_type_everywhere_typed("AbstractArrowPickupFix", input_schema.get_type(ENTITY), move |input| {
            updates.iter().fold(input, |input, update| update.update(input, update_pickup))
        })
    }
}

fn update_pickup(tag: Dynamic) -> Dynamic {
    if tag.get("pickup").result().is_some() {
        return tag;
    }
    let from_player = tag.get("player").as_boolean_or(true);
    let pickup = tag.create_byte(from_player as i8);
    tag.set("pickup", pickup).remove("player")
}
