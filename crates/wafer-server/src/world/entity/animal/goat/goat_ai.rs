use wafer_core::random::RandomSource;
use wafer_data::game_rule;

use super::goat;
use crate::server::level::ServerLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::entity::ai::activity_data::ActivityData;
use crate::world::entity::ai::behavior::animal_make_love::AnimalMakeLove;
use crate::world::entity::ai::behavior::animal_panic::AnimalPanic;
use crate::world::entity::ai::behavior::behavior_control::BehaviorControl;
use crate::world::entity::ai::behavior::count_down_cooldown_ticks::CountDownCooldownTicks;
use crate::world::entity::ai::behavior::do_nothing::DoNothing;
use crate::world::entity::ai::behavior::follow_temptation::FollowTemptation;
use crate::world::entity::ai::behavior::long_jump_mid_jump::LongJumpMidJump;
use crate::world::entity::ai::behavior::long_jump_to_random_pos::LongJumpToRandomPos;
use crate::world::entity::ai::behavior::look_at_target_sink::LookAtTargetSink;
use crate::world::entity::ai::behavior::move_to_target_sink::MoveToTargetSink;
use crate::world::entity::ai::behavior::prepare_ram_nearest_target::PrepareRamNearestTarget;
use crate::world::entity::ai::behavior::ram_target::RamTarget;
use crate::world::entity::ai::behavior::swim::Swim;
use crate::world::entity::ai::behavior::{baby_follow_adult, random_stroll, run_one, set_entity_look_target_sometimes, set_walk_target_from_look_target};
use crate::world::entity::ai::brain::Brain;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::schedule::Activity;
use crate::world::entity::{EntityId, entity_types};

pub const RAM_PREPARE_TIME: i32 = 20;
pub const RAM_MAX_DISTANCE: i32 = 7;
const ADULT_FOLLOW_RANGE: IntProvider = IntProvider::Uniform { min_inclusive: 5, max_inclusive: 16 };
const SPEED_MULTIPLIER_WHEN_IDLING: f32 = 1.0;
const SPEED_MULTIPLIER_WHEN_FOLLOWING_ADULT: f32 = 1.25;
const SPEED_MULTIPLIER_WHEN_TEMPTED: f32 = 1.25;
const SPEED_MULTIPLIER_WHEN_PANICKING: f32 = 2.0;
const SPEED_MULTIPLIER_WHEN_PREPARING_TO_RAM: f32 = 1.25;
const TIME_BETWEEN_LONG_JUMPS: IntProvider = IntProvider::Uniform { min_inclusive: 600, max_inclusive: 1200 };
pub const MAX_LONG_JUMP_HEIGHT: i32 = 5;
pub const MAX_LONG_JUMP_WIDTH: i32 = 5;
pub const MAX_JUMP_VELOCITY_MULTIPLIER: f32 = 3.5714288;
const TIME_BETWEEN_RAMS: IntProvider = IntProvider::Uniform { min_inclusive: 600, max_inclusive: 6000 };
const TIME_BETWEEN_RAMS_SCREAMER: IntProvider = IntProvider::Uniform { min_inclusive: 100, max_inclusive: 300 };
const SPEED_MULTIPLIER_WHEN_RAMMING: f32 = 3.0;
pub const RAM_MIN_DISTANCE: i32 = 4;
pub const ADULT_RAM_KNOCKBACK_FORCE: f32 = 2.5;
pub const BABY_RAM_KNOCKBACK_FORCE: f32 = 1.0;

fn ram_target_selector(target: EntityId, level: &ServerLevel) -> bool {
    let entity_type = level.entity(target).entity_type;
    entity_type != entity_types::GOAT.id
        && (level.game_rules.get_bool(game_rule::MOB_GRIEFING) || entity_type != entity_types::ARMOR_STAND.id)
        && level.world_border.is_within_bounds_aabb(&level.entity(target).bounding_box())
}

fn ram_target_conditions() -> TargetingConditions<'static> {
    TargetingConditions::for_combat().selector(&ram_target_selector)
}

pub fn init_memories(brain: &mut Brain, random: &mut (impl RandomSource + ?Sized)) {
    brain.set_memory(memory_module_type::LONG_JUMP_COOLDOWN_TICKS, TIME_BETWEEN_LONG_JUMPS.sample(random));
    brain.set_memory(memory_module_type::RAM_COOLDOWN_TICKS, TIME_BETWEEN_RAMS.sample(random));
}

pub fn get_activities() -> Vec<ActivityData> {
    vec![init_core_activity(), init_idle_activity(), init_long_jump_activity(), init_ram_activity()]
}

fn init_core_activity() -> ActivityData {
    ActivityData::create(
        Activity::CORE,
        0,
        vec![
            Box::new(Swim::new(0.8)),
            Box::new(AnimalPanic::new(SPEED_MULTIPLIER_WHEN_PANICKING)),
            Box::new(LookAtTargetSink::new(45, 90)),
            Box::new(MoveToTargetSink::default()),
            Box::new(CountDownCooldownTicks::new(memory_module_type::TEMPTATION_COOLDOWN_TICKS)),
            Box::new(CountDownCooldownTicks::new(memory_module_type::LONG_JUMP_COOLDOWN_TICKS)),
            Box::new(CountDownCooldownTicks::new(memory_module_type::RAM_COOLDOWN_TICKS)),
        ],
    )
}

