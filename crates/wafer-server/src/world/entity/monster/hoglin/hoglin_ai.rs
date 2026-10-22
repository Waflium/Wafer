use wafer_core::BlockPos;

use crate::server::level::ServerLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::entity::ai::activity_data::ActivityData;
use crate::world::entity::ai::behavior::animal_make_love::AnimalMakeLove;
use crate::world::entity::ai::behavior::behavior_control::BehaviorControl;
use crate::world::entity::ai::behavior::declarative::behavior_builder;
use crate::world::entity::ai::behavior::do_nothing::DoNothing;
use crate::world::entity::ai::behavior::look_at_target_sink::LookAtTargetSink;
use crate::world::entity::ai::behavior::move_to_target_sink::MoveToTargetSink;
use crate::world::entity::ai::behavior::{
    baby_follow_adult, become_passive_if_memory_present, behavior_utils, erase_memory_if, melee_attack, random_stroll, run_one, set_entity_look_target_sometimes,
    set_walk_target_away_from, set_walk_target_from_attack_target_if_target_out_of_reach, set_walk_target_from_look_target, start_attacking, stop_attacking_if_target_invalid,
};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::sensing::sensor;
use crate::world::entity::schedule::Activity;
use crate::world::entity::{EntityId, entity_types, sound_event};

const RETREAT_DURATION: IntProvider = IntProvider::Uniform { min_inclusive: 100, max_inclusive: 400 };
const ATTACK_DURATION: i64 = 200;
const DESIRED_DISTANCE_FROM_PIGLIN_WHEN_IDLING: i32 = 8;
const DESIRED_DISTANCE_FROM_PIGLIN_WHEN_RETREATING: i32 = 15;
const ATTACK_INTERVAL: i32 = 40;
const BABY_ATTACK_INTERVAL: i32 = 15;
const REPELLENT_PACIFY_TIME: i32 = 200;
const ADULT_FOLLOW_RANGE: IntProvider = IntProvider::Uniform { min_inclusive: 5, max_inclusive: 16 };
const SPEED_MULTIPLIER_WHEN_AVOIDING_REPELLENT: f32 = 1.0;
const SPEED_MULTIPLIER_WHEN_RETREATING: f32 = 1.3;
const SPEED_MULTIPLIER_WHEN_MAKING_LOVE: f32 = 0.6;
const SPEED_MULTIPLIER_WHEN_IDLING: f32 = 0.4;
const SPEED_MULTIPLIER_WHEN_FOLLOWING_ADULT: f32 = 0.6;

pub fn get_activities() -> Vec<ActivityData> {
    vec![init_core_activity(), init_idle_activity(), init_fight_activity(), init_retreat_activity()]
}

fn init_core_activity() -> ActivityData {
    ActivityData::create(Activity::CORE, 0, vec![Box::new(LookAtTargetSink::new(45, 90)), Box::new(MoveToTargetSink::default())])
}

fn is_adult(level: &ServerLevel, body: EntityId) -> bool {
    !level.is_baby(body)
}

fn init_idle_activity() -> ActivityData {
    ActivityData::create(
        Activity::IDLE,
        10,
        vec![
            Box::new(become_passive_if_memory_present::create(memory_module_type::NEAREST_REPELLENT, REPELLENT_PACIFY_TIME)),
            Box::new(AnimalMakeLove::with_speed(&entity_types::HOGLIN, SPEED_MULTIPLIER_WHEN_MAKING_LOVE, 2)),
            Box::new(set_walk_target_away_from::pos(memory_module_type::NEAREST_REPELLENT, SPEED_MULTIPLIER_WHEN_AVOIDING_REPELLENT, 8, true)),
            Box::new(start_attacking::create(find_nearest_valid_attack_target)),
            Box::new(behavior_builder::sequence(
                behavior_builder::trigger_if(is_adult),
                set_walk_target_away_from::entity(memory_module_type::NEAREST_VISIBLE_ADULT_PIGLIN, SPEED_MULTIPLIER_WHEN_IDLING, DESIRED_DISTANCE_FROM_PIGLIN_WHEN_IDLING, false),
            )),
            Box::new(set_entity_look_target_sometimes::create(8.0, IntProvider::Uniform { min_inclusive: 30, max_inclusive: 60 })),
            Box::new(baby_follow_adult::create(ADULT_FOLLOW_RANGE, SPEED_MULTIPLIER_WHEN_FOLLOWING_ADULT)),
            Box::new(create_idle_movement_behaviors()),
        ],
    )
}

