use wafer_core::BlockPos;

use super::path_navigation::PathNavigation;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::pathfinder::path::Path;

#[derive(Default)]
pub struct WallClimberNavigation {
    path_to_position: Option<BlockPos>,
}

impl PathNavigation {
    pub fn wall_climber(follow_range_base: f64) -> PathNavigation {
        let mut navigation = PathNavigation::new(follow_range_base);
        navigation.wall_climber_navigation = Some(WallClimberNavigation::default());
        navigation
    }

    fn wall_climber_navigation_mut(&mut self) -> &mut WallClimberNavigation {
        self.wall_climber_navigation.as_mut().expect("a wall climber navigation")
    }

    pub fn wall_climber_navigation_create_path(&mut self, level: &ServerLevel, mob: EntityId, pos: BlockPos, reach_range: i32) -> Option<Path> {
        self.wall_climber_navigation_mut().path_to_position = Some(pos);
        self.ground_path_navigation_create_path(level, mob, pos, reach_range)
    }

    pub fn wall_climber_navigation_create_path_to_entity(&mut self, level: &ServerLevel, mob: EntityId, target: EntityId, reach_range: i32) -> Option<Path> {
        self.wall_climber_navigation_mut().path_to_position = Some(level.entity(target).block_position());
        self.ground_path_navigation_create_path_to_entity(level, mob, target, reach_range)
    }

    pub fn wall_climber_navigation_move_to_entity(&mut self, level: &mut ServerLevel, mob: EntityId, target: EntityId, speed_modifier: f64) -> bool {
        let new_path = self.create_path_to_entity(level, mob, target, 0);
        if new_path.is_some() {
            return self.move_to_path(level, mob, new_path, speed_modifier);
        }
        self.wall_climber_navigation_mut().path_to_position = Some(level.entity(target).block_position());
        self.speed_modifier = speed_modifier;
        true
    }

    pub fn wall_climber_navigation_tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if !self.is_done() {
            self.path_navigation_tick(level, mob);
            return;
        }
        let Some(path_to_position) = self.wall_climber_navigation_mut().path_to_position else { return };
        let e = level.entity(mob);
        let (position, width) = (e.position(), e.dimensions.width as f64);
        if !path_to_position.closer_to_center_than(position, width)
            && (!(e.y() > path_to_position.y as f64)
                || !BlockPos::containing(path_to_position.x as f64, e.y(), path_to_position.z as f64).closer_to_center_than(position, width))
        {
            let speed_modifier = self.speed_modifier;
            let move_control = &mut level.mob_mut(mob).expect("a mob").ai.move_control;
            move_control.set_wanted_position(path_to_position.x as f64, path_to_position.y as f64, path_to_position.z as f64, speed_modifier);
        } else {
            self.wall_climber_navigation_mut().path_to_position = None;
        }
    }
}
