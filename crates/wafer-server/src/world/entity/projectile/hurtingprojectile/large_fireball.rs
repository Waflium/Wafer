use wafer_core::Vec3;
use wafer_data::game_rule;
use wafer_protocol::nbt::Nbt;

use super::abstract_hurting_projectile::{self, AbstractHurtingProjectile};
use super::fireball::fireball_defaults;
use crate::server::level::ServerLevel;
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind};
use crate::world::entity::{EntityDimensions, EntityId};
use crate::world::level::server_explosion::ExplosionInteraction;
use crate::world::phys::entity_hit_result::EntityHitResult;
use crate::world::phys::hit_result::HitResult;
use crate::world::level::block::entity::byte_or;

const ENTITY_TYPE: &str = "minecraft:fireball";
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(1.0, 1.0);
const DEFAULT_EXPLOSION_POWER: i8 = 1;

pub struct LargeFireball {
    pub abstract_hurting_projectile: AbstractHurtingProjectile,
    pub explosion_power: i32,
}

impl Default for LargeFireball {
    fn default() -> LargeFireball {
        LargeFireball { abstract_hurting_projectile: AbstractHurtingProjectile::default(), explosion_power: DEFAULT_EXPLOSION_POWER as i32 }
    }
}

impl LargeFireball {
    pub fn of_type() -> Projectile {
        Projectile::new(ENTITY_TYPE, DIMENSIONS, fireball_defaults(), ProjectileKind::LargeFireball(LargeFireball::default()))
    }

    pub fn from_mob(level: &ServerLevel, mob: EntityId, direction: Vec3, explosion_power: i32) -> Projectile {
        let kind = ProjectileKind::LargeFireball(LargeFireball { explosion_power, ..LargeFireball::default() });
        abstract_hurting_projectile::from_mob(level, ENTITY_TYPE, DIMENSIONS, fireball_defaults(), kind, mob, direction)
    }
}

fn large_fireball(projectile: &Projectile) -> &LargeFireball {
    match &projectile.kind {
        ProjectileKind::LargeFireball(large_fireball) => large_fireball,
        _ => unreachable!("not a large fireball"),
    }
}

impl Projectile {
    pub fn large_fireball_add_additional_save_data(&self, output: &mut Vec<(String, Nbt)>) {
        self.fireball_add_additional_save_data(output);
        output.push(("ExplosionPower".into(), Nbt::Byte(large_fireball(self).explosion_power as i8)));
    }

    pub fn large_fireball_read_additional_save_data(&mut self, input: &Nbt) {
        self.fireball_read_additional_save_data(input);
        let explosion_power = byte_or(input, "ExplosionPower", DEFAULT_EXPLOSION_POWER);
        if let ProjectileKind::LargeFireball(large_fireball) = &mut self.kind {
            large_fireball.explosion_power = explosion_power as i32;
        }
    }
}

impl ServerLevel {
    pub fn large_fireball_on_hit(&mut self, id: EntityId, hit_result: HitResult) {
        self.projectile_base_on_hit(id, hit_result);
        let grief = self.game_rules.get_bool(game_rule::MOB_GRIEFING);
        let explosion_power = large_fireball(self.projectile(id)).explosion_power;
        let position = self.entity(id).position();
        let indirect_source = self.projectile_get_owner(id).filter(|&owner| self.is_living(owner));
        self.explode(Some(id), indirect_source, None, position, explosion_power as f32, grief, ExplosionInteraction::Mob);
        self.discard(id);
    }

    pub fn large_fireball_on_hit_entity(&mut self, id: EntityId, hit_result: EntityHitResult) {
        let entity = hit_result.entity;
        let owner = self.projectile_get_owner(id);
        let damage_source = self.fireball(id, owner);
        self.hurt_server(entity, damage_source, 6.0);
        self.do_post_attack_effects(entity, &damage_source);
    }
}
