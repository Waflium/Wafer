use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, find_field};
use crate::datafixers::types::Type;
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::UNTAGGED_SPAWNER;

pub struct SpawnerDataFix {
    base: DataFixBase,
}

impl SpawnerDataFix {
    pub fn new(output_schema: &Arc<Schema>) -> SpawnerDataFix {
        SpawnerDataFix { base: DataFixBase::new(output_schema, true) }
    }
}

impl DataFix for SpawnerDataFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let old_type = self.get_input_schema().get_type(UNTAGGED_SPAWNER);
        let new_type = self.get_output_schema().get_type(UNTAGGED_SPAWNER);
        let spawn_data_finder = find_field(&old_type, "SpawnData");
        let new_spawn_data_type = find_field(&new_type, "SpawnData").ty();
        let spawn_potentials_finder = find_field(&old_type, "SpawnPotentials");
        let new_spawn_potentials_type = find_field(&new_type, "SpawnPotentials").ty();
        self.fix_type_everywhere_typed_to("Fix mob spawner data structure", old_type, new_type, move |spawner| {
            spawner
                .update_typed(&spawn_data_finder, Some(&new_spawn_data_type), |spawn_data| wrap_entity_to_spawn_data(&new_spawn_data_type, spawn_data))
                .update_typed(&spawn_potentials_finder, Some(&new_spawn_potentials_type), |spawn_potentials| {
                    wrap_spawn_potentials_to_weighted_entries(&new_spawn_potentials_type, spawn_potentials)
                })
        })
    }
}

fn wrap_entity_to_spawn_data(new_type: &Arc<Type>, spawn_data: Typed) -> Typed {
    Typed::new(new_type.clone(), Value::Pair(Box::new(spawn_data.value), Box::new(Value::Dynamic(Dynamic::empty()))))
}

fn wrap_spawn_potentials_to_weighted_entries(new_type: &Arc<Type>, spawn_potentials: Typed) -> Typed {
    let Value::List(entries) = spawn_potentials.value else { panic!("Not a list") };
    let wrapped_entries = entries
        .into_iter()
        .map(|entry| {
            let Value::Pair(first, second) = entry else { panic!("Not a pair") };
            let Value::Dynamic(remainder) = *second else { panic!("Not a remainder") };
            let weight = remainder.get("Weight").as_number().into_result().ok().map_or(1, |number| number.int_value());
            let new_entry_remainder = Dynamic::empty();
            let weight_tag = new_entry_remainder.create_int(weight);
            let new_entry_remainder = new_entry_remainder.set("weight", weight_tag);
            let new_inner_remainder = remainder.remove("Weight").remove("Entity");
            Value::Pair(Box::new(Value::Pair(first, Box::new(Value::Dynamic(new_inner_remainder)))), Box::new(Value::Dynamic(new_entry_remainder)))
        })
        .collect();
    Typed::new(new_type.clone(), Value::List(wrapped_entries))
}
