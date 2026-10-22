use crate::server::level::ServerLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::entity::ai::activity_data::ActivityData;
use crate::world::entity::ai::behavior::animal_make_love::AnimalMakeLove;
use crate::world::entity::ai::behavior::animal_panic::AnimalPanic;
use crate::world::entity::ai::behavior::behavior::{Behavior, BehaviorState};
use crate::world::entity::ai::behavior::behavior_control::BehaviorControl;
use crate::world::entity::ai::behavior::count_down_cooldown_ticks::CountDownCooldownTicks;
use crate::world::entity::ai::behavior::declarative::behavior_builder;
use crate::world::entity::ai::behavior::do_nothing::DoNothing;
use crate::world::entity::ai::behavior::follow_temptation::FollowTemptation;
use crate::world::entity::ai::behavior::look_at_target_sink::LookAtTargetSink;
use crate::world::entity::ai::behavior::move_to_target_sink::MoveToTargetSink;
use crate::world::entity::ai::behavior::random_look_around::RandomLookAround;
use crate::world::entity::ai::behavior::swim::Swim;
use crate::world::entity::ai::behavior::{baby_follow_adult, random_stroll, run_one, set_entity_look_target_sometimes, set_walk_target_from_look_target};
use crate::world::entity::ai::brain::Brain;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::mob::Mob;
use crate::world::entity::schedule::Activity;
use crate::world::entity::{EntityId, entity_types};

const SPEED_MULTIPLIER_WHEN_PANICKING: f32 = 4.0;
const SPEED_MULTIPLIER_WHEN_IDLING: f32 = 2.0;
const SPEED_MULTIPLIER_WHEN_TEMPTED: f32 = 2.5;
const SPEED_MULTIPLIER_WHEN_FOLLOWING_ADULT: f32 = 2.5;
const ADULT_FOLLOW_RANGE: IntProvider = IntProvider::Uniform { min_inclusive: 5, max_inclusive: 16 };

pub fn init_memories(_body: &mut Mob) {}

pub fn get_activities() -> Vec<ActivityData> {
    vec![init_core_activity(), init_idle_activity()]
}

fn init_core_activity() -> ActivityData {
    ActivityData::create(
        Activity::CORE,
        0,
        vec![
            Box::new(Swim::new(0.8)),
            Box::new(CamelPanic::new(SPEED_MULTIPLIER_WHEN_PANICKING)),
            Box::new(LookAtTargetSink::new(45, 90)),
            Box::new(MoveToTargetSink::default()),
            Box::new(CountDownCooldownTicks::new(memory_module_type::TEMPTATION_COOLDOWN_TICKS)),
            Box::new(CountDownCooldownTicks::new(memory_module_type::GAZE_COOLDOWN_TICKS)),
        ],
    )
}

fn can_move(level: &ServerLevel, body: EntityId) -> bool {
    !level.camel_refuse_to_move(body)
}

fn init_idle_activity() -> ActivityData {
    let follow: Vec<(Box<dyn BehaviorControl>, i32)> = vec![
        (Box::new(FollowTemptation::with_close_enough_distance(|_, _| SPEED_MULTIPLIER_WHEN_TEMPTED, |level, camel| if level.is_baby(camel) { 2.5 } else { 3.5 })), 1),
        (Box::new(behavior_builder::sequence(behavior_builder::trigger_if(can_move), baby_follow_adult::create(ADULT_FOLLOW_RANGE, SPEED_MULTIPLIER_WHEN_FOLLOWING_ADULT))), 1),
    ];
    let stroll: Vec<(Box<dyn BehaviorControl>, i32)> = vec![
        (Box::new(behavior_builder::sequence(behavior_builder::trigger_if(can_move), random_stroll::stroll(SPEED_MULTIPLIER_WHEN_IDLING))), 1),
        (Box::new(behavior_builder::sequence(behavior_builder::trigger_if(can_move), set_walk_target_from_look_target::create(SPEED_MULTIPLIER_WHEN_IDLING, 3))), 1),
        (Box::new(RandomSitting::new(20)), 1),
        (Box::new(DoNothing::new(30, 60)), 1),
    ];
    ActivityData::create_pairs(
        Activity::IDLE,
        vec![
            (0, Box::new(set_entity_look_target_sometimes::create_for_type(&entity_types::PLAYER, 6.0, IntProvider::Uniform { min_inclusive: 30, max_inclusive: 60 }))),
            (1, Box::new(AnimalMakeLove::new(&entity_types::CAMEL))),
            (2, Box::new(run_one::new(follow))),
            (3, Box::new(RandomLookAround::new(IntProvider::Uniform { min_inclusive: 150, max_inclusive: 250 }, 30.0, 0.0, 0.0))),
            (4, Box::new(run_one::with_entry_condition(vec![(memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent)], stroll))),
        ],
    )
}

pub fn update_activity(brain: &mut Brain) {
    brain.set_active_activity_to_first_valid(&[Activity::IDLE]);
}

pub struct CamelPanic {
    animal_panic: AnimalPanic,
}

impl CamelPanic {
    pub fn new(speed_multiplier: f32) -> CamelPanic {
        CamelPanic { animal_panic: AnimalPanic::new(speed_multiplier) }
    }
}

impl Behavior for CamelPanic {
    fn behavior(&self) -> &BehaviorState {
        self.animal_panic.behavior()
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        self.animal_panic.behavior_mut()
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        self.animal_panic.check_extra_start_conditions(level, body) && !level.is_mob_controlled(body)
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) -> bool {
        self.animal_panic.can_still_use(level, body, timestamp)
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        level.camel_stand_up_instantly(body);
        self.animal_panic.start(level, body, timestamp);
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        self.animal_panic.tick(level, body, timestamp);
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        self.animal_panic.stop(level, body, timestamp);
    }
}

pub struct RandomSitting {
    behavior: BehaviorState,
    minimal_pose_ticks: i32,
}

impl RandomSitting {
    pub fn new(minimal_pose_time_sec: i32) -> RandomSitting {
        RandomSitting { behavior: BehaviorState::new(Vec::new()), minimal_pose_ticks: minimal_pose_time_sec * 20 }
    }
}

impl Behavior for RandomSitting {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        let e = level.entity(body);
        !e.is_in_water()
            && level.camel_get_pose_time(body) >= self.minimal_pose_ticks as i64
            && !level.is_leashed(body)
            && e.on_ground
            && !level.has_controlling_passenger(body)
            && level.camel_can_camel_change_pose(body)
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        if level.camel_is_camel_sitting(body) {
            level.camel_stand_up(body);
        } else if !level.is_panicking(body) {
            level.camel_sit_down(body);
        }
    }
}
