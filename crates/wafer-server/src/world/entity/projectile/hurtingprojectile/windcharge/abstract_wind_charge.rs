use wafer_core::{AABB, Vec3};
use wafer_data::damage_types;

use crate::core::holder_set::HolderSet;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::projectile::hurtingprojectile::abstract_hurting_projectile::AbstractHurtingProjectile;
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind};
use crate::world::entity::{EntityDimensions, EntityId, entity_types};
use crate::world::level::block::BlockHitResult;
use crate::world::level::server_explosion::ExplosionDamageCalculator;
use crate::world::phys::entity_hit_result::EntityHitResult;
use crate::world::phys::hit_result::HitResult;

pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.3125, 0.3125).with_eye_height(0.0);

pub fn explosion_damage_calculator(knockback_multiplier: Option<f32>) -> ExplosionDamageCalculator {
    let immune_blocks = HolderSet::named("minecraft:block", "minecraft:blocks_wind_charge_explosions".to_string());
    ExplosionDamageCalculator::Simple { explodes_blocks: true, damages_entities: false, knockback_multiplier, immune_blocks: Some(immune_blocks) }
}

pub struct AbstractWindCharge {
    pub abstract_hurting_projectile: AbstractHurtingProjectile,
}

impl Default for AbstractWindCharge {
    fn default() -> AbstractWindCharge {
        AbstractWindCharge { abstract_hurting_projectile: AbstractHurtingProjectile { acceleration_power: 0.0 } }
    }
}

pub fn make_bounding_box(dimensions: EntityDimensions, position: Vec3) -> AABB {
    let width = dimensions.width / 2.0;
    let height = dimensions.height;
    let offset = 0.15f32;
    AABB::new(
        position.x - width as f64,
        position.y - offset as f64,
        position.z - width as f64,
        position.x + width as f64,
        position.y - offset as f64 + height as f64,
        position.z + width as f64,
    )
}

impl Projectile {
    pub fn is_abstract_wind_charge(&self) -> bool {
        matches!(self.kind, ProjectileKind::WindCharge(_) | ProjectileKind::BreezeWindCharge(_))
    }
}

impl ServerLevel {
    pub fn is_wind_charge(&self, id: EntityId) -> bool {
        self.as_projectile(id).is_some_and(Projectile::is_abstract_wind_charge)
    }

    pub fn abstract_wind_charge_can_collide_with(&self, entity: EntityId) -> bool {
        !self.is_wind_charge(entity)
    }

    pub fn abstract_wind_charge_can_hit_entity(&self, id: EntityId, entity: EntityId) -> bool {
        if self.is_wind_charge(entity) {
            false
        } else if self.entity(entity).entity_type == entity_types::END_CRYSTAL.id {
            false
        } else {
            self.abstract_hurting_projectile_can_hit_entity(id, entity)
        }
    }

    pub fn abstract_wind_charge_on_hit_entity(&mut self, id: EntityId, hit_result: EntityHitResult) {
        let owner = self.projectile_get_owner(id).filter(|&owner| self.is_living(owner));
        let entity = hit_result.entity;
        if let Some(owner) = owner {
            self.set_last_hurt_mob(owner, entity);
        }
        let source = DamageSource { causing_entity: owner, direct_entity: Some(id), ..DamageSource::new(damage_types::WIND_CHARGE) };
        if self.hurt_server(entity, source, 1.0) && self.is_living(entity) {
            self.do_post_attack_effects(entity, &source);
        }
        let position = self.entity(id).position();
        self.abstract_wind_charge_explode(id, position);
    }

    pub fn abstract_wind_charge_on_hit_block(&mut self, id: EntityId, hit_result: BlockHitResult) {
        self.projectile_base_on_hit_block(id, hit_result);
        let direction = hit_result.direction;
        let scaled_normal = Vec3::new(direction.step_x() as f64, direction.step_y() as f64, direction.step_z() as f64).multiply(0.25, 0.25, 0.25);
        let explosion_pos = hit_result.location.add_vec(scaled_normal);
        self.abstract_wind_charge_explode(id, explosion_pos);
        self.discard(id);
    }

    pub fn abstract_wind_charge_on_hit(&mut self, id: EntityId, hit_result: HitResult) {
        self.projectile_base_on_hit(id, hit_result);
        self.discard(id);
    }

    pub fn abstract_wind_charge_tick(&mut self, id: EntityId) {
        if self.entity(id).block_position().y > self.chunks.max_y() + 30 {
            let position = self.entity(id).position();
            self.abstract_wind_charge_explode(id, position);
            self.discard(id);
        } else {
            self.abstract_hurting_projectile_tick(id);
        }
    }

    fn abstract_wind_charge_explode(&mut self, id: EntityId, position: Vec3) {
        if matches!(self.projectile(id).kind, ProjectileKind::WindCharge(_)) {
            self.wind_charge_explode(id, position);
        } else {
            self.breeze_wind_charge_explode(id, position);
        }
    }
}
