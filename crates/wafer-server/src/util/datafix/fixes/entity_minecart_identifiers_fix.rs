use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::remainder_finder;
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::entity_rename_fix::EntityRenameFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct EntityMinecartIdentifiersFix;

impl EntityMinecartIdentifiersFix {
    pub fn new(output_schema: &Arc<Schema>) -> EntityRenameFix {
        let output = output_schema.clone();
        EntityRenameFix::new("EntityMinecartIdentifiersFix", output_schema, true, move |name, entity| {
            if name != "Minecart" {
                return (name.to_string(), entity);
            }
            let tag = match entity.get_or_create_typed(&remainder_finder()).value {
                Value::Dynamic(tag) => tag,
                _ => Dynamic::empty(),
            };
            let new_name = match tag.get("Type").as_int(0) {
                1 => "MinecartChest",
                2 => "MinecartFurnace",
                _ => "MinecartRideable",
            };
            let new_type = output.find_choice_type(ENTITY).choices().get(new_name).cloned().expect("a minecart type");
            (new_name.to_string(), crate::util::write_and_read_typed_or_throw(&entity, &new_type, |dynamic| dynamic.remove("Type")))
        })
    }
}
