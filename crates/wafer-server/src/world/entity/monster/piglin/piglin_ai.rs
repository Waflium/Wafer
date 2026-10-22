use std::cell::RefCell;
use std::rc::Rc;

use wafer_core::Vec3;
use wafer_core::random::{LegacyRandomSource, RandomSource};
use wafer_data::{EquipmentSlot, game_rule, item_tags, items};

use super::{
    remember_if_hoglin_was_killed, start_admiring_item_if_seen, start_hunting_hoglin, stop_admiring_if_item_too_far_away, stop_admiring_if_tired_of_trying_to_reach_item,
    stop_holding_item_if_no_longer_admiring,
};
use crate::server::level::ServerLevel;
use crate::util::time_util;
use crate::util::valueproviders::IntProvider;
use crate::world::entity::ai::activity_data::ActivityData;
use crate::world::entity::ai::behavior::behavior_control::BehaviorControl;
use crate::world::entity::ai::behavior::crossbow_attack::CrossbowAttack;
use crate::world::entity::ai::behavior::declarative::behavior_builder;
use crate::world::entity::ai::behavior::declarative::trigger::Trigger;
use crate::world::entity::ai::behavior::do_nothing::DoNothing;
use crate::world::entity::ai::behavior::gate_behavior::GateBehavior;
use crate::world::entity::ai::behavior::look_at_target_sink::LookAtTargetSink;
use crate::world::entity::ai::behavior::move_to_target_sink::MoveToTargetSink;
use crate::world::entity::ai::behavior::one_shot::OneShot;
use crate::world::entity::ai::behavior::set_entity_look_target_sometimes::Ticker;
use crate::world::entity::ai::behavior::spear_approach::SpearApproach;
use crate::world::entity::ai::behavior::spear_attack::SpearAttack;
use crate::world::entity::ai::behavior::spear_retreat::SpearRetreat;
use crate::world::entity::ai::behavior::{
    back_up_if_too_close, behavior_utils, copy_memory_with_expiry, dismount_or_skip_mounting, erase_memory_if, go_to_target_location, go_to_wanted_item, interact_with, interact_with_door,
    melee_attack, mount, random_stroll, run_one, set_entity_look_target, set_look_and_interact, set_walk_target_away_from, set_walk_target_from_attack_target_if_target_out_of_reach,
    set_walk_target_from_look_target, start_attacking, start_celebrating_if_target_dead, stop_attacking_if_target_invalid, stop_being_angry_if_target_dead, trigger_gate,
};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::sensing::sensor;
use crate::world::entity::ai::util::land_random_pos;
use crate::world::entity::schedule::Activity;
use crate::world::entity::{EntityId, LevelEntity, entity_types, sound_event};
use crate::world::item::ItemStack;
use crate::world::item::component::swing_animation::SwingAnimation;
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::level::storage::loot::loot_table::LootTable;
use crate::world::{InteractionHand, InteractionResult};

pub const BARTERING_ITEM: wafer_data::Item = items::GOLD_INGOT;
const PLAYER_ANGER_RANGE: f64 = 16.0;
const ANGER_DURATION: i64 = 600;
const ADMIRE_DURATION: i32 = 119;
const MAX_DISTANCE_TO_WALK_TO_ITEM: i32 = 9;
const MAX_TIME_TO_WALK_TO_ITEM: i32 = 200;
const HOW_LONG_TIME_TO_DISABLE_ADMIRE_WALKING_IF_CANT_REACH_ITEM: i32 = 200;
const CELEBRATION_TIME: i32 = 300;
pub const TIME_BETWEEN_HUNTS: IntProvider = time_util::range_of_seconds(30, 120);
const BABY_FLEE_DURATION_AFTER_GETTING_HIT: i64 = 100;
const HIT_BY_PLAYER_MEMORY_TIMEOUT: i64 = 400;
const MAX_WALK_DISTANCE_TO_START_RIDING: i32 = 8;
const RIDE_START_INTERVAL: IntProvider = time_util::range_of_seconds(10, 40);
const RIDE_DURATION: IntProvider = time_util::range_of_seconds(10, 30);
const RETREAT_DURATION: IntProvider = time_util::range_of_seconds(5, 20);
const MELEE_ATTACK_COOLDOWN: i32 = 20;
const EAT_COOLDOWN: i64 = 200;
const DESIRED_DISTANCE_FROM_ENTITY_WHEN_AVOIDING: i32 = 12;
const MAX_LOOK_DIST: f32 = 8.0;
const MAX_LOOK_DIST_FOR_PLAYER_HOLDING_LOVED_ITEM: f32 = 14.0;
const INTERACTION_RANGE: i32 = 8;
const MIN_DESIRED_DIST_FROM_TARGET_WHEN_HOLDING_CROSSBOW: i32 = 5;
const SPEED_WHEN_STRAFING_BACK_FROM_TARGET: f32 = 0.75;
const DESIRED_DISTANCE_FROM_ZOMBIFIED: f64 = 6.0;
const AVOID_ZOMBIFIED_DURATION: IntProvider = time_util::range_of_seconds(5, 7);
const BABY_AVOID_NEMESIS_DURATION: IntProvider = time_util::range_of_seconds(5, 7);
const PROBABILITY_OF_CELEBRATION_DANCE: f32 = 0.1;
const SPEED_MULTIPLIER_WHEN_AVOIDING: f32 = 1.0;
const SPEED_MULTIPLIER_WHEN_RETREATING: f32 = 1.0;
const SPEED_MULTIPLIER_WHEN_MOUNTING: f32 = 0.8;
const SPEED_MULTIPLIER_WHEN_GOING_TO_WANTED_ITEM: f32 = 1.0;
const SPEED_MULTIPLIER_WHEN_GOING_TO_CELEBRATE_LOCATION: f32 = 1.0;
const SPEED_MULTIPLIER_WHEN_DANCING: f32 = 0.6;
const SPEED_MULTIPLIER_WHEN_IDLING: f32 = 0.6;

