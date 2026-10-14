use wafer_data::EquipmentSlot;

use super::EntityId;
use super::equipment_table::EquipmentTable;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc, values::Equippable};
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::level::storage::loot::loot_table::{self, LootTable};

pub type LootParamsFactory = for<'a> fn(&'a mut ServerLevel, EntityId) -> LootParams<'a>;

impl ServerLevel {
    pub fn equip_table(&mut self, id: EntityId, equipment: &EquipmentTable, loot_params: LootParamsFactory) {
        self.equip_loot_table(id, &equipment.loot_table, loot_params, &equipment.slot_drop_chances);
    }

    pub fn equip_loot_table(&mut self, id: EntityId, loot_table: &str, loot_params: LootParamsFactory, drop_chances: &[(EquipmentSlot, f32)]) {
        self.equip_loot_table_with_seed(id, loot_table, loot_params, 0, drop_chances);
    }

    pub fn equip_loot_table_with_seed(&mut self, id: EntityId, loot_table: &str, loot_params: LootParamsFactory, optional_loot_table_seed: i64, drop_chances: &[(EquipmentSlot, f32)]) {
        let table = LootTable::get(loot_table);
        if std::ptr::eq(table, &loot_table::EMPTY) {
            return;
        }
        let mut possible_equipment = Vec::new();
        table.get_random_items_with_seed(loot_params(self, id), optional_loot_table_seed, &mut |_, item_stack| possible_equipment.push(item_stack));
        let mut inserted_into_slots = Vec::new();
        for to_equip in possible_equipment {
            let Some(slot) = resolve_slot(&to_equip, &inserted_into_slots) else { continue };
            let equipped = super::mob::limit(slot, to_equip);
            self.living_set_item_slot(id, slot, equipped);
            if let Some(&(_, drop_chance)) = drop_chances.iter().find(|(s, _)| *s == slot)
                && let Some(mob) = self.mob_mut(id)
            {
                mob.set_drop_chance(slot, drop_chance);
            }
            inserted_into_slots.push(slot);
        }
    }
}

pub fn resolve_slot(to_equip: &ItemStack, already_inserted_into_slots: &[EquipmentSlot]) -> Option<EquipmentSlot> {
    if to_equip.is_empty() {
        return None;
    }
    match to_equip.get(dc::EQUIPPABLE).map(Equippable::from_nbt) {
        Some(equippable) => EquipmentSlot::by_name(&equippable.slot).filter(|slot| !already_inserted_into_slots.contains(slot)),
        None => (!already_inserted_into_slots.contains(&EquipmentSlot::Mainhand)).then_some(EquipmentSlot::Mainhand),
    }
}
