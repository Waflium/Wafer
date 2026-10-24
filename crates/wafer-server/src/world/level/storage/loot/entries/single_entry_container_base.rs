use wafer_data::Item;
use wafer_protocol::nbt::Nbt;

use super::loot_pool_entry::{LootPoolEntry, LootPoolEntryKind};
use super::loot_pool_entry_container::EntryCommonFields;
use super::uniform_container_base::UniformContainerBase;
use crate::core::holder::Holder;
use crate::util::codec::{self, Record};
use crate::world::item::slot::slot_source::SlotSource;
use crate::world::level::storage::loot::functions::loot_item_function::LootItemFunction;
use crate::world::level::storage::loot::loot_context::LootContext;

pub enum SingleEntry {
    EmptyLootItem,
    LootItem(Item),
    DynamicLoot(String),
    SlotLoot(Holder<SlotSource>),
}

pub struct SingleEntryContainerBase {
    base: UniformContainerBase,
    pub(super) entry: SingleEntry,
}

impl SingleEntryContainerBase {
    pub fn decode_in(record: &mut Record, tag: &Nbt, ty: &str) -> Option<(SingleEntryContainerBase, EntryCommonFields)> {
        let entry = match ty {
            "minecraft:item" => record.required(tag, "name", |t| {
                let id = codec::identifier(t)?;
                Item::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:item]: {id}"))
            })
            .map(SingleEntry::LootItem),
            "minecraft:dynamic" => record.required(tag, "name", codec::identifier).map(SingleEntry::DynamicLoot),
            "minecraft:slots" => record.required(tag, "slot_source", SlotSource::decode_holder).map(SingleEntry::SlotLoot),
            _ => Some(SingleEntry::EmptyLootItem),
        };
        let (base, common) = UniformContainerBase::decode_in(record, tag);
        Some((SingleEntryContainerBase { base, entry: entry? }, common))
    }

    pub fn expand_raw<'a, 'e>(
        &'e self,
        context: &mut LootContext<'a>,
        modifiers: &[&'e Holder<LootItemFunction>],
        output: &mut dyn FnMut(&mut LootContext<'a>, LootPoolEntry<'e>),
    ) -> bool {
        let kind = match &self.entry {
            SingleEntry::EmptyLootItem => LootPoolEntryKind::Empty,
            SingleEntry::LootItem(item) => LootPoolEntryKind::Item(*item),
            SingleEntry::DynamicLoot(name) => LootPoolEntryKind::Dynamic(name),
            SingleEntry::SlotLoot(slot_source) => LootPoolEntryKind::Slots(slot_source),
        };
        output(context, self.base.entry(kind, modifiers));
        true
    }
}
