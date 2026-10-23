use wafer_core::{BlockPos, Vec3};
use wafer_data::blocks;

use crate::commands::arguments::blocks::BlockInput;
use crate::server::level::ServerLevel;
use crate::world::entity::ai::village::poi::poi_manager::Occupancy;
use crate::world::entity::ai::village::poi::poi_types::PoiType;
use crate::world::level::block::entity::test_instance_block_entity::{self, Data, Status, aabb_of};
use crate::world::level::block::mirror::Mirror;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::templatesystem::structure_template;

pub fn get_rotation_for_rotation_steps(rotation_steps: i32) -> Option<Rotation> {
    match rotation_steps {
        0 => Some(Rotation::None),
        1 => Some(Rotation::Clockwise90),
        2 => Some(Rotation::Clockwise180),
        3 => Some(Rotation::Counterclockwise90),
        _ => None,
    }
}

pub fn create_new_empty_test(level: &mut ServerLevel, id: &str, structure_pos: BlockPos, size: BlockPos, rotation: Rotation) -> BlockPos {
    let structure_bounding_box = get_structure_bounding_box(test_instance_block_entity::get_structure_pos(structure_pos), size, rotation);
    clear_space_for_structure(&structure_bounding_box, level);
    level.set_block(structure_pos, blocks::TEST_INSTANCE_BLOCK.default_state(), 3);
    let data = Data { test: Some(id.to_string()), size, rotation, ignore_entities: false, status: Status::Cleared, error_message: None };
    level.test_instance_block_entity_set(structure_pos, data);
    structure_pos
}

pub fn clear_space_for_structure(structure_bounding_box: &BoundingBox, level: &mut ServerLevel) {
    let ground_height = structure_bounding_box.min.y - 1;
    for pos in BlockPos::between_closed(structure_bounding_box.min, structure_bounding_box.max) {
        clear_block(ground_height, pos, level);
    }
    level.chunks.block_ticks.clear_area(structure_bounding_box);
    level.clear_block_events(structure_bounding_box);
    let bounds = aabb_of(structure_bounding_box);
    for entity in level.get_entities_of_type(|_, _| true, &bounds, |level, entity| !level.is_player(entity)) {
        level.discard(entity);
    }
}

pub fn get_transformed_far_corner(structure_position: BlockPos, size: BlockPos, rotation: Rotation) -> BlockPos {
    let far_corner_before_transform = structure_position.offset(size.x, size.y, size.z).offset(-1, -1, -1);
    structure_template::transform(far_corner_before_transform, Mirror::None, rotation, structure_position)
}

pub fn get_structure_bounding_box(north_west_corner: BlockPos, size: BlockPos, rotation: Rotation) -> BoundingBox {
    let far_corner = get_transformed_far_corner(north_west_corner, size, rotation);
    let bounding_box = BoundingBox::from_corners(north_west_corner, far_corner);
    let current_north_west_corner_x = bounding_box.min.x.min(bounding_box.max.x);
    let current_north_west_corner_z = bounding_box.min.z.min(bounding_box.max.z);
    bounding_box.move_by(BlockPos::new(north_west_corner.x - current_north_west_corner_x, 0, north_west_corner.z - current_north_west_corner_z))
}

pub fn find_test_containing_pos(pos: BlockPos, search_radius: i32, level: &mut ServerLevel) -> Option<BlockPos> {
    find_test_blocks(pos, search_radius, level).into_iter().find(|&test_block_pos| does_structure_contain(test_block_pos, pos, level))
}

pub fn find_nearest_test(relative_to_pos: BlockPos, search_radius: i32, level: &mut ServerLevel) -> Option<BlockPos> {
    find_test_blocks(relative_to_pos, search_radius, level).into_iter().min_by_key(|pos| pos.dist_manhattan(relative_to_pos))
}

pub fn find_test_blocks(center_pos: BlockPos, search_radius: i32, level: &mut ServerLevel) -> Vec<BlockPos> {
    level.chunks.poi_manager.find_all(&|poi_type| poi_type == PoiType::TestInstance, |_| true, center_pos, search_radius, Occupancy::Any)
}

pub fn looked_at_test_pos(pos: BlockPos, eye_position: Vec3, look_angle: Vec3, level: &mut ServerLevel) -> Option<BlockPos> {
    let start = eye_position;
    let end = start.add_vec(look_angle.scale(250.0));
    let mut hits: Vec<BlockPos> = Vec::new();
    for block_pos in find_test_blocks(pos, 250, level) {
        if level.test_instance_block_entity(block_pos).is_some() && level.test_instance_block_entity_get_structure_bounds(block_pos).clip(start, end).is_some() {
            hits.push(block_pos);
        }
    }
    hits.sort_by_key(|hit| pos.dist_sqr(*hit) as i64);
    hits.first().copied()
}

fn clear_block(air_if_above_this_y: i32, pos: BlockPos, level: &mut ServerLevel) {
    let block_state = if pos.y < air_if_above_this_y { blocks::STONE.default_state() } else { blocks::AIR.default_state() };
    let block_input = BlockInput { state: block_state, properties: Vec::new(), tag: None };
    block_input.place(level, pos, 818);
    level.update_neighbors_at(pos, block_state.block());
}

fn does_structure_contain(test_instance_block_pos: BlockPos, pos: BlockPos, level: &mut ServerLevel) -> bool {
    level.test_instance_block_entity(test_instance_block_pos).is_some() && level.test_instance_block_entity_get_structure_bounding_box(test_instance_block_pos).is_inside(pos)
}
