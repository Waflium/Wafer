use wafer_core::random::RandomSource;
use wafer_data::{entity_type_tags, game_rule, item_tags};

use crate::server::level::ServerLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::entity::ai::activity_data::ActivityData;
use crate::world::entity::ai::behavior::animal_make_love::AnimalMakeLove;
use crate::world::entity::ai::behavior::animal_panic::AnimalPanic;
use crate::world::entity::ai::behavior::behavior_control::BehaviorControl;
use crate::world::entity::ai::behavior::behavior_utils;
use crate::world::entity::ai::behavior::charge_attack::ChargeAttack;
use crate::world::entity::ai::behavior::count_down_cooldown_ticks::CountDownCooldownTicks;
use crate::world::entity::ai::behavior::follow_temptation::FollowTemptation;
use crate::world::entity::ai::behavior::gate_behavior::{GateBehavior, OrderPolicy, RunningPolicy};
use crate::world::entity::ai::behavior::look_at_target_sink::LookAtTargetSink;
use crate::world::entity::ai::behavior::move_to_target_sink::MoveToTargetSink;
use crate::world::entity::ai::behavior::{random_stroll, set_walk_target_from_look_target, start_attacking};
use crate::world::entity::ai::brain::Brain;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::ai::sensing::sensor;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::mob::Mob;
use crate::world::entity::schedule::Activity;
use crate::world::entity::{EntityId, entity_types};
use crate::world::item::ItemStack;

const SPEED_MULTIPLIER_WHEN_IDLING_IN_WATER: f32 = 1.0;
const SPEED_MULTIPLIER_WHEN_TEMPTED: f32 = 1.3;
const SPEED_MULTIPLIER_WHEN_MAKING_LOVE: f32 = 0.4;
const SPEED_MULTIPLIER_WHEN_PANICKING: f32 = 1.6;
const TIME_BETWEEN_NON_PLAYER_ATTACKS: IntProvider = IntProvider::Uniform { min_inclusive: 2400, max_inclusive: 3600 };
const SPEED_WHEN_ATTACKING: f32 = 0.6;
pub const ATTACK_KNOCKBACK_FORCE: f32 = 2.0;
const ANGER_DURATION: i64 = 400;
pub const TIME_BETWEEN_ATTACKS: i32 = 80;
pub const MAX_CHARGE_DISTANCE: f64 = 12.0;
pub const MAX_TARGET_DETECTION_DISTANCE: f64 = 11.0;

fn attack_target_selector(target: EntityId, level: &ServerLevel) -> bool {
    (level.game_rules.get_bool(game_rule::MOB_GRIEFING) || level.entity(target).entity_type != entity_types::ARMOR_STAND.id) && level.world_border.is_within_bounds_aabb(&level.entity(target).bounding_box())
}

pub fn attack_target_conditions() -> TargetingConditions<'static> {
    TargetingConditions::for_combat().selector(&attack_target_selector)
}

pub fn init_memories(body: &mut Mob, random: &mut (impl RandomSource + ?Sized)) {
    let cooldown = TIME_BETWEEN_NON_PLAYER_ATTACKS.sample(random);
    body.living.brain.set_memory(memory_module_type::ATTACK_TARGET_COOLDOWN, cooldown);
}

pub fn get_activities() -> Vec<ActivityData> {
    vec![init_core_activity(), init_idle_activity(), init_fight_activity()]
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
            Box::new(CountDownCooldownTicks::new(memory_module_type::CHARGE_COOLDOWN_TICKS)),
            Box::new(CountDownCooldownTicks::new(memory_module_type::ATTACK_TARGET_COOLDOWN)),
        ],
    )
}

pub fn swim_or_look_target() -> GateBehavior {
    let behaviors: Vec<(Box<dyn BehaviorControl>, i32)> =
        vec![(Box::new(random_stroll::swim(SPEED_MULTIPLIER_WHEN_IDLING_IN_WATER)), 2), (Box::new(set_walk_target_from_look_target::create(SPEED_MULTIPLIER_WHEN_IDLING_IN_WATER, 3)), 3)];
    GateBehavior::new(vec![(memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent)], Vec::new(), OrderPolicy::Ordered, RunningPolicy::TryAll, behaviors)
}

