use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{field_finder, named_choice, type_finder};
use crate::datafixers::types::Type;
use crate::util::datafix::extra_data_fix_utils;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

pub struct BeehiveFieldRenameFix {
    base: DataFixBase,
}

impl BeehiveFieldRenameFix {
    pub fn new(output_schema: &Arc<Schema>) -> BeehiveFieldRenameFix {
        BeehiveFieldRenameFix { base: DataFixBase::new(output_schema, true) }
    }
}

fn fix_beehive(beehive: Dynamic) -> Dynamic {
    beehive.remove("Bees")
}

fn fix_bee(mut bee: Dynamic) -> Dynamic {
    bee = bee.remove("EntityData");
    bee = bee.rename_field("TicksInHive", "ticks_in_hive");
    bee.rename_field("MinOccupationTicks", "min_ticks_in_hive")
}

impl DataFix for BeehiveFieldRenameFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let beehive_type = self.get_input_schema().get_choice_type(BLOCK_ENTITY, "minecraft:beehive");
        let beehive_f = named_choice("minecraft:beehive", beehive_type.clone());
        let bees_type = beehive_type.find_field_type("Bees").expect("a Bees field");
        let bee_type = match &*bees_type {
            Type::List(element) => element.clone(),
            other => panic!("Not a list type: {other:?}"),
        };
        let bees_f = field_finder("Bees", bees_type);
        let bee_f = type_finder(bee_type);
        let entity_type = self.get_input_schema().get_type(BLOCK_ENTITY);
        let new_entity_type = self.get_output_schema().get_type(BLOCK_ENTITY);
        let cast_type = new_entity_type.clone();
        self.fix_type_everywhere_typed_to("BeehiveFieldRenameFix", entity_type, new_entity_type, move |input| {
            extra_data_fix_utils::cast(
                &cast_type,
                input.update_typed(&beehive_f, None, |beehive| {
                    beehive.update_remainder(fix_beehive).update_typed(&bees_f, None, |bees| bees.update_typed(&bee_f, None, |bee| bee.update_remainder(fix_bee)))
                }),
            )
        })
    }
}
