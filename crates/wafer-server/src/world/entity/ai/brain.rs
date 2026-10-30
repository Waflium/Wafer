use wafer_core::Vec3;
use wafer_core::random::RandomSource;
use wafer_protocol::nbt::Nbt;

use super::activity_data::ActivityData;
use crate::util::profiling::profiler;
use super::behavior::behavior::Status;
use super::behavior::behavior_control::BehaviorControl;
use super::memory::expirable_value::ExpirableValue;
use super::memory::memory_map::{self, MemoryMap};
use super::memory::memory_module_type::{Erased, MemoryModuleType};
use super::memory::memory_slot::MemorySlot;
use super::memory::memory_status::MemoryStatus;
use super::memory::memory_value::{MemoryValue, MemoryValueType};
use super::sensing::sensor::Sensor;
use super::sensing::sensor_type::SensorType;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DecodeResult, Record};
use crate::world::attribute::environment_attribute::EnvironmentAttribute;
use crate::world::entity::EntityId;
use crate::world::entity::mob::Mob;
use crate::world::entity::schedule::Activity;

const SCHEDULE_UPDATE_DELAY: i64 = 20;

type BehaviorsByActivity = Vec<(Activity, Vec<Box<dyn BehaviorControl>>)>;

pub struct Brain {
    memory_slots: Vec<u8>,
    memories: Vec<(MemoryModuleType<Erased>, MemorySlot)>,
    sensors: Vec<Sensor>,
    available_behaviors_by_priority: Vec<(i32, BehaviorsByActivity)>,
    schedule: Option<EnvironmentAttribute<Activity>>,
    activity_requirements: Vec<(Activity, Vec<(MemoryModuleType<Erased>, MemoryStatus)>)>,
    activity_memories_to_erase_when_stopped: Vec<(Activity, Vec<MemoryModuleType<Erased>>)>,
    core_activities: Vec<Activity>,
    active_activities: Vec<Activity>,
    default_activity: Activity,
    last_schedule_update: i64,
    ticking: bool,
    refresh_requested: bool,
}

impl Default for Brain {
    fn default() -> Brain {
        Brain::new()
    }
}

pub struct Provider {
    memory_types: &'static [MemoryModuleType<Erased>],
    sensor_types: &'static [SensorType],
    activities: fn(&Mob) -> Vec<ActivityData>,
}

impl Provider {
    pub fn make_brain(&self, body: &mut Mob, packed_brain: Packed) -> Brain {
        let activities = (self.activities)(body);
        Brain::with(self.memory_types, self.sensor_types, activities, packed_brain.memories, &mut body.entity.random)
    }
}

pub const fn provider(sensor_types: &'static [SensorType], activities: fn(&Mob) -> Vec<ActivityData>) -> Provider {
    Provider { memory_types: &[], sensor_types, activities }
}

pub const fn provider_with_memories(memory_types: &'static [MemoryModuleType<Erased>], sensor_types: &'static [SensorType], activities: fn(&Mob) -> Vec<ActivityData>) -> Provider {
    Provider { memory_types, sensor_types, activities }
}

#[derive(Clone, Debug, Default)]
pub struct Packed {
    pub memories: MemoryMap,
}

impl Packed {
    pub const EMPTY: Packed = Packed { memories: MemoryMap::EMPTY };

    pub fn to_nbt(&self) -> Nbt {
        Nbt::Compound(vec![("memories".into(), self.memories.to_nbt())])
    }

    pub fn decode(nbt: &Nbt) -> DecodeResult<Packed> {
        if let Err(error) = codec::map(nbt) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let memories = record.required(nbt, "memories", MemoryMap::decode);
        record.build(|| Some(Packed { memories: memories? }))
    }
}

fn bucket(activity: Activity, capacity: usize) -> usize {
    let h = activity.hash_code() as u32;
    (h ^ (h >> 16)) as usize & (capacity - 1)
}