pub fn get_activities() -> Vec<ActivityData> {
    vec![init_core_activity(), init_idle_activity(), init_admire_item_activity(), init_fight_activity(), init_celebrate_activity(), init_retreat_activity(), init_ride_hoglin_activity()]
}

pub fn init_memories(level: &mut ServerLevel, body: EntityId) {
    let delay_until_first_hunt = TIME_BETWEEN_HUNTS.sample(&mut level.random);
    level.get_brain_mut(body).set_memory_with_expiry(memory_module_type::HUNTED_RECENTLY, true, delay_until_first_hunt as i64);
}

fn init_core_activity() -> ActivityData {
    ActivityData::create(
        Activity::CORE,
        0,
        vec![
            Box::new(LookAtTargetSink::new(45, 90)),
            Box::new(MoveToTargetSink::default()),
            Box::new(interact_with_door::create()),
            Box::new(baby_avoid_nemesis()),
            Box::new(avoid_zombified()),
            Box::new(stop_holding_item_if_no_longer_admiring::create()),
            Box::new(start_admiring_item_if_seen::create(ADMIRE_DURATION)),
            Box::new(start_celebrating_if_target_dead::create(CELEBRATION_TIME, wants_to_dance)),
            Box::new(stop_being_angry_if_target_dead::create()),
        ],
    )
}

fn init_idle_activity() -> ActivityData {
    ActivityData::create(
        Activity::IDLE,
        10,
        vec![
            Box::new(set_entity_look_target::create_with(is_player_holding_loved_item, MAX_LOOK_DIST_FOR_PLAYER_HOLDING_LOVED_ITEM)),
            Box::new(start_attacking::create_with(|level, body| !level.is_baby(body), find_nearest_valid_attack_target)),
            Box::new(behavior_builder::sequence(behavior_builder::trigger_if(|level, body| level.piglin_can_hunt(body)), start_hunting_hoglin::create())),
            Box::new(avoid_repellent()),
            Box::new(baby_sometimes_ride_baby_hoglin()),
            Box::new(create_idle_look_behaviors()),
            Box::new(create_idle_movement_behaviors()),
            Box::new(set_look_and_interact::create(&entity_types::PLAYER, 4)),
        ],
    )
}

fn init_fight_activity() -> ActivityData {
    ActivityData::create_with_memory(
        Activity::FIGHT,
        10,
        vec![
            Box::new(stop_attacking_if_target_invalid::create_when(|level, body, target| !is_nearest_valid_attack_target(level, body, target))),
            Box::new(behavior_builder::sequence(behavior_builder::trigger_if(has_crossbow), back_up_if_too_close::create(MIN_DESIRED_DIST_FROM_TARGET_WHEN_HOLDING_CROSSBOW, SPEED_WHEN_STRAFING_BACK_FROM_TARGET))),
            Box::new(set_walk_target_from_attack_target_if_target_out_of_reach::create_with(|_, _| 1.0)),
            Box::new(SpearApproach::new(1.0, 10.0)),
            Box::new(SpearAttack::new(1.0, 1.0, 2.0)),
            Box::new(SpearRetreat::new(1.0)),
            Box::new(melee_attack::create(MELEE_ATTACK_COOLDOWN)),
            Box::new(CrossbowAttack::default()),
            Box::new(remember_if_hoglin_was_killed::create()),
            Box::new(erase_memory_if::create(is_near_zombified, memory_module_type::ATTACK_TARGET)),
        ],
        memory_module_type::ATTACK_TARGET.erase(),
    )
}

fn init_celebrate_activity() -> ActivityData {
    let behaviors: Vec<(Box<dyn BehaviorControl>, i32)> = vec![
        (Box::new(set_entity_look_target::create_for_type(&entity_types::PIGLIN, MAX_LOOK_DIST)), 1),
        (Box::new(random_stroll::stroll_within(SPEED_MULTIPLIER_WHEN_DANCING, 2, 1)), 1),
        (Box::new(DoNothing::new(10, 20)), 1),
    ];
    ActivityData::create_with_memory(
        Activity::CELEBRATE,
        10,
        vec![
            Box::new(avoid_repellent()),
            Box::new(set_entity_look_target::create_with(is_player_holding_loved_item, MAX_LOOK_DIST_FOR_PLAYER_HOLDING_LOVED_ITEM)),
            Box::new(start_attacking::create_with(|level, body| !level.is_baby(body), find_nearest_valid_attack_target)),
            Box::new(behavior_builder::sequence(
                behavior_builder::trigger_if(|level, body| !level.piglin_is_dancing(body)),
                go_to_target_location::create(memory_module_type::CELEBRATE_LOCATION, 2, SPEED_MULTIPLIER_WHEN_GOING_TO_CELEBRATE_LOCATION),
            )),
            Box::new(behavior_builder::sequence(
                behavior_builder::trigger_if(|level, body| level.piglin_is_dancing(body)),
                go_to_target_location::create(memory_module_type::CELEBRATE_LOCATION, 4, SPEED_MULTIPLIER_WHEN_DANCING),
            )),
            Box::new(run_one::new(behaviors)),
        ],
        memory_module_type::CELEBRATE_LOCATION.erase(),
    )
}

