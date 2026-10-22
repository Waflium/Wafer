use std::cell::Cell;
use std::rc::Rc;

use super::long_jump::LongJump;
use super::shoot::Shoot;
use super::shoot_when_stuck::ShootWhenStuck;
use super::slide::Slide;
use crate::server::level::ServerLevel;
use crate::world::entity::ai::activity_data::ActivityData;
use crate::world::entity::ai::behavior::behavior::{Behavior, BehaviorState};
use crate::world::entity::ai::behavior::behavior_control::BehaviorControl;
use crate::world::entity::ai::behavior::do_nothing::DoNothing;
use crate::world::entity::ai::behavior::look_at_target_sink::LookAtTargetSink;
use crate::world::entity::ai::behavior::move_to_target_sink::MoveToTargetSink;
use crate::world::entity::ai::behavior::swim::Swim;
use crate::world::entity::ai::behavior::{random_stroll, run_one, start_attacking, stop_attacking_if_target_invalid};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::ai::sensing::sensor;
use crate::world::entity::data::Pose;
use crate::world::entity::schedule::Activity;
use crate::world::entity::{EntityId, sound_event};

const TICKS_TO_REMEMBER_SEEN_TARGET: i32 = 100;

pub fn get_activities() -> Vec<ActivityData> {
    vec![init_core_activity(), init_idle_activity(), init_fight_activity()]
}

fn init_core_activity() -> ActivityData {
    ActivityData::create(Activity::CORE, 0, vec![Box::new(Swim::new(0.8)), Box::new(LookAtTargetSink::new(45, 90))])
}

fn init_idle_activity() -> ActivityData {
    let movement: Vec<(Box<dyn BehaviorControl>, i32)> = vec![(Box::new(DoNothing::new(20, 100)), 1), (Box::new(random_stroll::stroll(0.6)), 2)];
    ActivityData::create_pairs(
        Activity::IDLE,
        vec![
            (0, Box::new(start_attacking::create(|level, breeze| level.get_brain(breeze).get_memory(memory_module_type::NEAREST_ATTACKABLE).copied()))),
            (1, Box::new(start_attacking::create(|level, breeze| level.breeze_get_hurt_by(breeze)))),
            (2, Box::new(SlideToTargetSink::new(20, 40))),
            (3, Box::new(run_one::new(movement))),
        ],
    )
}

fn init_fight_activity() -> ActivityData {
    let positives_left = Rc::new(Cell::new(0));
    let was_entity_attackable_last_n_ticks = move |level: &mut ServerLevel, body: EntityId, target: EntityId| {
        if sensor::is_entity_attackable(level, body, target) {
            positives_left.set(TICKS_TO_REMEMBER_SEEN_TARGET);
            true
        } else {
            positives_left.set(positives_left.get() - 1);
            positives_left.get() >= 0
        }
    };
    ActivityData::create_pairs_with_conditions(
        Activity::FIGHT,
        vec![
            (0, Box::new(stop_attacking_if_target_invalid::create_when(move |level, body, target| !was_entity_attackable_last_n_ticks(level, body, target)))),
            (1, Box::new(Shoot::default())),
            (2, Box::new(LongJump::default())),
            (3, Box::new(ShootWhenStuck::default())),
            (4, Box::new(Slide::default())),
        ],
        vec![(memory_module_type::ATTACK_TARGET.erase(), MemoryStatus::ValuePresent), (memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent)],
    )
}

pub fn update_activity(level: &mut ServerLevel, body: EntityId) {
    level.get_brain_mut(body).set_active_activity_to_first_valid(&[Activity::FIGHT, Activity::IDLE]);
}

pub struct SlideToTargetSink {
    move_to_target_sink: MoveToTargetSink,
}

impl SlideToTargetSink {
    pub fn new(min_timeout: i32, max_timeout: i32) -> SlideToTargetSink {
        SlideToTargetSink { move_to_target_sink: MoveToTargetSink::new(min_timeout, max_timeout) }
    }
}

impl Behavior for SlideToTargetSink {
    fn behavior(&self) -> &BehaviorState {
        self.move_to_target_sink.behavior()
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        self.move_to_target_sink.behavior_mut()
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        self.move_to_target_sink.check_extra_start_conditions(level, body)
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) -> bool {
        self.move_to_target_sink.can_still_use(level, body, timestamp)
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        self.move_to_target_sink.start(level, body, timestamp);
        level.entity_play_sound(body, sound_event("minecraft:entity.breeze.slide"), 1.0, 1.0);
        level.set_entity_pose(body, Pose::Sliding);
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        self.move_to_target_sink.tick(level, body, timestamp);
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        self.move_to_target_sink.stop(level, body, timestamp);
        level.set_entity_pose(body, Pose::Standing);
        if level.get_brain(body).has_memory_value(memory_module_type::ATTACK_TARGET) {
            level.get_brain_mut(body).set_memory_with_expiry(memory_module_type::BREEZE_SHOOT, (), 60);
        }
    }
}