impl Brain {
    pub fn new() -> Brain {
        let mut brain = Brain {
            memory_slots: Vec::new(),
            memories: Vec::new(),
            sensors: Vec::new(),
            available_behaviors_by_priority: Vec::new(),
            schedule: None,
            activity_requirements: Vec::new(),
            activity_memories_to_erase_when_stopped: Vec::new(),
            core_activities: Vec::new(),
            active_activities: Vec::new(),
            default_activity: Activity::IDLE,
            last_schedule_update: -9999,
            ticking: false,
            refresh_requested: false,
        };
        brain.set_core_activities(vec![Activity::CORE]);
        brain.use_default_activity();
        brain
    }

    pub fn with(
        memory_types: &[MemoryModuleType<Erased>],
        sensor_types: &[SensorType],
        activities: Vec<ActivityData>,
        memories: MemoryMap,
        random: &mut impl RandomSource,
    ) -> Brain {
        let mut brain = Brain::new();
        for &memory_type in memory_types {
            brain.register_memory(memory_type);
        }
        for &sensor_type in sensor_types {
            let mut new_sensor = sensor_type.create();
            new_sensor.randomly_delay_start(random);
            for memory_type in new_sensor.requires() {
                brain.register_memory(memory_type);
            }
            brain.sensors.push(new_sensor);
        }
        for activity in activities {
            brain.add_activity(activity.activity_type, activity.behavior_priority_pairs, activity.conditions, activity.memories_to_erase_when_stopped);
        }
        for (memory_type, value) in memories.iter() {
            brain.set_memory_internal_expirable(*memory_type, value.clone());
        }
        brain.set_core_activities(vec![Activity::CORE]);
        brain.use_default_activity();
        brain
    }

    fn register_memory(&mut self, memory_type: MemoryModuleType<Erased>) {
        if self.memory_slots.is_empty() {
            self.memory_slots = vec![0; wafer_data::registry::MEMORY_MODULE_TYPE.len()];
        }
        if self.memory_slots[memory_type.id()] == 0 {
            self.memories.push((memory_type, MemorySlot::create()));
            self.memory_slots[memory_type.id()] = self.memories.len() as u8;
        }
    }

    pub fn pack(&self) -> Packed {
        let mut builder = memory_map::Builder::new();
        for (memory_type, slot) in &self.memories {
            if let Some(value) = slot.value()
                && memory_type.can_serialize()
            {
                let value = if slot.can_expire() { ExpirableValue::of_expiring(value.clone(), slot.time_to_live()) } else { ExpirableValue::of(value.clone()) };
                builder.add(*memory_type, value);
            }
        }
        Packed { memories: builder.build() }
    }

    fn get_memory_slot_if_present<U>(&self, memory_type: MemoryModuleType<U>) -> Option<&MemorySlot> {
        match self.memory_slots.get(memory_type.id()) {
            Some(&index) if index > 0 => Some(&self.memories[index as usize - 1].1),
            _ => None,
        }
    }

    fn get_memory_slot_if_present_mut<U>(&mut self, memory_type: MemoryModuleType<U>) -> Option<&mut MemorySlot> {
        match self.memory_slots.get(memory_type.id()) {
            Some(&index) if index > 0 => Some(&mut self.memories[index as usize - 1].1),
            _ => None,
        }
    }

    fn get_memory_slot<U>(&self, memory_type: MemoryModuleType<U>) -> &MemorySlot {
        self.get_memory_slot_if_present(memory_type).unwrap_or_else(|| panic!("Unregistered memory fetched: {}", memory_type.name()))
    }

    pub fn has_memory_value<U>(&self, memory_type: MemoryModuleType<U>) -> bool {
        self.check_memory(memory_type.erase(), MemoryStatus::ValuePresent)
    }

    pub fn clear_memories(&mut self) {
        for (_, slot) in &mut self.memories {
            slot.clear();
        }
    }

    pub fn erase_memory<U>(&mut self, memory_type: MemoryModuleType<U>) {
        if let Some(slot) = self.get_memory_slot_if_present_mut(memory_type) {
            slot.clear();
        }
    }

    pub fn set_memory<U: MemoryValueType>(&mut self, memory_type: MemoryModuleType<U>, value: U::Value) {
        self.set_memory_internal(memory_type.erase(), Some(U::wrap(value)), None);
    }