fn init_admire_item_activity() -> ActivityData {
    ActivityData::create_with_memory(
        Activity::ADMIRE_ITEM,
        10,
        vec![
            Box::new(go_to_wanted_item::create_with(is_not_holding_loved_item_in_off_hand, SPEED_MULTIPLIER_WHEN_GOING_TO_WANTED_ITEM, true, MAX_DISTANCE_TO_WALK_TO_ITEM)),
            Box::new(stop_admiring_if_item_too_far_away::create(MAX_DISTANCE_TO_WALK_TO_ITEM)),
            Box::new(stop_admiring_if_tired_of_trying_to_reach_item::create(MAX_TIME_TO_WALK_TO_ITEM, HOW_LONG_TIME_TO_DISABLE_ADMIRE_WALKING_IF_CANT_REACH_ITEM)),
        ],
        memory_module_type::ADMIRING_ITEM.erase(),
    )
}

fn init_retreat_activity() -> ActivityData {
    ActivityData::create_with_memory(
        Activity::AVOID,
        10,
        vec![
            Box::new(set_walk_target_away_from::entity(memory_module_type::AVOID_TARGET, SPEED_MULTIPLIER_WHEN_RETREATING, DESIRED_DISTANCE_FROM_ENTITY_WHEN_AVOIDING, true)),
            Box::new(create_idle_look_behaviors()),
            Box::new(create_idle_movement_behaviors()),
            Box::new(erase_memory_if::create(wants_to_stop_fleeing, memory_module_type::AVOID_TARGET)),
        ],
        memory_module_type::AVOID_TARGET.erase(),
    )
}

fn init_ride_hoglin_activity() -> ActivityData {
    let mut look_behaviors: Vec<(Box<dyn Trigger>, i32)> = create_look_behaviors().into_iter().map(|(behavior, weight)| (Box::new(behavior) as Box<dyn Trigger>, weight)).collect();
    look_behaviors.push((Box::new(behavior_builder::trigger_if(|_, _| true)), 1));
    ActivityData::create_with_memory(
        Activity::RIDE,
        10,
        vec![
            Box::new(mount::create(SPEED_MULTIPLIER_WHEN_MOUNTING)),
            Box::new(set_entity_look_target::create_with(is_player_holding_loved_item, MAX_LOOK_DIST)),
            Box::new(behavior_builder::sequence(behavior_builder::trigger_if(|level, body| level.entity(body).vehicle.is_some()), trigger_gate::trigger_one_shuffled(look_behaviors))),
            Box::new(dismount_or_skip_mounting::create(MAX_WALK_DISTANCE_TO_START_RIDING, wants_to_stop_riding)),
        ],
        memory_module_type::RIDE_TARGET.erase(),
    )
}

fn create_look_behaviors() -> Vec<(OneShot, i32)> {
    vec![
        (set_entity_look_target::create_for_type(&entity_types::PLAYER, MAX_LOOK_DIST), 1),
        (set_entity_look_target::create_for_type(&entity_types::PIGLIN, MAX_LOOK_DIST), 1),
        (set_entity_look_target::create_with(|_, _| true, MAX_LOOK_DIST), 1),
    ]
}

fn create_idle_look_behaviors() -> GateBehavior {
    let mut behaviors: Vec<(Box<dyn BehaviorControl>, i32)> = create_look_behaviors().into_iter().map(|(behavior, weight)| (Box::new(behavior) as Box<dyn BehaviorControl>, weight)).collect();
    behaviors.push((Box::new(DoNothing::new(30, 60)), 1));
    run_one::new(behaviors)
}

fn create_idle_movement_behaviors() -> GateBehavior {
    let behaviors: Vec<(Box<dyn BehaviorControl>, i32)> = vec![
        (Box::new(random_stroll::stroll(SPEED_MULTIPLIER_WHEN_IDLING)), 2),
        (Box::new(interact_with::of(&entity_types::PIGLIN, INTERACTION_RANGE, memory_module_type::INTERACTION_TARGET, SPEED_MULTIPLIER_WHEN_IDLING, 2)), 2),
        (
            Box::new(behavior_builder::sequence(
                behavior_builder::trigger_if(doesnt_see_any_player_holding_loved_item),
                set_walk_target_from_look_target::create(SPEED_MULTIPLIER_WHEN_IDLING, 3),
            )),
            2,
        ),
        (Box::new(DoNothing::new(30, 60)), 1),
    ];
    run_one::new(behaviors)
}

fn avoid_repellent() -> OneShot {
    set_walk_target_away_from::pos(memory_module_type::NEAREST_REPELLENT, SPEED_MULTIPLIER_WHEN_AVOIDING, 8, false)
}

fn baby_avoid_nemesis() -> OneShot {
    copy_memory_with_expiry::create(|level, body| level.is_baby(body), memory_module_type::NEAREST_VISIBLE_NEMESIS, memory_module_type::AVOID_TARGET, BABY_AVOID_NEMESIS_DURATION)
}

fn avoid_zombified() -> OneShot {
    copy_memory_with_expiry::create(|level, body| is_near_zombified(level, body), memory_module_type::NEAREST_VISIBLE_ZOMBIFIED, memory_module_type::AVOID_TARGET, AVOID_ZOMBIFIED_DURATION)
}

pub fn update_activity(level: &mut ServerLevel, body: EntityId) {
    let brain = level.get_brain_mut(body);
    let old_activity = brain.get_active_non_core_activity();
    brain.set_active_activity_to_first_valid(&[Activity::ADMIRE_ITEM, Activity::FIGHT, Activity::AVOID, Activity::CELEBRATE, Activity::RIDE, Activity::IDLE]);
    let new_activity = brain.get_active_non_core_activity();
    if old_activity != new_activity
        && let Some(sound) = get_sound_for_current_activity(level, body)
    {
        level.make_sound(body, sound_event(sound));
    }
    let aggressive = level.get_brain(body).has_memory_value(memory_module_type::ATTACK_TARGET);
    level.mob_mut(body).expect("a piglin").set_aggressive(aggressive);
    if !level.get_brain(body).has_memory_value(memory_module_type::RIDE_TARGET) && is_baby_riding_baby(level, body) {
        level.stop_riding(body);
    }
    if !level.get_brain(body).has_memory_value(memory_module_type::CELEBRATE_LOCATION) {
        level.get_brain_mut(body).erase_memory(memory_module_type::DANCING);
    }
    let dancing = level.get_brain(body).has_memory_value(memory_module_type::DANCING);
    level.piglin_set_dancing(body, dancing);
}

