use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::item_stack_tag_fix::ItemStackTagFix;

const INSTRUMENTS: [&str; 8] = [
    "minecraft:ponder_goat_horn",
    "minecraft:sing_goat_horn",
    "minecraft:seek_goat_horn",
    "minecraft:feel_goat_horn",
    "minecraft:admire_goat_horn",
    "minecraft:call_goat_horn",
    "minecraft:yearn_goat_horn",
    "minecraft:dream_goat_horn",
];

pub struct GoatHornIdFix;

impl GoatHornIdFix {
    pub fn new(output_schema: &Arc<Schema>) -> ItemStackTagFix {
        ItemStackTagFix::remainder(output_schema, "GoatHornIdFix", |id| id == "minecraft:goat_horn", fix_item_stack_tag)
    }
}

fn fix_item_stack_tag(tag: Dynamic) -> Dynamic {
    let sound_variant = tag.get("SoundVariant").as_int(0);
    let sound_id = INSTRUMENTS[if sound_variant >= 0 && (sound_variant as usize) < INSTRUMENTS.len() { sound_variant as usize } else { 0 }];
    let instrument = tag.create_string(sound_id);
    tag.remove("SoundVariant").set("instrument", instrument)
}
