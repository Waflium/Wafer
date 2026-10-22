use wafer_core::random::RandomSource;
use wafer_core::{Vec3, mth};

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::attributes::Attribute;
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};

const MAX_LINE_OF_SIGHT_TEST_RANGE: f64 = 50.0;

pub fn random_point_behind_target(level: &mut ServerLevel, enemy: EntityId, random_source: EntityId) -> Vec3 {
    let spread_degrees = 90;
    let y_head_rot = level.entity(enemy).y_head_rot;
    let random = &mut level.entity_mut(random_source).random;
    let view_angle = y_head_rot + 180.0 + random.next_gaussian() as f32 * spread_degrees as f32 / 2.0;
    let r = mth::lerp_f(random.next_float(), 4.0, 8.0);
    let direction = Vec3::direction_from_rotation(0.0, view_angle).scale(r as f64);
    level.entity(enemy).position().add_vec(direction)
}

pub fn has_line_of_sight(level: &mut ServerLevel, breeze: EntityId, target: Vec3) -> bool {
    let from = level.entity(breeze).position();
    if target.distance_to(from) > get_max_line_of_sight_test_range(level, breeze) {
        return false;
    }
    let ctx = ClipContext { from, to: target, block: ClipBlock::Collider, fluid: ClipFluid::None, is_player: false };
    level.clip(&ctx).miss
}

fn get_max_line_of_sight_test_range(level: &ServerLevel, breeze: EntityId) -> f64 {
    MAX_LINE_OF_SIGHT_TEST_RANGE.max(level.attribute_value(breeze, Attribute::FollowRange))
}
