use wafer_core::{AABB, BlockPos, Direction, Vec3};
use wafer_data::BlockState;

use crate::server::level::ServerLevel;
use crate::world::entity::data::DataValue;
use crate::world::entity::item::ItemEntity;
use crate::world::entity::{Entity, EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::block::is_diode;
use crate::world::phys::shapes::collision_context::CollisionContext;

pub const DATA_DIRECTION: usize = 8;
pub const DEFAULT_DIRECTION: Direction = Direction::South;

pub fn get_direction(entity: &Entity) -> Direction {
    match entity.data.get(DATA_DIRECTION) {
        DataValue::Direction(direction) => *direction,
        _ => DEFAULT_DIRECTION,
    }
}

pub fn set_direction_raw(entity: &mut Entity, direction: Direction) {
    entity.data.set(DATA_DIRECTION, DataValue::Direction(direction));
}

pub fn set_direction(entity: &mut Entity, direction: Direction) {
    set_direction_raw(entity, direction);
    entity.y_rot = (direction.get_2d_data_value() * 90) as f32;
    entity.y_rot_o = entity.y_rot;
    super::block_attached_entity::recalculate_bounding_box(entity);
}

pub fn recalculate_bounding_box(entity: &mut Entity, pos: BlockPos, calculate_bounding_box: fn(&Entity, BlockPos, Direction) -> AABB) {
    let aabb = calculate_bounding_box(entity, pos, get_direction(entity));
    entity.set_pos_raw(aabb.center());
    entity.set_bounding_box(aabb);
}

fn is_supporting_block(state: BlockState) -> bool {
    state.is_solid() || is_diode(state)
}

#[derive(Clone, Copy)]
pub struct HangingView {
    pub id: EntityId,
    pub entity_type: i32,
    pub bounding_box: AABB,
    pub pop_box: AABB,
    pub direction: Direction,
    pub pos: BlockPos,
    pub x: f64,
    pub z: f64,
    pub context: CollisionContext,
}

impl HangingView {
    pub fn of(entity: &Entity, pop_box: AABB) -> HangingView {
        HangingView {
            id: entity.id,
            entity_type: entity.entity_type,
            bounding_box: entity.bounding_box(),
            pop_box,
            direction: get_direction(entity),
            pos: entity.block_attached_pos(),
            x: entity.x(),
            z: entity.z(),
            context: CollisionContext::of_detached(entity),
        }
    }

    fn calculate_support_box(&self) -> AABB {
        let d = self.direction;
        self.bounding_box.move_by((d.step_x() as f32 * -0.5) as f64, (d.step_y() as f32 * -0.5) as f64, (d.step_z() as f32 * -0.5) as f64).deflate_all(1.0e-7)
    }
}

impl ServerLevel {
    pub fn hanging_entity_survives(&mut self, id: EntityId) -> bool {
        let entity = self.entity(id);
        let view = HangingView::of(entity, entity.bounding_box());
        self.hanging_view_survives(&view)
    }

    pub fn hanging_view_survives(&mut self, view: &HangingView) -> bool {
        if self.hanging_entity_has_level_collision(view, &view.pop_box) {
            return false;
        }
        let support_box = view.calculate_support_box();
        let from = BlockPos::containing(support_box.min_x, support_box.min_y, support_box.min_z);
        let to = BlockPos::containing(support_box.max_x, support_box.max_y, support_box.max_z);
        let is_supported = !self.find_blocks_in(from, to).filter_state(|state| !is_supporting_block(state)).any_matched(self);
        is_supported && self.hanging_entity_can_coexist(view, false)
    }

    pub fn hanging_entity_can_coexist(&self, view: &HangingView, allow_intersecting_same_type: bool) -> bool {
        !self.get_entities(None, &view.pop_box).into_iter().any(|other| {
            let Some(other_entity) = self.entity_ref(other) else { return false };
            if !matches!(self.entities.get(&other), Some(LevelEntity::Painting(_) | LevelEntity::ItemFrame(_))) {
                return false;
            }
            let intersects_same_type = !allow_intersecting_same_type && other_entity.entity_type == view.entity_type;
            let is_same_direction = get_direction(other_entity) == view.direction;
            other != view.id && (intersects_same_type || is_same_direction)
        })
    }

    pub fn hanging_entity_has_level_collision(&self, view: &HangingView, pop_box: &AABB) -> bool {
        !self.for_each_block_collision(pop_box, &view.context, false, false, |_, c| c.shape.is_empty()) || !self.no_border_collision_at(view.x, view.z, pop_box)
    }

    pub fn hanging_entity_spawn_at_location(&mut self, id: EntityId, item_stack: ItemStack, y_offs: f32) {
        let entity = self.entity(id);
        let direction = get_direction(entity);
        let x = entity.x() + (direction.step_x() as f32 * 0.15f32) as f64;
        let y = entity.y() + y_offs as f64;
        let z = entity.z() + (direction.step_z() as f32 * 0.15f32) as f64;
        let mut item = ItemEntity::new(Vec3::new(x, y, z), item_stack);
        item.set_default_pickup_delay();
        self.add_fresh_entity(LevelEntity::Item(item));
    }
}
