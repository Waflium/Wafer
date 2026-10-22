use super::creaking;
use crate::server::level::ServerLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::entity::EntityId;
use crate::world::entity::ai::activity_data::ActivityData;
use crate::world::entity::ai::behavior::behavior::{Behavior, BehaviorState};
use crate::world::entity::ai::behavior::behavior_control::BehaviorControl;
use crate::world::entity::ai::behavior::do_nothing::DoNothing;
use crate::world::entity::ai::behavior::look_at_target_sink::LookAtTargetSink;
use crate::world::entity::ai::behavior::move_to_target_sink::MoveToTargetSink;
use crate::world::entity::ai::behavior::swim::Swim;
use crate::world::entity::ai::behavior::{
    melee_attack, random_stroll, run_one, set_entity_look_target_sometimes, set_walk_target_from_attack_target_if_target_out_of_reach, set_walk_target_from_look_target,
    start_attacking, stop_attacking_if_target_invalid,
};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::schedule::Activity;

struct CreakingSwim(Swim);

impl Behavior for CreakingSwim {
    fn behavior(&self) -> &BehaviorState {
        self.0.behavior()
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        self.0.behavior_mut()
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        level.creaking_can_move(body) && self.0.check_extra_start_conditions(level, body)
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) -> bool {
        self.0.can_still_use(level, body, timestamp)
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        self.0.start(level, body, timestamp);
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        self.0.tick(level, body, timestamp);
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        self.0.stop(level, body, timestamp);
    }
}

fn init_core_activity() -> ActivityData {
    ActivityData::create(Activity::CORE, 0, vec![Box::new(CreakingSwim(Swim::new(0.8))), Box::new(LookAtTargetSink::new(45, 90)), Box::new(MoveToTargetSink::default())])
}

fn init_idle_activity() -> ActivityData {
    let stroll: Vec<(Box<dyn BehaviorControl>, i32)> = vec![
        (Box::new(random_stroll::stroll(creaking::SPEED_MULTIPLIER_WHEN_IDLING)), 2),
        (Box::new(set_walk_target_from_look_target::create(creaking::SPEED_MULTIPLIER_WHEN_IDLING, 3)), 2),
        (Box::new(DoNothing::new(30, 60)), 1),
    ];
    ActivityData::create(
        Activity::IDLE,
        10,
        vec![
            Box::new(start_attacking::create_with(
                |level, mob| creaking::is_active(level.entity(mob)),
                |level, mob| level.get_brain(mob).get_memory(memory_module_type::NEAREST_VISIBLE_ATTACKABLE_PLAYER).copied(),
            )),
            Box::new(set_entity_look_target_sometimes::create(8.0, IntProvider::Uniform { min_inclusive: 30, max_inclusive: 60 })),
            Box::new(run_one::new(stroll)),
        ],
    )
}

fn is_attack_target_still_reachable(level: &ServerLevel, creaking: EntityId, target: EntityId) -> bool {
    level.get_brain(creaking).get_memory(memory_module_type::NEAREST_VISIBLE_ATTACKABLE_PLAYERS).is_some_and(|players| level.is_player(target) && players.contains(&target))
}

fn init_fight_activity() -> ActivityData {
    ActivityData::create_with_conditions(
        Activity::FIGHT,
        10,
        vec![
            Box::new(set_walk_target_from_attack_target_if_target_out_of_reach::create_with(|_, _| 1.0)),
            Box::new(melee_attack::create_with(|level, mob| level.creaking_can_move(mob), creaking::ATTACK_INTERVAL)),
            Box::new(stop_attacking_if_target_invalid::create_when(|level, body, target| !is_attack_target_still_reachable(level, body, target))),
        ],
        vec![(memory_module_type::ATTACK_TARGET.erase(), MemoryStatus::ValuePresent)],
    )
}

pub fn get_activities() -> Vec<ActivityData> {
    vec![init_core_activity(), init_idle_activity(), init_fight_activity()]
}

pub fn update_activity(level: &mut ServerLevel, creaking: EntityId) {
    if !level.creaking_can_move(creaking) {
        level.get_brain_mut(creaking).use_default_activity();
    } else {
        level.get_brain_mut(creaking).set_active_activity_to_first_valid(&[Activity::FIGHT, Activity::IDLE]);
    }
}