fn init_fight_activity() -> ActivityData {
    ActivityData::create_with_memory(
        Activity::FIGHT,
        10,
        vec![
            Box::new(become_passive_if_memory_present::create(memory_module_type::NEAREST_REPELLENT, REPELLENT_PACIFY_TIME)),
            Box::new(AnimalMakeLove::with_speed(&entity_types::HOGLIN, SPEED_MULTIPLIER_WHEN_MAKING_LOVE, 2)),
            Box::new(set_walk_target_from_attack_target_if_target_out_of_reach::create_with(|_, _| 1.0)),
            Box::new(behavior_builder::sequence(behavior_builder::trigger_if(is_adult), melee_attack::create(ATTACK_INTERVAL))),
            Box::new(behavior_builder::sequence(behavior_builder::trigger_if(|level, body| level.is_baby(body)), melee_attack::create(BABY_ATTACK_INTERVAL))),
            Box::new(stop_attacking_if_target_invalid::create()),
            Box::new(erase_memory_if::create(is_breeding, memory_module_type::ATTACK_TARGET)),
        ],
        memory_module_type::ATTACK_TARGET.erase(),
    )
}

fn init_retreat_activity() -> ActivityData {
    ActivityData::create_with_memory(
        Activity::AVOID,
        10,
        vec![
            Box::new(set_walk_target_away_from::entity(memory_module_type::AVOID_TARGET, SPEED_MULTIPLIER_WHEN_RETREATING, DESIRED_DISTANCE_FROM_PIGLIN_WHEN_RETREATING, false)),
            Box::new(create_idle_movement_behaviors()),
            Box::new(set_entity_look_target_sometimes::create(8.0, IntProvider::Uniform { min_inclusive: 30, max_inclusive: 60 })),
            Box::new(erase_memory_if::create(wants_to_stop_fleeing, memory_module_type::AVOID_TARGET)),
        ],
        memory_module_type::AVOID_TARGET.erase(),
    )
}

fn create_idle_movement_behaviors() -> crate::world::entity::ai::behavior::gate_behavior::GateBehavior {
    let behaviors: Vec<(Box<dyn BehaviorControl>, i32)> = vec![
        (Box::new(random_stroll::stroll(SPEED_MULTIPLIER_WHEN_IDLING)), 2),
        (Box::new(set_walk_target_from_look_target::create(SPEED_MULTIPLIER_WHEN_IDLING, 3)), 2),
        (Box::new(DoNothing::new(30, 60)), 1),
    ];
    run_one::new(behaviors)
}

pub fn update_activity(level: &mut ServerLevel, body: EntityId) {
    let brain = level.get_brain_mut(body);
    let old_activity = brain.get_active_non_core_activity();
    brain.set_active_activity_to_first_valid(&[Activity::FIGHT, Activity::AVOID, Activity::IDLE]);
    let new_activity = brain.get_active_non_core_activity();
    if old_activity != new_activity
        && let Some(sound) = get_sound_for_current_activity(level, body)
    {
        level.make_sound(body, sound_event(sound));
    }
    let aggressive = level.get_brain(body).has_memory_value(memory_module_type::ATTACK_TARGET);
    level.mob_mut(body).expect("a hoglin").set_aggressive(aggressive);
}

pub fn on_hit_target(level: &mut ServerLevel, attacker_body: EntityId, target: EntityId) {
    if !level.is_baby(attacker_body) {
        if level.entity(target).entity_type == entity_types::PIGLIN.id && piglins_outnumber_hoglins(level, attacker_body) {
            set_avoid_target(level, attacker_body, target);
            broadcast_retreat(level, attacker_body, target);
        } else {
            broadcast_attack_target(level, attacker_body, target);
        }
    }
}

fn broadcast_retreat(level: &mut ServerLevel, body: EntityId, target: EntityId) {
    for hoglin in get_visible_adult_hoglins(level, body) {
        retreat_from_nearest_target(level, hoglin, target);
    }
}

fn retreat_from_nearest_target(level: &mut ServerLevel, body: EntityId, new_avoid_target: EntityId) {
    let mut nearest = new_avoid_target;
    let avoid_target = level.get_brain(body).get_memory(memory_module_type::AVOID_TARGET).copied();
    nearest = behavior_utils::get_nearest_target(level, body, avoid_target, nearest);
    let attack_target = level.get_brain(body).get_memory(memory_module_type::ATTACK_TARGET).copied();
    nearest = behavior_utils::get_nearest_target(level, body, attack_target, nearest);
    set_avoid_target(level, body, nearest);
}

fn set_avoid_target(level: &mut ServerLevel, body: EntityId, avoid_target: EntityId) {
    let duration = RETREAT_DURATION.sample(&mut level.random);
    let brain = level.get_brain_mut(body);
    brain.erase_memory(memory_module_type::ATTACK_TARGET);
    brain.erase_memory(memory_module_type::WALK_TARGET);
    brain.set_memory_with_expiry(memory_module_type::AVOID_TARGET, avoid_target, duration as i64);
}

fn find_nearest_valid_attack_target(level: &mut ServerLevel, body: EntityId) -> Option<EntityId> {
    if !is_pacified(level, body) && !is_breeding(level, body) { level.get_brain(body).get_memory(memory_module_type::NEAREST_VISIBLE_ATTACKABLE_PLAYER).copied() } else { None }
}