fn is_baby_riding_baby(level: &ServerLevel, body: EntityId) -> bool {
    if !level.is_baby(body) {
        return false;
    }
    let Some(vehicle) = level.entity(body).vehicle else { return false };
    let vehicle_type = level.entity(vehicle).entity_type;
    (vehicle_type == entity_types::PIGLIN.id || vehicle_type == entity_types::HOGLIN.id) && level.is_baby(vehicle)
}

pub fn pick_up_item(level: &mut ServerLevel, body: EntityId, item_entity: EntityId) {
    stop_walking(level, body);
    let Some(LevelEntity::Item(item)) = level.entities.get(&item_entity) else { return };
    let taken = if item.item.is(items::GOLD_NUGGET) {
        let count = item.item.count;
        level.take(body, item_entity, count);
        let Some(LevelEntity::Item(item)) = level.entities.get(&item_entity) else { return };
        let taken = item.item.clone();
        level.discard(item_entity);
        taken
    } else {
        level.take(body, item_entity, 1);
        remove_one_item_from_item_entity(level, item_entity)
    };
    if is_loved_item(&taken) {
        level.get_brain_mut(body).erase_memory(memory_module_type::TIME_TRYING_TO_REACH_ADMIRE_ITEM);
        hold_in_offhand(level, body, taken);
        admire_gold_item(level, body);
    } else if is_food(&taken) && !has_eaten_recently(level, body) {
        eat(level, body);
    } else {
        let item_equipped = !level.equip_item_if_possible(body, taken.clone()).is_empty();
        if !item_equipped {
            put_in_inventory(level, body, taken);
        }
    }
}

fn hold_in_offhand(level: &mut ServerLevel, body: EntityId, item_stack: ItemStack) {
    if is_holding_item_in_off_hand(level, body) {
        let off_hand_item = level.get_item_in_hand(body, InteractionHand::OffHand).unwrap_or_else(ItemStack::empty);
        level.spawn_at_location(body, off_hand_item, 0.0);
    }
    level.piglin_hold_in_off_hand(body, item_stack);
}

fn remove_one_item_from_item_entity(level: &mut ServerLevel, item_entity: EntityId) -> ItemStack {
    let Some(LevelEntity::Item(item)) = level.entities.get_mut(&item_entity) else { return ItemStack::empty() };
    let removed_stack = item.item.split(1);
    if item.item.is_empty() {
        level.discard(item_entity);
    }
    removed_stack
}

pub fn stop_holding_off_hand_item(level: &mut ServerLevel, body: EntityId, bartering_enabled: bool) {
    let item_stack = level.get_item_in_hand(body, InteractionHand::OffHand).unwrap_or_else(ItemStack::empty);
    level.set_item_in_hand(body, InteractionHand::OffHand, ItemStack::empty());
    if !level.is_baby(body) {
        let barter_currency = is_barter_currency(&item_stack);
        if bartering_enabled && barter_currency {
            let barter_response_items = get_barter_response_items(level, body);
            throw_items(level, body, barter_response_items);
        } else if !barter_currency {
            let equipped = !level.equip_item_if_possible(body, item_stack.clone()).is_empty();
            if !equipped {
                put_in_inventory(level, body, item_stack);
            }
        }
    } else {
        let equipped = !level.equip_item_if_possible(body, item_stack.clone()).is_empty();
        if !equipped {
            let main_hand_item = level.get_item_in_hand(body, InteractionHand::MainHand).unwrap_or_else(ItemStack::empty);
            if is_loved_item(&main_hand_item) {
                put_in_inventory(level, body, main_hand_item);
            } else {
                throw_items(level, body, vec![main_hand_item]);
            }
            level.piglin_hold_in_main_hand(body, item_stack);
        }
    }
}

pub fn cancel_admiring(level: &mut ServerLevel, body: EntityId) {
    if is_admiring_item(level, body)
        && let Some(off_hand_item) = level.get_item_in_hand(body, InteractionHand::OffHand).filter(|item| !item.is_empty())
    {
        level.spawn_at_location(body, off_hand_item, 0.0);
        level.set_item_in_hand(body, InteractionHand::OffHand, ItemStack::empty());
    }
}

fn put_in_inventory(level: &mut ServerLevel, body: EntityId, item_stack: ItemStack) {
    let stuff_that_couldnt_fit_in_my_inventory = level.piglin_add_to_inventory(body, &item_stack);
    throw_items_toward_random_pos(level, body, vec![stuff_that_couldnt_fit_in_my_inventory]);
}

fn throw_items(level: &mut ServerLevel, body: EntityId, item_stacks: Vec<ItemStack>) {
    match level.get_brain(body).get_memory(memory_module_type::NEAREST_VISIBLE_PLAYER).copied() {
        Some(player) => throw_items_toward_player(level, body, player, item_stacks),
        None => throw_items_toward_random_pos(level, body, item_stacks),
    }
}

fn throw_items_toward_random_pos(level: &mut ServerLevel, body: EntityId, item_stacks: Vec<ItemStack>) {
    let target_pos = get_random_nearby_pos(level, body);
    throw_items_toward_pos(level, body, item_stacks, target_pos);
}

