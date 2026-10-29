use wafer_core::Vec3;
use wafer_protocol::nbt::Nbt;

use super::entity::entity_predicate::{self, EntityPredicate};
use super::tag_predicate::{self, TagPredicate};
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::damagesource::DamageSource;
use crate::world::item::component::types::{self, Pair, def, opt, record};

pub fn codec() -> Pair {
    record(vec![
        def("tags", types::list(tag_predicate::codec("minecraft:damage_type")), types::empty_list()),
        opt("direct_entity", entity_predicate::codec()),
        opt("source_entity", entity_predicate::codec()),
        opt("is_direct", types::bool()),
    ])
}

#[derive(Debug)]
pub struct DamageSourcePredicate {
    tags: Vec<TagPredicate>,
    direct_entity: Option<EntityPredicate>,
    source_entity: Option<EntityPredicate>,
    is_direct: Option<bool>,
}

impl DamageSourcePredicate {
    pub fn decode(tag: &Nbt) -> DataResult<DamageSourcePredicate> {
        codec::map(tag)?;
        let mut record = codec::Record::default();
        let tags = record.with_default(tag, "tags", Vec::new(), |t| codec::list_partial(t, |e| DecodeResult::from(TagPredicate::decode("minecraft:damage_type", e))));
        let direct_entity = record.optional(tag, "direct_entity", |t| DecodeResult::from(EntityPredicate::decode(t)));
        let source_entity = record.optional(tag, "source_entity", |t| DecodeResult::from(EntityPredicate::decode(t)));
        let is_direct = record.optional(tag, "is_direct", |t| DecodeResult::from(codec::bool_value(t)));
        record.build(|| Some(DamageSourcePredicate { tags, direct_entity, source_entity, is_direct })).into_result()
    }

    pub fn matches(&self, level: &mut ServerLevel, position: Option<Vec3>, source: &DamageSource) -> bool {
        if !self.tags.iter().all(|tag| tag.matches(source.ty.0 as i32)) {
            return false;
        }
        if self.direct_entity.as_ref().is_some_and(|direct| !direct.matches(level, position, source.direct_entity)) {
            return false;
        }
        if self.source_entity.as_ref().is_some_and(|entity| !entity.matches(level, position, source.causing_entity)) {
            return false;
        }
        self.is_direct.is_none_or(|is_direct| is_direct == source.is_direct())
    }
}
