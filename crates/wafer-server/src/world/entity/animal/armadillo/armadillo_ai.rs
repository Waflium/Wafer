use wafer_core::random::RandomSource;
use wafer_data::damage_type_tags;

use super::armadillo::{self, Armadillo, ArmadilloState};
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
use crate::world::entity::ai::behavior::one_shot::OneShot;
use crate::world::entity::ai::behavior::random_look_around::RandomLookAround;
use crate::world::entity::ai::behavior::swim::Swim;
use crate::world::entity::ai::behavior::{baby_follow_adult, random_stroll, run_one, set_entity_look_target_sometimes, set_walk_target_from_look_target};
use crate::world::entity::ai::brain::Brain;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::schedule::Activity;
use crate::world::entity::{EntityId, entity_types, sound_event};

const SPEED_MULTIPLIER_WHEN_PANICKING: f32 = 2.0;
const SPEED_MULTIPLIER_WHEN_IDLING: f32 = 1.0;
const SPEED_MULTIPLIER_WHEN_TEMPTED: f32 = 1.25;
const SPEED_MULTIPLIER_WHEN_FOLLOWING_ADULT: f32 = 1.25;
const SPEED_MULTIPLIER_WHEN_MAKING_LOVE: f32 = 1.0;
const DEFAULT_CLOSE_ENOUGH_DIST: f64 = 2.0;
const BABY_CLOSE_ENOUGH_DIST: f64 = 1.0;
const ADULT_FOLLOW_RANGE: IntProvider = IntProvider::Uniform { min_inclusive: 5, max_inclusive: 16 };

fn armadillo_rolling_out() -> OneShot {
    behavior_builder::create(|i| {
        i.absent(memory_module_type::DANGER_DETECTED_RECENTLY);
        |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            if armadillo::is_scared(level.entity(body)) {
                level.armadillo_roll_out(body);
                true
            } else {
                false
            }
        }
    })
}

pub fn get_activities() -> Vec<ActivityData> {
    vec![init_core_activity(), init_idle_activity(), init_scared_activity()]
}

fn init_core_activity() -> ActivityData {
    ActivityData::create(
        Activity::CORE,
        0,
        vec![
            Box::new(Swim::new(0.8)),
            Box::new(ArmadilloPanic::new(SPEED_MULTIPLIER_WHEN_PANICKING)),
            Box::new(LookAtTargetSink::new(45, 90)),
            Box::new(ArmadilloMoveToTargetSink(MoveToTargetSink::default())),
            Box::new(CountDownCooldownTicks::new(memory_module_type::TEMPTATION_COOLDOWN_TICKS)),
            Box::new(CountDownCooldownTicks::new(memory_module_type::GAZE_COOLDOWN_TICKS)),
            Box::new(armadillo_rolling_out()),
        ],
    )
}

fn init_idle_activity() -> ActivityData {
    let follow: Vec<(Box<dyn BehaviorControl>, i32)> = vec![
        (
            Box::new(FollowTemptation::with_close_enough_distance(
                |_, _| SPEED_MULTIPLIER_WHEN_TEMPTED,
                |level, armadillo| if level.is_baby(armadillo) { BABY_CLOSE_ENOUGH_DIST } else { DEFAULT_CLOSE_ENOUGH_DIST },
            )),
            1,
        ),
        (Box::new(baby_follow_adult::create(ADULT_FOLLOW_RANGE, SPEED_MULTIPLIER_WHEN_FOLLOWING_ADULT)), 1),
    ];
    let stroll: Vec<(Box<dyn BehaviorControl>, i32)> = vec![
        (Box::new(random_stroll::stroll(SPEED_MULTIPLIER_WHEN_IDLING)), 1),
        (Box::new(set_walk_target_from_look_target::create(SPEED_MULTIPLIER_WHEN_IDLING, 3)), 1),
        (Box::new(DoNothing::new(30, 60)), 1),
    ];
    ActivityData::create_pairs(
        Activity::IDLE,
        vec![
            (0, Box::new(set_entity_look_target_sometimes::create_for_type(&entity_types::PLAYER, 6.0, IntProvider::Uniform { min_inclusive: 30, max_inclusive: 60 }))),
            (1, Box::new(AnimalMakeLove::with_speed(&entity_types::ARMADILLO, SPEED_MULTIPLIER_WHEN_MAKING_LOVE, 1))),
            (2, Box::new(run_one::new(follow))),
            (3, Box::new(RandomLookAround::new(IntProvider::Uniform { min_inclusive: 150, max_inclusive: 250 }, 30.0, 0.0, 0.0))),
            (4, Box::new(run_one::with_entry_condition(vec![(memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent)], stroll))),
        ],
    )
}

fn init_scared_activity() -> ActivityData {
    ActivityData::create_pairs_with_conditions(
        Activity::PANIC,
        vec![(0, Box::new(ArmadilloBallUp::new()))],
        vec![
            (memory_module_type::DANGER_DETECTED_RECENTLY.erase(), MemoryStatus::ValuePresent),
            (memory_module_type::IS_PANICKING.erase(), MemoryStatus::ValueAbsent),
        ],
    )
}

