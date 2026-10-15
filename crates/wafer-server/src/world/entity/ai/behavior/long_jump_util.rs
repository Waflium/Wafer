use wafer_core::{Vec3, mth};

use crate::server::level::ServerLevel;
use crate::world::entity::data::Pose;
use crate::world::entity::{EntityDimensions, EntityId};

pub fn calculate_jump_vector_for_angle(level: &ServerLevel, body: EntityId, target_pos: Vec3, max_jump_velocity: f32, angle: i32, check_collision: bool) -> Option<Vec3> {
    let mob_pos = level.entity(body).position();
    let direction_vector_plane = Vec3::new(target_pos.x - mob_pos.x, 0.0, target_pos.z - mob_pos.z).normalize().scale(0.5);
    let target_position = target_pos.subtract(direction_vector_plane);
    let direction_vector = target_position.subtract(mob_pos);
    let angrad = angle as f32 * std::f32::consts::PI / 180.0;
    let xz_ang = direction_vector.z.atan2(direction_vector.x);
    let r2 = direction_vector.subtract(Vec3::new(0.0, direction_vector.y, 0.0)).length_sqr();
    let r = r2.sqrt();
    let y = direction_vector.y;
    let g = level.gravity(body);
    let sin2ang = ((2.0 * angrad) as f64).sin();
    let cos_angrad = (angrad as f64).cos();
    let cosangsqr = cos_angrad * cos_angrad;
    let sinangrad = (angrad as f64).sin();
    let cosangrad = (angrad as f64).cos();
    let sinxz_ang = xz_ang.sin();
    let cosxz_ang = xz_ang.cos();
    let v0sqr = r2 * g / (r * sin2ang - 2.0 * y * cosangsqr);
    if v0sqr < 0.0 {
        return None;
    }
    let v0 = v0sqr.sqrt();
    if v0 > max_jump_velocity as f64 {
        return None;
    }
    let v0r = v0 * cosangrad;
    let v0y = v0 * sinangrad;
    if check_collision {
        let samples = mth::ceil(r / v0r) * 2;
        let mut ri = 0.0;
        let mut previous_pos: Option<Vec3> = None;
        let mob_dimensions = level.mob(body).expect("a mob").get_dimensions(Pose::LongJumping);
        for _ in 0..samples - 1 {
            ri += r / samples as f64;
            let yi = sinangrad / cosangrad * ri - ri * ri * g / (2.0 * v0sqr * (cosangrad * cosangrad));
            let xi = ri * cosxz_ang;
            let zi = ri * sinxz_ang;
            let sample_pos = Vec3::new(mob_pos.x + xi, mob_pos.y + yi, mob_pos.z + zi);
            if let Some(previous_pos) = previous_pos
                && !is_clear_transition(level, body, mob_dimensions, previous_pos, sample_pos)
            {
                return None;
            }
            previous_pos = Some(sample_pos);
        }
    }
    Some(Vec3::new(v0r * cosxz_ang, v0y, v0r * sinxz_ang).scale(0.95f32 as f64))
}

fn is_clear_transition(level: &ServerLevel, body: EntityId, entity_dimensions: EntityDimensions, position1: Vec3, position2: Vec3) -> bool {
    let direction = position2.subtract(position1);
    let min_dimension = entity_dimensions.width.min(entity_dimensions.height) as f64;
    let checks = mth::ceil(direction.length() / min_dimension);
    let normalized_direction = direction.normalize();
    let mut next_point_to_check = position1;
    for i in 0..checks {
        next_point_to_check = if i == checks - 1 { position2 } else { next_point_to_check.add_vec(normalized_direction.scale(min_dimension * 0.9f32 as f64)) };
        if !level.no_collision_for(body, &entity_dimensions.make_bounding_box(next_point_to_check)) {
            return false;
        }
    }
    true
}
