use wafer_data::EquipmentSlot;
use wafer_protocol::nbt::Nbt;

use crate::advancements::predicates::item_predicate::ItemPredicate;
use crate::world::item::component::codec::Stream;
use crate::world::item::component::custom;
use crate::world::item::component::types::{Pair, opt, record};

pub fn codec() -> Pair {
    record(["head", "chest", "legs", "feet", "body", "mainhand", "offhand"].into_iter().map(|slot| opt(slot, (custom::item_predicate(), Stream::Nbt))).collect())
}
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;

#[derive(Debug)]
pub struct EntityEquipmentPredicate {
    head: Option<ItemPredicate>,
    chest: Option<ItemPredicate>,
    legs: Option<ItemPredicate>,
    feet: Option<ItemPredicate>,
    body: Option<ItemPredicate>,
    mainhand: Option<ItemPredicate>,
    offhand: Option<ItemPredicate>,
}

impl EntityEquipmentPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<EntityEquipmentPredicate> {
        codec::map(tag)?;
        let mut record = codec::Record::default();
        let mut slot = |key| record.field(codec::decode_optional(tag, key, ItemPredicate::decode));
        let (head, chest, legs, feet, body, mainhand, offhand) = (slot("head"), slot("chest"), slot("legs"), slot("feet"), slot("body"), slot("mainhand"), slot("offhand"));
        record.finish()?;
        Ok(EntityEquipmentPredicate {
            head: head.unwrap(),
            chest: chest.unwrap(),
            legs: legs.unwrap(),
            feet: feet.unwrap(),
            body: body.unwrap(),
            mainhand: mainhand.unwrap(),
            offhand: offhand.unwrap(),
        })
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        if !level.is_living(entity) {
            return false;
        }
        [
            (&self.head, EquipmentSlot::Head),
            (&self.chest, EquipmentSlot::Chest),
            (&self.legs, EquipmentSlot::Legs),
            (&self.feet, EquipmentSlot::Feet),
            (&self.body, EquipmentSlot::Body),
            (&self.mainhand, EquipmentSlot::Mainhand),
            (&self.offhand, EquipmentSlot::Offhand),
        ]
        .into_iter()
        .all(|(predicate, slot)| predicate.as_ref().is_none_or(|p| p.test(&level.item_by_slot(entity, slot).unwrap_or_else(ItemStack::empty))))
    }
}
