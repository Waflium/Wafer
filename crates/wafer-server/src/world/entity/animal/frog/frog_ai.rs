use wafer_core::BlockPos;
use wafer_core::random::RandomSource;
use wafer_data::{block_tags, blocks, fluid_tags, item_tags};

use super::shoot_tongue::ShootTongue;
use crate::server::level::ServerLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::entity::ai::activity_data::ActivityData;
use crate::world::entity::ai::behavior::animal_make_love::AnimalMakeLove;
use crate::world::entity::ai::behavior::animal_panic::AnimalPanic;
use crate::world::entity::ai::behavior::behavior_control::BehaviorControl;
use crate::world::entity::ai::behavior::count_down_cooldown_ticks::CountDownCooldownTicks;
use crate::world::entity::ai::behavior::croak::Croak;
use crate::world::entity::ai::behavior::declarative::behavior_builder;
use crate::world::entity::ai::behavior::follow_temptation::FollowTemptation;
use crate::world::entity::ai::behavior::gate_behavior::{GateBehavior, OrderPolicy, RunningPolicy};
use crate::world::entity::ai::behavior::long_jump_mid_jump::LongJumpMidJump;
use crate::world::entity::ai::behavior::long_jump_to_preferred_block::LongJumpToPreferredBlock;
use crate::world::entity::ai::behavior::long_jump_to_random_pos;
use crate::world::entity::ai::behavior::look_at_target_sink::LookAtTargetSink;
use crate::world::entity::ai::behavior::move_to_target_sink::MoveToTargetSink;
use crate::world::entity::ai::behavior::{
    behavior_utils, random_stroll, run_one, set_entity_look_target_sometimes, set_walk_target_from_look_target, start_attacking, stop_attacking_if_target_invalid, try_find_land,
    try_find_land_near_liquid, try_lay_spawn_on_fluid_near_land,
};
use crate::world::entity::ai::brain::Brain;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::schedule::Activity;
use crate::world::entity::{EntityId, entity_types};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::path_navigation_region::PathNavigationRegion;
use crate::world::level::pathfinder::path_type::PathType;
use crate::world::level::pathfinder::pathfinding_context::PathfindingContext;
use crate::world::level::pathfinder::walk_node_evaluator::get_path_type_static;

const SPEED_MULTIPLIER_WHEN_PANICKING: f32 = 2.0;
const SPEED_MULTIPLIER_WHEN_IDLING: f32 = 1.0;
const SPEED_MULTIPLIER_ON_LAND: f32 = 1.0;
const SPEED_MULTIPLIER_IN_WATER: f32 = 0.75;
const TIME_BETWEEN_LONG_JUMPS: IntProvider = IntProvider::Uniform { min_inclusive: 100, max_inclusive: 140 };
const MAX_LONG_JUMP_HEIGHT: i32 = 2;
const MAX_LONG_JUMP_WIDTH: i32 = 4;
const MAX_JUMP_VELOCITY_MULTIPLIER: f32 = 3.5714288;
const SPEED_MULTIPLIER_WHEN_TEMPTED: f32 = 1.25;

pub fn init_memories(brain: &mut Brain, random: &mut (impl RandomSource + ?Sized)) {
    brain.set_memory(memory_module_type::LONG_JUMP_COOLDOWN_TICKS, TIME_BETWEEN_LONG_JUMPS.sample(random));
}

pub fn get_activities() -> Vec<ActivityData> {
    vec![init_core_activity(), init_idle_activity(), init_swim_activity(), init_lay_spawn_activity(), init_tongue_activity(), init_jump_activity()]
}

fn init_core_activity() -> ActivityData {
    ActivityData::create(
        Activity::CORE,
        0,
        vec![
            Box::new(AnimalPanic::new(SPEED_MULTIPLIER_WHEN_PANICKING)),
            Box::new(LookAtTargetSink::new(45, 90)),
            Box::new(MoveToTargetSink::default()),
            Box::new(CountDownCooldownTicks::new(memory_module_type::TEMPTATION_COOLDOWN_TICKS)),
            Box::new(CountDownCooldownTicks::new(memory_module_type::LONG_JUMP_COOLDOWN_TICKS)),
        ],
    )
}

