use wafer_core::Vec3;
use wafer_data::mob_effects;

use super::spider::Spider;
use crate::server::level::ServerLevel;
use crate::world::difficulty::Difficulty;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::attributes::{Attribute, CAVE_SPIDER_ATTRIBUTES};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{EntityDimensions, EntityId};

pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.7, 0.5).with_eye_height(0.45);

#[derive(Default)]
pub struct CaveSpider {
    pub spider: Spider,
}

impl CaveSpider {
    pub fn create() -> Mob {
        Spider::create_with("minecraft:cave_spider", DIMENSIONS, &CAVE_SPIDER_ATTRIBUTES, MobKind::CaveSpider(CaveSpider::default()))
    }
}

impl ServerLevel {
    pub fn cave_spider_do_hurt_target(&mut self, id: EntityId, target: EntityId) -> bool {
        if !self.mob_do_hurt_target(id, target) {
            return false;
        }
        if self.is_living(target) {
            let poison_time = match self.difficulty {
                Difficulty::Normal => 7,
                Difficulty::Hard => 15,
                _ => 0,
            };
            if poison_time > 0 {
                self.add_effect(target, MobEffectInstance::new(mob_effects::POISON, poison_time * 20, 0), Some(id));
            }
        }
        true
    }

    pub fn cave_spider_finalize_spawn(&mut self, _id: EntityId, _difficulty: &DifficultyInstance, _spawn_reason: EntitySpawnReason, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
        group_data
    }

    pub fn cave_spider_get_vehicle_attachment_point(&self, id: EntityId, vehicle: EntityId) -> Vec3 {
        if self.entity(vehicle).dimensions.width <= self.entity(id).dimensions.width {
            Vec3::new(0.0, 0.21875 * self.attribute_value(id, Attribute::Scale) as f32 as f64, 0.0)
        } else {
            self.spider_get_vehicle_attachment_point(id, vehicle)
        }
    }
}
