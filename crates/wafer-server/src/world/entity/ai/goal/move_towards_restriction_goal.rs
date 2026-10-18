use wafer_core::Vec3;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::util::default_random_pos;

pub struct MoveTowardsRestrictionGoal {
    wanted_x: f64,
    wanted_y: f64,
    wanted_z: f64,
    speed_modifier: f64,
}

impl MoveTowardsRestrictionGoal {
    pub fn new(speed_modifier: f64) -> MoveTowardsRestrictionGoal {
        MoveTowardsRestrictionGoal { wanted_x: 0.0, wanted_y: 0.0, wanted_z: 0.0, speed_modifier }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let m = level.mob(mob).expect("a mob");
        if m.is_within_home(m.entity.block_position()) {
            return false;
        }
        let home_position = Vec3::at_bottom_center_of(m.home_position);
        let Some(pos) = default_random_pos::get_pos_towards(level, mob, 16, 7, home_position, std::f32::consts::FRAC_PI_2 as f64) else { return false };
        self.wanted_x = pos.x;
        self.wanted_y = pos.y;
        self.wanted_z = pos.z;
        true
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        !level.mob(mob).expect("a mob").ai.navigation.is_done()
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let (x, y, z, speed_modifier) = (self.wanted_x, self.wanted_y, self.wanted_z, self.speed_modifier);
        level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, x, y, z, speed_modifier));
    }
}
