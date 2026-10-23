use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::item_stack_tag_fix::ItemStackTagFix;

pub struct OminousBannerRenameFix;

impl OminousBannerRenameFix {
    pub fn new(output_schema: &Arc<Schema>) -> ItemStackTagFix {
        ItemStackTagFix::new(output_schema, "OminousBannerRenameFix", |id| id == "minecraft:white_banner", |tag| {
            let ty = tag.ty.clone();
            crate::util::write_and_read_typed_or_throw(&tag, &ty, fix_item_stack_tag)
        })
    }
}

fn fix_item_stack_tag(tag: Dynamic) -> Dynamic {
    tag.update("display", |display| {
        display.update("Name", |name| match name.as_string().into_result() {
            Ok(string) => name.create_string(string.replace("\"translate\":\"block.minecraft.illager_banner\"", "\"translate\":\"block.minecraft.ominous_banner\"")),
            Err(_) => name,
        })
    })
}
