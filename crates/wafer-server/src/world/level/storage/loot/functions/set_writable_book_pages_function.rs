use wafer_protocol::nbt::Nbt;

use super::list_operation::ListOperation;
use crate::util::codec::{self, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct SetWritableBookPagesFunction {
    pages: Vec<Nbt>,
    page_operation: ListOperation,
}

impl SetWritableBookPagesFunction {
    pub fn decode(tag: &Nbt) -> DecodeResult<SetWritableBookPagesFunction> {
        let mut record = codec::Record::default();
        let pages = record.required(tag, "pages", |t| {
            dc::writable_book_pages().0.decode_partial(t).map(|pages| pages.as_list().to_vec())
        });
        let page_operation = record.nested(|| ListOperation::decode(tag, 100));
        record.build(|| Some(SetWritableBookPagesFunction { pages: pages?, page_operation: page_operation? }))
    }

    pub fn run(&self, mut item_stack: ItemStack, _context: &mut LootContext) -> ItemStack {
        let original = item_stack.get(dc::WRITABLE_BOOK_CONTENT).and_then(|c| c.get("pages")).map_or(Vec::new(), |p| p.as_list().to_vec());
        let new_pages = self.page_operation.apply(&original, &self.pages, 100);
        let value = Nbt::Compound(vec![("pages".into(), Nbt::List(new_pages))].into());
        let value = dc::WRITABLE_BOOK_CONTENT.decode(&value).unwrap_or(value);
        item_stack.set(dc::WRITABLE_BOOK_CONTENT, Some(value));
        item_stack
    }
}