fn throw_items_toward_player(level: &mut ServerLevel, body: EntityId, player: EntityId, item_stacks: Vec<ItemStack>) {
    let target_pos = level.entity(player).position();
    throw_items_toward_pos(level, body, item_stacks, target_pos);
}

fn throw_items_toward_pos(level: &mut ServerLevel, body: EntityId, item_stacks: Vec<ItemStack>, target_pos: Vec3) {
    if !item_stacks.is_empty() {
        level.swing(body, InteractionHand::OffHand, SwingAnimation::DEFAULT, false);
        for item_stack in item_stacks {
            behavior_utils::throw_item(level, body, item_stack, target_pos.add(0.0, 1.0, 0.0));
        }
    }
}

fn get_barter_response_items(level: &mut ServerLevel, body: EntityId) -> Vec<ItemStack> {
    let loot_table = LootTable::get("minecraft:gameplay/piglin_bartering");
    let mut params = LootParams::new(level);
    params.this_entity = Some(body);
    loot_table.get_random_items(params)
}

fn wants_to_dance(level: &ServerLevel, _body: EntityId, killed_target: EntityId) -> bool {
    level.entity(killed_target).entity_type == entity_types::HOGLIN.id && LegacyRandomSource::new(level.game_time).next_float() < PROBABILITY_OF_CELEBRATION_DANCE
}

pub fn wants_to_pickup(level: &ServerLevel, body: EntityId, item_stack: &ItemStack) -> bool {
    if level.is_baby(body) && item_stack.get_item().is(&item_tags::IGNORED_BY_PIGLIN_BABIES) {
        false
    } else if item_stack.get_item().is(&item_tags::PIGLIN_REPELLENTS) {
        false
    } else if is_admiring_disabled(level, body) && level.get_brain(body).has_memory_value(memory_module_type::ATTACK_TARGET) {
        false
    } else if is_barter_currency(item_stack) {
        is_not_holding_loved_item_in_off_hand(level, body)
    } else {
        let has_space = level.piglin_can_add_to_inventory(body, item_stack);
        if item_stack.is(items::GOLD_NUGGET) {
            has_space
        } else if is_food(item_stack) {
            !has_eaten_recently(level, body) && has_space
        } else if !is_loved_item(item_stack) {
            level.piglin_can_replace_current_item(body, item_stack)
        } else {
            is_not_holding_loved_item_in_off_hand(level, body) && has_space
        }
    }
}

pub fn is_loved_item(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::PIGLIN_LOVED)
}

fn wants_to_stop_riding(level: &ServerLevel, body: EntityId, entity_being_ridden: EntityId) -> bool {
    let Some(mob_being_ridden) = level.mob(entity_being_ridden) else { return false };
    !mob_being_ridden.is_baby()
        || !level.is_alive(entity_being_ridden)
        || was_hurt_recently(level, body)
        || was_hurt_recently(level, entity_being_ridden)
        || level.entity(entity_being_ridden).entity_type == entity_types::PIGLIN.id && level.entity(entity_being_ridden).vehicle.is_none()
}

fn is_nearest_valid_attack_target(level: &mut ServerLevel, body: EntityId, target: EntityId) -> bool {
    find_nearest_valid_attack_target(level, body) == Some(target)
}

fn is_near_zombified(level: &ServerLevel, body: EntityId) -> bool {
    match level.get_brain(body).get_memory(memory_module_type::NEAREST_VISIBLE_ZOMBIFIED).copied() {
        Some(zombified) => closer_than(level, body, zombified, DESIRED_DISTANCE_FROM_ZOMBIFIED),
        None => false,
    }
}

fn closer_than(level: &ServerLevel, entity: EntityId, other: EntityId, distance: f64) -> bool {
    level.entity(entity).distance_to_sqr(level.entity(other)) < distance * distance
}

fn find_nearest_valid_attack_target(level: &mut ServerLevel, body: EntityId) -> Option<EntityId> {
    if is_near_zombified(level, body) {
        return None;
    }
    let angry_at = behavior_utils::get_living_entity_from_uuid_memory(level, body, memory_module_type::ANGRY_AT);
    if let Some(angry_at) = angry_at
        && sensor::is_entity_attackable_ignoring_line_of_sight(level, body, angry_at)
    {
        return Some(angry_at);
    }
    let brain = level.get_brain(body);
    if brain.has_memory_value(memory_module_type::UNIVERSAL_ANGER)
        && let Some(player) = brain.get_memory(memory_module_type::NEAREST_VISIBLE_ATTACKABLE_PLAYER).copied()
    {
        return Some(player);
    }
    if let Some(nemesis) = brain.get_memory(memory_module_type::NEAREST_VISIBLE_NEMESIS).copied() {
        return Some(nemesis);
    }
    let player_not_wearing_gold = brain.get_memory(memory_module_type::NEAREST_TARGETABLE_PLAYER_NOT_WEARING_GOLD).copied()?;
    sensor::is_entity_attackable(level, body, player_not_wearing_gold).then_some(player_not_wearing_gold)
}

pub fn anger_nearby_piglins(level: &mut ServerLevel, player: EntityId, only_if_they_see_the_player: bool) {
    let area = level.entity(player).bounding_box().inflate(PLAYER_ANGER_RANGE, PLAYER_ANGER_RANGE, PLAYER_ANGER_RANGE);
    let nearby_piglins = level.get_entities_matching(None, &area, |level, entity| !level.is_spectator(entity) && level.entity(entity).entity_type == entity_types::PIGLIN.id);
    for piglin in nearby_piglins {
        if !is_idle(level, piglin) {
            continue;
        }
        if only_if_they_see_the_player && !behavior_utils::can_see(level, piglin, player) {
            continue;
        }
        if level.game_rules.get_bool(game_rule::UNIVERSAL_ANGER) {
            set_anger_target_to_nearest_targetable_player_if_found(level, piglin, player);
        } else {
            set_anger_target(level, piglin, player);
        }
    }
}

