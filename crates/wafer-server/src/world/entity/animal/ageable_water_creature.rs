use wafer_core::random::RandomSource;
use wafer_data::damage_types;

use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::entity::ageable_mob::AgeableMob;
use crate::world::entity::TOTAL_AIR_SUPPLY;
use crate::world::entity::mob::Mob;
use crate::world::level::pathfinder::path_type::PathType;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

#[derive(Default)]
pub struct AgeableWaterCreature {
    pub ageable: AgeableMob,
}

impl AgeableWaterCreature {
    pub fn init(mob: &mut Mob) {
        mob.set_pathfinding_malus(PathType::Water, 0.0);
    }
}

pub fn check_spawn_obstruction(mob: &Mob, level: &mut (impl ServerLevelAccessor + ?Sized)) -> bool {
    level.is_unobstructed(&mob.entity.bounding_box())
}

pub fn get_ambient_sound_interval() -> i32 {
    120
}

pub fn is_pushed_by_fluid() -> bool {
    false
}

impl ServerLevel {
    pub fn ageable_water_creature_get_base_experience_reward(&mut self, id: EntityId) -> i32 {
        1 + self.entity_mut(id).random.next_int_bounded(3)
    }

    pub fn ageable_water_creature_handle_air_supply(&mut self, id: EntityId, pre_tick_air_supply: i32) {
        if self.is_alive(id) && !self.entity(id).is_in_water() {
            self.entity_mut(id).set_air_supply(pre_tick_air_supply - 1);
            if self.should_take_drowning_damage(id) {
                self.entity_mut(id).set_air_supply(0);
                self.hurt(id, DamageSource::new(damage_types::DROWN), 2.0);
            }
        } else {
            self.entity_mut(id).set_air_supply(TOTAL_AIR_SUPPLY);
        }
    }

    pub fn ageable_water_creature_base_tick(&mut self, id: EntityId) {
        let air_supply = self.entity(id).air_supply();
        self.mob_base_tick(id);
        self.then_tick(move |level| level.ageable_water_creature_handle_air_supply(id, air_supply));
    }
}
