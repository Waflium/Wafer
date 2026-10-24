use wafer_protocol::nbt::Nbt;

use super::loot_pool_entry::{LootPoolEntry, LootPoolEntryKind};
use super::loot_pool_entry_container::{EntryCommonFields, LootPoolEntryContainer};
use crate::core::holder::Holder;
use crate::util::codec::{self, Record};
use crate::world::level::storage::loot::functions::loot_item_function::LootItemFunction;

pub const DEFAULT_WEIGHT: i32 = 1;
pub const DEFAULT_QUALITY: i32 = 0;

pub struct UniformContainerBase {
    weight: i32,
    quality: i32,
}

impl UniformContainerBase {
    pub fn decode_in(record: &mut Record, tag: &Nbt) -> (UniformContainerBase, EntryCommonFields) {
        let weight = record.with_default(tag, "weight", DEFAULT_WEIGHT, codec::int);
        let quality = record.with_default(tag, "quality", DEFAULT_QUALITY, codec::int);
        (UniformContainerBase { weight, quality }, LootPoolEntryContainer::decode_common_in(record, tag))
    }

    pub fn entry<'e>(&self, kind: LootPoolEntryKind<'e>, modifiers: &[&'e Holder<LootItemFunction>]) -> LootPoolEntry<'e> {
        LootPoolEntry::new(self.weight, self.quality, kind, modifiers)
    }
}