fn init_idle_activity() -> ActivityData {
    let stroll: Vec<(Box<dyn BehaviorControl>, i32)> = vec![
        (Box::new(random_stroll::stroll(SPEED_MULTIPLIER_WHEN_IDLING)), 2),
        (Box::new(set_walk_target_from_look_target::create(SPEED_MULTIPLIER_WHEN_IDLING, 3)), 2),
        (Box::new(DoNothing::new(30, 60)), 1),
    ];
    ActivityData::create_pairs_with_conditions(
        Activity::IDLE,
        vec![
            (0, Box::new(set_entity_look_target_sometimes::create_for_type(&entity_types::PLAYER, 6.0, IntProvider::Uniform { min_inclusive: 30, max_inclusive: 60 }))),
            (0, Box::new(AnimalMakeLove::new(&entity_types::GOAT))),
            (1, Box::new(FollowTemptation::new(|_, _| SPEED_MULTIPLIER_WHEN_TEMPTED))),
            (2, Box::new(baby_follow_adult::create(ADULT_FOLLOW_RANGE, SPEED_MULTIPLIER_WHEN_FOLLOWING_ADULT))),
            (3, Box::new(run_one::new(stroll))),
        ],
        vec![
            (memory_module_type::RAM_COOLDOWN_TICKS.erase(), MemoryStatus::ValuePresent),
            (memory_module_type::LONG_JUMP_COOLDOWN_TICKS.erase(), MemoryStatus::ValuePresent),
        ],
    )
}

fn init_long_jump_activity() -> ActivityData {
    ActivityData::create_pairs_with_conditions(
        Activity::LONG_JUMP,
        vec![
            (0, Box::new(LongJumpMidJump::new(TIME_BETWEEN_LONG_JUMPS, "minecraft:entity.goat.step"))),
            (
                1,
                Box::new(LongJumpToRandomPos::new(TIME_BETWEEN_LONG_JUMPS, MAX_LONG_JUMP_HEIGHT, MAX_LONG_JUMP_WIDTH, MAX_JUMP_VELOCITY_MULTIPLIER, |level, goat| {
                    if goat::is_screaming_goat(level.entity(goat)) { "minecraft:entity.goat.screaming.long_jump" } else { "minecraft:entity.goat.long_jump" }
                })),
            ),
        ],
        vec![
            (memory_module_type::TEMPTING_PLAYER.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::BREED_TARGET.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::LONG_JUMP_COOLDOWN_TICKS.erase(), MemoryStatus::ValueAbsent),
        ],
    )
}

fn init_ram_activity() -> ActivityData {
    ActivityData::create_pairs_with_conditions(
        Activity::RAM,
        vec![
            (
                0,
                Box::new(RamTarget::new(
                    |level, goat| if goat::is_screaming_goat(level.entity(goat)) { TIME_BETWEEN_RAMS_SCREAMER } else { TIME_BETWEEN_RAMS },
                    ram_target_conditions(),
                    SPEED_MULTIPLIER_WHEN_RAMMING,
                    |level, goat| if level.is_baby(goat) { BABY_RAM_KNOCKBACK_FORCE as f64 } else { ADULT_RAM_KNOCKBACK_FORCE as f64 },
                    |level, goat| if goat::is_screaming_goat(level.entity(goat)) { "minecraft:entity.goat.screaming.ram_impact" } else { "minecraft:entity.goat.ram_impact" },
                    |_, _| "minecraft:entity.goat.horn_break",
                )),
            ),
            (
                1,
                Box::new(PrepareRamNearestTarget::new(
                    |level, mob| if goat::is_screaming_goat(level.entity(mob)) { TIME_BETWEEN_RAMS_SCREAMER.min_inclusive() } else { TIME_BETWEEN_RAMS.min_inclusive() },
                    RAM_MIN_DISTANCE,
                    RAM_MAX_DISTANCE,
                    SPEED_MULTIPLIER_WHEN_PREPARING_TO_RAM,
                    ram_target_conditions(),
                    RAM_PREPARE_TIME,
                    |level, goat| if goat::is_screaming_goat(level.entity(goat)) { "minecraft:entity.goat.screaming.prepare_ram" } else { "minecraft:entity.goat.prepare_ram" },
                )),
            ),
        ],
        vec![
            (memory_module_type::TEMPTING_PLAYER.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::BREED_TARGET.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::RAM_COOLDOWN_TICKS.erase(), MemoryStatus::ValueAbsent),
        ],
    )
}

pub fn update_activity(brain: &mut Brain) {
    brain.set_active_activity_to_first_valid(&[Activity::RAM, Activity::LONG_JUMP, Activity::IDLE]);
}
