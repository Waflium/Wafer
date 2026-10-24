use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;

pub fn codec() -> crate::world::item::component::types::Pair {
    use crate::world::item::component::types::{self, opt, record};
    record(
        ["is_on_ground", "is_on_fire", "is_sneaking", "is_sprinting", "is_swimming", "is_flying", "is_baby", "is_in_water", "is_fall_flying"]
            .into_iter()
            .map(|flag| opt(flag, types::bool()))
            .collect(),
    )
}

#[derive(Debug)]
pub struct EntityFlagsPredicate {
    is_on_ground: Option<bool>,
    is_on_fire: Option<bool>,
    is_crouching: Option<bool>,
    is_sprinting: Option<bool>,
    is_swimming: Option<bool>,
    is_flying: Option<bool>,
    is_baby: Option<bool>,
    is_in_water: Option<bool>,
    is_fall_flying: Option<bool>,
}

impl EntityFlagsPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<EntityFlagsPredicate> {
        codec::map(tag)?;
        let mut record = codec::Record::default();
        let mut flag = |key| record.field(codec::decode_optional(tag, key, codec::bool_value));
        let is_on_ground = flag("is_on_ground");
        let is_on_fire = flag("is_on_fire");
        let is_crouching = flag("is_sneaking");
        let is_sprinting = flag("is_sprinting");
        let is_swimming = flag("is_swimming");
        let is_flying = flag("is_flying");
        let is_baby = flag("is_baby");
        let is_in_water = flag("is_in_water");
        let is_fall_flying = flag("is_fall_flying");
        record.finish()?;
        Ok(EntityFlagsPredicate {
            is_on_ground: is_on_ground.unwrap(),
            is_on_fire: is_on_fire.unwrap(),
            is_crouching: is_crouching.unwrap(),
            is_sprinting: is_sprinting.unwrap(),
            is_swimming: is_swimming.unwrap(),
            is_flying: is_flying.unwrap(),
            is_baby: is_baby.unwrap(),
            is_in_water: is_in_water.unwrap(),
            is_fall_flying: is_fall_flying.unwrap(),
        })
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        let e = level.entity(entity);
        let differs = |expected: Option<bool>, actual: bool| expected.is_some_and(|expected| expected != actual);
        if differs(self.is_on_ground, e.on_ground) || differs(self.is_on_fire, level.is_on_fire(entity)) {
            return false;
        }
        if differs(self.is_crouching, e.is_crouching()) || differs(self.is_sprinting, e.is_sprinting()) || differs(self.is_swimming, e.is_swimming()) {
            return false;
        }
        let living = level.is_living(entity);
        if let Some(is_flying) = self.is_flying {
            let entity_is_flying = living && (e.is_fall_flying() || level.find_player(entity).is_some_and(|p| p.abilities.flying));
            if entity_is_flying != is_flying {
                return false;
            }
        }
        if differs(self.is_in_water, e.is_in_water()) {
            return false;
        }
        if living && differs(self.is_fall_flying, e.is_fall_flying()) {
            return false;
        }
        !(living && differs(self.is_baby, level.mob(entity).is_some_and(|mob| mob.is_baby())))
    }
}
