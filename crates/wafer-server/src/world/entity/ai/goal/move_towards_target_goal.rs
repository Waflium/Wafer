use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::util::default_random_pos;

pub struct MoveTowardsTargetGoal {
    target: Option<EntityId>,
    wanted_x: f64,
    wanted_y: f64,
    wanted_z: f64,
    speed_modifier: f64,
    within: f32,
}

impl MoveTowardsTargetGoal {
    pub fn new(speed_modifier: f64, within: f32) -> MoveTowardsTargetGoal {
        MoveTowardsTargetGoal { target: None, wanted_x: 0.0, wanted_y: 0.0, wanted_z: 0.0, speed_modifier, within }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.target = level.get_target(mob);
        let Some(target) = self.target else { return false };
        if level.entity(target).distance_to_sqr(level.entity(mob)) > (self.within * self.within) as f64 {
            return false;
        }
        let target_position = level.entity(target).position();
        let Some(pos) = default_random_pos::get_pos_towards(level, mob, 16, 7, target_position, std::f32::consts::FRAC_PI_2 as f64) else { return false };
        self.wanted_x = pos.x;
        self.wanted_y = pos.y;
        self.wanted_z = pos.z;
        true
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let Some(target) = self.target else { return false };
        !level.mob(mob).expect("a mob").ai.navigation.is_done()
            && level.is_alive(target)
            && level.entity(target).distance_to_sqr(level.entity(mob)) < (self.within * self.within) as f64
    }

    pub fn stop(&mut self) {
        self.target = None;
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let (x, y, z, speed_modifier) = (self.wanted_x, self.wanted_y, self.wanted_z, self.speed_modifier);
        level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, x, y, z, speed_modifier));
    }
}
