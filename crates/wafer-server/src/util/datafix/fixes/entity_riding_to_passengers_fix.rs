use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, field_finder, type_finder};
use crate::datafixers::types::{Type, and, equals, field, list, named, optional};
use crate::datafixers::value::Value;
use crate::util::datafix::extra_data_fix_utils;
use crate::util::datafix::fixes::references::{ENTITY, ENTITY_TREE, PLAYER};

pub struct EntityRidingToPassengersFix {
    base: DataFixBase,
}

impl EntityRidingToPassengersFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> EntityRidingToPassengersFix {
        EntityRidingToPassengersFix { base: DataFixBase::new(output_schema, changes_type) }
    }

    fn cap(&self, input_schema: &Arc<Schema>, output_type: &Arc<Schema>, old_entity_tree_type: Arc<Type>, new_entity_tree_type: Arc<Type>, entity_type: Arc<Type>) -> TypeRewriteRule {
        let old_type = named(ENTITY_TREE.type_name(), and(optional(field("Riding", old_entity_tree_type)), entity_type.clone()));
        let new_type = named(ENTITY_TREE.type_name(), and(optional(field("Passengers", list(new_entity_tree_type.clone()))), entity_type.clone()));
        let old_entity_type = input_schema.get_type(ENTITY_TREE);
        let new_entity_type = output_type.get_type(ENTITY_TREE);
        if !equals(&old_entity_type, &old_type, false) {
            panic!("Old entity type is not what was expected.");
        }
        if !equals(&new_entity_type, &new_type, true) {
            panic!("New entity type is not what was expected.");
        }
        let patched_entity_tree_type = extra_data_fix_utils::patch_sub_type(&old_type, &old_type, &new_type);
        let entity_finder = type_finder(entity_type);
        let new_entity_tree_value_finder = type_finder(new_type.clone());
        let riding_finder = field_finder("Riding", new_entity_tree_type);
        let old_player_type = input_schema.get_type(PLAYER);
        let new_player_type = output_type.get_type(PLAYER);
        TypeRewriteRule::seq(vec![
            self.fix_type_everywhere_to("EntityRidingToPassengerFix", old_type, new_type, move |badly_typed_input| {
                let input = Typed::new(patched_entity_tree_type.clone(), badly_typed_input);
                let maybe_riding = input.get_optional_typed(&riding_finder).and_then(|typed| typed.get_optional(&new_entity_tree_value_finder));
                let entity = input.get_optional(&entity_finder).expect("an entity");
                match maybe_riding {
                    None => Value::Pair(Box::new(Value::Right(Box::new(Value::Unit))), Box::new(entity)),
                    Some(riding) => add_passenger_to_top(riding, entity),
                }
            }),
            self.write_and_read("player RootVehicle injecter", old_player_type, new_player_type),
        ])
    }
}

fn add_passenger_to_top(root: Value, passenger_entity: Value) -> Value {
    let Value::Pair(passengers, root_entity) = root else { panic!("an entity tree") };
    let new_passenger = match *passengers {
        Value::Left(passengers) => match *passengers {
            Value::List(mut passengers) if !passengers.is_empty() => add_passenger_to_top(passengers.swap_remove(0), passenger_entity),
            _ => Value::Pair(Box::new(Value::Right(Box::new(Value::Unit))), Box::new(passenger_entity)),
        },
        _ => Value::Pair(Box::new(Value::Right(Box::new(Value::Unit))), Box::new(passenger_entity)),
    };
    Value::Pair(Box::new(Value::Left(Box::new(Value::List(vec![new_passenger])))), root_entity)
}

impl DataFix for EntityRidingToPassengersFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let input_schema = self.get_input_schema().clone();
        let output_schema = self.get_output_schema().clone();
        let old_entity_tree_type = input_schema.get_type_raw(ENTITY_TREE);
        let new_entity_tree_type = output_schema.get_type_raw(ENTITY_TREE);
        let entity_type = input_schema.get_type_raw(ENTITY);
        self.cap(&input_schema, &output_schema, old_entity_tree_type, new_entity_tree_type, entity_type)
    }
}
