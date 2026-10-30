use wafer_data::EquipmentSlot;
use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult, DecodeResult, Record};
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

    fn decode_drop_chances(tag: &Nbt) -> DecodeResult<Vec<(EquipmentSlot, f32)>> {
        let slot = |key: &str| EquipmentSlot::by_name(key).ok_or_else(|| format!("Unknown element name:{key}"));
        let chances = |tag: &Nbt| {
            codec::base_map_partial(tag, slot, |value| DecodeResult::from(codec::float(value))).map(|entries| {
                let mut chances: Vec<(EquipmentSlot, f32)> = Vec::new();
                for (slot, chance) in entries {
                    match chances.iter_mut().find(|(s, _)| *s == slot) {
                        Some(entry) => entry.1 = chance,
                        None => chances.push((slot, chance)),
                    }
                }
                chances
            })
        };
        codec::either_result(DecodeResult::from(codec::float(tag)), || chances(tag)).map(|either| match either {
            Ok(drop_chance) => EquipmentTable::create_for_all_slots(drop_chance),
            Err(chances) => chances,
        })
    }

    pub fn decode(tag: &Nbt) -> DataResult<EquipmentTable> {
        EquipmentTable::decode_partial(tag).into_result()
    }

    pub fn decode_partial(tag: &Nbt) -> DecodeResult<EquipmentTable> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let loot_table = record.required(tag, "loot_table", codec::identifier);
        let slot_drop_chances = record.with_default(tag, "slot_drop_chances", Vec::new(), EquipmentTable::decode_drop_chances);
        record.build(|| Some(EquipmentTable { loot_table: loot_table?, slot_drop_chances }))
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
        Nbt::Compound(fields.into())
    }
}
