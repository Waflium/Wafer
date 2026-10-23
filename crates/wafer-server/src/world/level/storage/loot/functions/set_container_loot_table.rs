use wafer_protocol::nbt::Nbt;

use crate::core::holder::RegistryElement;
use crate::util::codec::{self, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::loot_table::LootTable;

pub struct SetContainerLootTable {
    loot_table_id: String,
    seed: i64,
}

impl SetContainerLootTable {
    pub fn decode(tag: &Nbt) -> DecodeResult<SetContainerLootTable> {
        let mut record = codec::Record::default();
        let loot_table_id = record.required(tag, "loot_table_id", |t| match t {
            Nbt::String(_) => {
                let id = codec::identifier(t)?;
                if LootTable::contains_key(&id) { Ok(id) } else { Err(format!("Missing element ResourceKey[minecraft:loot_table / {id}]")) }
            }
            _ => Err("Only tag names supported".to_string()),
        });
        let seed = record.with_default(tag, "seed", 0, codec::long);
        record.build(|| Some(SetContainerLootTable { loot_table_id: loot_table_id?, seed }))
    }

    pub fn run(&self, mut item_stack: ItemStack, _context: &mut LootContext) -> ItemStack {
        if item_stack.is_empty() {
            return item_stack;
        }
        let mut fields = vec![("loot_table".to_string(), Nbt::String(self.loot_table_id.clone()))];
        if self.seed != 0 {
            fields.push(("seed".into(), Nbt::Long(self.seed)));
        }
        item_stack.set(dc::CONTAINER_LOOT, Some(Nbt::Compound(fields)));
        item_stack
    }
}
