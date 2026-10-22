use wafer_core::{AABB, Direction};

pub fn get_movement_area(aabb: &AABB, direction: Direction, amount: f64) -> AABB {
    let delta = amount * direction.axis_direction().step() as f64;
    let min = delta.min(0.0);
    let max = delta.max(0.0);
    match direction {
        Direction::West => AABB::new(aabb.min_x + min, aabb.min_y, aabb.min_z, aabb.min_x + max, aabb.max_y, aabb.max_z),
        Direction::East => AABB::new(aabb.max_x + min, aabb.min_y, aabb.min_z, aabb.max_x + max, aabb.max_y, aabb.max_z),
        Direction::Down => AABB::new(aabb.min_x, aabb.min_y + min, aabb.min_z, aabb.max_x, aabb.min_y + max, aabb.max_z),
        Direction::Up => AABB::new(aabb.min_x, aabb.max_y + min, aabb.min_z, aabb.max_x, aabb.max_y + max, aabb.max_z),
        Direction::North => AABB::new(aabb.min_x, aabb.min_y, aabb.min_z + min, aabb.max_x, aabb.max_y, aabb.min_z + max),
        Direction::South => AABB::new(aabb.min_x, aabb.min_y, aabb.max_z + min, aabb.max_x, aabb.max_y, aabb.max_z + max),
    }
}
