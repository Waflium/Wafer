use std::sync::atomic::{AtomicU64, Ordering};

use wafer_core::random::RandomSource;

use super::sensor_type::SensorType;
use super::{adult_sensor, hurt_by_sensor, nearest_item_sensor, nearest_living_entity_sensor, player_sensor, tempting_sensor};
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

pub struct Sensor {
    sensor_type: SensorType,
    scan_rate: i32,
    time_to_tick: i64,
}

impl Sensor {
    pub fn new(sensor_type: SensorType) -> Sensor {
        Sensor::with_scan_rate(sensor_type, DEFAULT_SCAN_RATE)
    }

    pub fn with_scan_rate(sensor_type: SensorType, scan_rate: i32) -> Sensor {
        Sensor { sensor_type, scan_rate, time_to_tick: 0 }
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
            SensorType::NearestAdult => adult_sensor::do_tick(level, body),
            SensorType::FoodTemptations => tempting_sensor::do_tick(level, body, tempting_sensor::for_animal),
        }
    }

    pub fn requires(&self) -> Vec<MemoryModuleType<Erased>> {
        match self.sensor_type {
            SensorType::NearestItems => nearest_item_sensor::requires(),
            SensorType::NearestLivingEntities => nearest_living_entity_sensor::requires(),
            SensorType::NearestPlayers => player_sensor::requires(),
            SensorType::HurtBy => hurt_by_sensor::requires(),
            SensorType::NearestAdult => adult_sensor::requires(),
            SensorType::FoodTemptations => tempting_sensor::requires(),
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

pub fn is_entity_attackable(level: &mut ServerLevel, body: EntityId, target: EntityId) -> bool {
    if level.get_brain(body).is_memory_value(memory_module_type::ATTACK_TARGET, &target) {
        attack_target_conditions_ignore_invisibility_testing().test(level, Some(body), target)
    } else {
        attack_target_conditions().test(level, Some(body), target)
    }
}
