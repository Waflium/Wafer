use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::entity_rename_fix::EntityRenameFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct EntityHorseSplitFix;

impl EntityHorseSplitFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> EntityRenameFix {
        let output = output_schema.clone();
        EntityRenameFix::new("EntityHorseSplitFix", output_schema, changes_type, move |name, entity| {
            if name != "EntityHorse" {
                return (name.to_string(), entity);
            }
            let tag = entity.get_remainder();
            let new_name = match tag.get("Type").as_int(0) {
                1 => "Donkey",
                2 => "Mule",
                3 => "ZombieHorse",
                4 => "SkeletonHorse",
                _ => "Horse",
            };
            let new_type = output.find_choice_type(ENTITY).choices().get(new_name).cloned().expect("a horse type");
            (new_name.to_string(), crate::util::write_and_read_typed_or_throw(&entity, &new_type, |dynamic| dynamic.remove("Type")))
        })
    }
}
