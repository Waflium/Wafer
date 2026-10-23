use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::abstract_uuid_fix::{AbstractUUIDFix, NamedChoiceUpdate, replace_uuid_ml_tag, replace_uuid_string};
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

pub struct BlockEntityUUIDFix {
    abstract_uuid_fix: AbstractUUIDFix,
}

impl BlockEntityUUIDFix {
    pub fn new(output_schema: &Arc<Schema>) -> BlockEntityUUIDFix {
        BlockEntityUUIDFix { abstract_uuid_fix: AbstractUUIDFix::new(output_schema, BLOCK_ENTITY) }
    }
}

impl DataFix for BlockEntityUUIDFix {
    fn base(&self) -> &DataFixBase {
        &self.abstract_uuid_fix.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let conduit = NamedChoiceUpdate::new(self.get_input_schema(), self.get_output_schema(), BLOCK_ENTITY, "minecraft:conduit");
        let skull = NamedChoiceUpdate::new(self.get_input_schema(), self.get_output_schema(), BLOCK_ENTITY, "minecraft:skull");
        self.fix_type_everywhere_typed("BlockEntityUUIDFix", self.get_input_schema().get_type(self.abstract_uuid_fix.type_reference), move |input| {
            let input = conduit.update(input, update_conduit);
            skull.update(input, update_skull)
        })
    }
}

fn update_skull(tag: Dynamic) -> Dynamic {
    match tag.get("Owner").result() {
        Some(owner_tag) => {
            let owner_tag = replace_uuid_string(&owner_tag, "Id", "Id").unwrap_or(owner_tag);
            tag.remove("Owner").set("SkullOwner", owner_tag)
        }
        None => tag,
    }
}

fn update_conduit(tag: Dynamic) -> Dynamic {
    replace_uuid_ml_tag(&tag, "target_uuid", "Target").unwrap_or(tag)
}