    pub fn set_memory_with_expiry<U: MemoryValueType>(&mut self, memory_type: MemoryModuleType<U>, value: U::Value, time_to_live: i64) {
        self.set_memory_internal(memory_type.erase(), Some(U::wrap(value)), Some(time_to_live));
    }

    pub fn set_memory_optional<U: MemoryValueType>(&mut self, memory_type: MemoryModuleType<U>, optional_value: Option<U::Value>) {
        self.set_memory_internal(memory_type.erase(), optional_value.map(U::wrap), None);
    }

    fn set_memory_internal_expirable(&mut self, memory_type: MemoryModuleType<Erased>, value: ExpirableValue) {
        self.set_memory_internal(memory_type, Some(value.value), value.time_to_live);
    }

    fn set_memory_internal(&mut self, memory_type: MemoryModuleType<Erased>, value: Option<MemoryValue>, time_to_live: Option<i64>) {
        let Some(slot) = self.get_memory_slot_if_present_mut(memory_type) else { return };
        match value.filter(|value| !value.is_empty_collection()) {
            None => slot.clear(),
            Some(value) => match time_to_live {
                Some(time_to_live) => slot.set_with_expiry(value, time_to_live),
                None => slot.set(value),
            },
        }
    }

    pub fn get_memory<U: MemoryValueType>(&self, memory_type: MemoryModuleType<U>) -> Option<&U::Value> {
        self.get_memory_slot(memory_type).value().map(U::unwrap)
    }

    pub fn get_memory_internal<U: MemoryValueType>(&self, memory_type: MemoryModuleType<U>) -> Option<Option<&U::Value>> {
        self.get_memory_slot_if_present(memory_type).map(|slot| slot.value().map(U::unwrap))
    }

    pub fn get_memory_internal_erased(&self, memory_type: MemoryModuleType<Erased>) -> Option<bool> {
        self.get_memory_slot_if_present(memory_type).map(MemorySlot::has_value)
    }

    pub fn get_time_until_expiry<U>(&self, memory_type: MemoryModuleType<U>) -> i64 {
        self.get_memory_slot(memory_type).time_to_live()
    }

    pub fn is_memory_value<U: MemoryValueType>(&self, memory_type: MemoryModuleType<U>, value: &U::Value) -> bool
    where
        U::Value: PartialEq,
    {
        self.get_memory_slot_if_present(memory_type).and_then(MemorySlot::value).is_some_and(|slot_value| U::unwrap(slot_value) == value)
    }

    pub fn check_memory(&self, memory_type: MemoryModuleType<Erased>, status: MemoryStatus) -> bool {
        match self.get_memory_slot_if_present(memory_type) {
            None => false,
            Some(slot) => {
                status == MemoryStatus::Registered
                    || status == MemoryStatus::ValuePresent && slot.has_value()
                    || status == MemoryStatus::ValueAbsent && !slot.has_value()
            }
        }
    }

    pub fn set_schedule(&mut self, schedule: EnvironmentAttribute<Activity>) {
        self.schedule = Some(schedule);
    }

    pub fn set_core_activities(&mut self, activities: Vec<Activity>) {
        self.core_activities = activities;
    }

    pub fn get_active_activities(&self) -> &[Activity] {
        &self.active_activities
    }

    pub fn get_running_behaviors(&self) -> Vec<String> {
        let mut running_behaviours = Vec::new();
        for (_, behaviours_by_activities) in &self.available_behaviors_by_priority {
            for (_, behaviors) in behaviours_by_activities {
                for behavior in behaviors {
                    if behavior.get_status() == Status::Running {
                        running_behaviours.push(behavior.debug_string());
                    }
                }
            }
        }
        running_behaviours
    }

    pub fn use_default_activity(&mut self) {
        self.set_active_activity(self.default_activity);
    }

    pub fn get_active_non_core_activity(&self) -> Option<Activity> {
        self.active_activities.iter().copied().find(|activity| !self.core_activities.contains(activity))
    }

