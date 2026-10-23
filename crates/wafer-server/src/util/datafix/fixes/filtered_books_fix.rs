use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::util::datafix::fixes::item_stack_tag_fix::ItemStackTagFix;

pub struct FilteredBooksFix;

impl FilteredBooksFix {
    pub fn new(output_schema: &Arc<Schema>) -> ItemStackTagFix {
        ItemStackTagFix::new(output_schema, "Remove filtered text from books", |id| id == "minecraft:writable_book" || id == "minecraft:written_book", fix_item_stack_tag)
    }
}

fn fix_item_stack_tag(tag: Typed) -> Typed {
    let ty = tag.ty.clone();
    crate::util::write_and_read_typed_or_throw(&tag, &ty, |dynamic| dynamic.remove("filtered_title").remove("filtered_pages"))
}
