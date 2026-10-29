use wafer_core::BlockPos;
use wafer_core::random::RandomSource;
use wafer_data::{blocks, damage_types, fluid_tags};

use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{EntityId, TOTAL_AIR_SUPPLY};
use crate::world::level::pathfinder::path_type::PathType;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

pub const AMBIENT_SOUND_INTERVAL: i32 = 120;

pub fn init(mob: &mut Mob) {
    mob.set_pathfinding_malus(PathType::Water, 0.0);
}

pub fn check_spawn_obstruction(mob: &Mob, level: &mut (impl ServerLevelAccessor + ?Sized)) -> bool {
    level.is_unobstructed(&mob.entity.bounding_box())
}

pub fn get_ambient_sound_interval() -> i32 {
    AMBIENT_SOUND_INTERVAL
}

pub fn is_pushed_by_fluid() -> bool {
    false
}

pub fn check_surface_water_animal_spawn_rules(level: &mut (impl ServerLevelAccessor + ?Sized), pos: BlockPos) -> bool {
    let sea_level = level.get_sea_level();
    let min_spawn_level = sea_level - 13;
    pos.y >= min_spawn_level && pos.y <= sea_level && level.fluid_state(pos.below()).is(&fluid_tags::WATER) && level.block_state(pos.above()).is_block(blocks::WATER)
}

impl Mob {
    pub fn is_water_animal(&self) -> bool {
        matches!(self.kind, MobKind::Cod(_) | MobKind::Salmon(_) | MobKind::Pufferfish(_) | MobKind::TropicalFish(_) | MobKind::Tadpole(_))
    }
}

impl ServerLevel {
    pub fn water_animal_get_base_experience_reward(&mut self, id: EntityId) -> i32 {
        1 + self.entity_mut(id).random.next_int_bounded(3)
    }

    fn handle_air_supply(&mut self, id: EntityId, pre_tick_air_supply: i32) {
        if self.is_alive(id) && !self.entity(id).is_in_water() {
            self.entity_mut(id).set_air_supply(pre_tick_air_supply - 1);
            if self.should_take_drowning_damage(id) {
                self.entity_mut(id).set_air_supply(0);
                self.hurt_server(id, DamageSource::new(damage_types::DROWN), 2.0);
            }
        } else {
            self.entity_mut(id).set_air_supply(TOTAL_AIR_SUPPLY);
        }
    }

    pub fn water_animal_base_tick(&mut self, id: EntityId) {
        let air_supply = self.entity(id).air_supply();
        self.mob_base_tick(id);
        self.then_tick(move |level| level.handle_air_supply(id, air_supply));
    }
}
