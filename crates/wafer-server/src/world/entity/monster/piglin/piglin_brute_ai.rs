use wafer_core::random::RandomSource;

use super::piglin_ai;
use crate::core::global_pos::GlobalPos;
use crate::server::level::ServerLevel;
use crate::world::entity::ai::activity_data::ActivityData;
use crate::world::entity::ai::behavior::behavior_control::BehaviorControl;
use crate::world::entity::ai::behavior::do_nothing::DoNothing;
use crate::world::entity::ai::behavior::gate_behavior::GateBehavior;
use crate::world::entity::ai::behavior::look_at_target_sink::LookAtTargetSink;
use crate::world::entity::ai::behavior::move_to_target_sink::MoveToTargetSink;
use crate::world::entity::ai::behavior::{
    behavior_utils, interact_with, interact_with_door, melee_attack, random_stroll, run_one, set_entity_look_target, set_look_and_interact,
    set_walk_target_from_attack_target_if_target_out_of_reach, start_attacking, stop_attacking_if_target_invalid, stop_being_angry_if_target_dead, stroll_around_poi, stroll_to_poi,
};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::sensing::sensor;
use crate::world::entity::schedule::Activity;
use crate::world::entity::{EntityId, entity_types};

const MELEE_ATTACK_COOLDOWN: i32 = 20;
const ACTIVITY_SOUND_LIKELIHOOD_PER_TICK: f64 = 0.0125;
const MAX_LOOK_DIST: f32 = 8.0;
const INTERACTION_RANGE: i32 = 8;
const SPEED_MULTIPLIER_WHEN_IDLING: f32 = 0.6;
const HOME_CLOSE_ENOUGH_DISTANCE: i32 = 2;
const HOME_TOO_FAR_DISTANCE: i32 = 100;
const HOME_STROLL_AROUND_DISTANCE: i32 = 5;

pub fn get_activities() -> Vec<ActivityData> {
    vec![init_core_activity(), init_idle_activity(), init_fight_activity()]
}

pub fn init_memories(level: &mut ServerLevel, body: EntityId) {
    let current_global_pos = GlobalPos::of(level.dimension, level.entity(body).block_position());
    level.get_brain_mut(body).set_memory(memory_module_type::HOME, current_global_pos);
}

fn init_core_activity() -> ActivityData {
    ActivityData::create(
        Activity::CORE,
        0,
        vec![Box::new(LookAtTargetSink::new(45, 90)), Box::new(MoveToTargetSink::default()), Box::new(interact_with_door::create()), Box::new(stop_being_angry_if_target_dead::create())],
    )
}

fn init_idle_activity() -> ActivityData {
    ActivityData::create(
        Activity::IDLE,
        10,
        vec![
            Box::new(start_attacking::create(find_nearest_valid_attack_target)),
            Box::new(create_idle_look_behaviors()),
            Box::new(create_idle_movement_behaviors()),
            Box::new(set_look_and_interact::create(&entity_types::PLAYER, 4)),
        ],
    )
}

fn init_fight_activity() -> ActivityData {
    ActivityData::create_with_memory(
        Activity::FIGHT,
        10,
        vec![
            Box::new(stop_attacking_if_target_invalid::create_when(|level, body, target| !is_nearest_valid_attack_target(level, body, target))),
            Box::new(set_walk_target_from_attack_target_if_target_out_of_reach::create_with(|_, _| 1.0)),
            Box::new(melee_attack::create(MELEE_ATTACK_COOLDOWN)),
        ],
        memory_module_type::ATTACK_TARGET.erase(),
    )
}