fn look_at_players() -> Box<dyn BehaviorControl> {
    Box::new(set_entity_look_target_sometimes::create_for_type(&entity_types::PLAYER, 6.0, IntProvider::Uniform { min_inclusive: 30, max_inclusive: 60 }))
}

fn start_attacking() -> Box<dyn BehaviorControl> {
    Box::new(start_attacking::create_with(|level, body| can_attack(level, body), |level, body| level.get_brain(body).get_memory(memory_module_type::NEAREST_ATTACKABLE).copied()))
}

fn init_idle_activity() -> ActivityData {
    let stroll: Vec<(Box<dyn BehaviorControl>, i32)> = vec![
        (Box::new(random_stroll::stroll(SPEED_MULTIPLIER_WHEN_IDLING)), 1),
        (Box::new(set_walk_target_from_look_target::create(SPEED_MULTIPLIER_WHEN_IDLING, 3)), 1),
        (Box::new(Croak::default()), 3),
        (Box::new(behavior_builder::trigger_if(|level, body| level.entity(body).on_ground)), 2),
    ];
    ActivityData::create_pairs_with_conditions(
        Activity::IDLE,
        vec![
            (0, look_at_players()),
            (0, Box::new(AnimalMakeLove::new(&entity_types::FROG))),
            (1, Box::new(FollowTemptation::new(|_, _| SPEED_MULTIPLIER_WHEN_TEMPTED))),
            (2, start_attacking()),
            (3, Box::new(try_find_land::create(6, SPEED_MULTIPLIER_ON_LAND))),
            (4, Box::new(run_one::with_entry_condition(vec![(memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent)], stroll))),
        ],
        vec![
            (memory_module_type::LONG_JUMP_MID_JUMP.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::IS_IN_WATER.erase(), MemoryStatus::ValueAbsent),
        ],
    )
}

fn init_swim_activity() -> ActivityData {
    let swim: Vec<(Box<dyn BehaviorControl>, i32)> = vec![
        (Box::new(random_stroll::swim(SPEED_MULTIPLIER_IN_WATER)), 1),
        (Box::new(random_stroll::stroll_from_water(SPEED_MULTIPLIER_WHEN_IDLING, true)), 1),
        (Box::new(set_walk_target_from_look_target::create(SPEED_MULTIPLIER_WHEN_IDLING, 3)), 1),
        (Box::new(behavior_builder::trigger_if(|level, body| level.entity(body).is_in_water())), 5),
    ];
    ActivityData::create_pairs_with_conditions(
        Activity::SWIM,
        vec![
            (0, look_at_players()),
            (1, Box::new(FollowTemptation::new(|_, _| SPEED_MULTIPLIER_WHEN_TEMPTED))),
            (2, start_attacking()),
            (3, Box::new(try_find_land::create(8, 1.5))),
            (
                5,
                Box::new(GateBehavior::new(
                    vec![(memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent)],
                    Vec::new(),
                    OrderPolicy::Ordered,
                    RunningPolicy::TryAll,
                    swim,
                )),
            ),
        ],
        vec![
            (memory_module_type::LONG_JUMP_MID_JUMP.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::IS_IN_WATER.erase(), MemoryStatus::ValuePresent),
        ],
    )
}

