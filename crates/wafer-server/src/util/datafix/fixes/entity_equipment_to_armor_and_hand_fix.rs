use std::sync::Arc;

use wafer_protocol::nbt::Nbt;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::types::{Type, and_all, equals, field, list, named, optional};
use crate::datafixers::value::{self, Value};
use crate::util::datafix::fixes::references::*;

pub struct EntityEquipmentToArmorAndHandFix {
    base: DataFixBase,
}

impl EntityEquipmentToArmorAndHandFix {
    pub fn new(output_schema: &Arc<Schema>) -> EntityEquipmentToArmorAndHandFix {
        EntityEquipmentToArmorAndHandFix { base: DataFixBase::new(output_schema, true) }
    }

    fn cap(&self, old_item_stack_type: Arc<Type>, new_item_stack_type: Arc<Type>) -> TypeRewriteRule {
        let old_equipment_type = named(ENTITY_EQUIPMENT.type_name(), optional(field("Equipment", list(old_item_stack_type))));
        let new_equipment_type = named(
            ENTITY_EQUIPMENT.type_name(),
            and_all(vec![
                optional(field("ArmorItems", list(new_item_stack_type.clone()))),
                optional(field("HandItems", list(new_item_stack_type.clone()))),
                optional(field("body_armor_item", new_item_stack_type.clone())),
                optional(field("saddle", new_item_stack_type.clone())),
            ]),
        );
        if !equals(&old_equipment_type, &self.get_input_schema().get_type(ENTITY_EQUIPMENT), false) {
            panic!("Input entity_equipment type does not match expected");
        }
        if !equals(&new_equipment_type, &self.get_output_schema().get_type(ENTITY_EQUIPMENT), false) {
            panic!("Output entity_equipment type does not match expected");
        }
        let empty_stack = value::decode(&new_item_stack_type, Some(Nbt::Compound(Vec::new())))
            .value
            .expect("Could not parse newly created empty itemstack.")
            .0;
        TypeRewriteRule::seq(vec![
            self.fix_type_everywhere_typed("EntityEquipmentToArmorAndHandFix - drop chances", self.get_input_schema().get_type(ENTITY), |typed| {
                typed.update_remainder(fix_drop_chances)
            }),
            self.fix_type_everywhere("EntityEquipmentToArmorAndHandFix - equipment", old_equipment_type, new_equipment_type, move |equipment_field| {
                let no_item = || Value::Right(Box::new(Value::Unit));
                let items = match equipment_field {
                    Value::Left(items) => match *items {
                        Value::List(items) => items,
                        _ => Vec::new(),
                    },
                    _ => Vec::new(),
                };
                let mut hand_items = no_item();
                let mut armor_items = no_item();
                if !items.is_empty() {
                    hand_items = Value::Left(Box::new(Value::List(vec![items[0].clone(), empty_stack.clone()])));
                }
                if items.len() > 1 {
                    let mut armor = vec![empty_stack.clone(); 4];
                    for i in 1..items.len().min(5) {
                        armor[i - 1] = items[i].clone();
                    }
                    armor_items = Value::Left(Box::new(Value::List(armor)));
                }
                Value::Pair(Box::new(armor_items), Box::new(Value::Pair(Box::new(hand_items), Box::new(Value::Pair(Box::new(no_item()), Box::new(no_item()))))))
            }),
        ])
    }
}

impl DataFix for EntityEquipmentToArmorAndHandFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.cap(self.get_input_schema().get_type_raw(ITEM_STACK), self.get_output_schema().get_type_raw(ITEM_STACK))
    }
}

fn fix_drop_chances(tag: Dynamic) -> Dynamic {
    let drop_chances = tag.get("DropChances").as_stream_opt();
    let drop_chances = if drop_chances.is_success() { drop_chances.value } else { None };
    let mut tag = tag.remove("DropChances");
    if let Some(drop_chances) = drop_chances {
        let mut chances = drop_chances.into_iter().map(|value| value.as_float(0.0)).chain(std::iter::repeat(0.0));
        let mut next = || chances.next().unwrap_or(0.0);
        let hand_chance = next();
        if tag.get("HandDropChances").result().is_none() {
            let list = tag.create_list([hand_chance, 0.0].into_iter().map(|chance| tag.create_float(chance)));
            tag = tag.set("HandDropChances", list);
        }
        if tag.get("ArmorDropChances").result().is_none() {
            let armor = [next(), next(), next(), next()];
            let list = tag.create_list(armor.into_iter().map(|chance| tag.create_float(chance)));
            tag = tag.set("ArmorDropChances", list);
        }
    }
    tag
}
