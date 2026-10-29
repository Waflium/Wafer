use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::damage_types;

use super::projectile::{Projectile, ProjectileKind};
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::data::entity_defaults;
use crate::world::entity::entity_reference::EntityReference;
use crate::world::entity::{EntityDimensions, EntityId};
use crate::world::level::BlockGetter;
use crate::world::level::block::BlockHitResult;
use crate::world::phys::entity_hit_result::EntityHitResult;

const ENTITY_TYPE: &str = "minecraft:llama_spit";
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.25, 0.25);

pub struct LlamaSpit;

impl LlamaSpit {
    pub fn of_type() -> Projectile {
        Projectile::new(ENTITY_TYPE, DIMENSIONS, entity_defaults(), ProjectileKind::LlamaSpit(LlamaSpit))
    }

    pub fn from_owner(level: &ServerLevel, owner: EntityId) -> Projectile {
        let mut spit = LlamaSpit::of_type();
        let o = level.entity(owner);
        spit.owner = Some(EntityReference::of(o.uuid));
        let y_body_rot = level.living(owner).y_body_rot * (std::f64::consts::PI / 180.0) as f32;
        let width = o.dimensions.width + 1.0;
        let x = o.x() - width as f64 * 0.5 * mth::sin(y_body_rot as f64) as f64;
        let y = o.eye_y() - 0.1f32 as f64;
        let z = o.z() + width as f64 * 0.5 * mth::cos(y_body_rot as f64) as f64;
        spit.entity.set_pos(Vec3::new(x, y, z));
        spit
    }
}

impl ServerLevel {
    pub fn llama_spit_tick(&mut self, id: EntityId) {
        self.projectile_base_tick(id);
        self.then_tick(move |level| level.llama_spit_tick_end(id));
    }

    fn llama_spit_tick_end(&mut self, id: EntityId) {
        let movement = self.entity(id).delta_movement;
        let hit_result = self.get_hit_result_on_move_vector(id, |level, entity| level.can_hit_entity(id, entity));
        self.hit_target_or_deflect_self(id, hit_result);
        let e = self.entity(id);
        let (x, y, z) = (e.x() + movement.x, e.y() + movement.y, e.z() + movement.z);
        self.update_rotation(id);
        let bb = self.entity(id).bounding_box();
        let from = BlockPos::containing(bb.min_x, bb.min_y, bb.min_z);
        let to = BlockPos::containing(bb.max_x, bb.max_y, bb.max_z);
        let touches_no_air = !BlockPos::between_closed(from, to).any(|pos| self.block_state(pos).is_air());
        if touches_no_air || self.entity(id).is_in_water() {
            self.discard(id);
        } else {
            self.entity_mut(id).set_delta_movement(movement.scale(0.99f32 as f64));
            self.apply_gravity(id);
            self.entity_mut(id).set_pos(Vec3::new(x, y, z));
            self.entity_moved(id);
        }
    }

    pub fn llama_spit_on_hit_entity(&mut self, id: EntityId, hit_result: EntityHitResult) {
        let Some(owner) = self.projectile_get_owner(id).filter(|&owner| self.is_living(owner)) else { return };
        let target = hit_result.entity;
        let damage_source = DamageSource::source(damage_types::SPIT, Some(id), Some(owner));
        if self.hurt_server(target, damage_source.clone(), 1.0) {
            self.do_post_attack_effects(target, &damage_source);
        }
    }

    pub fn llama_spit_on_hit_block(&mut self, id: EntityId, hit_result: BlockHitResult) {
        self.projectile_base_on_hit_block(id, hit_result);
        self.discard(id);
    }
}
