use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3};
use wafer_data::blocks;

use super::allay::{self, MAX_NOTEBLOCK_DISTANCE, THROW_SOUND_PITCHES};
use crate::advancements::triggers::criteria_triggers;
use crate::core::global_pos::GlobalPos;
use crate::server::level::game_mode::GameType;
use crate::server::level::{ServerLevel, SoundSource};
use crate::util::valueproviders::IntProvider;
use crate::world::entity::ai::activity_data::ActivityData;
use crate::world::entity::ai::behavior::animal_panic::AnimalPanic;
use crate::world::entity::ai::behavior::behavior_control::BehaviorControl;
use crate::world::entity::ai::behavior::block_pos_tracker::BlockPosTracker;
use crate::world::entity::ai::behavior::count_down_cooldown_ticks::CountDownCooldownTicks;
use crate::world::entity::ai::behavior::do_nothing::DoNothing;
use crate::world::entity::ai::behavior::entity_tracker::EntityTracker;
use crate::world::entity::ai::behavior::go_and_give_items_to_target::GoAndGiveItemsToTarget;
use crate::world::entity::ai::behavior::look_at_target_sink::LookAtTargetSink;
use crate::world::entity::ai::behavior::move_to_target_sink::MoveToTargetSink;
use crate::world::entity::ai::behavior::position_tracker::PositionTracker;
use crate::world::entity::ai::behavior::swim::Swim;
use crate::world::entity::ai::behavior::{
    behavior_utils, go_to_wanted_item, random_stroll, run_one, set_entity_look_target_sometimes, set_walk_target_from_look_target, stay_close_to_target,
};
use crate::world::entity::ai::brain::Brain;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::schedule::Activity;
use crate::world::entity::{EntityId, sound_event};
use crate::world::level::BlockGetter;

const SPEED_MULTIPLIER_WHEN_IDLING: f32 = 1.0;
const SPEED_MULTIPLIER_WHEN_FOLLOWING_DEPOSIT_TARGET: f32 = 2.25;
const SPEED_MULTIPLIER_WHEN_RETRIEVING_ITEM: f32 = 1.75;
const SPEED_MULTIPLIER_WHEN_PANICKING: f32 = 2.5;
const CLOSE_ENOUGH_TO_TARGET: i32 = 4;
const TOO_FAR_FROM_TARGET: i32 = 16;
const MAX_LOOK_DISTANCE: f32 = 6.0;
const MIN_WAIT_DURATION: i32 = 30;
const MAX_WAIT_DURATION: i32 = 60;
const DISTANCE_TO_WANTED_ITEM: i32 = 32;
const GIVE_ITEM_TIMEOUT_DURATION: i32 = 20;
const THROW_VELOCITY: Vec3 = Vec3::new(0.2f32 as f64, 0.3f32 as f64, 0.2f32 as f64);
const ITEM_PICKUP_COOLDOWN_DURATION: i32 = 60;

pub fn get_activities() -> Vec<ActivityData> {
    vec![init_core_activity(), init_idle_activity()]
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
            Box::new(CountDownCooldownTicks::new(memory_module_type::LIKED_NOTEBLOCK_COOLDOWN_TICKS)),
            Box::new(CountDownCooldownTicks::new(memory_module_type::ITEM_PICKUP_COOLDOWN_TICKS)),
        ],
    )
}

fn init_idle_activity() -> ActivityData {
    let stroll: Vec<(Box<dyn BehaviorControl>, i32)> = vec![
        (Box::new(random_stroll::fly(SPEED_MULTIPLIER_WHEN_IDLING)), 2),
        (Box::new(set_walk_target_from_look_target::create(SPEED_MULTIPLIER_WHEN_IDLING, 3)), 2),
        (Box::new(DoNothing::new(MIN_WAIT_DURATION, MAX_WAIT_DURATION)), 1),
    ];
    ActivityData::create(
        Activity::IDLE,
        0,
        vec![
            Box::new(go_to_wanted_item::create_with(|_, _| true, SPEED_MULTIPLIER_WHEN_RETRIEVING_ITEM, true, DISTANCE_TO_WANTED_ITEM)),
            Box::new(GoAndGiveItemsToTarget::new(
                get_item_deposit_position,
                SPEED_MULTIPLIER_WHEN_FOLLOWING_DEPOSIT_TARGET,
                GIVE_ITEM_TIMEOUT_DURATION,
                throw_item,
                memory_module_type::ITEM_PICKUP_COOLDOWN_TICKS,
                ITEM_PICKUP_COOLDOWN_DURATION,
                has_item_to_throw,
            )),
            Box::new(stay_close_to_target::create(
                get_item_deposit_position,
                |level, allay| !has_wanted_item(level, allay),
                CLOSE_ENOUGH_TO_TARGET,
                TOO_FAR_FROM_TARGET,
                SPEED_MULTIPLIER_WHEN_FOLLOWING_DEPOSIT_TARGET,
            )),
            Box::new(set_entity_look_target_sometimes::create(MAX_LOOK_DISTANCE, IntProvider::Uniform { min_inclusive: 30, max_inclusive: 60 })),
            Box::new(run_one::new(stroll)),
        ],
    )
}