pub fn update_activity(brain: &mut Brain) {
    brain.set_active_activity_to_first_valid(&[Activity::PANIC, Activity::IDLE]);
}

struct ArmadilloMoveToTargetSink(MoveToTargetSink);

impl Behavior for ArmadilloMoveToTargetSink {
    fn behavior(&self) -> &BehaviorState {
        self.0.behavior()
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        self.0.behavior_mut()
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        if level.mob(body).is_some_and(|mob| matches!(mob.kind, crate::world::entity::mob::MobKind::Armadillo(_)) && armadillo::is_scared(&mob.entity)) {
            false
        } else {
            self.0.check_extra_start_conditions(level, body)
        }
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

const BALL_UP_STAY_IN_STATE: i32 = 5 * 60 * 20;
const DANGER_DETECTED_RECENTLY_DANGER_THRESHOLD: i64 = 75;

pub struct ArmadilloBallUp {
    behavior: BehaviorState,
    next_peek_timer: i32,
    danger_was_around: bool,
}

impl ArmadilloBallUp {
    pub fn new() -> ArmadilloBallUp {
        ArmadilloBallUp { behavior: BehaviorState::with_time_out(Vec::new(), BALL_UP_STAY_IN_STATE), next_peek_timer: 0, danger_was_around: false }
    }

    fn pick_next_peek_timer(&self, level: &mut ServerLevel, body: EntityId) -> i32 {
        ArmadilloState::Scared.animation_duration() + level.entity_mut(body).random.next_int_between_inclusive(100, 400)
    }
}

impl Behavior for ArmadilloBallUp {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        if self.next_peek_timer > 0 {
            self.next_peek_timer -= 1;
        }
        if Armadillo::should_switch_to_scared_state(level.mob(body).expect("an armadillo")) {
            Armadillo::switch_to_state(level.mob_mut(body).expect("an armadillo"), ArmadilloState::Scared);
            if level.entity(body).on_ground {
                level.entity_play_sound(body, sound_event("minecraft:entity.armadillo.land"), 1.0, 1.0);
            }
            return;
        }
        let state = armadillo::get_state(level.entity(body));
        let danger_tick_counter = level.get_brain(body).get_time_until_expiry(memory_module_type::DANGER_DETECTED_RECENTLY);
        let danger_is_around = danger_tick_counter > DANGER_DETECTED_RECENTLY_DANGER_THRESHOLD;
        if danger_is_around != self.danger_was_around {
            self.next_peek_timer = self.pick_next_peek_timer(level, body);
        }
        self.danger_was_around = danger_is_around;
        if state == ArmadilloState::Scared {
            if self.next_peek_timer == 0 && level.entity(body).on_ground && danger_is_around {
                level.broadcast_entity_event(body, 64);
                self.next_peek_timer = self.pick_next_peek_timer(level, body);
            }
            if danger_tick_counter < ArmadilloState::Unrolling.animation_duration() as i64 {
                level.entity_play_sound(body, sound_event("minecraft:entity.armadillo.unroll_start"), 1.0, 1.0);
                Armadillo::switch_to_state(level.mob_mut(body).expect("an armadillo"), ArmadilloState::Unrolling);
            }
        } else if state == ArmadilloState::Unrolling && danger_tick_counter > ArmadilloState::Unrolling.animation_duration() as i64 {
            Armadillo::switch_to_state(level.mob_mut(body).expect("an armadillo"), ArmadilloState::Scared);
        }
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        let e = level.entity(body);
        e.on_ground && !e.is_in_liquid()
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        armadillo::get_state(level.entity(body)).is_threatened()
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        level.armadillo_roll_up(body);
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        if !armadillo::can_stay_rolled_up(level, body) {
            level.armadillo_roll_out(body);
        }
    }
}

pub struct ArmadilloPanic {
    animal_panic: AnimalPanic,
}

impl ArmadilloPanic {
    pub fn new(speed_multiplier: f32) -> ArmadilloPanic {
        ArmadilloPanic { animal_panic: AnimalPanic::with_panic_causing_damage_types(speed_multiplier, |_, _| &damage_type_tags::PANIC_ENVIRONMENTAL_CAUSES) }
    }
}

impl Behavior for ArmadilloPanic {
    fn behavior(&self) -> &BehaviorState {
        self.animal_panic.behavior()
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        self.animal_panic.behavior_mut()
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        self.animal_panic.check_extra_start_conditions(level, body)
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) -> bool {
        self.animal_panic.can_still_use(level, body, timestamp)
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        level.armadillo_roll_out(body);
        self.animal_panic.start(level, body, timestamp);
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        self.animal_panic.tick(level, body, timestamp);
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        self.animal_panic.stop(level, body, timestamp);
    }
}