    pub fn set_active_activity_if_possible(&mut self, activity: Activity) {
        if self.activity_requirements_are_met(activity) {
            self.set_active_activity(activity);
        } else {
            self.use_default_activity();
        }
    }

    fn set_active_activity(&mut self, activity: Activity) {
        if !self.is_active(activity) {
            self.erase_memories_for_other_activites_than(activity);
            self.active_activities.clear();
            self.active_activities.extend_from_slice(&self.core_activities);
            if !self.active_activities.contains(&activity) {
                self.active_activities.push(activity);
            }
        }
    }

    fn erase_memories_for_other_activites_than(&mut self, activity: Activity) {
        let old_activities = self.active_activities.clone();
        for old_activity in old_activities {
            if old_activity != activity
                && let Some(memory_module_types) = self.activity_memories_to_erase_when_stopped.iter().find(|(a, _)| *a == old_activity).map(|(_, memories)| memories.clone())
            {
                for memory_module_type in memory_module_types {
                    self.erase_memory(memory_module_type);
                }
            }
        }
    }

    pub fn update_activity_from_schedule(level: &mut ServerLevel, body: EntityId, game_time: i64, pos: Vec3) {
        let brain = level.get_brain(body);
        if game_time - brain.last_schedule_update > SCHEDULE_UPDATE_DELAY {
            let schedule = brain.schedule;
            let scheduled_activity = match schedule {
                Some(schedule) => level.environment_attributes().get_value(schedule, pos),
                None => Activity::IDLE,
            };
            let brain = level.get_brain_mut(body);
            brain.last_schedule_update = game_time;
            if !brain.active_activities.contains(&scheduled_activity) {
                brain.set_active_activity_if_possible(scheduled_activity);
            }
        }
    }

    pub fn update_activity_from_schedule_detached(&mut self, level: &ServerLevel, game_time: i64, pos: Vec3) {
        if game_time - self.last_schedule_update > SCHEDULE_UPDATE_DELAY {
            let scheduled_activity = match self.schedule {
                Some(schedule) => level.environment_attributes().get_value(schedule, pos),
                None => Activity::IDLE,
            };
            self.last_schedule_update = game_time;
            if !self.active_activities.contains(&scheduled_activity) {
                self.set_active_activity_if_possible(scheduled_activity);
            }
        }
    }

    pub fn set_active_activity_to_first_valid(&mut self, activities: &[Activity]) {
        for &activity in activities {
            if self.activity_requirements_are_met(activity) {
                self.set_active_activity(activity);
                break;
            }
        }
    }

    pub fn set_default_activity(&mut self, activity: Activity) {
        self.default_activity = activity;
    }

    pub fn add_activity(
        &mut self,
        activity: Activity,
        behavior_priority_pairs: Vec<(i32, Box<dyn BehaviorControl>)>,
        conditions: Vec<(MemoryModuleType<Erased>, MemoryStatus)>,
        memories_to_erase_when_stopped: Vec<MemoryModuleType<Erased>>,
    ) {
        match self.activity_requirements.iter_mut().find(|(a, _)| *a == activity) {
            Some((_, requirements)) => *requirements = conditions,
            None => self.activity_requirements.push((activity, conditions)),
        }
        if !memories_to_erase_when_stopped.is_empty() {
            match self.activity_memories_to_erase_when_stopped.iter_mut().find(|(a, _)| *a == activity) {
                Some((_, memories)) => *memories = memories_to_erase_when_stopped,
                None => self.activity_memories_to_erase_when_stopped.push((activity, memories_to_erase_when_stopped)),
            }
        }
        for (priority, behavior) in behavior_priority_pairs {
            for required_memory in behavior.get_required_memories() {
                self.register_memory(required_memory);
            }
            let index = match self.available_behaviors_by_priority.binary_search_by_key(&priority, |(p, _)| *p) {
                Ok(index) => index,
                Err(index) => {
                    self.available_behaviors_by_priority.insert(index, (priority, Vec::new()));
                    index
                }
            };
            let by_activity = &mut self.available_behaviors_by_priority[index].1;
            match by_activity.iter_mut().find(|(a, _)| *a == activity) {
                Some((_, behaviors)) => behaviors.push(behavior),
                None => {
                    by_activity.push((activity, vec![behavior]));
                    let mut capacity = 16;
                    while by_activity.len() > capacity * 3 / 4 {
                        capacity *= 2;
                    }
                    by_activity.sort_by_key(|(a, _)| bucket(*a, capacity));
                }
            }
        }
    }