pub fn mob_interact(level: &mut ServerLevel, body: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
    let player_held_item_stack = level.get_item_in_hand(player, hand).unwrap_or_else(ItemStack::empty);
    if can_admire(level, body, &player_held_item_stack) {
        let mut held = player_held_item_stack;
        let taken = held.consume_and_return(1, level, Some(player));
        level.set_item_in_hand(player, hand, held);
        hold_in_offhand(level, body, taken);
        admire_gold_item(level, body);
        stop_walking(level, body);
        InteractionResult::SUCCESS
    } else {
        InteractionResult::Pass
    }
}

pub fn can_admire(level: &ServerLevel, body: EntityId, player_held_item_stack: &ItemStack) -> bool {
    !is_admiring_disabled(level, body) && !is_admiring_item(level, body) && !level.is_baby(body) && is_barter_currency(player_held_item_stack)
}

pub fn was_hurt_by(level: &mut ServerLevel, body: EntityId, attacker: EntityId) {
    let attacker_type = level.entity(attacker).entity_type;
    if attacker_type == entity_types::PIGLIN.id {
        return;
    }
    if is_holding_item_in_off_hand(level, body) {
        stop_holding_off_hand_item(level, body, false);
    }
    let brain = level.get_brain_mut(body);
    brain.erase_memory(memory_module_type::CELEBRATE_LOCATION);
    brain.erase_memory(memory_module_type::DANCING);
    brain.erase_memory(memory_module_type::ADMIRING_ITEM);
    if level.is_player(attacker) {
        level.get_brain_mut(body).set_memory_with_expiry(memory_module_type::ADMIRING_DISABLED, true, HIT_BY_PLAYER_MEMORY_TIMEOUT);
    }
    if let Some(avoid_target) = get_avoid_target(level, body)
        && level.entity(avoid_target).entity_type != attacker_type
    {
        level.get_brain_mut(body).erase_memory(memory_module_type::AVOID_TARGET);
    }
    if level.is_baby(body) {
        level.get_brain_mut(body).set_memory_with_expiry(memory_module_type::AVOID_TARGET, attacker, BABY_FLEE_DURATION_AFTER_GETTING_HIT);
        if sensor::is_entity_attackable_ignoring_line_of_sight(level, body, attacker) {
            broadcast_anger_target(level, body, attacker);
        }
    } else if attacker_type == entity_types::HOGLIN.id && hoglins_outnumber_piglins(level, body) {
        set_avoid_target_and_dont_hunt_for_a_while(level, body, attacker);
        broadcast_retreat(level, body, attacker);
    } else {
        maybe_retaliate(level, body, attacker);
    }
}

pub fn maybe_retaliate(level: &mut ServerLevel, body: EntityId, attacker: EntityId) {
    if level.get_brain(body).is_active(Activity::AVOID) {
        return;
    }
    if !sensor::is_entity_attackable_ignoring_line_of_sight(level, body, attacker) {
        return;
    }
    if behavior_utils::is_other_target_much_further_away_than_current_attack_target(level, body, attacker, 4.0) {
        return;
    }
    if level.is_player(attacker) && level.game_rules.get_bool(game_rule::UNIVERSAL_ANGER) {
        set_anger_target_to_nearest_targetable_player_if_found(level, body, attacker);
        broadcast_universal_anger(level, body);
    } else {
        set_anger_target(level, body, attacker);
        broadcast_anger_target(level, body, attacker);
    }
}

pub fn get_sound_for_current_activity(level: &ServerLevel, body: EntityId) -> Option<&'static str> {
    level.get_brain(body).get_active_non_core_activity().map(|activity| get_sound_for_activity(level, body, activity))
}

fn get_sound_for_activity(level: &ServerLevel, body: EntityId, activity: Activity) -> &'static str {
    if activity == Activity::FIGHT {
        "minecraft:entity.piglin.angry"
    } else if level.abstract_piglin_is_converting(body) {
        "minecraft:entity.piglin.retreat"
    } else if activity == Activity::AVOID && is_near_avoid_target(level, body) {
        "minecraft:entity.piglin.retreat"
    } else if activity == Activity::ADMIRE_ITEM {
        "minecraft:entity.piglin.admiring_item"
    } else if activity == Activity::CELEBRATE {
        "minecraft:entity.piglin.celebrate"
    } else if sees_player_holding_loved_item(level, body) {
        "minecraft:entity.piglin.jealous"
    } else if is_near_repellent(level, body) {
        "minecraft:entity.piglin.retreat"
    } else {
        "minecraft:entity.piglin.ambient"
    }
}

fn is_near_avoid_target(level: &ServerLevel, body: EntityId) -> bool {
    match level.get_brain(body).get_memory(memory_module_type::AVOID_TARGET).copied() {
        Some(avoid_target) => closer_than(level, avoid_target, body, 12.0),
        None => false,
    }
}

fn get_visible_adult_piglins(level: &ServerLevel, body: EntityId) -> Vec<EntityId> {
    level.get_brain(body).get_memory(memory_module_type::NEAREST_VISIBLE_ADULT_PIGLINS).cloned().unwrap_or_default()
}

fn get_adult_piglins(level: &ServerLevel, body: EntityId) -> Vec<EntityId> {
    level.get_brain(body).get_memory(memory_module_type::NEARBY_ADULT_PIGLINS).cloned().unwrap_or_default()
}

