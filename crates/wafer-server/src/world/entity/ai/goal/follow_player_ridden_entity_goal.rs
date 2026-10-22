use wafer_core::Vec3;

use super::goal::reduced_tick_delay;
use crate::server::level::ServerLevel;
use crate::world::entity::{EntityId, MoverType};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EntityTypeToFollow {
    AbstractBoat,
    AbstractNautilus,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FollowEntityGoal {
    GoToEntity,
    GoInEntityDirection,
}

pub struct FollowPlayerRiddenEntityGoal {
    time_to_recalc_path: i32,
    entity_type_to_follow: EntityTypeToFollow,
    following: Option<EntityId>,
    current_goal: FollowEntityGoal,
}

impl FollowPlayerRiddenEntityGoal {
    pub fn new(entity_type_to_follow: EntityTypeToFollow) -> FollowPlayerRiddenEntityGoal {
        FollowPlayerRiddenEntityGoal { time_to_recalc_path: 0, entity_type_to_follow, following: None, current_goal: FollowEntityGoal::GoToEntity }
    }

    fn entities_to_follow(&self, level: &ServerLevel, mob: EntityId) -> Vec<EntityId> {
        let area = level.entity(mob).bounding_box().inflate_all(5.0);
        level.get_entities_matching(None, &area, |level, entity| match self.entity_type_to_follow {
            EntityTypeToFollow::AbstractBoat => level.is_boat(entity),
            EntityTypeToFollow::AbstractNautilus => level.is_abstract_nautilus(entity),
        })
    }

    pub fn can_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        if let Some(following) = self.following
            && level.entity(following).has_moved_horizontally_recently()
        {
            return true;
        }
        self.entities_to_follow(level, mob)
            .into_iter()
            .any(|entity| level.controlling_passenger(entity).is_some_and(|controlling_player| level.is_player(controlling_player) && level.entity(controlling_player).has_moved_horizontally_recently()))
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel) -> bool {
        self.following.is_some_and(|following| level.is_passenger(following) && level.entity(following).has_moved_horizontally_recently())
    }

    pub fn start(&mut self, level: &ServerLevel, mob: EntityId) {
        for entity in self.entities_to_follow(level, mob) {
            if let Some(player) = level.controlling_passenger(entity).filter(|&passenger| level.is_player(passenger)) {
                self.following = Some(player);
                break;
            }
        }
        self.time_to_recalc_path = 0;
        self.current_goal = FollowEntityGoal::GoToEntity;
    }

    pub fn stop(&mut self) {
        self.following = None;
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let speed = if self.current_goal == FollowEntityGoal::GoInEntityDirection { 0.01 } else { 0.015 };
        let living = level.living(mob);
        let input = Vec3::new(living.xxa as f64, living.yya as f64, living.zza as f64);
        level.entity_mut(mob).move_relative(speed, input);
        let movement = level.entity(mob).delta_movement;
        level.move_entity(mob, MoverType::SelfMove, movement);
        self.time_to_recalc_path -= 1;
        if self.time_to_recalc_path <= 0 {
            self.time_to_recalc_path = reduced_tick_delay(10);
            let following = self.following.expect("a followed player");
            if self.current_goal == FollowEntityGoal::GoToEntity {
                let f = level.entity(following);
                let behind_entity_pos = f.block_position().relative(f.get_direction().opposite()).offset(0, -1, 0);
                level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, behind_entity_pos.x as f64, behind_entity_pos.y as f64, behind_entity_pos.z as f64, 1.0));
                if level.entity(mob).distance_to(level.entity(following)) < 4.0 {
                    self.time_to_recalc_path = 0;
                    self.current_goal = FollowEntityGoal::GoInEntityDirection;
                }
            } else if self.current_goal == FollowEntityGoal::GoInEntityDirection {
                let f = level.entity(following);
                let direction = f.get_motion_direction();
                let go_to = f.block_position().relative_n(direction, 10);
                level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, go_to.x as f64, (go_to.y - 1) as f64, go_to.z as f64, 1.0));
                if level.entity(mob).distance_to(level.entity(following)) > 12.0 {
                    self.time_to_recalc_path = 0;
                    self.current_goal = FollowEntityGoal::GoToEntity;
                }
            }
        }
    }
}
