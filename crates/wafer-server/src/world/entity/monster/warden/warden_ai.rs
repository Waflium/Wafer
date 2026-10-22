use wafer_core::BlockPos;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::activity_data::ActivityData;
use crate::world::entity::ai::behavior::behavior_control::BehaviorControl;
use crate::world::entity::ai::behavior::block_pos_tracker::BlockPosTracker;
use crate::world::entity::ai::behavior::declarative::behavior_builder;
use crate::world::entity::ai::behavior::do_nothing::DoNothing;
use crate::world::entity::ai::behavior::look_at_target_sink::LookAtTargetSink;
use crate::world::entity::ai::behavior::move_to_target_sink::MoveToTargetSink;
use crate::world::entity::ai::behavior::one_shot::OneShot;
use crate::world::entity::ai::behavior::position_tracker::PositionTracker;
use crate::world::entity::ai::behavior::swim::Swim;
use crate::world::entity::ai::behavior::warden::digging::Digging;
use crate::world::entity::ai::behavior::warden::emerging::Emerging;
use crate::world::entity::ai::behavior::warden::force_unmount::ForceUnmount;
use crate::world::entity::ai::behavior::warden::roar::Roar;
use crate::world::entity::ai::behavior::warden::sonic_boom::SonicBoom;
use crate::world::entity::ai::behavior::warden::sniffing::Sniffing;
use crate::world::entity::ai::behavior::warden::{set_roar_target, set_warden_look_target, try_to_sniff};
use crate::world::entity::ai::behavior::{
    go_to_target_location, melee_attack, random_stroll, run_one, set_entity_look_target, set_walk_target_from_attack_target_if_target_out_of_reach, stop_attacking_if_target_invalid,
};
use crate::world::entity::ai::brain::Brain;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::mob::Mob;
use crate::world::entity::schedule::Activity;

const SPEED_MULTIPLIER_WHEN_IDLING: f32 = 0.5;
const SPEED_MULTIPLIER_WHEN_INVESTIGATING: f32 = 0.7;
const SPEED_MULTIPLIER_WHEN_FIGHTING: f32 = 1.2;
const MELEE_ATTACK_COOLDOWN: i32 = 18;
const DIGGING_DURATION: i32 = 100;
pub const EMERGE_DURATION: i32 = 134;
pub const ROAR_DURATION: i32 = 84;
const SNIFFING_DURATION: i32 = 84;
pub const DIGGING_COOLDOWN: i32 = 1200;
const DISTURBANCE_LOCATION_EXPIRY_TIME: i64 = 100;

fn dig_cooldown_setter() -> OneShot {
    behavior_builder::create(|i| {
        let cooldown = i.registered(memory_module_type::DIG_COOLDOWN);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            if cooldown.try_get(level, body).is_some() {
                cooldown.set_with_expiry(level, body, (), DIGGING_COOLDOWN as i64);
            }
            true
        }
    })
}

pub fn get_activities(body: &Mob) -> Vec<ActivityData> {
    vec![
        init_core_activity(),
        init_emerge_activity(),
        init_digging_activity(),
        init_idle_activity(),
        init_roar_activity(),
        init_fight_activity(body),
        init_investigate_activity(),
        init_sniffing_activity(),
    ]
}

pub fn update_activity(brain: &mut Brain) {
    brain.set_active_activity_to_first_valid(&[Activity::EMERGE, Activity::DIG, Activity::ROAR, Activity::FIGHT, Activity::INVESTIGATE, Activity::SNIFF, Activity::IDLE]);
}

fn init_core_activity() -> ActivityData {
    ActivityData::create(
        Activity::CORE,
        0,
        vec![Box::new(Swim::new(0.8)), Box::new(set_warden_look_target::create()), Box::new(LookAtTargetSink::new(45, 90)), Box::new(MoveToTargetSink::default())],
    )
}

fn init_emerge_activity() -> ActivityData {
    ActivityData::create_with_memory(Activity::EMERGE, 5, vec![Box::new(Emerging::new(EMERGE_DURATION))], memory_module_type::IS_EMERGING.erase())
}

fn init_digging_activity() -> ActivityData {
    ActivityData::create_pairs_with_conditions(
        Activity::DIG,
        vec![(0, Box::new(ForceUnmount::default())), (1, Box::new(Digging::new(DIGGING_DURATION)))],
        vec![(memory_module_type::ROAR_TARGET.erase(), MemoryStatus::ValueAbsent), (memory_module_type::DIG_COOLDOWN.erase(), MemoryStatus::ValueAbsent)],
    )
}