pub fn is_wearing_safe_armor(level: &ServerLevel, living_entity: EntityId) -> bool {
    [EquipmentSlot::Feet, EquipmentSlot::Legs, EquipmentSlot::Chest, EquipmentSlot::Head]
        .into_iter()
        .any(|slot| level.item_by_slot_ref(living_entity, slot).is_some_and(|item| item.get_item().is(&item_tags::PIGLIN_SAFE_ARMOR)))
}

fn stop_walking(level: &mut ServerLevel, body: EntityId) {
    level.get_brain_mut(body).erase_memory(memory_module_type::WALK_TARGET);
    level.mob_mut(body).expect("a piglin").ai.navigation.stop();
}

fn baby_sometimes_ride_baby_hoglin() -> OneShot {
    let ticker = Rc::new(RefCell::new(Ticker::new(RIDE_START_INTERVAL)));
    copy_memory_with_expiry::create(
        move |level, body| level.is_baby(body) && ticker.borrow_mut().tick_down_and_check(&mut level.random),
        memory_module_type::NEAREST_VISIBLE_BABY_HOGLIN,
        memory_module_type::RIDE_TARGET,
        RIDE_DURATION,
    )
}

pub fn broadcast_anger_target(level: &mut ServerLevel, body: EntityId, target: EntityId) {
    for piglin in get_adult_piglins(level, body) {
        let target_is_hoglin = level.entity(target).entity_type == entity_types::HOGLIN.id;
        if !(target_is_hoglin && (!level.abstract_piglin_can_hunt(piglin) || !level.hoglin_can_be_hunted(target))) {
            set_anger_target_if_closer_than_current(level, piglin, target);
        }
    }
}

fn broadcast_universal_anger(level: &mut ServerLevel, body: EntityId) {
    for piglin in get_adult_piglins(level, body) {
        if let Some(player) = get_nearest_visible_targetable_player(level, piglin) {
            set_anger_target(level, piglin, player);
        }
    }
}

pub fn set_anger_target(level: &mut ServerLevel, body: EntityId, target: EntityId) {
    if !sensor::is_entity_attackable_ignoring_line_of_sight(level, body, target) {
        return;
    }
    let target_uuid = level.entity(target).uuid;
    let brain = level.get_brain_mut(body);
    brain.erase_memory(memory_module_type::CANT_REACH_WALK_TARGET_SINCE);
    brain.set_memory_with_expiry(memory_module_type::ANGRY_AT, target_uuid, ANGER_DURATION);
    if level.entity(target).entity_type == entity_types::HOGLIN.id && level.abstract_piglin_can_hunt(body) {
        dont_kill_any_more_hoglins_for_a_while(level, body);
    }
    if level.is_player(target) && level.game_rules.get_bool(game_rule::UNIVERSAL_ANGER) {
        level.get_brain_mut(body).set_memory_with_expiry(memory_module_type::UNIVERSAL_ANGER, true, ANGER_DURATION);
    }
}

fn set_anger_target_to_nearest_targetable_player_if_found(level: &mut ServerLevel, body: EntityId, target_if_no_player_found: EntityId) {
    match get_nearest_visible_targetable_player(level, body) {
        Some(nearest_player) => set_anger_target(level, body, nearest_player),
        None => set_anger_target(level, body, target_if_no_player_found),
    }
}

fn set_anger_target_if_closer_than_current(level: &mut ServerLevel, body: EntityId, new_target: EntityId) {
    let current_target = get_anger_target(level, body);
    let nearest = behavior_utils::get_nearest_target(level, body, current_target, new_target);
    if current_target != Some(nearest) {
        set_anger_target(level, body, nearest);
    }
}

fn get_anger_target(level: &ServerLevel, body: EntityId) -> Option<EntityId> {
    behavior_utils::get_living_entity_from_uuid_memory(level, body, memory_module_type::ANGRY_AT)
}

pub fn get_avoid_target(level: &ServerLevel, body: EntityId) -> Option<EntityId> {
    level.get_brain(body).get_memory(memory_module_type::AVOID_TARGET).copied()
}

pub fn get_nearest_visible_targetable_player(level: &ServerLevel, body: EntityId) -> Option<EntityId> {
    let brain = level.get_brain(body);
    if brain.has_memory_value(memory_module_type::NEAREST_VISIBLE_ATTACKABLE_PLAYER) { brain.get_memory(memory_module_type::NEAREST_VISIBLE_ATTACKABLE_PLAYER).copied() } else { None }
}

fn broadcast_retreat(level: &mut ServerLevel, body: EntityId, target: EntityId) {
    for abstract_piglin in get_visible_adult_piglins(level, body) {
        if level.entity(abstract_piglin).entity_type == entity_types::PIGLIN.id {
            retreat_from_nearest_target(level, abstract_piglin, target);
        }
    }
}

fn retreat_from_nearest_target(level: &mut ServerLevel, body: EntityId, new_avoid_target: EntityId) {
    let mut nearest = new_avoid_target;
    let avoid_target = level.get_brain(body).get_memory(memory_module_type::AVOID_TARGET).copied();
    nearest = behavior_utils::get_nearest_target(level, body, avoid_target, nearest);
    let attack_target = level.get_brain(body).get_memory(memory_module_type::ATTACK_TARGET).copied();
    nearest = behavior_utils::get_nearest_target(level, body, attack_target, nearest);
    set_avoid_target_and_dont_hunt_for_a_while(level, body, nearest);
}

fn wants_to_stop_fleeing(level: &ServerLevel, body: EntityId) -> bool {
    let brain = level.get_brain(body);
    let Some(avoided_entity) = brain.get_memory(memory_module_type::AVOID_TARGET).copied() else { return true };
    if level.entity(avoided_entity).entity_type == entity_types::HOGLIN.id {
        piglins_equal_or_outnumber_hoglins(level, body)
    } else if is_zombified(level, avoided_entity) {
        !brain.is_memory_value(memory_module_type::NEAREST_VISIBLE_ZOMBIFIED, &avoided_entity)
    } else {
        false
    }
}