fn init_lay_spawn_activity() -> ActivityData {
    let stroll: Vec<(Box<dyn BehaviorControl>, i32)> = vec![
        (Box::new(random_stroll::stroll(SPEED_MULTIPLIER_WHEN_IDLING)), 2),
        (Box::new(set_walk_target_from_look_target::create(SPEED_MULTIPLIER_WHEN_IDLING, 3)), 1),
        (Box::new(Croak::default()), 2),
        (Box::new(behavior_builder::trigger_if(|level, body| level.entity(body).on_ground)), 1),
    ];
    ActivityData::create_pairs_with_conditions(
        Activity::LAY_SPAWN,
        vec![
            (0, look_at_players()),
            (1, start_attacking()),
            (2, Box::new(try_find_land_near_liquid::create(8, SPEED_MULTIPLIER_WHEN_IDLING, &fluid_tags::FROG_TRIES_TO_FIND_LAND_NEAR))),
            (3, Box::new(try_lay_spawn_on_fluid_near_land::create(blocks::FROGSPAWN))),
            (4, Box::new(run_one::new(stroll))),
        ],
        vec![
            (memory_module_type::LONG_JUMP_MID_JUMP.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::IS_PREGNANT.erase(), MemoryStatus::ValuePresent),
        ],
    )
}

fn init_jump_activity() -> ActivityData {
    ActivityData::create_pairs_with_conditions(
        Activity::LONG_JUMP,
        vec![
            (0, Box::new(LongJumpMidJump::new(TIME_BETWEEN_LONG_JUMPS, "minecraft:entity.frog.step"))),
            (
                1,
                Box::new(LongJumpToPreferredBlock::new(
                    TIME_BETWEEN_LONG_JUMPS,
                    MAX_LONG_JUMP_HEIGHT,
                    MAX_LONG_JUMP_WIDTH,
                    MAX_JUMP_VELOCITY_MULTIPLIER,
                    |_, _| "minecraft:entity.frog.long_jump",
                    &block_tags::FROG_PREFER_JUMP_TO,
                    0.5,
                    is_acceptable_landing_spot,
                )),
            ),
        ],
        vec![
            (memory_module_type::TEMPTING_PLAYER.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::BREED_TARGET.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::LONG_JUMP_COOLDOWN_TICKS.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::IS_IN_WATER.erase(), MemoryStatus::ValueAbsent),
        ],
    )
}

fn init_tongue_activity() -> ActivityData {
    ActivityData::create_with_memory(
        Activity::TONGUE,
        0,
        vec![Box::new(stop_attacking_if_target_invalid::create()), Box::new(ShootTongue::new("minecraft:entity.frog.tongue", "minecraft:entity.frog.eat"))],
        memory_module_type::ATTACK_TARGET.erase(),
    )
}

fn is_acceptable_landing_spot(level: &mut ServerLevel, body: EntityId, target_pos: BlockPos) -> bool {
    let below = target_pos.below();
    if level.fluid_state(target_pos).is_empty() && level.fluid_state(below).is_empty() && level.fluid_state(target_pos.above()).is_empty() {
        let bs = level.block_state(target_pos);
        let bs_below = level.block_state(below);
        if !bs.is(&block_tags::FROG_PREFER_JUMP_TO) && !bs_below.is(&block_tags::FROG_PREFER_JUMP_TO) {
            let (path_type, path_type_below) = {
                let context = PathfindingContext::new(PathNavigationRegion::new(level), body);
                (get_path_type_static(&context, target_pos.x, target_pos.y, target_pos.z), get_path_type_static(&context, below.x, below.y, below.z))
            };
            if path_type != PathType::Trapdoor && (!bs.is_air() || path_type_below != PathType::Trapdoor) {
                long_jump_to_random_pos::default_acceptable_landing_spot(level, body, target_pos)
            } else {
                true
            }
        } else {
            true
        }
    } else {
        false
    }
}

fn can_attack(level: &ServerLevel, mob: EntityId) -> bool {
    !behavior_utils::is_breeding(level, mob)
}

pub fn update_activity(brain: &mut Brain) {
    brain.set_active_activity_to_first_valid(&[Activity::TONGUE, Activity::LAY_SPAWN, Activity::LONG_JUMP, Activity::SWIM, Activity::IDLE]);
}

pub fn get_temptations(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::FROG_FOOD)
}