fn create_idle_look_behaviors() -> GateBehavior {
    let behaviors: Vec<(Box<dyn BehaviorControl>, i32)> = vec![
        (Box::new(set_entity_look_target::create_for_type(&entity_types::PLAYER, MAX_LOOK_DIST)), 1),
        (Box::new(set_entity_look_target::create_for_type(&entity_types::PIGLIN, MAX_LOOK_DIST)), 1),
        (Box::new(set_entity_look_target::create_for_type(&entity_types::PIGLIN_BRUTE, MAX_LOOK_DIST)), 1),
        (Box::new(set_entity_look_target::create_with(|_, _| true, MAX_LOOK_DIST)), 1),
        (Box::new(DoNothing::new(30, 60)), 1),
    ];
    run_one::new(behaviors)
}

fn create_idle_movement_behaviors() -> GateBehavior {
    let behaviors: Vec<(Box<dyn BehaviorControl>, i32)> = vec![
        (Box::new(random_stroll::stroll(SPEED_MULTIPLIER_WHEN_IDLING)), 2),
        (Box::new(interact_with::of(&entity_types::PIGLIN, INTERACTION_RANGE, memory_module_type::INTERACTION_TARGET, SPEED_MULTIPLIER_WHEN_IDLING, 2)), 2),
        (Box::new(interact_with::of(&entity_types::PIGLIN_BRUTE, INTERACTION_RANGE, memory_module_type::INTERACTION_TARGET, SPEED_MULTIPLIER_WHEN_IDLING, 2)), 2),
        (Box::new(stroll_to_poi::create(memory_module_type::HOME, SPEED_MULTIPLIER_WHEN_IDLING, HOME_CLOSE_ENOUGH_DISTANCE, HOME_TOO_FAR_DISTANCE)), 2),
        (Box::new(stroll_around_poi::create(memory_module_type::HOME, SPEED_MULTIPLIER_WHEN_IDLING, HOME_STROLL_AROUND_DISTANCE)), 2),
        (Box::new(DoNothing::new(30, 60)), 1),
    ];
    run_one::new(behaviors)
}

pub fn update_activity(level: &mut ServerLevel, body: EntityId) {
    let brain = level.get_brain_mut(body);
    let old_activity = brain.get_active_non_core_activity();
    brain.set_active_activity_to_first_valid(&[Activity::FIGHT, Activity::IDLE]);
    let new_activity = brain.get_active_non_core_activity();
    if old_activity != new_activity {
        play_activity_sound(level, body);
    }
    let aggressive = level.get_brain(body).has_memory_value(memory_module_type::ATTACK_TARGET);
    level.mob_mut(body).expect("a piglin brute").set_aggressive(aggressive);
}

fn is_nearest_valid_attack_target(level: &mut ServerLevel, body: EntityId, target: EntityId) -> bool {
    find_nearest_valid_attack_target(level, body) == Some(target)
}

fn find_nearest_valid_attack_target(level: &mut ServerLevel, body: EntityId) -> Option<EntityId> {
    let angry_at = behavior_utils::get_living_entity_from_uuid_memory(level, body, memory_module_type::ANGRY_AT);
    if let Some(angry_at) = angry_at
        && sensor::is_entity_attackable_ignoring_line_of_sight(level, body, angry_at)
    {
        return Some(angry_at);
    }
    let brain = level.get_brain(body);
    brain.get_memory(memory_module_type::NEAREST_VISIBLE_ATTACKABLE_PLAYER).or_else(|| brain.get_memory(memory_module_type::NEAREST_VISIBLE_NEMESIS)).copied()
}

pub fn was_hurt_by(level: &mut ServerLevel, body: EntityId, attacker: EntityId) {
    if !level.is_abstract_piglin(attacker) {
        piglin_ai::maybe_retaliate(level, body, attacker);
    }
}

pub fn maybe_play_activity_sound(level: &mut ServerLevel, body: EntityId) {
    if (level.random.next_float() as f64) < ACTIVITY_SOUND_LIKELIHOOD_PER_TICK {
        play_activity_sound(level, body);
    }
}

fn play_activity_sound(level: &mut ServerLevel, body: EntityId) {
    if level.get_brain(body).get_active_non_core_activity() == Some(Activity::FIGHT) {
        level.piglin_brute_play_angry_sound(body);
    }
}
