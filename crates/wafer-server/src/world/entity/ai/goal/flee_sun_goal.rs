use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3};
use wafer_data::EquipmentSlot;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub struct FleeSunGoal {
    wanted_x: f64,
    wanted_y: f64,
    wanted_z: f64,
    speed_modifier: f64,
}

impl FleeSunGoal {
    pub fn new(speed_modifier: f64) -> FleeSunGoal {
        FleeSunGoal { wanted_x: 0.0, wanted_y: 0.0, wanted_z: 0.0, speed_modifier }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if level.get_target(mob).is_some() {
            false
        } else if !level.is_bright_outside() {
            false
        } else if !level.is_on_fire(mob) {
            false
        } else if !level.can_see_sky(level.entity(mob).block_position()) {
            false
        } else if level.item_by_slot_ref(mob, EquipmentSlot::Head).is_some() {
            false
        } else {
            self.set_wanted_pos(level, mob)
        }
    }

    pub fn set_wanted_pos(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let Some(pos) = self.get_hide_pos(level, mob) else { return false };
        self.wanted_x = pos.x;
        self.wanted_y = pos.y;
        self.wanted_z = pos.z;
        true
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        !level.mob(mob).expect("a mob").ai.navigation.is_done()
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let (x, y, z, speed_modifier) = (self.wanted_x, self.wanted_y, self.wanted_z, self.speed_modifier);
        level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, x, y, z, speed_modifier));
    }

    fn get_hide_pos(&self, level: &mut ServerLevel, mob: EntityId) -> Option<Vec3> {
        let pos = level.entity(mob).block_position();
        for _ in 0..10 {
            let random = &mut level.entity_mut(mob).random;
            let x = random.next_int_bounded(20) - 10;
            let y = random.next_int_bounded(6) - 3;
            let z = random.next_int_bounded(20) - 10;
            let random_pos: BlockPos = pos.offset(x, y, z);
            if !level.can_see_sky(random_pos) && level.get_walk_target_value(mob, random_pos) < 0.0 {
                return Some(Vec3::at_bottom_center_of(random_pos));
            }
        }
        None
    }
}
