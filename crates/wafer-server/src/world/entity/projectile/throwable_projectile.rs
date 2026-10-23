use wafer_core::BlockPos;
use wafer_data::blocks;

use crate::server::level::ServerLevel;
use crate::world::entity::inside_block::InsideBlockEffects;
use crate::world::level::BlockGetter;
use crate::world::level::block;
use crate::world::entity::EntityId;
use crate::world::phys::hit_result::Type;

impl ServerLevel {
    pub fn throwable_projectile_tick(&mut self, id: EntityId) {
        self.throwable_projectile_handle_first_tick_bubble_column(id);
        self.apply_gravity(id);
        self.throwable_projectile_apply_inertia(id);
        let result = self.get_hit_result_on_move_vector(id, |level, entity| level.can_hit_entity(id, entity));
        let new_position = if result.get_type() != Type::Miss {
            result.get_location()
        } else {
            let e = self.entity(id);
            e.position().add_vec(e.delta_movement)
        };
        self.entity_mut(id).set_pos(new_position);
        self.entity_moved(id);
        self.update_rotation(id);
        self.apply_effects_from_blocks(id);
        self.projectile_base_tick(id);
        if result.get_type() != Type::Miss && self.is_alive(id) {
            self.hit_target_or_deflect_self(id, result);
        }
    }

    fn throwable_projectile_handle_first_tick_bubble_column(&mut self, id: EntityId) {
        if !self.entity(id).first_tick {
            return;
        }
        let bb = self.entity(id).bounding_box();
        let (from, to) = (BlockPos::containing(bb.min_x, bb.min_y, bb.min_z), BlockPos::containing(bb.max_x, bb.max_y, bb.max_z));
        for pos in BlockPos::between_closed(from, to) {
            let state = self.block_state(pos);
            if state.is_block(blocks::BUBBLE_COLUMN) {
                block::of(state).entity_inside(state, self, pos, id, &mut InsideBlockEffects::default(), true);
            }
        }
    }

    fn throwable_projectile_apply_inertia(&mut self, id: EntityId) {
        let inertia = if self.entity(id).is_in_water() { 0.8f32 } else { self.air_drag(id) };
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.scale(inertia as f64));
    }
}
