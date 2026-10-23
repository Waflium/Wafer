use wafer_core::{BlockPos, Vec3};

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::util::{default_random_pos, land_random_pos};
use crate::world::entity::ai::village::poi::poi_manager::Occupancy;
use crate::world::entity::ai::village::poi::poi_types::PoiType;
use crate::world::entity::mob::Mob;
use crate::world::level::block::door_block;
use crate::world::level::pathfinder::path::Path;

pub struct MoveThroughVillageGoal {
    speed_modifier: f64,
    path: Option<Path>,
    poi_pos: BlockPos,
    only_at_night: bool,
    visited: Vec<BlockPos>,
    distance_to_poi: i32,
    can_deal_with_doors: fn(&Mob) -> bool,
}

impl MoveThroughVillageGoal {
    pub fn new(speed_modifier: f64, only_at_night: bool, distance_to_poi: i32, can_deal_with_doors: fn(&Mob) -> bool) -> MoveThroughVillageGoal {
        MoveThroughVillageGoal { speed_modifier, path: None, poi_pos: BlockPos::ZERO, only_at_night, visited: Vec::new(), distance_to_poi, can_deal_with_doors }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.update_visited();
        if self.only_at_night && level.is_bright_outside() {
            return false;
        }
        let pos = level.entity(mob).block_position();
        if !level.is_close_to_village(pos, 6) {
            return false;
        }
        let visited = self.visited.clone();
        let land_pos = land_random_pos::get_pos_weighted(level, mob, 15, 7, |level, p| {
            if !level.is_village(p) {
                return f64::NEG_INFINITY;
            }
            let new_poi_pos = level.chunks.poi_manager.find(&PoiType::is_village, |poi| !visited.contains(&poi), p, 10, Occupancy::IsOccupied);
            new_poi_pos.map_or(f64::NEG_INFINITY, |poi_pos| -poi_pos.dist_sqr(pos))
        });
        let Some(land_pos) = land_pos else { return false };
        let target = level.chunks.poi_manager.find(&PoiType::is_village, |poi| !visited.contains(&poi), BlockPos::containing(land_pos.x, land_pos.y, land_pos.z), 10, Occupancy::IsOccupied);
        let Some(target) = target else { return false };
        self.poi_pos = target;
        let can_deal_with_doors = (self.can_deal_with_doors)(level.mob(mob).expect("a mob"));
        let poi_pos = self.poi_pos;
        self.path = level.get_navigation(mob, |navigation, level| {
            navigation.set_can_open_doors(can_deal_with_doors);
            let path = navigation.create_path(level, mob, poi_pos, 0);
            navigation.set_can_open_doors(true);
            path
        });
        if self.path.is_none() {
            let partial_step = default_random_pos::get_pos_towards(level, mob, 10, 7, Vec3::at_bottom_center_of(self.poi_pos), std::f32::consts::FRAC_PI_2 as f64);
            let Some(partial_step) = partial_step else { return false };
            self.path = level.get_navigation(mob, |navigation, level| {
                navigation.set_can_open_doors(can_deal_with_doors);
                let path = navigation.create_path_to(level, mob, partial_step.x, partial_step.y, partial_step.z, 0);
                navigation.set_can_open_doors(true);
                path
            });
            if self.path.is_none() {
                return false;
            }
        }
        let node_count = self.path.as_ref().map_or(0, Path::get_node_count);
        for i in 0..node_count {
            let node = self.path.as_ref().expect("a path").get_node(i);
            let (x, y, z) = (node.x, node.y, node.z);
            if door_block::is_wooden_door(level, BlockPos::new(x, y + 1, z)) {
                self.path = level.get_navigation(mob, |navigation, level| navigation.create_path_to(level, mob, x as f64, y as f64, z as f64, 0));
                break;
            }
        }
        self.path.is_some()
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        if level.mob(mob).expect("a mob").ai.navigation.is_done() {
            return false;
        }
        let e = level.entity(mob);
        !self.poi_pos.closer_to_center_than(e.position(), e.dimensions.width as f64 + self.distance_to_poi as f64)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let (path, speed_modifier) = (self.path.clone(), self.speed_modifier);
        level.get_navigation(mob, |navigation, level| navigation.move_to_path(level, mob, path, speed_modifier));
    }

    pub fn stop(&mut self, level: &ServerLevel, mob: EntityId) {
        if level.mob(mob).expect("a mob").ai.navigation.is_done() || self.poi_pos.closer_to_center_than(level.entity(mob).position(), self.distance_to_poi as f64) {
            self.visited.push(self.poi_pos);
        }
    }

    fn update_visited(&mut self) {
        if self.visited.len() > 15 {
            self.visited.remove(0);
        }
    }
}
