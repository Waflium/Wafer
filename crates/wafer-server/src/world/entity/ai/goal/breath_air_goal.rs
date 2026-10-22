use wafer_core::{BlockPos, Direction, Vec3};
use wafer_data::blocks;

use crate::server::level::ServerLevel;
use crate::world::entity::{EntityId, MoverType};
use crate::world::level::BlockGetter;

#[derive(Default)]
pub struct BreathAirGoal;

impl BreathAirGoal {
    pub fn can_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        level.entity(mob).air_supply() < 140
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        self.can_use(level, mob)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.get_navigation(mob, |navigation, _| navigation.stop());
        self.find_air_position(level, mob);
    }

    fn find_air_position(&self, level: &mut ServerLevel, mob: EntityId) {
        let pos = level.entity(mob).block_position();
        let mut destination_pos = None;
        for candidate in neighbor_column(pos.x, pos.y, pos.z, pos.y + 8) {
            if gives_air(level, candidate) {
                destination_pos = Some(candidate);
                break;
            }
        }
        let e = level.entity(mob);
        let destination_pos = destination_pos.unwrap_or_else(|| BlockPos::containing(e.x(), e.y() + 8.0, e.z()));
        level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, destination_pos.x as f64, (destination_pos.y + 1) as f64, destination_pos.z as f64, 1.0));
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.find_air_position(level, mob);
        let living = level.living(mob);
        let input = Vec3::new(living.xxa as f64, living.yya as f64, living.zza as f64);
        level.entity_mut(mob).move_relative(0.02, input);
        let movement = level.entity(mob).delta_movement;
        level.move_entity(mob, MoverType::SelfMove, movement);
    }
}

fn gives_air(level: &mut ServerLevel, pos: BlockPos) -> bool {
    let state = level.block_state(pos);
    (level.fluid_state(pos).is_empty() || state.is_block(blocks::BUBBLE_COLUMN)) && state.is_pathfindable_land()
}

pub fn neighbor_column(start_x: i32, start_y: i32, start_z: i32, end_y: i32) -> impl Iterator<Item = BlockPos> {
    let y_direction = if end_y > start_y { 1 } else { -1 };
    let height = (end_y - start_y).abs() + 1;
    let steps = [(0, 0), (Direction::North.step_x(), Direction::North.step_z()), (Direction::East.step_x(), Direction::East.step_z()), (Direction::South.step_x(), Direction::South.step_z()), (Direction::West.step_x(), Direction::West.step_z())];
    (0..steps.len() as i32 * height).map(move |index| {
        let y = index % height;
        let (step_x, step_z) = steps[(index / height) as usize];
        BlockPos::new(start_x + step_x, start_y + y * y_direction, start_z + step_z)
    })
}
