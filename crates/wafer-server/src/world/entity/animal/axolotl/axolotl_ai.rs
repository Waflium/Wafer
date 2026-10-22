use wafer_data::fluid_tags;

use super::play_dead::PlayDead;
use super::validate_play_dead;
use crate::server::level::ServerLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::entity::ai::activity_data::ActivityData;
use crate::world::entity::ai::behavior::animal_make_love::AnimalMakeLove;
use crate::world::entity::ai::behavior::behavior_control::BehaviorControl;
use crate::world::entity::ai::behavior::count_down_cooldown_ticks::CountDownCooldownTicks;
use crate::world::entity::ai::behavior::declarative::behavior_builder;
use crate::world::entity::ai::behavior::follow_temptation::FollowTemptation;
use crate::world::entity::ai::behavior::gate_behavior::{GateBehavior, OrderPolicy, RunningPolicy};
use crate::world::entity::ai::behavior::look_at_target_sink::LookAtTargetSink;
use crate::world::entity::ai::behavior::move_to_target_sink::MoveToTargetSink;
use crate::world::entity::ai::behavior::{
    baby_follow_adult, behavior_utils, erase_memory_if, melee_attack, random_stroll, run_one, set_entity_look_target_sometimes, set_walk_target_from_attack_target_if_target_out_of_reach,
    set_walk_target_from_look_target, start_attacking, stop_attacking_if_target_invalid, try_find_liquid,
};
use crate::world::entity::ai::brain::Brain;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::schedule::Activity;
use crate::world::entity::{EntityId, entity_types};
use crate::world::entity::ai::util::goal_utils;

const ADULT_FOLLOW_RANGE: IntProvider = IntProvider::Uniform { min_inclusive: 5, max_inclusive: 16 };
const SPEED_MULTIPLIER_WHEN_MAKING_LOVE: f32 = 0.2;
const SPEED_MULTIPLIER_ON_LAND: f32 = 0.15;
const SPEED_MULTIPLIER_WHEN_IDLING_IN_WATER: f32 = 0.5;
const SPEED_MULTIPLIER_WHEN_CHASING_IN_WATER: f32 = 0.6;
const SPEED_MULTIPLIER_WHEN_FOLLOWING_ADULT_IN_WATER: f32 = 0.6;

pub fn get_activities() -> Vec<ActivityData> {
    vec![init_core_activity(), init_idle_activity(), init_fight_activity(), init_play_dead_activity()]
}

fn init_play_dead_activity() -> ActivityData {
    ActivityData::new(
        Activity::PLAY_DEAD,
        vec![(0, Box::new(PlayDead::default())), (1, Box::new(erase_memory_if::create(behavior_utils::is_breeding, memory_module_type::PLAY_DEAD_TICKS)))],
        vec![(memory_module_type::PLAY_DEAD_TICKS.erase(), MemoryStatus::ValuePresent)],
        vec![memory_module_type::PLAY_DEAD_TICKS.erase()],
    )
}

fn init_fight_activity() -> ActivityData {
    ActivityData::create_with_memory(
        Activity::FIGHT,
        0,
        vec![
            Box::new(stop_attacking_if_target_invalid::create_with_callback(ServerLevel::axolotl_on_stop_attacking)),
            Box::new(set_walk_target_from_attack_target_if_target_out_of_reach::create_with(get_speed_modifier_chasing)),
            Box::new(melee_attack::create(20)),
            Box::new(erase_memory_if::create(behavior_utils::is_breeding, memory_module_type::ATTACK_TARGET)),
        ],
        memory_module_type::ATTACK_TARGET.erase(),
    )
}

fn init_core_activity() -> ActivityData {
    ActivityData::create(
        Activity::CORE,
        0,
        vec![
            Box::new(LookAtTargetSink::new(45, 90)),
            Box::new(MoveToTargetSink::default()),
            Box::new(validate_play_dead::create()),
            Box::new(CountDownCooldownTicks::new(memory_module_type::TEMPTATION_COOLDOWN_TICKS)),
        ],
    )
}

