use std::sync::Arc;

use wafer_protocol::nbt::Nbt;

use crate::datafixers::dsl::ConstType;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, field_finder, point};
use crate::datafixers::types::{const_type, named};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::{ENTITY, ITEM_NAME, ITEM_STACK};

pub struct EntityHorseSaddleFix;

impl EntityHorseSaddleFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> NamedEntityFix {
        let input_schema = if changes_type { output_schema.get_parent().clone() } else { output_schema.clone() };
        let item_name_type = named(ITEM_NAME.type_name(), const_type(ConstType::NamespacedString));
        let item_stack_type = input_schema.get_type_raw(ITEM_STACK);
        NamedEntityFix::new(output_schema, changes_type, "EntityHorseSaddleFix", ENTITY, "EntityHorse", move |entity| {
            let id_finder = field_finder("id", item_name_type.clone());
            let saddle_finder = field_finder("SaddleItem", item_stack_type.clone());
            let saddle = entity.get_optional_typed(&saddle_finder);
            let tag = entity.get_remainder();
            if saddle.is_none() && tag.get("Saddle").as_boolean_or(false) {
                let new_saddle = Typed::new(item_stack_type.clone(), point(&item_stack_type).expect("a saddle item stack"));
                let new_saddle = new_saddle.set(&id_finder, Typed::new(item_name_type.clone(), Value::Prim(Nbt::String("minecraft:saddle".to_string()))));
                let saddle_tag = tag.empty_map();
                let count = saddle_tag.create_byte(1);
                let saddle_tag = saddle_tag.set("Count", count);
                let damage = saddle_tag.create_short(0);
                let saddle_tag = saddle_tag.set("Damage", damage);
                let new_saddle = new_saddle.set_remainder(saddle_tag);
                entity.set(&saddle_finder, new_saddle).set_remainder(tag)
            } else {
                entity
            }
        })
    }
}
