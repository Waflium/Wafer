use super::nautilus_ai;
use crate::world::entity::ai::activity_data::ActivityData;
use crate::world::entity::ai::behavior::charge_attack::ChargeAttack;
use crate::world::entity::ai::behavior::count_down_cooldown_ticks::CountDownCooldownTicks;
use crate::world::entity::ai::behavior::follow_temptation::FollowTemptation;
use crate::world::entity::ai::behavior::look_at_target_sink::LookAtTargetSink;
use crate::world::entity::ai::behavior::move_to_target_sink::MoveToTargetSink;
use crate::world::entity::ai::behavior::start_attacking;
use crate::world::entity::ai::brain::Brain;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::schedule::Activity;

const SPEED_MULTIPLIER_WHEN_TEMPTED: f32 = 0.9;
const SPEED_WHEN_ATTACKING: f32 = 0.5;

pub fn get_activities() -> Vec<ActivityData> {
    vec![init_core_activity(), init_idle_activity(), init_fight_activity()]
}

fn init_core_activity() -> ActivityData {
    ActivityData::create(
        Activity::CORE,
        0,
        vec![
            Box::new(LookAtTargetSink::new(45, 90)),
            Box::new(MoveToTargetSink::default()),
            Box::new(CountDownCooldownTicks::new(memory_module_type::TEMPTATION_COOLDOWN_TICKS)),
            Box::new(CountDownCooldownTicks::new(memory_module_type::CHARGE_COOLDOWN_TICKS)),
            Box::new(CountDownCooldownTicks::new(memory_module_type::ATTACK_TARGET_COOLDOWN)),
        ],
    )
}

fn init_idle_activity() -> ActivityData {
    ActivityData::create_pairs(
        Activity::IDLE,
        vec![
            (1, Box::new(FollowTemptation::with_close_enough_distance(|_, _| SPEED_MULTIPLIER_WHEN_TEMPTED, |level, mob| if level.is_baby(mob) { 2.5 } else { 3.5 }))),
            (2, Box::new(start_attacking::create(nautilus_ai::find_nearest_valid_attack_target))),
            (3, Box::new(nautilus_ai::swim_or_look_target())),
        ],
    )
}

fn init_fight_activity() -> ActivityData {
    ActivityData::create_pairs_with_conditions(
        Activity::FIGHT,
        vec![(
            0,
            Box::new(ChargeAttack::new(
                nautilus_ai::TIME_BETWEEN_ATTACKS,
                nautilus_ai::attack_target_conditions(),
                SPEED_WHEN_ATTACKING,
                nautilus_ai::ATTACK_KNOCKBACK_FORCE,
                nautilus_ai::MAX_CHARGE_DISTANCE,
                nautilus_ai::MAX_TARGET_DETECTION_DISTANCE,
                "minecraft:entity.zombie_nautilus.dash",
            )),
        )],
        nautilus_ai::fight_conditions(),
    )
}

pub fn update_activity(brain: &mut Brain) {
    brain.set_active_activity_to_first_valid(&[Activity::FIGHT, Activity::IDLE]);
}