fn has_item_to_throw(level: &ServerLevel, allay: EntityId) -> bool {
    !allay::allay(level.mob(allay).expect("an allay")).inventory.is_empty()
}

pub fn update_activity(brain: &mut Brain) {
    brain.set_active_activity_to_first_valid(&[Activity::IDLE]);
}

fn get_item_deposit_position(level: &mut ServerLevel, allay: EntityId) -> Option<PositionTracker> {
    if let Some(position) = level.get_brain(allay).get_memory(memory_module_type::LIKED_NOTEBLOCK_POSITION).cloned() {
        if should_deposit_items_at_liked_noteblock(level, allay, &position) {
            return Some(PositionTracker::BlockPos(BlockPosTracker::new(position.pos().above())));
        }
        level.get_brain_mut(allay).erase_memory(memory_module_type::LIKED_NOTEBLOCK_POSITION);
    }
    get_liked_player_position_tracker(level, allay)
}

fn has_wanted_item(level: &ServerLevel, allay: EntityId) -> bool {
    level.get_brain(allay).has_memory_value(memory_module_type::NEAREST_VISIBLE_WANTED_ITEM)
}

fn should_deposit_items_at_liked_noteblock(level: &mut ServerLevel, allay: EntityId, liked_noteblock_pos: &GlobalPos) -> bool {
    let liked_noteblock_cooldown = level.get_brain(allay).get_memory(memory_module_type::LIKED_NOTEBLOCK_COOLDOWN_TICKS).copied();
    liked_noteblock_pos.is_close_enough(level.dimension, level.entity(allay).block_position(), MAX_NOTEBLOCK_DISTANCE)
        && level.block_state(liked_noteblock_pos.pos()).is_block(blocks::NOTE_BLOCK)
        && liked_noteblock_cooldown.is_some()
}

fn get_liked_player_position_tracker(level: &ServerLevel, allay: EntityId) -> Option<PositionTracker> {
    get_liked_player(level, allay).map(|player| PositionTracker::Entity(EntityTracker::new(level, player, true)))
}

pub fn get_liked_player(level: &ServerLevel, allay: EntityId) -> Option<EntityId> {
    let liked_player = *level.get_brain(allay).get_memory(memory_module_type::LIKED_PLAYER)?;
    let player = level.entity_manager.visible_entity_storage.get(liked_player).filter(|&player| level.is_player(player))?;
    let game_type = level.player(player).game_mode.game_type;
    let closer = level.entity(player).position().closer_than(level.entity(allay).position(), 64.0);
    ((game_type == GameType::Survival || game_type == GameType::Creative) && closer).then_some(player)
}

fn throw_item(level: &mut ServerLevel, thrower: EntityId, target_pos: Vec3) {
    let item = allay::allay_mut(level.mob_mut(thrower).expect("an allay")).inventory.remove_item(0, 1);
    if item.is_empty() {
        return;
    }
    behavior_utils::throw_item_with(level, thrower, item.clone(), target_pos.add(0.0, 1.0, 0.0), THROW_VELOCITY, 0.2);
    if let Some(player) = get_liked_player(level, thrower) {
        criteria_triggers::ALLAY_DROP_ITEM_ON_BLOCK.trigger(level, player, BlockPos::containing_vec(target_pos).below(), &item);
    }
    if level.game_time % 7 == 0 && level.random.next_double() < 0.9 {
        let pitch = THROW_SOUND_PITCHES[level.random.next_int_bounded(THROW_SOUND_PITCHES.len() as i32) as usize];
        level.play_sound_from_entity(None, thrower, sound_event("minecraft:entity.allay.item_thrown"), SoundSource::Neutral, 1.0, pitch);
    }
}
