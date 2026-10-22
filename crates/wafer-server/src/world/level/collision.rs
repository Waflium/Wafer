use std::cmp::Ordering;

use wafer_core::shapes::{BooleanOp, VoxelShape, join_is_not_empty};
use wafer_core::{AABB, Axis, BlockPos, ChunkPos, Vec3, mth};
use wafer_data::blocks;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::BlockGetter;
use crate::world::phys::shapes::collision_context::CollisionContext;

#[derive(Clone, Copy, Debug)]
pub struct Collider<'a> {
    pub shape: &'a VoxelShape,
    pub shape_offset: [f64; 3],
    pub offset: [f64; 3],
}

impl<'a> Collider<'a> {
    pub fn new(shape: &'a VoxelShape, offset: [f64; 3]) -> Collider<'a> {
        Collider { shape, shape_offset: [0.0; 3], offset }
    }

    pub fn collide(&self, axis: Axis, moving: &AABB, distance: f64) -> f64 {
        self.shape.collide(self.shape_offset, self.offset, axis, moving, distance)
    }

    pub fn coords(&self, axis: Axis) -> impl Iterator<Item = f64> + '_ {
        let (shape_offset, offset) = (self.shape_offset[axis as usize], self.offset[axis as usize]);
        self.shape.coords(axis).iter().map(move |c| c + shape_offset + offset)
    }

    pub fn moved(&self) -> VoxelShape {
        let [sx, sy, sz] = self.shape_offset;
        let [x, y, z] = self.offset;
        if self.shape_offset == [0.0; 3] { self.shape.moved(x, y, z) } else { self.shape.moved(sx, sy, sz).moved(x, y, z) }
    }

    pub fn to_aabbs(&self) -> Vec<AABB> {
        let [sx, sy, sz] = self.shape_offset;
        let [x, y, z] = self.offset;
        self.shape.to_aabbs().iter().map(|b| b.move_by(sx, sy, sz).move_by(x, y, z)).collect()
    }
}

static FULL_BLOCK: std::sync::LazyLock<VoxelShape> = std::sync::LazyLock::new(VoxelShape::block);

pub fn shapes_collide(axis: Axis, moving: &AABB, colliders: &[Collider], mut distance: f64) -> f64 {
    for collider in colliders {
        if distance.abs() < 1.0e-7 {
            return 0.0;
        }
        distance = collider.collide(axis, moving, distance);
    }
    distance
}

pub fn collide_with_shapes(movement: Vec3, bounding_box: &AABB, colliders: &[Collider]) -> Vec3 {
    if colliders.is_empty() {
        return movement;
    }
    let mut resolved = Vec3::ZERO;
    for axis in Axis::step_order(movement.x, movement.z) {
        let axis_movement = movement.get(axis);
        if axis_movement != 0.0 {
            let collision = shapes_collide(axis, &bounding_box.move_vec(resolved), colliders, axis_movement);
            resolved = resolved.with(axis, collision);
        }
    }
    resolved
}

pub fn compare_block_pos(a: BlockPos, b: BlockPos) -> Ordering {
    if a.y == b.y {
        if a.z == b.z { a.x.cmp(&b.x) } else { a.z.cmp(&b.z) }
    } else {
        a.y.cmp(&b.y)
    }
}

impl ServerLevel {
    pub fn block_collisions(&self, box_: &AABB, context: &CollisionContext, only_suffocating_blocks: bool) -> Vec<(BlockPos, Collider<'static>)> {
        let mut out = Vec::new();
        self.for_each_block_collision(box_, context, only_suffocating_blocks, false, |pos, collider| {
            out.push((pos, collider));
            true
        });
        out
    }