fn init_idle_activity() -> ActivityData {
    let follow: Vec<(Box<dyn BehaviorControl>, i32)> = vec![
        (Box::new(FollowTemptation::new(get_speed_modifier)), 1),
        (Box::new(baby_follow_adult::create_with(ADULT_FOLLOW_RANGE, get_speed_modifier_following_adult, memory_module_type::NEAREST_VISIBLE_ADULT, false)), 1),
    ];
    let stroll: Vec<(Box<dyn BehaviorControl>, i32)> = vec![
        (Box::new(random_stroll::swim(SPEED_MULTIPLIER_WHEN_IDLING_IN_WATER)), 2),
        (Box::new(random_stroll::stroll_from_water(SPEED_MULTIPLIER_ON_LAND, false)), 2),
        (Box::new(set_walk_target_from_look_target::create_with(can_set_walk_target_from_look_target, get_speed_modifier, 3)), 3),
        (Box::new(behavior_builder::trigger_if(|level, body| level.entity(body).is_in_water())), 5),
        (Box::new(behavior_builder::trigger_if(|level, body| level.entity(body).on_ground)), 5),
    ];
    ActivityData::create_pairs(
        Activity::IDLE,
        vec![
            (0, Box::new(set_entity_look_target_sometimes::create_for_type(&entity_types::PLAYER, 6.0, IntProvider::Uniform { min_inclusive: 30, max_inclusive: 60 }))),
            (1, Box::new(AnimalMakeLove::with_speed(&entity_types::AXOLOTL, SPEED_MULTIPLIER_WHEN_MAKING_LOVE, 2))),
            (2, Box::new(run_one::new(follow))),
            (3, Box::new(start_attacking::create(find_nearest_valid_attack_target))),
            (3, Box::new(try_find_liquid::create(6, SPEED_MULTIPLIER_ON_LAND, &fluid_tags::AXOLOTL_TRIES_TO_FIND))),
            (
                4,
                Box::new(GateBehavior::new(
                    vec![(memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent)],
                    Vec::new(),
                    OrderPolicy::Ordered,
                    RunningPolicy::TryAll,
                    stroll,
                )),
            ),
        ],
    )
}

fn can_set_walk_target_from_look_target(level: &ServerLevel, body: EntityId) -> bool {
    match level.get_brain(body).get_memory(memory_module_type::LOOK_TARGET) {
        Some(look_target) => {
            let pos = look_target.current_block_position(level);
            goal_utils::is_water(level, pos) == level.entity(body).is_in_water()
        }
        None => false,
    }
}

pub fn update_activity(brain: &mut Brain) {
    let old_activity = brain.get_active_non_core_activity();
    if old_activity != Some(Activity::PLAY_DEAD) {
        brain.set_active_activity_to_first_valid(&[Activity::PLAY_DEAD, Activity::FIGHT, Activity::IDLE]);
        if old_activity == Some(Activity::FIGHT) && brain.get_active_non_core_activity() != Some(Activity::FIGHT) {
            brain.set_memory_with_expiry(memory_module_type::HAS_HUNTING_COOLDOWN, true, 2400);
        }
    }
}

fn get_speed_modifier_chasing(level: &ServerLevel, mob: EntityId) -> f32 {
    if level.entity(mob).is_in_water() { SPEED_MULTIPLIER_WHEN_CHASING_IN_WATER } else { SPEED_MULTIPLIER_ON_LAND }
}

fn get_speed_modifier_following_adult(level: &ServerLevel, mob: EntityId) -> f32 {
    if level.entity(mob).is_in_water() { SPEED_MULTIPLIER_WHEN_FOLLOWING_ADULT_IN_WATER } else { SPEED_MULTIPLIER_ON_LAND }
}

fn get_speed_modifier(level: &ServerLevel, mob: EntityId) -> f32 {
    if level.entity(mob).is_in_water() { SPEED_MULTIPLIER_WHEN_IDLING_IN_WATER } else { SPEED_MULTIPLIER_ON_LAND }
}

fn find_nearest_valid_attack_target(level: &mut ServerLevel, body: EntityId) -> Option<EntityId> {
    if behavior_utils::is_breeding(level, body) { None } else { level.get_brain(body).get_memory(memory_module_type::NEAREST_ATTACKABLE).copied() }
}
