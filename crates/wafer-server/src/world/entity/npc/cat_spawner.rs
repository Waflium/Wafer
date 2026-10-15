use wafer_core::random::RandomSource;
use wafer_core::{AABB, BlockPos, Vec3};

use crate::server::level::ServerLevel;
use crate::world::entity::ai::village::poi::poi_manager::Occupancy;
use crate::world::entity::ai::village::poi::poi_types::PoiType;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::entity_types;
use crate::world::entity::spawn_placements;
use crate::world::entity::LevelEntity;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

const TICK_DELAY: i32 = 1200;

#[derive(Default)]
pub struct CatSpawner {
    next_tick: i32,
}

impl CatSpawner {
    pub fn tick(&mut self, level: &mut ServerLevel, _spawn_enemies: bool) {
        self.next_tick -= 1;
        if self.next_tick > 0 {
            return;
        }
        self.next_tick = TICK_DELAY;
        let Some(player) = level.get_random_player() else { return };
        let random = &mut level.random;
        let x = (8 + random.next_int_bounded(24)) * if random.next_boolean() { -1 } else { 1 };
        let z = (8 + random.next_int_bounded(24)) * if random.next_boolean() { -1 } else { 1 };
        let spawn_pos = level.entity(player).block_position().offset(x, 0, z);
        let delta = 10;
        if level.has_chunks_at(spawn_pos.x - delta, spawn_pos.z - delta, spawn_pos.x + delta, spawn_pos.z + delta)
            && spawn_placements::is_spawn_position_ok(&entity_types::CAT, level, spawn_pos)
        {
            if level.is_close_to_village(spawn_pos, 2) {
                self.spawn_in_village(level, spawn_pos);
            } else if level.get_structure_with_piece_at(spawn_pos, &cats_spawn_in).is_some() {
                self.spawn_in_hut(level, spawn_pos);
            }
        }
    }

    fn spawn_in_village(&mut self, level: &mut ServerLevel, spawn_pos: BlockPos) {
        let radius = 48;
        if level.chunks.poi_manager.get_count_in_range(&|poi_type| poi_type == PoiType::Home, spawn_pos, radius, Occupancy::IsOccupied) > 4 {
            let cats = count_cats(level, AABB::of_block(spawn_pos).inflate(48.0, 8.0, 48.0));
            if cats < 5 {
                self.spawn_cat(spawn_pos, level, false);
            }
        }
    }

    fn spawn_in_hut(&mut self, level: &mut ServerLevel, spawn_pos: BlockPos) {
        if count_cats(level, AABB::of_block(spawn_pos).inflate(16.0, 8.0, 16.0)) == 0 {
            self.spawn_cat(spawn_pos, level, true);
        }
    }

    fn spawn_cat(&mut self, spawn_pos: BlockPos, level: &mut ServerLevel, make_persistent: bool) {
        let Some(cat) = ServerLevel::create_entity("minecraft:cat") else { return };
        let difficulty = level.current_difficulty_at(spawn_pos);
        let (cat, _) = level.finalize_spawn_detached(cat, &difficulty, EntitySpawnReason::Natural, None);
        let LevelEntity::Mob(mut cat) = cat else { return };
        if make_persistent {
            cat.persistence_required = true;
        }
        let pos = Vec3::at_bottom_center_of(spawn_pos);
        cat.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
        level.add_fresh_entity_with_passengers(LevelEntity::Mob(cat));
    }
}

fn cats_spawn_in(structure: crate::world::level::levelgen::structure::structure::StructureHolder) -> bool {
    wafer_data::tag("minecraft:worldgen/structure", "minecraft:cats_spawn_in").contains(&(structure.0 as i32))
}

fn count_cats(level: &ServerLevel, area: AABB) -> usize {
    level.get_entities(None, &area).into_iter().filter(|&entity| level.is_cat(entity)).count()
}
