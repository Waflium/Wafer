use super::sniffer::State;
use crate::server::level::ServerLevel;
use crate::world::entity::ai::activity_data::ActivityData;
use crate::world::entity::ai::behavior::animal_make_love::AnimalMakeLove;
use crate::world::entity::ai::behavior::animal_panic::AnimalPanic;
use crate::world::entity::ai::behavior::behavior::{Behavior, BehaviorState};
use crate::world::entity::ai::behavior::behavior_control::BehaviorControl;
use crate::world::entity::ai::behavior::count_down_cooldown_ticks::CountDownCooldownTicks;
use crate::world::entity::ai::behavior::do_nothing::DoNothing;
use crate::world::entity::ai::behavior::follow_temptation::FollowTemptation;
use crate::world::entity::ai::behavior::look_at_target_sink::LookAtTargetSink;
use crate::world::entity::ai::behavior::move_to_target_sink::MoveToTargetSink;
use crate::world::entity::ai::behavior::swim::Swim;
use crate::world::entity::ai::behavior::{random_stroll, run_one, set_entity_look_target, set_walk_target_from_look_target};
use crate::world::entity::ai::brain::Brain;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::entity::schedule::Activity;
use crate::world::entity::{EntityId, entity_types};

const MAX_LOOK_DISTANCE: f32 = 6.0;
const SNIFFING_COOLDOWN_TICKS: i64 = 9600;
const SPEED_MULTIPLIER_WHEN_IDLING: f32 = 1.0;
const SPEED_MULTIPLIER_WHEN_PANICKING: f32 = 2.0;
const SPEED_MULTIPLIER_WHEN_SNIFFING: f32 = 1.25;
const SPEED_MULTIPLIER_WHEN_TEMPTED: f32 = 1.25;

pub fn get_activities() -> Vec<ActivityData> {
    vec![init_core_activity(), init_idle_activity(), init_sniffing_activity(), init_dig_activity()]
}

fn reset_sniffing(level: &mut ServerLevel, body: EntityId) {
    let brain = level.get_brain_mut(body);
    brain.erase_memory(memory_module_type::SNIFFER_DIGGING);
    brain.erase_memory(memory_module_type::SNIFFER_SNIFFING_TARGET);
    level.sniffer_transition_to(body, State::Idling);
}

fn init_core_activity() -> ActivityData {
    ActivityData::create(
        Activity::CORE,
        0,
        vec![
            Box::new(Swim::new(0.8)),
            Box::new(ResetSniffingOnStart(AnimalPanic::new(SPEED_MULTIPLIER_WHEN_PANICKING))),
            Box::new(MoveToTargetSink::new(500, 700)),
            Box::new(CountDownCooldownTicks::new(memory_module_type::TEMPTATION_COOLDOWN_TICKS)),
        ],
    )
}

fn init_sniffing_activity() -> ActivityData {
    ActivityData::create_pairs_with_conditions(
        Activity::SNIFF,
        vec![(0, Box::new(Searching::new()))],
        vec![
            (memory_module_type::IS_PANICKING.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::SNIFFER_SNIFFING_TARGET.erase(), MemoryStatus::ValuePresent),
            (memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValuePresent),
        ],
    )
}

fn init_dig_activity() -> ActivityData {
    ActivityData::create_pairs_with_conditions(
        Activity::DIG,
        vec![(0, Box::new(Digging::new(160, 180))), (0, Box::new(FinishedDigging::new(40)))],
        vec![
            (memory_module_type::IS_PANICKING.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::SNIFFER_DIGGING.erase(), MemoryStatus::ValuePresent),
        ],
    )
}

fn init_idle_activity() -> ActivityData {
    let wander: Vec<(Box<dyn BehaviorControl>, i32)> = vec![
        (Box::new(set_walk_target_from_look_target::create(SPEED_MULTIPLIER_WHEN_IDLING, 3)), 2),
        (Box::new(Scenting::new(40, 80)), 1),
        (Box::new(Sniffing::new(40, 80)), 1),
        (Box::new(set_entity_look_target::create_for_type(&entity_types::PLAYER, MAX_LOOK_DISTANCE)), 1),
        (Box::new(random_stroll::stroll(SPEED_MULTIPLIER_WHEN_IDLING)), 1),
        (Box::new(DoNothing::new(5, 20)), 2),
    ];
    ActivityData::create_pairs_with_conditions(
        Activity::IDLE,
        vec![
            (0, Box::new(ResetSniffingOnStart(AnimalMakeLove::new(&entity_types::SNIFFER)))),
            (
                1,
                Box::new(ResetSniffingOnStart(FollowTemptation::with_close_enough_distance(
                    |_, _| SPEED_MULTIPLIER_WHEN_TEMPTED,
                    |level, sniffer| if level.is_baby(sniffer) { 2.5 } else { 3.5 },
                ))),
            ),
            (2, Box::new(LookAtTargetSink::new(45, 90))),
            (3, Box::new(FeelingHappy::new(40, 100))),
            (4, Box::new(run_one::new(wander))),
        ],
        vec![(memory_module_type::SNIFFER_DIGGING.erase(), MemoryStatus::ValueAbsent)],
    )
}

