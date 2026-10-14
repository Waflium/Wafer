use std::collections::HashMap;

use wafer_core::ChunkPos;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::mob_category::MobCategory;

#[derive(Default)]
pub struct LocalMobCapCalculator {
    players_near_chunk: HashMap<i64, Vec<EntityId>>,
    player_mob_counts: HashMap<EntityId, MobCounts>,
}

impl LocalMobCapCalculator {
    fn get_players_near<'a>(players_near_chunk: &'a mut HashMap<i64, Vec<EntityId>>, level: &mut ServerLevel, pos: ChunkPos) -> &'a [EntityId] {
        players_near_chunk.entry(pos.pack()).or_insert_with(|| level.get_players_close_for_spawning(pos))
    }

    pub fn add_mob(&mut self, level: &mut ServerLevel, pos: ChunkPos, category: MobCategory) {
        for &player in Self::get_players_near(&mut self.players_near_chunk, level, pos) {
            self.player_mob_counts.entry(player).or_default().add(category);
        }
    }

    pub fn can_spawn(&mut self, level: &mut ServerLevel, mob_category: MobCategory, pos: ChunkPos) -> bool {
        let players = Self::get_players_near(&mut self.players_near_chunk, level, pos);
        players.iter().any(|player| self.player_mob_counts.get(player).is_none_or(|mob_counts| mob_counts.can_spawn(mob_category)))
    }
}

#[derive(Default)]
struct MobCounts {
    counts: [i32; 8],
}

impl MobCounts {
    fn add(&mut self, category: MobCategory) {
        self.counts[category as usize] += 1;
    }

    fn can_spawn(&self, category: MobCategory) -> bool {
        self.counts[category as usize] < category.get_max_instances_per_chunk()
    }
}
