use wafer_core::Vec3;
use wafer_data::game_rule;

use super::abstract_hurting_projectile::{self, AbstractHurtingProjectile};
use super::fireball::fireball_defaults;
use crate::server::level::ServerLevel;
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind};
use crate::world::entity::{EntityDimensions, EntityId};
use crate::world::level::BlockGetter;
use crate::world::level::block::{BlockHitResult, base_fire_block};
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::phys::entity_hit_result::EntityHitResult;
use crate::world::phys::hit_result::HitResult;

const ENTITY_TYPE: &str = "minecraft:small_fireball";
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.3125, 0.3125);

#[derive(Default)]
pub struct SmallFireball {
    pub abstract_hurting_projectile: AbstractHurtingProjectile,
}

impl SmallFireball {
    pub fn of_type() -> Projectile {
        Projectile::new(ENTITY_TYPE, DIMENSIONS, fireball_defaults(), ProjectileKind::SmallFireball(SmallFireball::default()))
    }

    pub fn from_mob(level: &ServerLevel, mob: EntityId, direction: Vec3) -> Projectile {
        abstract_hurting_projectile::from_mob(level, ENTITY_TYPE, DIMENSIONS, fireball_defaults(), ProjectileKind::SmallFireball(SmallFireball::default()), mob, direction)
    }

    pub fn at(x: f64, y: f64, z: f64, direction: Vec3) -> Projectile {
        abstract_hurting_projectile::at(ENTITY_TYPE, DIMENSIONS, fireball_defaults(), ProjectileKind::SmallFireball(SmallFireball::default()), x, y, z, direction)
    }
}

impl ServerLevel {
    pub fn small_fireball_on_hit_entity(&mut self, id: EntityId, hit_result: EntityHitResult) {
        let entity = hit_result.entity;
        let owner = self.projectile_get_owner(id);
        let remaining_fire_ticks = self.entity(entity).remaining_fire_ticks;
        self.ignite_for_seconds(entity, 5.0);
        let damage_source = self.fireball(id, owner);
        if !self.hurt_server(entity, damage_source, 5.0) {
            self.set_remaining_fire_ticks(entity, remaining_fire_ticks);
        } else {
            self.do_post_attack_effects(entity, &damage_source);
        }
    }

    pub fn small_fireball_on_hit_block(&mut self, id: EntityId, hit_result: BlockHitResult) {
        self.projectile_base_on_hit_block(id, hit_result);
        let owner = self.projectile_get_owner(id);
        if !owner.is_some_and(|owner| self.is_mob(owner)) || self.game_rules.get_bool(game_rule::MOB_GRIEFING) {
            let pos = hit_result.block_pos.relative(hit_result.direction);
            if self.block_state(pos).is_air() {
                let fire = base_fire_block::get_state(self, pos);
                self.set_block(pos, fire, UPDATE_ALL);
            }
        }
    }

    pub fn small_fireball_on_hit(&mut self, id: EntityId, hit_result: HitResult) {
        self.projectile_base_on_hit(id, hit_result);
        self.discard(id);
    }
}
