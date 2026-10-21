use wafer_core::{BlockPos, Vec3};
use wafer_data::{registry, registry_id};

use crate::world::entity::attributes::GIANT_ATTRIBUTES;
use crate::world::entity::data::mob_defaults;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::monster::monster;
use crate::world::entity::{Entity, EntityDimensions};
use crate::world::level::server_level_accessor::ServerLevelAccessor;

pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(3.6, 12.0).with_eye_height(10.44).with_vehicle_attachment(&[Vec3::new(0.0, 3.75f32 as f64, 0.0)]);

#[derive(Default)]
pub struct Giant;

impl Giant {
    pub fn create() -> Mob {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:giant"), DIMENSIONS, mob_defaults());
        let mut mob = Mob::new(entity, LivingEntity::new(&GIANT_ATTRIBUTES), MobKind::Giant(Giant));
        mob.xp_reward = monster::XP_REWARD;
        mob
    }
}

impl crate::server::level::ServerLevel {
    pub fn giant_get_walk_target_value(&self, pos: BlockPos) -> f32 {
        self.get_pathfinding_cost_from_light_levels(pos)
    }
}

pub fn get_walk_target_value(pos: BlockPos, level: &mut (impl ServerLevelAccessor + ?Sized)) -> f32 {
    level.get_pathfinding_cost_from_light_levels(pos)
}
