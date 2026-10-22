use wafer_core::Vec3;
use wafer_data::{BlockState, damage_types, mob_effects};
use wafer_protocol::nbt::Nbt;

use super::abstract_hurting_projectile::{self, AbstractHurtingProjectile};
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::effect::mob_effect_instance::MobEffectInstance;
use crate::world::entity::boss::wither::wither_boss;
use crate::world::entity::data::{DataValue, entity_defaults};
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind};
use crate::world::entity::{Entity, EntityDimensions, EntityId};
use crate::world::level::server_explosion::ExplosionInteraction;
use crate::world::phys::entity_hit_result::EntityHitResult;
use crate::world::phys::hit_result::HitResult;

const ENTITY_TYPE: &str = "minecraft:wither_skull";
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.3125, 0.3125);
pub const DATA_DANGEROUS: usize = 8;
const DEFAULT_DANGEROUS: bool = false;

#[derive(Default)]
pub struct WitherSkull {
    pub abstract_hurting_projectile: AbstractHurtingProjectile,
}

fn wither_skull_defaults() -> Vec<DataValue> {
    let mut data = entity_defaults();
    data.push(DataValue::Boolean(DEFAULT_DANGEROUS));
    data
}

impl WitherSkull {
    pub fn of_type() -> Projectile {
        Projectile::new(ENTITY_TYPE, DIMENSIONS, wither_skull_defaults(), ProjectileKind::WitherSkull(WitherSkull::default()))
    }

    pub fn from_mob(level: &ServerLevel, mob: EntityId, direction: Vec3) -> Projectile {
        abstract_hurting_projectile::from_mob(level, ENTITY_TYPE, DIMENSIONS, wither_skull_defaults(), ProjectileKind::WitherSkull(WitherSkull::default()), mob, direction)
    }
}

pub fn is_dangerous(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_DANGEROUS), DataValue::Boolean(true))
}

pub fn set_dangerous(entity: &mut Entity, value: bool) {
    entity.data.set(DATA_DANGEROUS, DataValue::Boolean(value));
}

pub fn get_inertia(entity: &Entity) -> f32 {
    if is_dangerous(entity) { 0.73 } else { 0.95 }
}

pub fn get_block_explosion_resistance(dangerous: bool, block: BlockState, resistance: f32) -> f32 {
    if dangerous && wither_boss::can_destroy(block) { 0.8f32.min(resistance) } else { resistance }
}

impl Projectile {
    pub fn wither_skull_add_additional_save_data(&self, output: &mut Vec<(String, Nbt)>) {
        self.abstract_hurting_projectile_add_additional_save_data(output);
        output.push(("dangerous".into(), Nbt::Byte(is_dangerous(&self.entity) as i8)));
    }

    pub fn wither_skull_read_additional_save_data(&mut self, input: &Nbt) {
        self.abstract_hurting_projectile_read_additional_save_data(input);
        let dangerous = input.get("dangerous").and_then(Nbt::as_i64).is_some_and(|v| v != 0);
        set_dangerous(&mut self.entity, dangerous);
    }
}

impl ServerLevel {
    pub fn is_wither_skull(&self, id: EntityId) -> bool {
        self.as_projectile(id).is_some_and(|projectile| matches!(projectile.kind, ProjectileKind::WitherSkull(_)))
    }

    pub fn wither_skull_on_hit_entity(&mut self, id: EntityId, hit_result: EntityHitResult) {
        let target = hit_result.entity;
        let owner = self.projectile_get_owner(id);
        let was_hurt = match owner.filter(|&owner| self.is_living(owner)) {
            Some(living_owner) => {
                let damage_source = DamageSource::source(damage_types::WITHER_SKULL, Some(id), Some(living_owner));
                let was_hurt = self.hurt_server(target, damage_source.clone(), 8.0);
                if was_hurt {
                    if self.is_alive(target) {
                        self.do_post_attack_effects(target, &damage_source);
                    } else {
                        self.heal(living_owner, 5.0);
                    }
                }
                was_hurt
            }
            None => self.hurt_server(target, DamageSource::new(damage_types::MAGIC), 5.0),
        };
        if was_hurt && self.is_living(target) {
            let wither_seconds = match self.difficulty {
                Difficulty::Normal => 10,
                Difficulty::Hard => 40,
                _ => 0,
            };
            if wither_seconds > 0 {
                let effect_source = owner.unwrap_or(id);
                self.add_effect(target, MobEffectInstance::new(mob_effects::WITHER, 20 * wither_seconds, 1), Some(effect_source));
            }
        }
    }

    pub fn wither_skull_on_hit(&mut self, id: EntityId, hit_result: HitResult) {
        self.projectile_base_on_hit(id, hit_result);
        let position = self.entity(id).position();
        let indirect_source = self.projectile_get_owner(id).filter(|&owner| self.is_living(owner));
        let dangerous = is_dangerous(self.entity(id));
        self.explode_with_calculator(
            Some(id),
            indirect_source,
            None,
            crate::world::level::server_explosion::ExplosionDamageCalculator::WitherSkull { dangerous },
            position,
            1.0,
            false,
            ExplosionInteraction::Mob,
        );
        self.discard(id);
    }
}