pub fn is_pos_near_nearest_repellent(mob: &crate::world::entity::mob::Mob, pos: BlockPos) -> bool {
    mob.living.brain.get_memory(memory_module_type::NEAREST_REPELLENT).is_some_and(|repellent_pos| repellent_pos.closer_than(pos, 8.0))
}

fn wants_to_stop_fleeing(level: &ServerLevel, body: EntityId) -> bool {
    is_adult(level, body) && !piglins_outnumber_hoglins(level, body)
}

fn piglins_outnumber_hoglins(level: &ServerLevel, body: EntityId) -> bool {
    if level.is_baby(body) {
        return false;
    }
    let brain = level.get_brain(body);
    let piglin_count = brain.get_memory(memory_module_type::VISIBLE_ADULT_PIGLIN_COUNT).copied().unwrap_or(0);
    let hoglin_count = brain.get_memory(memory_module_type::VISIBLE_ADULT_HOGLIN_COUNT).copied().unwrap_or(0) + 1;
    piglin_count > hoglin_count
}

pub fn was_hurt_by(level: &mut ServerLevel, body: EntityId, attacker: EntityId) {
    let brain = level.get_brain_mut(body);
    brain.erase_memory(memory_module_type::PACIFIED);
    brain.erase_memory(memory_module_type::BREED_TARGET);
    if level.is_baby(body) {
        retreat_from_nearest_target(level, body, attacker);
    } else {
        maybe_retaliate(level, body, attacker);
    }
}

fn maybe_retaliate(level: &mut ServerLevel, body: EntityId, attacker: EntityId) {
    let attacker_type = level.entity(attacker).entity_type;
    if level.get_brain(body).is_active(Activity::AVOID) && attacker_type == entity_types::PIGLIN.id {
        return;
    }
    if attacker_type == entity_types::HOGLIN.id {
        return;
    }
    if behavior_utils::is_other_target_much_further_away_than_current_attack_target(level, body, attacker, 4.0) {
        return;
    }
    if sensor::is_entity_attackable(level, body, attacker) {
        set_attack_target(level, body, attacker);
        broadcast_attack_target(level, body, attacker);
    }
}

fn set_attack_target(level: &mut ServerLevel, body: EntityId, target: EntityId) {
    let brain = level.get_brain_mut(body);
    brain.erase_memory(memory_module_type::CANT_REACH_WALK_TARGET_SINCE);
    brain.erase_memory(memory_module_type::BREED_TARGET);
    brain.set_memory_with_expiry(memory_module_type::ATTACK_TARGET, target, ATTACK_DURATION);
}

fn broadcast_attack_target(level: &mut ServerLevel, body: EntityId, target: EntityId) {
    for hoglin in get_visible_adult_hoglins(level, body) {
        set_attack_target_if_closer_than_current(level, hoglin, target);
    }
}

fn set_attack_target_if_closer_than_current(level: &mut ServerLevel, body: EntityId, new_target: EntityId) {
    if !is_pacified(level, body) {
        let current_target = level.get_brain(body).get_memory(memory_module_type::ATTACK_TARGET).copied();
        let nearest = behavior_utils::get_nearest_target(level, body, current_target, new_target);
        set_attack_target(level, body, nearest);
    }
}

pub fn get_sound_for_current_activity(level: &ServerLevel, body: EntityId) -> Option<&'static str> {
    level.get_brain(body).get_active_non_core_activity().map(|activity| get_sound_for_activity(level, body, activity))
}

fn get_sound_for_activity(level: &ServerLevel, body: EntityId, activity: Activity) -> &'static str {
    if activity == Activity::AVOID || level.hoglin_is_converting(body) {
        "minecraft:entity.hoglin.retreat"
    } else if activity == Activity::FIGHT {
        "minecraft:entity.hoglin.angry"
    } else if is_near_repellent(level, body) {
        "minecraft:entity.hoglin.retreat"
    } else {
        "minecraft:entity.hoglin.ambient"
    }
}

fn get_visible_adult_hoglins(level: &ServerLevel, body: EntityId) -> Vec<EntityId> {
    level.get_brain(body).get_memory(memory_module_type::NEAREST_VISIBLE_ADULT_HOGLINS).cloned().unwrap_or_default()
}

fn is_near_repellent(level: &ServerLevel, body: EntityId) -> bool {
    level.get_brain(body).has_memory_value(memory_module_type::NEAREST_REPELLENT)
}

fn is_breeding(level: &ServerLevel, body: EntityId) -> bool {
    level.get_brain(body).has_memory_value(memory_module_type::BREED_TARGET)
}

pub fn is_pacified(level: &ServerLevel, body: EntityId) -> bool {
    level.get_brain(body).has_memory_value(memory_module_type::PACIFIED)
}
