use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{OpticFinder, Typed, find_field};
use crate::datafixers::types::{Type, and_all, equals, field, list, named, optional, remainder_type};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::{ENTITY_EQUIPMENT, ITEM_STACK};

pub struct EquipmentFormatFix {
    base: DataFixBase,
}

impl EquipmentFormatFix {
    pub fn new(output_schema: &Arc<Schema>) -> EquipmentFormatFix {
        EquipmentFormatFix { base: DataFixBase::new(output_schema, true) }
    }
}

impl DataFix for EquipmentFormatFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let old_item_stack_type = self.get_input_schema().get_type_raw(ITEM_STACK);
        let new_item_stack_type = self.get_output_schema().get_type_raw(ITEM_STACK);
        let id_finder = find_field(&old_item_stack_type, "id");
        self.fix(old_item_stack_type, new_item_stack_type, id_finder)
    }
}

impl EquipmentFormatFix {
    fn fix(&self, old_item_stack_type: Arc<Type>, new_item_stack_type: Arc<Type>, id_finder: OpticFinder) -> TypeRewriteRule {
        let old_equipment_type = named(
            ENTITY_EQUIPMENT.type_name(),
            and_all(vec![
                optional(field("ArmorItems", list(old_item_stack_type.clone()))),
                optional(field("HandItems", list(old_item_stack_type.clone()))),
                optional(field("body_armor_item", old_item_stack_type.clone())),
                optional(field("saddle", old_item_stack_type.clone())),
            ]),
        );
        let new_equipment_type = named(
            ENTITY_EQUIPMENT.type_name(),
            optional(field(
                "equipment",
                and_all(vec![
                    optional(field("mainhand", new_item_stack_type.clone())),
                    optional(field("offhand", new_item_stack_type.clone())),
                    optional(field("feet", new_item_stack_type.clone())),
                    and_all(vec![
                        optional(field("legs", new_item_stack_type.clone())),
                        optional(field("chest", new_item_stack_type.clone())),
                        optional(field("head", new_item_stack_type.clone())),
                        and_all(vec![optional(field("body", new_item_stack_type.clone())), optional(field("saddle", new_item_stack_type.clone())), remainder_type()]),
                    ]),
                ]),
            )),
        );
        if !equals(&old_equipment_type, &self.get_input_schema().get_type(ENTITY_EQUIPMENT), false) {
            panic!("Input entity_equipment type does not match expected");
        }
        if !equals(&new_equipment_type, &self.get_output_schema().get_type(ENTITY_EQUIPMENT), false) {
            panic!("Output entity_equipment type does not match expected");
        }
        self.fix_type_everywhere_to("EquipmentFormatFix", old_equipment_type, new_equipment_type, move |named_old_equipment| {
            let is_placeholder = |item_stack: &Value| Typed::new(old_item_stack_type.clone(), item_stack.clone()).get_optional(&id_finder).is_none();
            let Value::Pair(armor, rest) = named_old_equipment else { panic!("Unexpected entity_equipment value") };
            let Value::Pair(hand, rest) = *rest else { panic!("Unexpected entity_equipment value") };
            let Value::Pair(body, saddle) = *rest else { panic!("Unexpected entity_equipment value") };
            let armor_items = items_or_empty(*armor);
            let hand_items = items_or_empty(*hand);
            let body = *body;
            let saddle = *saddle;
            let feet = get_item_from_list(0, &armor_items, &is_placeholder);
            let legs = get_item_from_list(1, &armor_items, &is_placeholder);
            let chest = get_item_from_list(2, &armor_items, &is_placeholder);
            let head = get_item_from_list(3, &armor_items, &is_placeholder);
            let mainhand = get_item_from_list(0, &hand_items, &is_placeholder);
            let offhand = get_item_from_list(1, &hand_items, &is_placeholder);
            if are_all_empty(&[&body, &saddle, &feet, &legs, &chest, &head, &mainhand, &offhand]) {
                return Value::Right(Box::new(Value::Unit));
            }
            Value::Left(Box::new(pair_all(vec![mainhand, offhand, feet, legs, chest, head, body, saddle, Value::Dynamic(Dynamic::empty())])))
        })
    }
}

fn items_or_empty(value: Value) -> Vec<Value> {
    match value {
        Value::Left(items) => match *items {
            Value::List(items) => items,
            other => panic!("Unexpected item list value: {other:?}"),
        },
        _ => Vec::new(),
    }
}

fn pair_all(values: Vec<Value>) -> Value {
    let mut values = values.into_iter().rev();
    let last = values.next().expect("at least one value");
    values.fold(last, |rest, value| Value::Pair(Box::new(value), Box::new(rest)))
}

fn are_all_empty(fields: &[&Value]) -> bool {
    fields.iter().all(|field| matches!(field, Value::Right(_)))
}

fn get_item_from_list(index: usize, items: &[Value], is_placeholder: &impl Fn(&Value) -> bool) -> Value {
    if index >= items.len() {
        return Value::Right(Box::new(Value::Unit));
    }
    let item = &items[index];
    if is_placeholder(item) { Value::Right(Box::new(Value::Unit)) } else { Value::Left(Box::new(item.clone())) }
}
