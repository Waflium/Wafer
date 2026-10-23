use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;

pub fn codec() -> crate::world::item::component::types::Pair {
    crate::world::item::component::types::string()
}

#[derive(Debug)]
pub struct TeamPredicate {
    team: String,
}

impl TeamPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<TeamPredicate> {
        Ok(TeamPredicate { team: codec::string(tag)?.to_string() })
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        level.team_of(entity).is_some_and(|team| team.name() == self.team)
    }
}
