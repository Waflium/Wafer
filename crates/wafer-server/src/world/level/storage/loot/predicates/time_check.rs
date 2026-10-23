use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DecodeResult};
use crate::world::clock::ServerClockManager;
use crate::world::level::storage::loot::int_range_predicate::IntRangePredicate;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::validatable;
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub struct TimeCheck {
    clock: usize,
    period: Option<i64>,
    value: IntRangePredicate,
}

impl TimeCheck {
    pub fn decode(tag: &Nbt) -> DecodeResult<TimeCheck> {
        let mut record = codec::Record::default();
        let clock = record.required(tag, "clock", |t| {
            let id = codec::identifier(t)?;
            ServerClockManager::id_of(&id).ok_or_else(|| format!("Missing element ResourceKey[minecraft:world_clock / {id}]"))
        });
        let period = record.optional(tag, "period", codec::long);
        let value = record.required(tag, "value", IntRangePredicate::decode);
        record.build(|| Some(TimeCheck { clock: clock?, period, value: value? }))
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        let mut time = context.params.level.clock_ticks[self.clock];
        if let Some(period) = self.period {
            time = time.wrapping_rem(period);
        }
        self.value.test(context, time as i32)
    }
}

impl TimeCheck {
    pub fn validate(&self, context: &ValidationContext) {
        validatable::validate(context, "value", &self.value);
    }
}
