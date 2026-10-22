use std::sync::atomic::{AtomicU64, Ordering};

use wafer_core::random::RandomSource;

use super::mob_sensor::MobSensor;
use super::nearest_bed_sensor::NearestBedSensor;
use super::sensor_type::{self, SensorType};
use super::{
    adult_sensor, axolotl_attackables_sensor, frog_attackables_sensor, golem_sensor, hurt_by_sensor, is_in_water_sensor, nearest_bed_sensor, nearest_item_sensor, nearest_living_entity_sensor, player_sensor, secondary_poi_sensor,
    tempting_sensor, villager_babies_sensor, villager_hostiles_sensor,
};
use crate::world::entity::animal::frog::frog_ai;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, Erased, MemoryModuleType};
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::attributes::Attribute;

const DEFAULT_SCAN_RATE: i32 = 20;
const DEFAULT_TARGETING_RANGE: f64 = 16.0;

static TARGETING_RANGE: AtomicU64 = AtomicU64::new(DEFAULT_TARGETING_RANGE.to_bits());

fn targeting_range() -> f64 {
    f64::from_bits(TARGETING_RANGE.load(Ordering::Relaxed))
}

fn target_conditions() -> TargetingConditions<'static> {
    TargetingConditions::for_non_combat().range(targeting_range())
}

fn target_conditions_ignore_invisibility_testing() -> TargetingConditions<'static> {
    TargetingConditions::for_non_combat().range(targeting_range()).ignore_invisibility_testing()
}

fn attack_target_conditions() -> TargetingConditions<'static> {
    TargetingConditions::for_combat().range(targeting_range())
}

fn attack_target_conditions_ignore_invisibility_testing() -> TargetingConditions<'static> {
    TargetingConditions::for_combat().range(targeting_range()).ignore_invisibility_testing()
}

fn attack_target_conditions_ignore_line_of_sight() -> TargetingConditions<'static> {
    TargetingConditions::for_combat().range(targeting_range()).ignore_line_of_sight()
}

fn attack_target_conditions_ignore_invisibility_and_line_of_sight() -> TargetingConditions<'static> {
    TargetingConditions::for_combat().range(targeting_range()).ignore_line_of_sight().ignore_invisibility_testing()
}

pub struct Sensor {
    sensor_type: SensorType,
    scan_rate: i32,
    time_to_tick: i64,
    nearest_bed: Option<Box<NearestBedSensor>>,
}

impl Sensor {
    pub fn new(sensor_type: SensorType) -> Sensor {
        Sensor::with_scan_rate(sensor_type, DEFAULT_SCAN_RATE)
    }

    pub fn with_scan_rate(sensor_type: SensorType, scan_rate: i32) -> Sensor {
        let nearest_bed = (sensor_type == SensorType::NearestBed).then(Box::default);
        Sensor { sensor_type, scan_rate, time_to_tick: 0, nearest_bed }
    }

    pub fn get_type(&self) -> SensorType {
        self.sensor_type
    }

    pub fn randomly_delay_start(&mut self, random: &mut impl RandomSource) {
        self.time_to_tick = random.next_int_bounded(self.scan_rate) as i64;
    }

    #[cfg(test)]
    pub fn set_time_to_tick(&mut self, time_to_tick: i64) {
        self.time_to_tick = time_to_tick;
    }

    pub fn tick(&mut self, level: &mut ServerLevel, body: EntityId) {
        self.time_to_tick -= 1;
        if self.time_to_tick <= 0 {
            self.time_to_tick = self.scan_rate as i64;
            update_targeting_condition_ranges(level, body);
            self.do_tick(level, body);
        }
    }

