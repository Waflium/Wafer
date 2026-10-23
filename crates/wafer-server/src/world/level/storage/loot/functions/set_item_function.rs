use wafer_data::Item;
use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct SetItemFunction {
    item: Item,
}

pub fn decode_item(tag: &Nbt) -> DataResult<Item> {
    let id = codec::identifier(tag)?;
    Item::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:item]: {id}"))
}

impl SetItemFunction {
    pub fn decode(tag: &Nbt) -> DecodeResult<SetItemFunction> {
        let mut record = codec::Record::default();
        let item = record.required(tag, "item", decode_item);
        record.build(|| Some(SetItemFunction { item: item? }))
    }

    pub fn run(&self, item_stack: ItemStack, _context: &mut LootContext) -> ItemStack {
        item_stack.transmute_copy(self.item, item_stack.count)
    }
}