fn init_idle_activity() -> ActivityData {
    ActivityData::create_pairs(
        Activity::IDLE,
        vec![
            (1, Box::new(AnimalMakeLove::with_speed(&entity_types::NAUTILUS, SPEED_MULTIPLIER_WHEN_MAKING_LOVE, 2))),
            (2, Box::new(FollowTemptation::with_close_enough_distance(|_, _| SPEED_MULTIPLIER_WHEN_TEMPTED, |level, mob| if level.is_baby(mob) { 2.5 } else { 3.5 }))),
            (3, Box::new(start_attacking::create(find_nearest_valid_attack_target))),
            (4, Box::new(swim_or_look_target())),
        ],
    )
}

pub fn fight_conditions() -> Vec<(memory_module_type::MemoryModuleType<memory_module_type::Erased>, MemoryStatus)> {
    vec![
        (memory_module_type::ATTACK_TARGET.erase(), MemoryStatus::ValuePresent),
        (memory_module_type::TEMPTING_PLAYER.erase(), MemoryStatus::ValueAbsent),
        (memory_module_type::BREED_TARGET.erase(), MemoryStatus::ValueAbsent),
        (memory_module_type::CHARGE_COOLDOWN_TICKS.erase(), MemoryStatus::ValueAbsent),
    ]
}

fn init_fight_activity() -> ActivityData {
    ActivityData::create_pairs_with_conditions(
        Activity::FIGHT,
        vec![(
            0,
            Box::new(ChargeAttack::new(
                TIME_BETWEEN_ATTACKS,
                attack_target_conditions(),
                SPEED_WHEN_ATTACKING,
                ATTACK_KNOCKBACK_FORCE,
                MAX_CHARGE_DISTANCE,
                MAX_TARGET_DETECTION_DISTANCE,
                "minecraft:entity.nautilus.dash",
            )),
        )],
        fight_conditions(),
    )
}

pub fn find_nearest_valid_attack_target(level: &mut ServerLevel, body: EntityId) -> Option<EntityId> {
    if behavior_utils::is_breeding(level, body) || !level.entity(body).is_in_water() || level.is_baby(body) || level.is_tame(body) {
        return None;
    }
    let angry_at = behavior_utils::get_living_entity_from_uuid_memory(level, body, memory_module_type::ANGRY_AT)
        .filter(|&entity| level.entity(entity).is_in_water() && sensor::is_entity_attackable_ignoring_line_of_sight(level, body, entity));
    if angry_at.is_some() {
        return angry_at;
    }
    if level.get_brain(body).has_memory_value(memory_module_type::ATTACK_TARGET_COOLDOWN) {
        return None;
    }
    let cooldown = TIME_BETWEEN_NON_PLAYER_ATTACKS.sample(&mut level.random);
    level.get_brain_mut(body).set_memory(memory_module_type::ATTACK_TARGET_COOLDOWN, cooldown);
    if level.random.next_float() < 0.5 {
        return None;
    }
    let nearest = level.get_brain(body).get_memory(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES).cloned();
    nearest.and_then(|entities| entities.find_closest(level, |level, mob| is_hostile_target(level, mob)))
}

pub fn set_anger_target(level: &mut ServerLevel, body: EntityId, target: EntityId) {
    if sensor::is_entity_attackable_ignoring_line_of_sight(level, body, target) {
        let target_uuid = level.entity(target).uuid;
        let brain = level.get_brain_mut(body);
        brain.erase_memory(memory_module_type::CANT_REACH_WALK_TARGET_SINCE);
        brain.set_memory_with_expiry(memory_module_type::ANGRY_AT, target_uuid, ANGER_DURATION);
    }
}

fn is_hostile_target(level: &ServerLevel, mob: EntityId) -> bool {
    let e = level.entity(mob);
    e.is_in_water() && entity_type_tags::NAUTILUS_HOSTILES.contains(e.entity_type as usize)
}

pub fn update_activity(brain: &mut Brain) {
    brain.set_active_activity_to_first_valid(&[Activity::FIGHT, Activity::IDLE]);
}

pub fn get_temptations(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::NAUTILUS_FOOD)
}
