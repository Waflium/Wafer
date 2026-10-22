use wafer_core::Vec3;
use wafer_data::damage_type_tags;

use crate::server::level::ServerLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::entity::ai::activity_data::ActivityData;
use crate::world::entity::ai::behavior::animal_panic::AnimalPanic;
use crate::world::entity::ai::behavior::behavior_control::BehaviorControl;
use crate::world::entity::ai::behavior::count_down_cooldown_ticks::CountDownCooldownTicks;
use crate::world::entity::ai::behavior::follow_temptation::FollowTemptation;
use crate::world::entity::ai::behavior::look_at_target_sink::LookAtTargetSink;
use crate::world::entity::ai::behavior::move_to_target_sink::MoveToTargetSink;
use crate::world::entity::ai::behavior::swim::Swim;
use crate::world::entity::ai::behavior::{baby_follow_adult, random_stroll, run_one, set_walk_target_from_look_target};
use crate::world::entity::ai::brain::Brain;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::ai::util::air_and_water_random_pos;
use crate::world::entity::schedule::Activity;
use crate::world::entity::{EntityId, calculate_view_vector};

const SPEED_MULTIPLIER_WHEN_IDLING: f32 = 1.0;
const SPEED_MULTIPLIER_WHEN_TEMPTED: f32 = 1.25;
const SPEED_MULTIPLIER_WHEN_FOLLOWING_ADULT: f32 = 1.1;
const BABY_GHAST_CLOSE_ENOUGH_DIST: f64 = 3.0;
const ADULT_FOLLOW_RANGE: IntProvider = IntProvider::Uniform { min_inclusive: 3, max_inclusive: 16 };

pub fn get_activities() -> Vec<ActivityData> {
    vec![init_core_activity(), init_idle_activity(), init_panic_activity()]
}

fn fly_panic_pos(level: &mut ServerLevel, mob: EntityId) -> Option<Vec3> {
    let e = level.entity(mob);
    let view = calculate_view_vector(e.x_rot_o, level.living(mob).y_head_rot_o);
    air_and_water_random_pos::get_pos(level, mob, 5, 4, 0, view.x, view.z, std::f32::consts::FRAC_PI_2 as f64)
}

fn init_core_activity() -> ActivityData {
    ActivityData::create(
        Activity::CORE,
        0,
        vec![
            Box::new(Swim::new(0.8)),
            Box::new(AnimalPanic::with_position_getter(super::happy_ghast::SPEED_MULTIPLIER_WHEN_PANICKING, |_, _| &damage_type_tags::PANIC_CAUSES, fly_panic_pos)),
            Box::new(LookAtTargetSink::new(45, 90)),
            Box::new(MoveToTargetSink::default()),
            Box::new(CountDownCooldownTicks::new(memory_module_type::TEMPTATION_COOLDOWN_TICKS)),
        ],
    )
}

fn init_idle_activity() -> ActivityData {
    let stroll: Vec<(Box<dyn BehaviorControl>, i32)> =
        vec![(Box::new(random_stroll::fly(SPEED_MULTIPLIER_WHEN_IDLING)), 1), (Box::new(set_walk_target_from_look_target::create(SPEED_MULTIPLIER_WHEN_IDLING, 3)), 1)];
    ActivityData::create_pairs(
        Activity::IDLE,
        vec![
            (1, Box::new(FollowTemptation::with_look_in_the_eyes(|_, _| SPEED_MULTIPLIER_WHEN_TEMPTED, |_, _| BABY_GHAST_CLOSE_ENOUGH_DIST, true))),
            (2, Box::new(baby_follow_adult::create_with(ADULT_FOLLOW_RANGE, |_, _| SPEED_MULTIPLIER_WHEN_FOLLOWING_ADULT, memory_module_type::NEAREST_VISIBLE_PLAYER, true))),
            (3, Box::new(baby_follow_adult::create_with(ADULT_FOLLOW_RANGE, |_, _| SPEED_MULTIPLIER_WHEN_FOLLOWING_ADULT, memory_module_type::NEAREST_VISIBLE_ADULT, true))),
            (4, Box::new(run_one::new(stroll))),
        ],
    )
}

fn init_panic_activity() -> ActivityData {
    ActivityData::create_pairs_with_conditions(Activity::PANIC, Vec::new(), vec![(memory_module_type::IS_PANICKING.erase(), MemoryStatus::ValuePresent)])
}

pub fn update_activity(brain: &mut Brain) {
    brain.set_active_activity_to_first_valid(&[Activity::PANIC, Activity::IDLE]);
}