    pub fn for_each_block_collision(&self, box_: &AABB, context: &CollisionContext, only_suffocating_blocks: bool, always_collide_with_fluid: bool, visit: impl FnMut(BlockPos, Collider<'static>) -> bool) -> bool {
        for_each_block_collision(|pos| self.chunks.chunk_without_loading(pos), box_, context, only_suffocating_blocks, always_collide_with_fluid, visit)
    }
}

pub fn for_each_block_collision<'c>(
    chunk_for_collisions: impl Fn(ChunkPos) -> Option<&'c crate::world::level::chunk::LevelChunk>,
    box_: &AABB,
    context: &CollisionContext,
    only_suffocating_blocks: bool,
    always_collide_with_fluid: bool,
    mut visit: impl FnMut(BlockPos, Collider<'static>) -> bool,
) -> bool {
    let x0 = mth::floor(box_.min_x - 1.0e-7) - 1;
    let x1 = mth::floor(box_.max_x + 1.0e-7) + 1;
    let y0 = mth::floor(box_.min_y - 1.0e-7) - 1;
    let y1 = mth::floor(box_.max_y + 1.0e-7) + 1;
    let z0 = mth::floor(box_.min_z - 1.0e-7) - 1;
    let z1 = mth::floor(box_.max_z + 1.0e-7) + 1;
    let (width, height, depth) = (x1 - x0 + 1, y1 - y0 + 1, z1 - z0 + 1);
    let mut entity_shape: Option<VoxelShape> = None;
    let mut cached_chunk: Option<(ChunkPos, Option<&crate::world::level::chunk::LevelChunk>)> = None;
    for index in 0..width * height * depth {
        let (cx, slice) = (index % width, index / width);
        let (cy, cz) = (slice % height, slice / height);
        let mut face_type = 0;
        if cx == 0 || cx == width - 1 {
            face_type += 1;
        }
        if cy == 0 || cy == height - 1 {
            face_type += 1;
        }
        if cz == 0 || cz == depth - 1 {
            face_type += 1;
        }
        if face_type == 3 {
            continue;
        }
        let (x, y, z) = (x0 + cx, y0 + cy, z0 + cz);
        let chunk_pos = ChunkPos::new(x >> 4, z >> 4);
        let chunk = match cached_chunk {
            Some((p, c)) if p == chunk_pos => c,
            _ => {
                let c = chunk_for_collisions(chunk_pos);
                cached_chunk = Some((chunk_pos, c));
                c
            }
        };
        let Some(chunk) = chunk else { continue };
        let pos = BlockPos::new(x, y, z);
        let state = chunk.block_state(pos);
        if only_suffocating_blocks && !state.is_suffocating() {
            continue;
        }
        if face_type == 1 && !state.has_large_collision_shape() || face_type == 2 && !state.is_block(blocks::MOVING_PISTON) {
            continue;
        }
        if state.is_block(blocks::MOVING_PISTON) {
            let shapes = match chunk.block_entities.map.get(&pos).map(|entity| &entity.kind) {
                Some(crate::world::level::block::entity::BlockEntityKind::PistonMoving(piston)) => piston.collision_shapes(pos),
                _ => [None, None],
            };
            for (shape, shape_offset) in shapes.into_iter().flatten() {
                if shape.is_empty() {
                    continue;
                }
                let collider = Collider { shape, shape_offset, offset: [x as f64, y as f64, z as f64] };
                let moved = collider.moved();
                let entity_shape = entity_shape.get_or_insert_with(|| VoxelShape::from_boxes(&[*box_]));
                if join_is_not_empty(&moved, entity_shape, BooleanOp::And) && !visit(pos, collider) {
                    return false;
                }
            }
            continue;
        }
        let liquid_block = always_collide_with_fluid && (state.is_block(blocks::WATER) || state.is_block(blocks::LAVA));
        let (shape, shape_offset) = if liquid_block {
            (&*FULL_BLOCK, [0.0; 3])
        } else if let Some(shape) = context.get_liquid_block_collision_shape(state, pos, || chunk.block_state(pos.above()).fluid_state()) {
            (shape, [0.0; 3])
        } else {
            context.get_collision_shape(state, pos)
        };
        let collider = Collider { shape, shape_offset, offset: [x as f64, y as f64, z as f64] };
        let collides = if liquid_block || state.is_collision_shape_full_block() {
            box_.intersects_bounds(x as f64, y as f64, z as f64, x as f64 + 1.0, y as f64 + 1.0, z as f64 + 1.0)
        } else if !shape.is_empty() {
            let moved = collider.moved();
            let entity_shape = entity_shape.get_or_insert_with(|| VoxelShape::from_boxes(&[*box_]));
            join_is_not_empty(&moved, entity_shape, BooleanOp::And)
        } else {
            false
        };
        if collides && !visit(pos, collider) {
            return false;
        }
    }
    true
}

impl ServerLevel {
    pub fn no_collision(&self, box_: &AABB) -> bool {
        self.for_each_block_collision(box_, &CollisionContext::EMPTY, false, false, |_, c| c.shape.is_empty())
    }

    pub fn no_block_and_liquid_collision(&self, box_: &AABB) -> bool {
        self.for_each_block_collision(box_, &CollisionContext::EMPTY, false, true, |_, c| c.shape.is_empty())
    }

    pub fn no_collision_for(&self, entity: EntityId, box_: &AABB) -> bool {
        self.for_each_block_collision(box_, &CollisionContext::of(self, entity), false, false, |_, c| c.shape.is_empty())
            && self.entity_collisions(Some(entity), box_).is_empty()
            && self.no_border_collision(entity, box_)
    }

