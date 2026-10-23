use wafer_protocol::nbt::Nbt;

use crate::server::level::game_mode::GameType;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::types::{self, Pair};

pub fn codec() -> Pair {
    types::list(types::enumeration(&["survival", "creative", "adventure", "spectator"]))
}

#[derive(Debug)]
pub struct GameTypePredicate {
    types: Vec<GameType>,
}

impl GameTypePredicate {
    pub fn any() -> GameTypePredicate {
        GameTypePredicate { types: vec![GameType::Survival, GameType::Creative, GameType::Adventure, GameType::Spectator] }
    }

    pub fn decode(tag: &Nbt) -> DataResult<GameTypePredicate> {
        let types = codec::list_of(tag, |t| {
            codec::string_enum(t, &[
                ("survival", GameType::Survival),
                ("creative", GameType::Creative),
                ("adventure", GameType::Adventure),
                ("spectator", GameType::Spectator),
            ])
        })?;
        Ok(GameTypePredicate { types })
    }

    pub fn matches(&self, game_type: GameType) -> bool {
        self.types.contains(&game_type)
    }
}
