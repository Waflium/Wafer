use wafer_core::Vec3;

use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::entity::ai::util::land_random_pos;

const MAX_XZ_DIST: i32 = 10;
const MAX_Y_DIST: i32 = 7;

pub fn stroll(speed_modifier: f32) -> OneShot {
    stroll_from_water(speed_modifier, true)
}

pub fn stroll_from_water(speed_modifier: f32, may_stroll_from_water: bool) -> OneShot {
    stroll_fly_or_swim(
        speed_modifier,
        |level, body| land_random_pos::get_pos(level, body, MAX_XZ_DIST, MAX_Y_DIST),
        move |level, body| may_stroll_from_water || !level.entity(body).is_in_water(),
    )
}

fn stroll_fly_or_swim(
    speed_modifier: f32,
    fetch_target_pos: impl Fn(&mut ServerLevel, EntityId) -> Option<Vec3> + 'static,
    can_run: impl Fn(&ServerLevel, EntityId) -> bool + 'static,
) -> OneShot {
    behavior_builder::create(|i| {
        let walk_target = i.absent(memory_module_type::WALK_TARGET);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            if !can_run(level, body) {
                return false;
            }
            let path_goal_pos = fetch_target_pos(level, body);
            walk_target.set_or_erase(level, body, path_goal_pos.map(|pos| WalkTarget::of_vec(pos, speed_modifier, 0)));
            true
        }
    })
}
