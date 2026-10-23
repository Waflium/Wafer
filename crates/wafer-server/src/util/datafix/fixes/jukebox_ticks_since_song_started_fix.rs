use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

pub struct JukeboxTicksSinceSongStartedFix;

impl JukeboxTicksSinceSongStartedFix {
    pub fn new(output_schema: &Arc<Schema>) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, false, "JukeboxTicksSinceSongStartedFix", BLOCK_ENTITY, "minecraft:jukebox", fix)
    }
}

pub fn fix_tag(input: Dynamic) -> Dynamic {
    let ticks_since_song_started = input.get("TickCount").as_long(0).wrapping_sub(input.get("RecordStartTick").as_long(0));
    let result = input.remove("IsPlaying").remove("TickCount").remove("RecordStartTick");
    if ticks_since_song_started > 0 {
        let ticks = result.create_long(ticks_since_song_started);
        result.set("ticks_since_song_started", ticks)
    } else {
        result
    }
}

fn fix(entity: Typed) -> Typed {
    entity.update_remainder(fix_tag)
}
