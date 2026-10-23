use crate::world::item::component::types::{self, Pair, def, record, req};
use crate::world::item::component::codec::Codec;

pub fn direct_codec() -> Pair {
    record(vec![
        req("message_id", types::string()),
        req("scaling", types::enumeration(&["never", "when_caused_by_living_non_player", "always"])),
        req("exhaustion", types::float(Codec::float())),
        def("effects", types::enumeration(&["hurt", "thorns", "drowning", "burning", "poking", "freezing"]), types::s("hurt")),
        def("death_message_type", types::enumeration(&["default", "fall_variants", "intentional_game_design"]), types::s("default")),
    ])
}
