use wafer_data::Item;
use wafer_protocol::Writer;
use wafer_protocol::nbt::Nbt;

use crate::core::component::data_component_exact_predicate::DataComponentExactPredicate;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component::codec as component_codec;

#[derive(Clone, Debug, PartialEq)]
pub struct ItemCost {
    pub item: Item,
    pub count: i32,
    pub components: DataComponentExactPredicate,
    pub item_stack: ItemStack,
}

pub fn decode_item(tag: &Nbt) -> DataResult<Item> {
    let id = component_codec::Codec::Element("minecraft:item").decode(tag)?;
    if id == Nbt::String("minecraft:air".into()) {
        return Err("Item must not be minecraft:air".into());
    }
    Item::by_name(id.as_str().unwrap_or("")).ok_or_else(|| "unknown item".to_string())
}

impl ItemCost {
    pub fn new(item: Item, count: i32, components: DataComponentExactPredicate) -> ItemCost {
        let item_stack = ItemCost::create_stack(item, count, &components);
        ItemCost { item, count, components, item_stack }
    }

    pub fn of(item: Item, count: i32) -> ItemCost {
        ItemCost::new(item, count, DataComponentExactPredicate::default())
    }

    fn create_stack(item: Item, count: i32, components: &DataComponentExactPredicate) -> ItemStack {
        ItemStack::with_patch(item, count, &components.as_patch())
    }

    pub fn test(&self, item_stack: &ItemStack) -> bool {
        item_stack.is(self.item) && self.components.test(item_stack)
    }

    pub fn decode(tag: &Nbt) -> DataResult<ItemCost> {
        codec::map(tag)?;
        let item = codec::decode_field(tag, "id", decode_item)?;
        let count = codec::decode_or(tag, "count", 1, |t| {
            let count = codec::int(t)?;
            if count > 0 { Ok(count) } else { Err(format!("Value must be positive: {count}")) }
        })?;
        let components = codec::decode_or(tag, "components", DataComponentExactPredicate::default(), DataComponentExactPredicate::decode)?;
        Ok(ItemCost::new(item, count, components))
    }

    pub fn to_nbt(&self) -> Nbt {
        let mut fields = vec![("id".to_string(), Nbt::String(self.item.name().into())), ("count".to_string(), Nbt::Int(self.count))];
        if !self.components.is_empty() {
            fields.push(("components".to_string(), self.components.to_nbt()));
        }
        Nbt::Compound(fields.into())
    }

    pub fn write(&self, w: &mut Writer) {
        w.varint(self.item.0 as i32);
        w.varint(self.count);
        self.components.write(w).expect("component value encodes");
    }

    pub fn write_optional(cost: Option<&ItemCost>, w: &mut Writer) {
        w.bool(cost.is_some());
        if let Some(cost) = cost {
            cost.write(w);
        }
    }
}
