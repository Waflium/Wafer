use wafer_core::Vec3;
use wafer_protocol::nbt::Nbt;

use super::entity_predicate::{self, EntityPredicate};
use crate::advancements::predicates::min_max_bounds::{Ints, ints_codec};
use crate::world::item::component::types::{self, Pair, def, opt, record};

pub fn codec() -> Pair {
    record(vec![def("blocks_set_on_fire", ints_codec(), types::empty_map()), opt("entity_struck", entity_predicate::codec())])
}
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::{EntityId, LevelEntity};

#[derive(Debug)]
pub struct LightningBoltPredicate {
    blocks_set_on_fire: Ints,
    entity_struck: Option<Box<EntityPredicate>>,
}

impl LightningBoltPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<LightningBoltPredicate> {
        codec::map(tag)?;
        let mut record = codec::Record::default();
        let blocks_set_on_fire = record.field(codec::decode_or(tag, "blocks_set_on_fire", Ints::ANY, Ints::decode));
        let entity_struck = record.field(codec::decode_optional(tag, "entity_struck", |t| Ok(Box::new(EntityPredicate::decode(t)?))));
        record.finish()?;
        Ok(LightningBoltPredicate { blocks_set_on_fire: blocks_set_on_fire.unwrap(), entity_struck: entity_struck.unwrap() })
    }

    pub fn matches(&self, level: &mut ServerLevel, entity: EntityId, position: Option<Vec3>) -> bool {
        let Some(LevelEntity::LightningBolt(bolt)) = level.find_entity(entity) else { return false };
        if !self.blocks_set_on_fire.matches(bolt.blocks_set_on_fire) {
            return false;
        }
        let Some(entity_struck) = &self.entity_struck else { return true };
        let hit: Vec<EntityId> = bolt.hit_entities().collect();
        hit.into_iter().any(|e| entity_struck.matches(level, position, Some(e)))
    }
}
