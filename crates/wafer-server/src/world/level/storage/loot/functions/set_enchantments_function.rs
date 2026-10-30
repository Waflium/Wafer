use wafer_core::mth;
use wafer_data::items;
use wafer_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::util::codec::{self, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::item::enchantment::enchantment::Enchantment;
use crate::world::item::enchantment::enchantment_helper::update_enchantments;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::providers::number::ints::ContextIntProvider;
use crate::world::level::storage::loot::validatable;
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub struct SetEnchantmentsFunction {
    enchantments: Vec<(String, Holder<ContextIntProvider>)>,
    add: bool,
}

impl SetEnchantmentsFunction {
    pub fn decode(tag: &Nbt) -> DecodeResult<SetEnchantmentsFunction> {
        let mut record = codec::Record::default();
        let enchantments = record.with_default(tag, "enchantments", Vec::new(), |t| {
            codec::base_map_partial(t, |k| Enchantment::decode_holder(&Nbt::String(k.to_string())), ContextIntProvider::decode_holder_partial)
        });
        let add = record.with_default(tag, "add", false, codec::bool_value);
        record.build(|| Some(SetEnchantmentsFunction { enchantments, add }))
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        if item_stack.is(items::BOOK) {
            item_stack = item_stack.transmute_copy(items::ENCHANTED_BOOK, item_stack.count);
        }
        let mut levels = Vec::with_capacity(self.enchantments.len());
        update_enchantments(&mut item_stack, |enchantments| {
            for (enchantment, level_provider) in &self.enchantments {
                let level = level_provider.value().get_int(context);
                let level = if self.add { enchantments.level(enchantment).wrapping_add(level) } else { level };
                levels.push((enchantment, mth::clamp_i(level, 0, 255)));
            }
            for (enchantment, level) in levels.drain(..) {
                enchantments.set(enchantment, level);
            }
        });
        item_stack
    }
}

impl SetEnchantmentsFunction {
    pub fn validate(&self, context: &ValidationContext) {
        for (enchantment, holder) in &self.enchantments {
            validatable::validate_holder_in(&context.for_map_field("enchantments", enchantment), holder);
        }
    }
}
