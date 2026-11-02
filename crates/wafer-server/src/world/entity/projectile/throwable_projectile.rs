use wafer_data::blocks;

use crate::server::level::ServerLevel;
use crate::world::entity::inside_block::InsideBlockEffects;
use crate::world::level::LevelReader;
use crate::world::level::block;
use crate::world::level::blockscan::block_matcher::BlockMatcher;
use crate::world::entity::EntityId;
use crate::world::phys::hit_result::{HitResult, Type};

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
        self.then_tick(move |level| level.throwable_projectile_tick_end(id, result));
    }

    fn throwable_projectile_tick_end(&mut self, id: EntityId, result: HitResult) {
        if result.get_type() != Type::Miss && self.is_alive(id) {
            crate::server::main_thread_frames::within(&["net.minecraft.world.entity.projectile.ThrowableProjectile.tick(ThrowableProjectile.java:65)"], || self.hit_target_or_deflect_self(id, result));
        }
    }

    fn throwable_projectile_handle_first_tick_bubble_column(&mut self, id: EntityId) {
        if !self.entity(id).first_tick {
            return;
        }
        let bb = self.entity(id).bounding_box();
        self.find_blocks_in_aabb(&bb).filter_state(|state| state.is_block(blocks::BUBBLE_COLUMN)).for_each(|level, pos, state| {
            block::of(state).entity_inside(state, level, pos, id, &mut InsideBlockEffects::default(), true);
        });
    }

    fn throwable_projectile_apply_inertia(&mut self, id: EntityId) {
        let inertia = if self.entity(id).is_in_water() { 0.8f32 } else { self.air_drag(id) };
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.scale(inertia as f64));
    }
}