fn init_idle_activity() -> ActivityData {
    let stroll: Vec<(Box<dyn BehaviorControl>, i32)> = vec![(Box::new(random_stroll::stroll(SPEED_MULTIPLIER_WHEN_IDLING)), 2), (Box::new(DoNothing::new(30, 60)), 1)];
    ActivityData::create(
        Activity::IDLE,
        10,
        vec![
            Box::new(set_roar_target::create(|level, body| level.warden_get_entity_angry_at(body))),
            Box::new(try_to_sniff::create()),
            Box::new(run_one::with_entry_condition(vec![(memory_module_type::IS_SNIFFING.erase(), MemoryStatus::ValueAbsent)], stroll)),
        ],
    )
}

fn init_investigate_activity() -> ActivityData {
    ActivityData::create_with_memory(
        Activity::INVESTIGATE,
        5,
        vec![
            Box::new(set_roar_target::create(|level, body| level.warden_get_entity_angry_at(body))),
            Box::new(go_to_target_location::create(memory_module_type::DISTURBANCE_LOCATION, 2, SPEED_MULTIPLIER_WHEN_INVESTIGATING)),
        ],
        memory_module_type::DISTURBANCE_LOCATION.erase(),
    )
}

fn init_sniffing_activity() -> ActivityData {
    ActivityData::create_with_memory(
        Activity::SNIFF,
        5,
        vec![Box::new(set_roar_target::create(|level, body| level.warden_get_entity_angry_at(body))), Box::new(Sniffing::new(SNIFFING_DURATION))],
        memory_module_type::IS_SNIFFING.erase(),
    )
}

fn init_roar_activity() -> ActivityData {
    ActivityData::create_with_memory(Activity::ROAR, 10, vec![Box::new(Roar::default())], memory_module_type::ROAR_TARGET.erase())
}

fn init_fight_activity(body: &Mob) -> ActivityData {
    let follow_range = body.living.attributes.value(Attribute::FollowRange) as f32;
    let body_id = body.entity.id;
    ActivityData::create_with_memory(
        Activity::FIGHT,
        10,
        vec![
            Box::new(dig_cooldown_setter()),
            Box::new(stop_attacking_if_target_invalid::create_with(
                |level, body, target| !level.warden_get_anger_level(body).is_angry() || !level.warden_can_target_entity(body, Some(target)),
                on_target_invalid,
                false,
            )),
            Box::new(set_entity_look_target::create_with(move |level, entity| is_target(level, body_id, entity), follow_range)),
            Box::new(set_walk_target_from_attack_target_if_target_out_of_reach::create_with(|_, _| SPEED_MULTIPLIER_WHEN_FIGHTING)),
            Box::new(SonicBoom::default()),
            Box::new(melee_attack::create(MELEE_ATTACK_COOLDOWN)),
        ],
        memory_module_type::ATTACK_TARGET.erase(),
    )
}

fn is_target(level: &ServerLevel, body: EntityId, living: EntityId) -> bool {
    level.get_brain(body).get_memory(memory_module_type::ATTACK_TARGET).is_some_and(|&target| target == living)
}

fn on_target_invalid(level: &mut ServerLevel, body: EntityId, attack_target: EntityId) {
    if !level.warden_can_target_entity(body, Some(attack_target)) {
        level.warden_clear_anger(body, attack_target);
    }
    set_dig_cooldown(level, body);
}

pub fn set_dig_cooldown(level: &mut ServerLevel, body: EntityId) {
    if level.get_brain(body).has_memory_value(memory_module_type::DIG_COOLDOWN) {
        level.get_brain_mut(body).set_memory_with_expiry(memory_module_type::DIG_COOLDOWN, (), DIGGING_COOLDOWN as i64);
    }
}

pub fn set_disturbance_location(level: &mut ServerLevel, body: EntityId, position: BlockPos) {
    if level.world_border.is_within_bounds_pos(position) && level.warden_get_entity_angry_at(body).is_none() && level.get_brain(body).get_memory(memory_module_type::ATTACK_TARGET).is_none() {
        set_dig_cooldown(level, body);
        let brain = level.get_brain_mut(body);
        brain.set_memory_with_expiry(memory_module_type::SNIFF_COOLDOWN, (), DISTURBANCE_LOCATION_EXPIRY_TIME);
        brain.set_memory_with_expiry(memory_module_type::LOOK_TARGET, PositionTracker::BlockPos(BlockPosTracker::new(position)), DISTURBANCE_LOCATION_EXPIRY_TIME);
        brain.set_memory_with_expiry(memory_module_type::DISTURBANCE_LOCATION, position, DISTURBANCE_LOCATION_EXPIRY_TIME);
        brain.erase_memory(memory_module_type::WALK_TARGET);
    }
}
