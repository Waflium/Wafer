use super::goal::reduced_tick_delay;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::pathfinder::path_type::PathType;

pub struct FollowOwnerGoal {
    owner: Option<EntityId>,
    speed_modifier: f64,
    time_to_recalc_path: i32,
    stop_distance: f32,
    start_distance: f32,
    old_water_cost: f32,
}

impl FollowOwnerGoal {
    pub fn new(speed_modifier: f64, start_distance: f32, stop_distance: f32) -> FollowOwnerGoal {
        FollowOwnerGoal { owner: None, speed_modifier, time_to_recalc_path: 0, stop_distance, start_distance, old_water_cost: 0.0 }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let Some(owner) = level.get_owner(mob) else { return false };
        if level.unable_to_move_to_owner(mob) {
            return false;
        }
        if level.entity(mob).distance_to_sqr(level.entity(owner)) < (self.start_distance * self.start_distance) as f64 {
            return false;
        }
        self.owner = Some(owner);
        true
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if level.mob(mob).expect("a tamable animal").ai.navigation.is_done() {
            return false;
        }
        if level.unable_to_move_to_owner(mob) {
            return false;
        }
        let Some(owner) = self.owner.and_then(|owner| level.entity_ref(owner)) else { return false };
        !(level.entity(mob).distance_to_sqr(owner) <= (self.stop_distance * self.stop_distance) as f64)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.time_to_recalc_path = 0;
        let tamable = level.mob(mob).expect("a tamable animal");
        self.old_water_cost = tamable.get_pathfinding_malus(PathType::Water);
        tamable.set_pathfinding_malus(PathType::Water, 0.0);
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.owner = None;
        level.get_navigation(mob, |navigation, _| navigation.stop());
        level.mob(mob).expect("a tamable animal").set_pathfinding_malus(PathType::Water, self.old_water_cost);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let Some(owner) = self.owner.filter(|&owner| level.entity_ref(owner).is_some()) else { return };
        let is_owner_far_away = level.should_try_teleport_to_owner(mob);
        if !is_owner_far_away {
            let max_head_x_rot = level.mob(mob).expect("a tamable animal").get_max_head_x_rot() as f32;
            level.set_look_at_entity(mob, owner, 10.0, max_head_x_rot);
        }
        self.time_to_recalc_path -= 1;
        if self.time_to_recalc_path <= 0 {
            self.time_to_recalc_path = reduced_tick_delay(10);
            if is_owner_far_away {
                level.try_to_teleport_to_owner(mob);
            } else {
                let speed_modifier = self.speed_modifier;
                level.get_navigation(mob, |navigation, level| navigation.move_to_entity(level, mob, owner, speed_modifier));
            }
        }
    }
}