    fn do_tick(&mut self, level: &mut ServerLevel, body: EntityId) {
        match self.sensor_type {
            SensorType::NearestItems => nearest_item_sensor::do_tick(level, body),
            SensorType::NearestLivingEntities => nearest_living_entity_sensor::do_tick(level, body),
            SensorType::NearestPlayers => player_sensor::do_tick(level, body),
            SensorType::HurtBy => hurt_by_sensor::do_tick(level, body),
            SensorType::ArmadilloScareDetected => sensor_type::ARMADILLO_SCARE_DETECTED.do_tick(level, body),
            SensorType::NearestAdult => adult_sensor::do_tick(level, body),
            SensorType::NearestAdultAnyType => super::adult_sensor_any_type::do_tick(level, body),
            SensorType::AxolotlAttackables => axolotl_attackables_sensor::do_tick(level, body),
            SensorType::FoodTemptations => tempting_sensor::do_tick(level, body, tempting_sensor::for_animal),
            SensorType::FrogTemptations => tempting_sensor::do_tick(level, body, |_, _, item_stack| frog_ai::get_temptations(item_stack)),
            SensorType::NautilusTemptations => tempting_sensor::do_tick(level, body, |_, _, item_stack| crate::world::entity::animal::nautilus::nautilus_ai::get_temptations(item_stack)),
            SensorType::FrogAttackables => frog_attackables_sensor::do_tick(level, body),
            SensorType::IsInWater => is_in_water_sensor::do_tick(level, body),
            SensorType::HoglinSpecificSensor => super::hoglin_specific_sensor::do_tick(level, body),
            SensorType::PiglinSpecificSensor => super::piglin_specific_sensor::do_tick(level, body),
            SensorType::PiglinBruteSpecificSensor => super::piglin_brute_specific_sensor::do_tick(level, body),
            SensorType::BreezeAttackEntitySensor => super::breeze_attack_entity_sensor::do_tick(level, body),
            SensorType::WardenEntitySensor => super::warden_entity_sensor::do_tick(level, body),
            SensorType::NearestBed => self.nearest_bed.as_mut().expect("a nearest bed sensor").do_tick(level, body),
            SensorType::VillagerHostiles => villager_hostiles_sensor::do_tick(level, body),
            SensorType::VillagerBabies => villager_babies_sensor::do_tick(level, body),
            SensorType::SecondaryPois => secondary_poi_sensor::do_tick(level, body),
            SensorType::GolemDetected => golem_sensor::do_tick(level, body),
        }
    }

    pub fn requires(&self) -> Vec<MemoryModuleType<Erased>> {
        match self.sensor_type {
            SensorType::NearestItems => nearest_item_sensor::requires(),
            SensorType::NearestLivingEntities => nearest_living_entity_sensor::requires(),
            SensorType::NearestPlayers => player_sensor::requires(),
            SensorType::HurtBy => hurt_by_sensor::requires(),
            SensorType::ArmadilloScareDetected => MobSensor::requires(),
            SensorType::NearestAdult | SensorType::NearestAdultAnyType => adult_sensor::requires(),
            SensorType::AxolotlAttackables => axolotl_attackables_sensor::requires(),
            SensorType::FoodTemptations | SensorType::FrogTemptations | SensorType::NautilusTemptations => tempting_sensor::requires(),
            SensorType::FrogAttackables => frog_attackables_sensor::requires(),
            SensorType::IsInWater => is_in_water_sensor::requires(),
            SensorType::HoglinSpecificSensor => super::hoglin_specific_sensor::requires(),
            SensorType::PiglinSpecificSensor => super::piglin_specific_sensor::requires(),
            SensorType::PiglinBruteSpecificSensor => super::piglin_brute_specific_sensor::requires(),
            SensorType::BreezeAttackEntitySensor => super::breeze_attack_entity_sensor::requires(),
            SensorType::WardenEntitySensor => super::warden_entity_sensor::requires(),
            SensorType::NearestBed => nearest_bed_sensor::requires(),
            SensorType::VillagerHostiles => villager_hostiles_sensor::requires(),
            SensorType::VillagerBabies => villager_babies_sensor::requires(),
            SensorType::SecondaryPois => secondary_poi_sensor::requires(),
            SensorType::GolemDetected => golem_sensor::requires(),
        }
    }
}

fn update_targeting_condition_ranges(level: &ServerLevel, body: EntityId) {
    let follow_range = level.attribute_value(body, Attribute::FollowRange);
    TARGETING_RANGE.store(follow_range.to_bits(), Ordering::Relaxed);
}

pub fn is_entity_targetable(level: &mut ServerLevel, body: EntityId, entity: EntityId) -> bool {
    if level.get_brain(body).is_memory_value(memory_module_type::ATTACK_TARGET, &entity) {
        target_conditions_ignore_invisibility_testing().test(level, Some(body), entity)
    } else {
        target_conditions().test(level, Some(body), entity)
    }
}

pub fn is_entity_attackable_ignoring_line_of_sight(level: &mut ServerLevel, body: EntityId, target: EntityId) -> bool {
    if level.get_brain(body).is_memory_value(memory_module_type::ATTACK_TARGET, &target) {
        attack_target_conditions_ignore_invisibility_and_line_of_sight().test(level, Some(body), target)
    } else {
        attack_target_conditions_ignore_line_of_sight().test(level, Some(body), target)
    }
}

pub fn is_entity_attackable(level: &mut ServerLevel, body: EntityId, target: EntityId) -> bool {
    if level.get_brain(body).is_memory_value(memory_module_type::ATTACK_TARGET, &target) {
        attack_target_conditions_ignore_invisibility_testing().test(level, Some(body), target)
    } else {
        attack_target_conditions().test(level, Some(body), target)
    }
}
