use wafer_core::Vec3;
use wafer_core::random::RandomSource;
use wafer_data::{EquipmentSlot, items};

use super::zombie::{Zombie, ZombieGroupData};
use crate::server::level::ServerLevel;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::attributes::ZOMBIE_ATTRIBUTES;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{EntityDimensions, EntityId};
use crate::world::item::ItemStack;

pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.6, 1.95)
    .with_eye_height(1.74)
    .with_passenger_attachments(&[Vec3::new(0.0, 2.075f32 as f64, 0.0)])
    .with_vehicle_attachment(&[Vec3::new(0.0, 0.7f32 as f64, 0.0)]);
pub const BABY_DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(0.49, 0.98).with_eye_height(0.825).with_vehicle_attachment(&[Vec3::new(0.0, 0.1875f32 as f64, 0.0)]);
const CAMEL_HUSK_DIMENSIONS: EntityDimensions = EntityDimensions::scalable(1.7, 2.375);

#[derive(Default)]
pub struct Husk {
    pub zombie: Zombie,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HuskGroupData {
    pub zombie_group_data: ZombieGroupData,
    pub tried_to_spawn_camel_husk: bool,
}

impl HuskGroupData {
    pub fn new(group_data: ZombieGroupData) -> HuskGroupData {
        HuskGroupData { zombie_group_data: group_data, tried_to_spawn_camel_husk: false }
    }
}

impl Husk {
    pub fn create() -> Mob {
        Zombie::create_with("minecraft:husk", DIMENSIONS, &ZOMBIE_ATTRIBUTES, MobKind::Husk(Husk::default()))
    }
}

impl ServerLevel {
    pub fn husk_finalize_spawn(&mut self, id: EntityId, difficulty: &DifficultyInstance, spawn_reason: EntitySpawnReason, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
        let mut group_data = self.zombie_finalize_spawn(id, difficulty, spawn_reason, group_data);
        let difficulty_modifier = difficulty.get_special_multiplier();
        if spawn_reason != EntitySpawnReason::Conversion {
            let can_pick_up_loot = self.random.next_float() < 0.55 * difficulty_modifier;
            self.mob_mut(id).expect("a husk").can_pick_up_loot = can_pick_up_loot;
        }
        let zombie_group_data = match group_data {
            Some(SpawnGroupData::Zombie(zombie_data)) => Some(zombie_data),
            Some(SpawnGroupData::Husk(husk_data)) => Some(husk_data.zombie_group_data),
            _ => None,
        };
        if let Some(zombie_group_data) = zombie_group_data {
            let mut husk_group_data = HuskGroupData::new(zombie_group_data);
            husk_group_data.tried_to_spawn_camel_husk = spawn_reason != EntitySpawnReason::Natural;
            group_data = Some(SpawnGroupData::Husk(husk_group_data));
        }
        if let Some(SpawnGroupData::Husk(husk_group_data)) = &mut group_data
            && !husk_group_data.tried_to_spawn_camel_husk
        {
            let pos = self.entity(id).block_position();
            let spawn_aabb = CAMEL_HUSK_DIMENSIONS.make_bounding_box(Vec3::new(pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5));
            if self.no_collision(&spawn_aabb) {
                husk_group_data.tried_to_spawn_camel_husk = true;
                if self.random.next_float() < 0.1 {
                    self.living_set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::new(items::IRON_SPEAR, 1));
                }
            }
        }
        group_data
    }
}
