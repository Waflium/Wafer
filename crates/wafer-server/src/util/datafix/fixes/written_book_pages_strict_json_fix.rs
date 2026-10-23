use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{find_field, type_finder};
use crate::util::datafix::fixes::item_stack_tag_fix::ItemStackTagFix;
use crate::util::datafix::fixes::references::{ITEM_STACK, TEXT_COMPONENT};
use crate::util::datafix::legacy_component_data_fix_utils;

pub struct WrittenBookPagesStrictJsonFix;

impl WrittenBookPagesStrictJsonFix {
    pub fn new(output_schema: &Arc<Schema>) -> ItemStackTagFix {
        let text_component_type = output_schema.get_type(TEXT_COMPONENT);
        let item_stack_type = output_schema.get_type(ITEM_STACK);
        let tag_finder = find_field(&item_stack_type, "tag");
        let pages_finder = find_field(&tag_finder.ty(), "pages");
        let page_finder = type_finder(text_component_type);
        ItemStackTagFix::new(
            output_schema,
            "WrittenBookPagesStrictJsonFix",
            |id| id == "minecraft:written_book",
            move |tag| tag.update_typed(&pages_finder, None, |pages| pages.update(&page_finder, |page| page.map_string(legacy_component_data_fix_utils::rewrite_from_lenient))),
        )
    }
}
