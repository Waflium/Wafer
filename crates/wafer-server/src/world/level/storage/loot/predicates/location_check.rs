use wafer_protocol::nbt::Nbt;

use crate::advancements::predicates::location_predicate::LocationPredicate;
use crate::util::codec::{self, DecodeResult};
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::parameters::loot_context_params::LootContextParam;

pub struct LocationCheck {
    predicate: Option<LocationPredicate>,
    offset: (i32, i32, i32),
}

impl LocationCheck {
    pub fn decode(tag: &Nbt) -> DecodeResult<LocationCheck> {
        let mut record = codec::Record::default();
        let predicate = record.optional(tag, "predicate", LocationPredicate::decode);
        let offset = record.nested(|| {
            let mut offset_record = codec::Record::default();
            let x = offset_record.with_default(tag, "offsetX", 0, codec::int);
            let y = offset_record.with_default(tag, "offsetY", 0, codec::int);
            let z = offset_record.with_default(tag, "offsetZ", 0, codec::int);
            offset_record.build(|| Some((x, y, z)))
        });
        record.build(|| Some(LocationCheck { predicate, offset: offset? }))
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        let Some(pos) = context.params.origin else { return false };
        let Some(predicate) = &self.predicate else { return true };
        let (x, y, z) = self.offset;
        predicate.matches(context.level(), pos.x + x as f64, pos.y + y as f64, pos.z + z as f64)
    }
}

impl LocationCheck {
    pub fn get_referenced_context_params(&self) -> Vec<LootContextParam> {
        vec![LootContextParam::Origin]
    }
}
