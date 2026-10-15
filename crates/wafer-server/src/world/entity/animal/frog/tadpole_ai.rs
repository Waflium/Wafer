use crate::util::valueproviders::IntProvider;
use crate::world::entity::ai::activity_data::ActivityData;
use crate::world::entity::ai::behavior::animal_panic::AnimalPanic;
use crate::world::entity::ai::behavior::behavior_control::BehaviorControl;
use crate::world::entity::ai::behavior::count_down_cooldown_ticks::CountDownCooldownTicks;
use crate::world::entity::ai::behavior::declarative::behavior_builder;
use crate::world::entity::ai::behavior::follow_temptation::FollowTemptation;
use crate::world::entity::ai::behavior::gate_behavior::{GateBehavior, OrderPolicy, RunningPolicy};
use crate::world::entity::ai::behavior::look_at_target_sink::LookAtTargetSink;
use crate::world::entity::ai::behavior::move_to_target_sink::MoveToTargetSink;
use crate::world::entity::ai::behavior::{random_stroll, set_entity_look_target_sometimes, set_walk_target_from_look_target};
use crate::world::entity::ai::brain::Brain;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::entity_types;
use crate::world::entity::schedule::Activity;

const SPEED_MULTIPLIER_WHEN_PANICKING: f32 = 2.0;
const SPEED_MULTIPLIER_WHEN_IDLING_IN_WATER: f32 = 0.5;
const SPEED_MULTIPLIER_WHEN_TEMPTED: f32 = 1.25;

pub fn get_activities() -> Vec<ActivityData> {
    vec![init_core_activity(), init_idle_activity()]
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
        ],
    )
}

fn init_idle_activity() -> ActivityData {
    let swim: Vec<(Box<dyn BehaviorControl>, i32)> = vec![
        (Box::new(random_stroll::swim(SPEED_MULTIPLIER_WHEN_IDLING_IN_WATER)), 2),
        (Box::new(set_walk_target_from_look_target::create(SPEED_MULTIPLIER_WHEN_IDLING_IN_WATER, 3)), 3),
        (Box::new(behavior_builder::trigger_if(|level, body| level.entity(body).is_in_water())), 5),
    ];
    ActivityData::create_pairs(
        Activity::IDLE,
        vec![
            (0, Box::new(set_entity_look_target_sometimes::create_for_type(&entity_types::PLAYER, 6.0, IntProvider::Uniform { min_inclusive: 30, max_inclusive: 60 }))),
            (1, Box::new(FollowTemptation::new(|_, _| SPEED_MULTIPLIER_WHEN_TEMPTED))),
            (
                2,
                Box::new(GateBehavior::new(
                    vec![(memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent)],
                    Vec::new(),
                    OrderPolicy::Ordered,
                    RunningPolicy::TryAll,
                    swim,
                )),
            ),
        ],
    )
}

pub fn update_activity(brain: &mut Brain) {
    brain.set_active_activity_to_first_valid(&[Activity::IDLE]);
}