fn piglins_equal_or_outnumber_hoglins(level: &ServerLevel, body: EntityId) -> bool {
    !hoglins_outnumber_piglins(level, body)
}

fn hoglins_outnumber_piglins(level: &ServerLevel, body: EntityId) -> bool {
    let brain = level.get_brain(body);
    let piglin_count = brain.get_memory(memory_module_type::VISIBLE_ADULT_PIGLIN_COUNT).copied().unwrap_or(0) + 1;
    let hoglin_count = brain.get_memory(memory_module_type::VISIBLE_ADULT_HOGLIN_COUNT).copied().unwrap_or(0);
    hoglin_count > piglin_count
}

fn set_avoid_target_and_dont_hunt_for_a_while(level: &mut ServerLevel, body: EntityId, target: EntityId) {
    let brain = level.get_brain_mut(body);
    brain.erase_memory(memory_module_type::ANGRY_AT);
    brain.erase_memory(memory_module_type::ATTACK_TARGET);
    brain.erase_memory(memory_module_type::WALK_TARGET);
    let retreat_duration = RETREAT_DURATION.sample(&mut level.random);
    level.get_brain_mut(body).set_memory_with_expiry(memory_module_type::AVOID_TARGET, target, retreat_duration as i64);
    dont_kill_any_more_hoglins_for_a_while(level, body);
}

pub fn dont_kill_any_more_hoglins_for_a_while(level: &mut ServerLevel, body: EntityId) {
    let duration = TIME_BETWEEN_HUNTS.sample(&mut level.random);
    level.get_brain_mut(body).set_memory_with_expiry(memory_module_type::HUNTED_RECENTLY, true, duration as i64);
}

fn eat(level: &mut ServerLevel, body: EntityId) {
    level.get_brain_mut(body).set_memory_with_expiry(memory_module_type::ATE_RECENTLY, true, EAT_COOLDOWN);
}

fn get_random_nearby_pos(level: &mut ServerLevel, body: EntityId) -> Vec3 {
    land_random_pos::get_pos(level, body, 4, 2).unwrap_or_else(|| level.entity(body).position())
}

fn has_eaten_recently(level: &ServerLevel, body: EntityId) -> bool {
    level.get_brain(body).has_memory_value(memory_module_type::ATE_RECENTLY)
}

pub fn is_idle(level: &ServerLevel, body: EntityId) -> bool {
    level.get_brain(body).is_active(Activity::IDLE)
}

fn has_crossbow(level: &ServerLevel, body: EntityId) -> bool {
    level.is_holding(body, items::CROSSBOW)
}

fn admire_gold_item(level: &mut ServerLevel, body: EntityId) {
    level.get_brain_mut(body).set_memory_with_expiry(memory_module_type::ADMIRING_ITEM, true, ADMIRE_DURATION as i64);
}

fn is_admiring_item(level: &ServerLevel, body: EntityId) -> bool {
    level.get_brain(body).has_memory_value(memory_module_type::ADMIRING_ITEM)
}

fn is_barter_currency(item_stack: &ItemStack) -> bool {
    item_stack.is(BARTERING_ITEM)
}

fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::PIGLIN_FOOD)
}

fn is_near_repellent(level: &ServerLevel, body: EntityId) -> bool {
    level.get_brain(body).has_memory_value(memory_module_type::NEAREST_REPELLENT)
}

fn sees_player_holding_loved_item(level: &ServerLevel, body: EntityId) -> bool {
    level.get_brain(body).has_memory_value(memory_module_type::NEAREST_PLAYER_HOLDING_WANTED_ITEM)
}

fn doesnt_see_any_player_holding_loved_item(level: &ServerLevel, body: EntityId) -> bool {
    !sees_player_holding_loved_item(level, body)
}

pub fn is_player_holding_loved_item(level: &ServerLevel, entity: EntityId) -> bool {
    level.is_player(entity)
        && [EquipmentSlot::Mainhand, EquipmentSlot::Offhand].into_iter().any(|slot| level.item_by_slot_ref(entity, slot).is_some_and(is_loved_item))
}

fn is_admiring_disabled(level: &ServerLevel, body: EntityId) -> bool {
    level.get_brain(body).has_memory_value(memory_module_type::ADMIRING_DISABLED)
}

fn was_hurt_recently(level: &ServerLevel, body: EntityId) -> bool {
    level.get_brain_if_present(body).is_some_and(|brain| brain.has_memory_value(memory_module_type::HURT_BY))
}

fn is_holding_item_in_off_hand(level: &ServerLevel, body: EntityId) -> bool {
    level.item_by_slot_ref(body, EquipmentSlot::Offhand).is_some()
}

fn is_not_holding_loved_item_in_off_hand(level: &ServerLevel, body: EntityId) -> bool {
    level.item_by_slot_ref(body, EquipmentSlot::Offhand).is_none_or(|off_hand_item| !is_loved_item(off_hand_item))
}

pub fn is_zombified(level: &ServerLevel, entity: EntityId) -> bool {
    let entity_type = level.entity(entity).entity_type;
    entity_type == entity_types::ZOMBIFIED_PIGLIN.id || entity_type == entity_types::ZOGLIN.id
}

pub fn find_nearby_adult_piglins(level: &ServerLevel, body: EntityId) -> Vec<EntityId> {
    let living_entities = level.get_brain(body).get_memory(memory_module_type::NEAREST_LIVING_ENTITIES).cloned().unwrap_or_default();
    living_entities.into_iter().filter(|&entity| level.is_abstract_piglin(entity) && !level.is_baby(entity)).collect()
}