pub fn update_activity(brain: &mut Brain) {
    brain.set_active_activity_to_first_valid(&[Activity::DIG, Activity::SNIFF, Activity::IDLE]);
}

struct ResetSniffingOnStart<B: Behavior>(B);

impl<B: Behavior> Behavior for ResetSniffingOnStart<B> {
    fn behavior(&self) -> &BehaviorState {
        self.0.behavior()
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        self.0.behavior_mut()
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        self.0.check_extra_start_conditions(level, body)
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) -> bool {
        self.0.can_still_use(level, body, timestamp)
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        reset_sniffing(level, body);
        self.0.start(level, body, timestamp);
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        self.0.tick(level, body, timestamp);
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        self.0.stop(level, body, timestamp);
    }
}

struct Digging {
    behavior: BehaviorState,
}

impl Digging {
    fn new(min: i32, max: i32) -> Digging {
        Digging {
            behavior: BehaviorState::with_durations(
                vec![
                    (memory_module_type::IS_PANICKING.erase(), MemoryStatus::ValueAbsent),
                    (memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent),
                    (memory_module_type::SNIFFER_DIGGING.erase(), MemoryStatus::ValuePresent),
                    (memory_module_type::SNIFF_COOLDOWN.erase(), MemoryStatus::ValueAbsent),
                ],
                min,
                max,
            ),
        }
    }
}

impl Behavior for Digging {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, sniffer: EntityId) -> bool {
        level.sniffer_can_sniff(sniffer)
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, sniffer: EntityId, _timestamp: i64) -> bool {
        level.get_brain(sniffer).has_memory_value(memory_module_type::SNIFFER_DIGGING) && level.sniffer_can_dig(sniffer) && !level.is_in_love(sniffer)
    }

    fn start(&mut self, level: &mut ServerLevel, sniffer: EntityId, _timestamp: i64) {
        level.sniffer_transition_to(sniffer, State::Digging);
    }

    fn stop(&mut self, level: &mut ServerLevel, sniffer: EntityId, timestamp: i64) {
        if self.timed_out(timestamp) {
            level.get_brain_mut(sniffer).set_memory_with_expiry(memory_module_type::SNIFF_COOLDOWN, (), SNIFFING_COOLDOWN_TICKS);
        } else {
            reset_sniffing(level, sniffer);
        }
    }
}

struct FeelingHappy {
    behavior: BehaviorState,
}

impl FeelingHappy {
    fn new(min: i32, max: i32) -> FeelingHappy {
        FeelingHappy { behavior: BehaviorState::with_durations(vec![(memory_module_type::SNIFFER_HAPPY.erase(), MemoryStatus::ValuePresent)], min, max) }
    }
}

impl Behavior for FeelingHappy {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn can_still_use(&mut self, _level: &mut ServerLevel, _sniffer: EntityId, _timestamp: i64) -> bool {
        true
    }

    fn start(&mut self, level: &mut ServerLevel, sniffer: EntityId, _timestamp: i64) {
        level.sniffer_transition_to(sniffer, State::FeelingHappy);
    }

    fn stop(&mut self, level: &mut ServerLevel, sniffer: EntityId, _timestamp: i64) {
        level.sniffer_transition_to(sniffer, State::Idling);
        level.get_brain_mut(sniffer).erase_memory(memory_module_type::SNIFFER_HAPPY);
    }
}

struct FinishedDigging {
    behavior: BehaviorState,
}

impl FinishedDigging {
    fn new(duration: i32) -> FinishedDigging {
        FinishedDigging {
            behavior: BehaviorState::with_durations(
                vec![
                    (memory_module_type::IS_PANICKING.erase(), MemoryStatus::ValueAbsent),
                    (memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent),
                    (memory_module_type::SNIFFER_DIGGING.erase(), MemoryStatus::ValuePresent),
                    (memory_module_type::SNIFF_COOLDOWN.erase(), MemoryStatus::ValuePresent),
                ],
                duration,
                duration,
            ),
        }
    }
}

impl Behavior for FinishedDigging {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, sniffer: EntityId, _timestamp: i64) -> bool {
        level.get_brain(sniffer).has_memory_value(memory_module_type::SNIFFER_DIGGING)
    }

    fn start(&mut self, level: &mut ServerLevel, sniffer: EntityId, _timestamp: i64) {
        level.sniffer_transition_to(sniffer, State::Rising);
    }

    fn stop(&mut self, level: &mut ServerLevel, sniffer: EntityId, timestamp: i64) {
        let finished = self.timed_out(timestamp);
        level.sniffer_transition_to(sniffer, State::Idling);
        level.sniffer_on_digging_complete(sniffer, finished);
        let brain = level.get_brain_mut(sniffer);
        brain.erase_memory(memory_module_type::SNIFFER_DIGGING);
        brain.set_memory(memory_module_type::SNIFFER_HAPPY, true);
    }
}

