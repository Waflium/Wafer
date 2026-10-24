use wafer_protocol::nbt::Nbt;

use super::loot_pool_entries;
use super::loot_pool_entry::LootPoolEntry;
use super::loot_pool_entry_container::{EntryCommonFields, LootPoolEntryContainer};
use crate::core::holder::Holder;
use crate::util::codec::{self, Record};
use crate::world::level::storage::loot::functions::loot_item_function::LootItemFunction;
use crate::world::level::storage::loot::loot_context::LootContext;

#[derive(Clone, Copy)]
pub enum CompositeEntry {
    AlternativesEntry,
    SequentialEntry,
    EntryGroup,
}

pub struct CompositeEntryBase {
    pub(super) children: Vec<LootPoolEntryContainer>,
    pub(super) entry: CompositeEntry,
}

impl CompositeEntryBase {
    pub fn decode_in(record: &mut Record, tag: &Nbt, ty: &str) -> (CompositeEntryBase, EntryCommonFields) {
        let entry = match ty {
            "minecraft:alternatives" => CompositeEntry::AlternativesEntry,
            "minecraft:sequence" => CompositeEntry::SequentialEntry,
            _ => CompositeEntry::EntryGroup,
        };
        let children = record.with_default(tag, "children", Vec::new(), |t| codec::list_partial(t, loot_pool_entries::decode_partial));
        (CompositeEntryBase { children, entry }, LootPoolEntryContainer::decode_common_in(record, tag))
    }

    pub fn expand_raw<'a, 'e>(
        &'e self,
        context: &mut LootContext<'a>,
        modifiers: &[&'e Holder<LootItemFunction>],
        output: &mut dyn FnMut(&mut LootContext<'a>, LootPoolEntry<'e>),
    ) -> bool {
        match self.entry {
            CompositeEntry::AlternativesEntry => {
                for entry in &self.children {
                    if entry.expand(context, modifiers, output) {
                        return true;
                    }
                }
                false
            }
            CompositeEntry::SequentialEntry => {
                for entry in &self.children {
                    if !entry.expand(context, modifiers, output) {
                        return false;
                    }
                }
                true
            }
            CompositeEntry::EntryGroup => {
                for entry in &self.children {
                    entry.expand(context, modifiers, output);
                }
                true
            }
        }
    }
}
