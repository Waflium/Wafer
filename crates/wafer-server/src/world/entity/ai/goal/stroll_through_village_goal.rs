use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3};

use super::goal::reduced_tick_delay;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::util::land_random_pos;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

const DISTANCE_THRESHOLD: f64 = 10.0;

pub struct StrollThroughVillageGoal {
    interval: i32,
    wanted_pos: Option<BlockPos>,
}

impl StrollThroughVillageGoal {
    pub fn new(interval: i32) -> StrollThroughVillageGoal {
        StrollThroughVillageGoal { interval: reduced_tick_delay(interval), wanted_pos: None }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if level.has_controlling_passenger(mob) {
            return false;
        }
        if level.is_bright_outside() {
            return false;
        }
        if level.entity_mut(mob).random.next_int_bounded(self.interval) != 0 {
            return false;
        }
        let pos = level.entity(mob).block_position();
        if !level.is_close_to_village(pos, 6) {
            return false;
        }
        let land_pos = land_random_pos::get_pos_weighted(level, mob, 15, 7, |level, p| -level.sections_to_village(BlockPos::new(p.x >> 4, p.y >> 4, p.z >> 4)) as f64);
        self.wanted_pos = land_pos.map(|land_pos| BlockPos::containing(land_pos.x, land_pos.y, land_pos.z));
        self.wanted_pos.is_some()
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        let navigation = &level.mob(mob).expect("a mob").ai.navigation;
        self.wanted_pos.is_some() && !navigation.is_done() && navigation.get_target_pos() == self.wanted_pos
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let Some(wanted_pos) = self.wanted_pos else { return };
        let position = level.entity(mob).position();
        if level.mob(mob).expect("a mob").ai.navigation.is_done() && !wanted_pos.closer_to_center_than(position, DISTANCE_THRESHOLD) {
            let long_distance_target = Vec3::at_bottom_center_of(wanted_pos);
            let self_vector = position;
            let distance = self_vector.subtract(long_distance_target);
            let long_distance_target = distance.scale(0.4).add_vec(long_distance_target);
            let move_target = long_distance_target.subtract(self_vector).normalize().scale(10.0).add_vec(self_vector);
            let path_target = BlockPos::containing(move_target.x, move_target.y, move_target.z);
            let path_target = level.get_heightmap_pos(HeightmapType::MotionBlockingNoLeaves, path_target);
            let moved = level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, path_target.x as f64, path_target.y as f64, path_target.z as f64, 1.0));
            if !moved {
                self.move_randomly(level, mob);
            }
        }
    }

    fn move_randomly(&self, level: &mut ServerLevel, mob: EntityId) {
        let e = level.entity_mut(mob);
        let block_position = e.block_position();
        let x = -8 + e.random.next_int_bounded(16);
        let z = -8 + e.random.next_int_bounded(16);
        let path_target = level.get_heightmap_pos(HeightmapType::MotionBlockingNoLeaves, block_position.offset(x, 0, z));
        level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, path_target.x as f64, path_target.y as f64, path_target.z as f64, 1.0));
    }
}
