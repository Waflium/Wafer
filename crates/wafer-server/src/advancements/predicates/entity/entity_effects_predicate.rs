use wafer_protocol::nbt::Nbt;

use crate::advancements::predicates::mob_effects_predicate::{self, MobEffectsPredicate};
use crate::world::item::component::types::Pair;

pub fn codec() -> Pair {
    mob_effects_predicate::codec()
}
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec::Wrap;
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct EntityEffectsPredicate {
    effects: MobEffectsPredicate,
}

impl EntityEffectsPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<EntityEffectsPredicate> {
        codec::wrapped(Wrap::Xmap, || MobEffectsPredicate::decode(tag)).map(|effects| EntityEffectsPredicate { effects })
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        level.is_living(entity) && self.effects.matches(&level.living(entity).active_effects)
    }
}