    pub fn no_collision_for_new(&self, entity: &crate::world::entity::LevelEntity, box_: &AABB) -> bool {
        let no_block_collision = self.for_each_block_collision(box_, &CollisionContext::EMPTY, false, false, |_, c| c.shape.is_empty());
        let no_entity_collision = box_.size() < 1.0e-7
            || self.get_entities(None, &box_.inflate_all(1.0e-7)).into_iter().all(|other| !(self.can_be_collided_with(other, None) || self.is_pushable_immutable(other)));
        let border = &self.world_border;
        let no_border_collision =
            !border.is_inside_close_to_border(entity.entity(), box_) || !join_is_not_empty(border.get_collision_shape(), &VoxelShape::from_boxes(&[*box_]), BooleanOp::And);
        no_block_collision && no_entity_collision && no_border_collision
    }

    pub fn no_border_collision(&self, entity: EntityId, box_: &AABB) -> bool {
        self.border_collision(entity, box_).is_none_or(|shape| !join_is_not_empty(shape, &VoxelShape::from_boxes(&[*box_]), BooleanOp::And))
    }

    pub fn no_border_collision_at(&self, x: f64, z: f64, box_: &AABB) -> bool {
        let border = &self.world_border;
        !border.is_inside_close_to_border_at(x, z, box_) || !join_is_not_empty(border.get_collision_shape(), &VoxelShape::from_boxes(&[*box_]), BooleanOp::And)
    }

    pub fn border_collision(&self, entity: EntityId, box_: &AABB) -> Option<&VoxelShape> {
        let border = &self.world_border;
        border.is_inside_close_to_border(self.entity(entity), box_).then(|| border.get_collision_shape())
    }

    pub fn find_supporting_block(&self, entity: EntityId, position: Vec3, box_: &AABB) -> Option<BlockPos> {
        let mut main_support: Option<BlockPos> = None;
        let mut main_support_distance = f64::MAX;
        for (pos, _) in self.block_collisions(box_, &CollisionContext::of(self, entity), false) {
            let distance = pos.dist_to_center_sqr(position.x, position.y, position.z);
            if distance < main_support_distance
                || distance == main_support_distance && main_support.is_none_or(|m| compare_block_pos(m, pos) == Ordering::Less)
            {
                main_support = Some(pos);
                main_support_distance = distance;
            }
        }
        main_support
    }

    pub fn find_free_position(&self, context: &CollisionContext, allowed: &AABB, preferred: Vec3, size_x: f64, size_y: f64, size_z: f64) -> Option<Vec3> {
        let search = allowed.inflate(size_x, size_y, size_z);
        let mut expanded = Vec::new();
        for (_, collider) in self.block_collisions(&search, context, false) {
            let boxes = collider.to_aabbs();
            let Some(bounds) = boxes.iter().copied().reduce(|a, b| a.minmax(&b)) else { continue };
            if self.world_border.is_within_bounds_aabb(&bounds) {
                expanded.extend(boxes.iter().map(|b| b.inflate(size_x / 2.0, size_y / 2.0, size_z / 2.0)));
            }
        }
        let coords = |min: f64, max: f64, edges: &dyn Fn(&AABB) -> [f64; 2]| {
            let mut c = vec![min, max];
            c.extend(expanded.iter().flat_map(edges).filter(|&v| v > min && v < max));
            c.sort_by(f64::total_cmp);
            let mut merged: Vec<f64> = Vec::with_capacity(c.len());
            for v in c {
                if merged.last().is_none_or(|&last| last < v - 1.0e-7) {
                    merged.push(v);
                }
            }
            merged
        };
        let xs = coords(allowed.min_x, allowed.max_x, &|b| [b.min_x, b.max_x]);
        let ys = coords(allowed.min_y, allowed.max_y, &|b| [b.min_y, b.max_y]);
        let zs = coords(allowed.min_z, allowed.max_z, &|b| [b.min_z, b.max_z]);
        let mut closest: Option<(Vec3, f64)> = None;
        for x in xs.windows(2) {
            for y in ys.windows(2) {
                for z in zs.windows(2) {
                    let (mx, my, mz) = ((x[0] + x[1]) / 2.0, (y[0] + y[1]) / 2.0, (z[0] + z[1]) / 2.0);
                    if expanded.iter().any(|b| b.contains(mx, my, mz)) {
                        continue;
                    }
                    let point = Vec3::new(mth::clamp(preferred.x, x[0], x[1]), mth::clamp(preferred.y, y[0], y[1]), mth::clamp(preferred.z, z[0], z[1]));
                    let distance = preferred.distance_to_sqr(point);
                    if closest.is_none_or(|(_, d)| distance < d) {
                        closest = Some((point, distance));
                    }
                }
            }
        }
        closest.map(|(point, _)| point)
    }

