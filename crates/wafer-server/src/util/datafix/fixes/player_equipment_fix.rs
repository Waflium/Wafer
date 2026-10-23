use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::PLAYER;

pub struct PlayerEquipmentFix {
    base: DataFixBase,
}

impl PlayerEquipmentFix {
    pub fn new(output_schema: &Arc<Schema>) -> PlayerEquipmentFix {
        PlayerEquipmentFix { base: DataFixBase::new(output_schema, true) }
    }
}

fn slot_translation(slot: i32) -> Option<&'static str> {
    match slot {
        100 => Some("feet"),
        101 => Some("legs"),
        102 => Some("chest"),
        103 => Some("head"),
        -106 => Some("offhand"),
        _ => None,
    }
}

impl DataFix for PlayerEquipmentFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let old_player_type = self.get_input_schema().get_type_raw(PLAYER);
        let new_player_type = self.get_output_schema().get_type_raw(PLAYER);
        self.write_fix_and_read("Player Equipment Fix", old_player_type, new_player_type, |tag| {
            let creator = tag.empty_map();
            let mut equipment: Vec<(&'static str, Dynamic)> = Vec::new();
            let tag = tag.update("Inventory", |inventory| {
                inventory.create_list(inventory.as_stream().into_iter().filter(|item| {
                    let inventory_slot = item.get("Slot").as_int(-1);
                    let equipment_slot = slot_translation(inventory_slot);
                    if let Some(equipment_slot) = equipment_slot {
                        let value = item.clone().remove("Slot");
                        match equipment.iter_mut().find(|(slot, _)| *slot == equipment_slot) {
                            Some(existing) => existing.1 = value,
                            None => equipment.push((equipment_slot, value)),
                        }
                    }
                    equipment_slot.is_none()
                }))
            });
            let equipment = creator.create_map(equipment.into_iter().map(|(slot, item)| (creator.create_string(slot), item)));
            tag.set("equipment", equipment)
        })
    }
}