    pub fn remove_all_behaviors(&mut self) {
        self.available_behaviors_by_priority.clear();
    }

    pub fn is_active(&self, activity: Activity) -> bool {
        self.active_activities.contains(&activity)
    }

    pub fn tick_in_zone(level: &mut ServerLevel, body: EntityId, zone: &str) {
        let _zone = profiler::zone(zone);
        Brain::tick(level, body);
    }

    pub fn tick(level: &mut ServerLevel, body: EntityId) {
        level.get_brain_mut(body).forget_outdated_memories();
        Brain::tick_sensors(level, body);
        let brain = level.get_brain_mut(body);
        brain.ticking = true;
        let mut behaviors = std::mem::take(&mut brain.available_behaviors_by_priority);
        let mut retired_activities = None;
        Brain::start_each_non_running_behavior(level, body, &mut behaviors, &mut retired_activities);
        Brain::tick_each_running_behavior(level, body, &mut behaviors, &mut retired_activities);
        if let Some(brain) = level.get_brain_mut_if_present(body) {
            brain.ticking = false;
            if retired_activities.is_none() {
                brain.available_behaviors_by_priority = behaviors;
            }
        }
    }

    pub fn is_ticking(&self) -> bool {
        self.ticking
    }

    pub fn request_refresh(&mut self) {
        self.refresh_requested = true;
    }

    fn is_activity_active(level: &ServerLevel, body: EntityId, retired_activities: &Option<Vec<Activity>>, activity: Activity) -> bool {
        match retired_activities {
            Some(activities) => activities.contains(&activity),
            None => level.get_brain_if_present(body).is_some_and(|brain| brain.active_activities.contains(&activity)),
        }
    }

    fn refresh_if_requested(level: &mut ServerLevel, body: EntityId, behaviors: &mut [(i32, BehaviorsByActivity)], retired_activities: &mut Option<Vec<Activity>>, timestamp: i64) {
        let Some(brain) = level.get_brain_mut_if_present(body) else { return };
        if !brain.refresh_requested {
            return;
        }
        brain.refresh_requested = false;
        let old_activities = brain.active_activities.clone();
        if retired_activities.is_none() {
            for (_, by_activity) in behaviors.iter_mut() {
                for (_, list) in by_activity.iter_mut() {
                    for behavior in list.iter_mut() {
                        if behavior.get_status() == Status::Running {
                            behavior.do_stop(level, body, timestamp);
                        }
                    }
                }
            }
        }
        level.villager_rebuild_brain(body);
        if let Some(brain) = level.get_brain_mut_if_present(body) {
            brain.ticking = true;
        }
        *retired_activities = Some(retired_activities.take().unwrap_or(old_activities));
    }

    fn tick_sensors(level: &mut ServerLevel, body: EntityId) {
        let mut sensors = std::mem::take(&mut level.get_brain_mut(body).sensors);
        for sensor in &mut sensors {
            sensor.tick(level, body);
        }
        if let Some(brain) = level.get_brain_mut_if_present(body) {
            brain.sensors = sensors;
        }
    }

    fn forget_outdated_memories(&mut self) {
        for (_, slot) in &mut self.memories {
            slot.tick();
        }
    }

    pub fn stop_all(level: &mut ServerLevel, body: EntityId) {
        let timestamp = level.game_time;
        Brain::with_behaviors(level, body, |level, behaviors| {
            for (_, by_activity) in behaviors {
                for (_, list) in by_activity {
                    for behavior in list {
                        if behavior.get_status() == Status::Running {
                            behavior.do_stop(level, body, timestamp);
                        }
                    }
                }
            }
        });
    }

