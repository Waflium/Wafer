use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{OpticFinder, named_choice};
use crate::util::datafix::extra_data_fix_utils;
use crate::util::datafix::fixes::references::{ENTITY, PLAYER};

pub struct InlineBlockPosFormatFix {
    base: DataFixBase,
}

impl InlineBlockPosFormatFix {
    pub fn new(output_schema: &Arc<Schema>) -> InlineBlockPosFormatFix {
        InlineBlockPosFormatFix { base: DataFixBase::new(output_schema, false) }
    }

    fn entity_finder(&self, choice_name: &str) -> OpticFinder {
        named_choice(choice_name, self.get_input_schema().get_choice_type(ENTITY, choice_name))
    }
}

impl DataFix for InlineBlockPosFormatFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let vex_finder = self.entity_finder("minecraft:vex");
        let phantom_finder = self.entity_finder("minecraft:phantom");
        let turtle_finder = self.entity_finder("minecraft:turtle");
        let block_attached_finders = [
            self.entity_finder("minecraft:item_frame"),
            self.entity_finder("minecraft:glow_item_frame"),
            self.entity_finder("minecraft:painting"),
            self.entity_finder("minecraft:leash_knot"),
        ];
        TypeRewriteRule::seq(vec![
            self.fix_type_everywhere_typed("InlineBlockPosFormatFix - player", self.get_input_schema().get_type(PLAYER), |player| player.update_remainder(fix_player)),
            self.fix_type_everywhere_typed("InlineBlockPosFormatFix - entity", self.get_input_schema().get_type(ENTITY), move |entity| {
                let mut entity = entity
                    .update_remainder(fix_living_entity)
                    .update_typed(&vex_finder, None, |vex| vex.update_remainder(fix_vex))
                    .update_typed(&phantom_finder, None, |phantom| phantom.update_remainder(fix_phantom))
                    .update_typed(&turtle_finder, None, |turtle| turtle.update_remainder(fix_turtle));
                for block_attached_finder in &block_attached_finders {
                    entity = entity.update_typed(block_attached_finder, None, |block_attached| block_attached.update_remainder(fix_block_attached));
                }
                entity
            }),
        ])
    }
}

fn fix_player(tag: Dynamic) -> Dynamic {
    let mut tag = fix_living_entity(tag);
    let spawn_x = tag.get("SpawnX").as_number().into_result().ok();
    let spawn_y = tag.get("SpawnY").as_number().into_result().ok();
    let spawn_z = tag.get("SpawnZ").as_number().into_result().ok();
    if let (Some(spawn_x), Some(spawn_y), Some(spawn_z)) = (spawn_x, spawn_y, spawn_z) {
        let mut respawn = tag.create_map([(tag.create_string("pos"), extra_data_fix_utils::create_block_pos(&tag, spawn_x.int_value(), spawn_y.int_value(), spawn_z.int_value()))]);
        respawn = Dynamic::copy_field(&tag, "SpawnAngle", respawn, "angle");
        respawn = Dynamic::copy_field(&tag, "SpawnDimension", respawn, "dimension");
        respawn = Dynamic::copy_field(&tag, "SpawnForced", respawn, "forced");
        tag = tag.remove("SpawnX").remove("SpawnY").remove("SpawnZ").remove("SpawnAngle").remove("SpawnDimension").remove("SpawnForced");
        tag = tag.set("respawn", respawn);
    }
    if let Some(entered_nether_pos) = tag.get("enteredNetherPosition").result() {
        let entered_nether_pos = tag.create_list([
            tag.create_double(entered_nether_pos.get("x").as_double(0.0)),
            tag.create_double(entered_nether_pos.get("y").as_double(0.0)),
            tag.create_double(entered_nether_pos.get("z").as_double(0.0)),
        ]);
        tag = tag.remove("enteredNetherPosition").set("entered_nether_pos", entered_nether_pos);
    }
    tag
}

fn fix_living_entity(tag: Dynamic) -> Dynamic {
    extra_data_fix_utils::fix_inline_block_pos(tag, "SleepingX", "SleepingY", "SleepingZ", "sleeping_pos")
}

fn fix_vex(tag: Dynamic) -> Dynamic {
    extra_data_fix_utils::fix_inline_block_pos(tag.rename_field("LifeTicks", "life_ticks"), "BoundX", "BoundY", "BoundZ", "bound_pos")
}

fn fix_phantom(tag: Dynamic) -> Dynamic {
    extra_data_fix_utils::fix_inline_block_pos(tag.rename_field("Size", "size"), "AX", "AY", "AZ", "anchor_pos")
}

fn fix_turtle(tag: Dynamic) -> Dynamic {
    let tag = tag.remove("TravelPosX").remove("TravelPosY").remove("TravelPosZ");
    let tag = extra_data_fix_utils::fix_inline_block_pos(tag, "HomePosX", "HomePosY", "HomePosZ", "home_pos");
    tag.rename_field("HasEgg", "has_egg")
}

fn fix_block_attached(tag: Dynamic) -> Dynamic {
    extra_data_fix_utils::fix_inline_block_pos(tag, "TileX", "TileY", "TileZ", "block_pos")
}
