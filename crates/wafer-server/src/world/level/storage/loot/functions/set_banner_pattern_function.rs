use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct SetBannerPatternFunction {
    patterns: Nbt,
    append: bool,
}

impl SetBannerPatternFunction {
    pub fn decode(tag: &Nbt) -> DecodeResult<SetBannerPatternFunction> {
        let mut record = codec::Record::default();
        let patterns = record.required(tag, "patterns", |t| dc::BANNER_PATTERNS.decode(t));
        let append = record.required(tag, "append", codec::bool_value);
        record.build(|| Some(SetBannerPatternFunction { patterns: patterns?, append: append? }))
    }

    pub fn run(&self, mut item_stack: ItemStack, _context: &mut LootContext) -> ItemStack {
        if self.append {
            let mut layers = item_stack.get(dc::BANNER_PATTERNS).map_or(Vec::new(), |p| p.as_list().to_vec());
            layers.extend_from_slice(self.patterns.as_list());
            item_stack.set(dc::BANNER_PATTERNS, Some(Nbt::List(layers)));
        } else {
            item_stack.set(dc::BANNER_PATTERNS, Some(self.patterns.clone()));
        }
        item_stack
    }
}