    fn start_each_non_running_behavior(level: &mut ServerLevel, body: EntityId, behaviors: &mut [(i32, BehaviorsByActivity)], retired_activities: &mut Option<Vec<Activity>>) {
        let time = level.game_time;
        for p in 0..behaviors.len() {
            for a in 0..behaviors[p].1.len() {
                let activity = behaviors[p].1[a].0;
                if Brain::is_activity_active(level, body, retired_activities, activity) {
                    for b in 0..behaviors[p].1[a].1.len() {
                        let behavior = &mut behaviors[p].1[a].1[b];
                        if behavior.get_status() == Status::Stopped && level.get_brain_if_present(body).is_some() {
                            behavior.try_start(level, body, time);
                            Brain::refresh_if_requested(level, body, behaviors, retired_activities, time);
                        }
                    }
                }
            }
        }
    }

    fn tick_each_running_behavior(level: &mut ServerLevel, body: EntityId, behaviors: &mut [(i32, BehaviorsByActivity)], retired_activities: &mut Option<Vec<Activity>>) {
        let timestamp = level.game_time;
        for p in 0..behaviors.len() {
            for a in 0..behaviors[p].1.len() {
                for b in 0..behaviors[p].1[a].1.len() {
                    let behavior = &mut behaviors[p].1[a].1[b];
                    if behavior.get_status() == Status::Running && level.get_brain_if_present(body).is_some() {
                        behavior.tick_or_stop(level, body, timestamp);
                        Brain::refresh_if_requested(level, body, behaviors, retired_activities, timestamp);
                    }
                }
            }
        }
    }

    fn with_behaviors(level: &mut ServerLevel, body: EntityId, f: impl FnOnce(&mut ServerLevel, &mut Vec<(i32, BehaviorsByActivity)>)) {
        let Some(brain) = level.get_brain_mut_if_present(body) else { return };
        let mut behaviors = std::mem::take(&mut brain.available_behaviors_by_priority);
        f(level, &mut behaviors);
        if let Some(brain) = level.get_brain_mut_if_present(body) {
            brain.available_behaviors_by_priority = behaviors;
        }
    }

    fn activity_requirements_are_met(&self, activity: Activity) -> bool {
        match self.activity_requirements.iter().find(|(a, _)| *a == activity) {
            None => false,
            Some((_, requirements)) => requirements.iter().all(|&(memory_type, memory_status)| self.check_memory(memory_type, memory_status)),
        }
    }

    #[cfg(test)]
    pub fn seed_for_test(&mut self, seed: i64, sensor_delay: i64) {
        for sensor in &mut self.sensors {
            sensor.set_time_to_tick(sensor_delay);
        }
        for (_, by_activity) in &mut self.available_behaviors_by_priority {
            for (_, behaviors) in by_activity {
                for behavior in behaviors {
                    let behavior: &mut dyn std::any::Any = behavior.as_mut();
                    if let Some(gate) = behavior.downcast_mut::<super::behavior::gate_behavior::GateBehavior>() {
                        gate.set_seed(seed);
                    }
                }
            }
        }
    }

    pub fn is_brain_dead(&self) -> bool {
        self.memories.is_empty() && self.sensors.is_empty() && self.available_behaviors_by_priority.is_empty()
    }
}

impl ServerLevel {
    pub fn get_brain(&self, id: EntityId) -> &Brain {
        &self.living(id).brain
    }

    pub fn get_brain_mut(&mut self, id: EntityId) -> &mut Brain {
        &mut self.living_mut(id).brain
    }

    pub fn get_brain_if_present(&self, id: EntityId) -> Option<&Brain> {
        match self.find_player(id) {
            Some(player) => Some(&player.living.brain),
            None => self.mob(id).map(|mob| &mob.living.brain),
        }
    }

    pub fn get_brain_mut_if_present(&mut self, id: EntityId) -> Option<&mut Brain> {
        if self.is_player(id) {
            return Some(&mut self.player_mut(id).living.brain);
        }
        self.mob_mut(id).map(|mob| &mut mob.living.brain)
    }
}