struct Scenting {
    behavior: BehaviorState,
}

impl Scenting {
    fn new(min: i32, max: i32) -> Scenting {
        Scenting {
            behavior: BehaviorState::with_durations(
                vec![
                    (memory_module_type::IS_PANICKING.erase(), MemoryStatus::ValueAbsent),
                    (memory_module_type::SNIFFER_DIGGING.erase(), MemoryStatus::ValueAbsent),
                    (memory_module_type::SNIFFER_SNIFFING_TARGET.erase(), MemoryStatus::ValueAbsent),
                    (memory_module_type::SNIFFER_HAPPY.erase(), MemoryStatus::ValueAbsent),
                    (memory_module_type::BREED_TARGET.erase(), MemoryStatus::ValueAbsent),
                ],
                min,
                max,
            ),
        }
    }
}

impl Behavior for Scenting {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, sniffer: EntityId) -> bool {
        !level.sniffer_is_tempted(sniffer)
    }

    fn can_still_use(&mut self, _level: &mut ServerLevel, _sniffer: EntityId, _timestamp: i64) -> bool {
        true
    }

    fn start(&mut self, level: &mut ServerLevel, sniffer: EntityId, _timestamp: i64) {
        level.sniffer_transition_to(sniffer, State::Scenting);
    }

    fn stop(&mut self, level: &mut ServerLevel, sniffer: EntityId, _timestamp: i64) {
        level.sniffer_transition_to(sniffer, State::Idling);
    }
}

struct Searching {
    behavior: BehaviorState,
}

impl Searching {
    fn new() -> Searching {
        Searching {
            behavior: BehaviorState::with_time_out(
                vec![
                    (memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValuePresent),
                    (memory_module_type::IS_PANICKING.erase(), MemoryStatus::ValueAbsent),
                    (memory_module_type::SNIFFER_SNIFFING_TARGET.erase(), MemoryStatus::ValuePresent),
                ],
                600,
            ),
        }
    }
}

impl Behavior for Searching {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, sniffer: EntityId) -> bool {
        level.sniffer_can_sniff(sniffer)
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, sniffer: EntityId, _timestamp: i64) -> bool {
        if !level.sniffer_can_sniff(sniffer) {
            level.sniffer_transition_to(sniffer, State::Idling);
            return false;
        }
        let brain = level.get_brain(sniffer);
        let walk_target = brain.get_memory(memory_module_type::WALK_TARGET).map(|target| target.get_target().current_block_position(level));
        let sniffing_target = brain.get_memory(memory_module_type::SNIFFER_SNIFFING_TARGET).copied();
        match (walk_target, sniffing_target) {
            (Some(walk_target), Some(sniffing_target)) => sniffing_target == walk_target,
            _ => false,
        }
    }

    fn start(&mut self, level: &mut ServerLevel, sniffer: EntityId, _timestamp: i64) {
        level.sniffer_transition_to(sniffer, State::Searching);
    }

    fn stop(&mut self, level: &mut ServerLevel, sniffer: EntityId, _timestamp: i64) {
        if level.sniffer_can_dig(sniffer) && level.sniffer_can_sniff(sniffer) {
            level.get_brain_mut(sniffer).set_memory(memory_module_type::SNIFFER_DIGGING, true);
        }
        let brain = level.get_brain_mut(sniffer);
        brain.erase_memory(memory_module_type::WALK_TARGET);
        brain.erase_memory(memory_module_type::SNIFFER_SNIFFING_TARGET);
    }
}

struct Sniffing {
    behavior: BehaviorState,
}

impl Sniffing {
    fn new(min: i32, max: i32) -> Sniffing {
        Sniffing {
            behavior: BehaviorState::with_durations(
                vec![
                    (memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent),
                    (memory_module_type::SNIFFER_SNIFFING_TARGET.erase(), MemoryStatus::ValueAbsent),
                    (memory_module_type::SNIFF_COOLDOWN.erase(), MemoryStatus::ValueAbsent),
                ],
                min,
                max,
            ),
        }
    }
}

impl Behavior for Sniffing {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        !level.is_baby(body) && level.sniffer_can_sniff(body)
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        level.sniffer_can_sniff(body)
    }

    fn start(&mut self, level: &mut ServerLevel, sniffer: EntityId, _timestamp: i64) {
        level.sniffer_transition_to(sniffer, State::Sniffing);
    }

    fn stop(&mut self, level: &mut ServerLevel, sniffer: EntityId, timestamp: i64) {
        let finished = self.timed_out(timestamp);
        level.sniffer_transition_to(sniffer, State::Idling);
        if finished && let Some(position) = level.sniffer_calculate_dig_position(sniffer) {
            let brain = level.get_brain_mut(sniffer);
            brain.set_memory(memory_module_type::SNIFFER_SNIFFING_TARGET, position);
            brain.set_memory(memory_module_type::WALK_TARGET, WalkTarget::of_block_pos(position, SPEED_MULTIPLIER_WHEN_SNIFFING, 0));
        }
    }
}
