use wafer_data::EquipmentSlot;
use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::enchantment::enchantment_helper::equipment_slot_name;

#[derive(Clone, PartialEq, Debug)]
pub struct EquipmentTable {
    pub loot_table: String,
    pub slot_drop_chances: Vec<(EquipmentSlot, f32)>,
}

impl EquipmentTable {
    fn create_for_all_slots(drop_chance: f32) -> Vec<(EquipmentSlot, f32)> {
        EquipmentSlot::VALUES.iter().map(|&slot| (slot, drop_chance)).collect()
    }

    pub fn drop_chance(&self, slot: EquipmentSlot) -> Option<f32> {
        self.slot_drop_chances.iter().find(|(s, _)| *s == slot).map(|&(_, chance)| chance)
    }

    fn decode_drop_chances(tag: &Nbt) -> DataResult<Vec<(EquipmentSlot, f32)>> {
        if let Ok(drop_chance) = codec::float(tag) {
            return Ok(EquipmentTable::create_for_all_slots(drop_chance));
        }
        let mut chances: Vec<(EquipmentSlot, f32)> = Vec::new();
        for (key, value) in codec::map(tag)? {
            let slot = EquipmentSlot::by_name(key).ok_or_else(|| format!("Unknown element name:{key}"))?;
            let chance = codec::float(value)?;
            match chances.iter_mut().find(|(s, _)| *s == slot) {
                Some(entry) => entry.1 = chance,
                None => chances.push((slot, chance)),
            }
        }
        Ok(chances)
    }

    pub fn decode(tag: &Nbt) -> DataResult<EquipmentTable> {
        let loot_table = codec::decode_field(tag, "loot_table", codec::identifier)?;
        let slot_drop_chances = codec::decode_or(tag, "slot_drop_chances", Vec::new(), EquipmentTable::decode_drop_chances)?;
        Ok(EquipmentTable { loot_table, slot_drop_chances })
    }

    pub fn encode(&self) -> Nbt {
        let mut fields = vec![("loot_table".to_string(), Nbt::String(self.loot_table.clone()))];
        if !self.slot_drop_chances.is_empty() {
            let first = self.slot_drop_chances[0].1;
            let drop_chances_the_same = self.slot_drop_chances.iter().all(|&(_, chance)| chance.to_bits() == first.to_bits());
            let all_slots_are_present = EquipmentSlot::VALUES.iter().all(|&slot| self.drop_chance(slot).is_some());
            let chances = if drop_chances_the_same && all_slots_are_present {
                Nbt::Float(first)
            } else {
                Nbt::Compound(self.slot_drop_chances.iter().map(|&(slot, chance)| (equipment_slot_name(slot).to_string(), Nbt::Float(chance))).collect())
            };
            fields.push(("slot_drop_chances".into(), chances));
        }
        Nbt::Compound(fields)
    }
}
