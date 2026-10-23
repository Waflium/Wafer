use wafer_protocol::nbt::Nbt;

use crate::advancements::predicates::nbt_predicate::NbtPredicate;
use crate::util::codec::{self, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct SetCustomDataFunction {
    tag: Vec<(String, Nbt)>,
}

impl SetCustomDataFunction {
    pub fn decode(tag: &Nbt) -> DecodeResult<SetCustomDataFunction> {
        let mut record = codec::Record::default();
        let fields = record.required(tag, "tag", |t| {
            NbtPredicate::decode(t).map(|predicate| {
                let Nbt::Compound(fields) = predicate.tag else { unreachable!("compound tag") };
                fields
            })
        });
        record.build(|| Some(SetCustomDataFunction { tag: fields? }))
    }

    pub fn run(&self, mut item_stack: ItemStack, _context: &mut LootContext) -> ItemStack {
        let mut custom_data = match item_stack.get(dc::CUSTOM_DATA) {
            Some(Nbt::Compound(fields)) => fields.clone(),
            _ => Vec::new(),
        };
        crate::nbt::merge(&mut custom_data, &self.tag);
        item_stack.set(dc::CUSTOM_DATA, if custom_data.is_empty() { None } else { Some(Nbt::Compound(custom_data)) });
        item_stack
    }
}
