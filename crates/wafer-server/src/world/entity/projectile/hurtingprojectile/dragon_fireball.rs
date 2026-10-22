use wafer_core::Vec3;
use wafer_data::mob_effects;
use wafer_protocol::nbt::Nbt;

use super::abstract_hurting_projectile::{self, AbstractHurtingProjectile};
use crate::core::particles::particle_types;
use crate::server::level::ServerLevel;
use crate::world::effect::mob_effect_instance::MobEffectInstance;
use crate::world::entity::area_effect_cloud::AreaEffectCloud;
use crate::world::entity::data::entity_defaults;
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind};
use crate::world::entity::{EntityDimensions, EntityId, LevelEntity};
use crate::world::phys::hit_result::{HitResult, Type};

const ENTITY_TYPE: &str = "minecraft:dragon_fireball";
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(1.0, 1.0);

#[derive(Default)]
pub struct DragonFireball {
    pub abstract_hurting_projectile: AbstractHurtingProjectile,
}

impl DragonFireball {
    pub fn of_type() -> Projectile {
        Projectile::new(ENTITY_TYPE, DIMENSIONS, entity_defaults(), ProjectileKind::DragonFireball(DragonFireball::default()))
    }

    pub fn from_mob(level: &ServerLevel, mob: EntityId, direction: Vec3) -> Projectile {
        abstract_hurting_projectile::from_mob(level, ENTITY_TYPE, DIMENSIONS, entity_defaults(), ProjectileKind::DragonFireball(DragonFireball::default()), mob, direction)
    }
}

impl ServerLevel {
    pub fn is_dragon_fireball(&self, id: EntityId) -> bool {
        self.as_projectile(id).is_some_and(|projectile| matches!(projectile.kind, ProjectileKind::DragonFireball(_)))
    }

    pub fn dragon_fireball_on_hit(&mut self, id: EntityId, hit_result: HitResult) {
        let owned_hit = match &hit_result {
            HitResult::Entity(entity_hit_result) => self.projectile_owned_by(id, entity_hit_result.entity),
            _ => false,
        };
        let is_entity = hit_result.get_type() == Type::Entity;
        self.projectile_base_on_hit(id, hit_result);
        if is_entity && owned_hit {
            return;
        }
        let Some(e) = self.entity_ref(id) else { return };
        let area = e.bounding_box().inflate(4.0, 2.0, 4.0);
        let (x, y, z) = (e.x(), e.y(), e.z());
        let entities_of_class = self.get_entities_matching(None, &area, |level, entity| level.is_living(entity));
        let mut cloud = AreaEffectCloud::new(x, y, z);
        if let Some(owner) = self.projectile_get_owner(id).filter(|&owner| self.is_living(owner)) {
            cloud.set_owner(Some(self.entity(owner).uuid));
        }
        let particle = particle_types::parse("minecraft:dragon_breath", &Nbt::Compound(vec![("power".into(), Nbt::Float(1.0))])).expect("a dragon breath particle");
        cloud.set_custom_particle(Some(particle));
        cloud.set_radius(3.0);
        cloud.duration = 600;
        cloud.radius_per_tick = (7.0 - cloud.get_radius()) / cloud.duration as f32;
        cloud.set_potion_duration_scale(0.25);
        cloud.add_effect(&MobEffectInstance::new(mob_effects::INSTANT_DAMAGE, 1, 1));
        for entity in entities_of_class {
            let dist = self.entity(id).distance_to_sqr(self.entity(entity));
            if dist < 16.0 {
                let t = self.entity(entity);
                cloud.entity.set_pos(Vec3::new(t.x(), t.y(), t.z()));
                break;
            }
        }
        let block_position = self.entity(id).block_position();
        let silent = self.entity(id).is_silent();
        self.level_event(None, crate::server::level::events::PARTICLES_DRAGON_FIREBALL_SPLASH, block_position, if silent { -1 } else { 1 });
        self.add_fresh_entity(LevelEntity::AreaEffectCloud(Box::new(cloud)));
        self.discard(id);
    }
}