    pub fn has_chunks_at(&self, x0: i32, z0: i32, x1: i32, z1: i32) -> bool {
        for chunk_x in x0 >> 4..=x1 >> 4 {
            for chunk_z in z0 >> 4..=z1 >> 4 {
                if !self.chunks.has_chunk(ChunkPos::new(chunk_x, chunk_z)) {
                    return false;
                }
            }
        }
        true
    }

    pub fn has_chunk_at(&self, pos: BlockPos) -> bool {
        self.chunks.has_chunk(ChunkPos::new(pos.x >> 4, pos.z >> 4))
    }

    pub fn contains_any_liquid(&mut self, box_: &AABB) -> bool {
        let from = BlockPos::containing(box_.min_x, box_.min_y, box_.min_z);
        let to = BlockPos::containing(box_.max_x, box_.max_y, box_.max_z);
        for z in from.z..=to.z {
            for y in from.y..=to.y {
                for x in from.x..=to.x {
                    if !self.block_state(BlockPos::new(x, y, z)).fluid_state().is_empty() {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn all_air_in(&mut self, box_: &AABB) -> bool {
        let from = BlockPos::containing(box_.min_x, box_.min_y, box_.min_z);
        let to = BlockPos::containing(box_.max_x, box_.max_y, box_.max_z);
        for z in from.z..=to.z {
            for y in from.y..=to.y {
                for x in from.x..=to.x {
                    if !self.block_state(BlockPos::new(x, y, z)).is_air() {
                        return false;
                    }
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use wafer_core::{AABB, BlockPos, Vec3};
    use wafer_data::blocks;

    use super::collide_with_shapes;
    use crate::server::level::testing;
    use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};
    use crate::world::level::update_flags::UPDATE_ALL;
    use crate::world::phys::shapes::collision_context::CollisionContext;

    fn player_box(x: f64, z: f64) -> AABB {
        AABB::new(x - 0.3, -60.0, z - 0.3, x + 0.3, -58.2, z + 0.3)
    }

    #[test]
    fn bamboo_collides_at_its_offset() {
        let mut level = testing::level();
        testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        let (first, second) = (BlockPos::new(4, -60, 4), BlockPos::new(4, -60, 5));
        let bamboo = blocks::BAMBOO.default_state();
        level.set_block(first, bamboo, UPDATE_ALL);
        level.set_block(second, bamboo, UPDATE_ALL);
        assert!(level.no_collision(&player_box(4.0, 5.2166666)));
        assert!(!level.no_collision(&player_box(4.5, 5.5)));

        let offset = bamboo.get_offset(second);
        let from = player_box(3.5, 5.5);
        let colliders = level.block_collisions(&from.expand_towards(1.0, 0.0, 0.0), &CollisionContext::EMPTY, false);
        let colliders: Vec<_> = colliders.into_iter().map(|(_, collider)| collider).collect();
        let moved = collide_with_shapes(Vec3::new(1.0, 0.0, 0.0), &from, &colliders);
        assert_eq!(moved.x, 6.5 / 16.0 + offset.x + 4.0 - from.max_x);

        let ctx = ClipContext { from: Vec3::new(1.5, -59.5, 5.7), to: Vec3::new(6.5, -59.5, 5.7), block: ClipBlock::Collider, fluid: ClipFluid::None, is_player: false };
        let hit = level.clip(&ctx);
        assert!(!hit.miss);
        assert_eq!(hit.hit.block_pos, second);
        assert!((hit.hit.location.x - (6.5 / 16.0 + offset.x + 4.0)).abs() < 1.0e-9);
    }

    #[test]
    fn flower_outline_clips_at_its_offset() {
        let mut level = testing::level();
        testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        let pos = BlockPos::new(6, -60, 6);
        let poppy = blocks::POPPY.default_state();
        level.set_block(pos, poppy, UPDATE_ALL);
        let offset = poppy.get_offset(pos);
        let z = 6.5 + offset.z;
        let ctx = ClipContext { from: Vec3::new(5.0, -59.8, z), to: Vec3::new(8.0, -59.8, z), block: ClipBlock::Outline, fluid: ClipFluid::None, is_player: true };
        let hit = level.clip(&ctx);
        assert!(!hit.miss);
        assert_eq!(hit.hit.block_pos, pos);
        assert!((hit.hit.location.x - (5.0 / 16.0 + offset.x + 6.0)).abs() < 1.0e-9);
    }
}
