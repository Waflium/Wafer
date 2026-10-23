use wafer_core::{BlockPos, Vec3};

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};
use crate::world::phys::shapes::collision_context::CollisionContext;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PlayerDetector {
    NoCreativePlayers,
    #[default]
    IncludingCreativePlayers,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum EntitySelector {
    #[default]
    SelectFromLevel,
}

impl EntitySelector {
    pub fn get_players(self, level: &ServerLevel, selector: impl Fn(&ServerLevel, EntityId) -> bool) -> Vec<EntityId> {
        match self {
            EntitySelector::SelectFromLevel => level.players.keys().copied().filter(|&player| selector(level, player)).collect(),
        }
    }
}

impl PlayerDetector {
    pub fn detect(self, level: &mut ServerLevel, selector: EntitySelector, spawner_pos: BlockPos, required_player_range: f64, require_line_of_sight: bool) -> Vec<u128> {
        let players = selector.get_players(level, |level, p| {
            let player = level.player(p);
            player.entity.block_position().closer_than(spawner_pos, required_player_range)
                && (self == PlayerDetector::IncludingCreativePlayers || !player.game_mode.game_type.is_creative())
                && !level.is_spectator(p)
        });
        let mut uuids = Vec::with_capacity(players.len());
        for player in players {
            let eye_position = level.entity(player).eye_position();
            if !require_line_of_sight || in_line_of_sight(level, Vec3::at_center_of(spawner_pos), eye_position) {
                uuids.push(level.player(player).entity.uuid);
            }
        }
        uuids
    }
}

fn in_line_of_sight(level: &mut ServerLevel, origin: Vec3, dest: Vec3) -> bool {
    let hit_result = level.clip(&ClipContext { from: dest, to: origin, block: ClipBlock::Visual, fluid: ClipFluid::None, collision_context: CollisionContext::EMPTY });
    hit_result.hit.block_pos == BlockPos::containing_vec(origin) || hit_result.miss
}
