use wafer_protocol::nbt::Nbt;

use super::damage_source_predicate::DamageSourcePredicate;
use super::entity::entity_predicate::EntityPredicate;
use super::min_max_bounds::Doubles;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct DamagePredicate {
    dealt_damage: Doubles,
    taken_damage: Doubles,
    source_entity: Option<EntityPredicate>,
    blocked: Option<bool>,
    ty: Option<DamageSourcePredicate>,
}

impl DamagePredicate {
    pub fn decode(tag: &Nbt) -> DataResult<DamagePredicate> {
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = codec::Record::default();
            let dealt_damage = record.with_default(tag, "dealt", Doubles::ANY, Doubles::decode_partial);
            let taken_damage = record.with_default(tag, "taken", Doubles::ANY, Doubles::decode_partial);
            let source_entity = record.optional(tag, "source_entity", EntityPredicate::decode);
            let blocked = record.optional(tag, "blocked", codec::bool_value);
            let ty = record.optional(tag, "type", DamageSourcePredicate::decode);
            record.build(|| Some(DamagePredicate { dealt_damage, taken_damage, source_entity, blocked, ty }))
        })
        .into_result()
    }

    pub fn matches(&self, level: &mut ServerLevel, player: EntityId, source: &DamageSource, original_damage: f32, actual_damage: f32, blocked: bool) -> bool {
        if !self.dealt_damage.matches(original_damage as f64) {
            return false;
        }
        if !self.taken_damage.matches(actual_damage as f64) {
            return false;
        }
        if self.source_entity.as_ref().is_some_and(|source_entity| !source_entity.matches_player(level, player, source.causing_entity)) {
            return false;
        }
        if self.blocked.is_some_and(|expected| expected != blocked) {
            return false;
        }
        self.ty.as_ref().is_none_or(|ty| {
            let position = level.entity(player).position();
            ty.matches(level, Some(position), source)
        })
    }
}
