use wafer_protocol::nbt::Nbt;

use super::set_written_book_pages_function::{put, written_book_content};
use crate::util::codec::{self, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct SetBookCoverFunction {
    author: Option<String>,
    title: Option<Nbt>,
    generation: Option<i32>,
}

impl SetBookCoverFunction {
    pub fn decode(tag: &Nbt) -> DecodeResult<SetBookCoverFunction> {
        let mut record = codec::Record::default();
        let title = record.optional(tag, "title", |t| {
            let probe = Nbt::Compound(vec![("title".into(), t.clone()), ("author".into(), Nbt::String(String::new()))].into());
            let content = dc::WRITTEN_BOOK_CONTENT.decode(&probe)?;
            Ok::<Nbt, String>(content.get("title").cloned().unwrap_or_else(|| t.clone()))
        });
        let author = record.optional(tag, "author", |t| Ok::<String, String>(codec::string(t)?.to_string()));
        let generation = record.optional(tag, "generation", |t| codec::int_range(t, 0, 3));
        record.build(|| Some(SetBookCoverFunction { author, title, generation }))
    }

    pub fn run(&self, mut item_stack: ItemStack, _context: &mut LootContext) -> ItemStack {
        let mut content = written_book_content(&item_stack);
        if let Some(title) = &self.title {
            put(&mut content, "title", title.clone());
        }
        if let Some(author) = &self.author {
            put(&mut content, "author", Nbt::String(author.clone()));
        }
        if let Some(generation) = self.generation {
            put(&mut content, "generation", Nbt::Int(generation));
        }
        let value = Nbt::Compound(content.into());
        let value = dc::WRITTEN_BOOK_CONTENT.decode(&value).unwrap_or(value);
        item_stack.set(dc::WRITTEN_BOOK_CONTENT, Some(value));
        item_stack
    }
}
