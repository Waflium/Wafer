use wafer_core::BlockPos;
use wafer_data::block_tags;
use wafer_data::properties::OCCUPIED;

use super::behavior_control::BehaviorControl;
use super::declarative::behavior_builder;
use super::declarative::trigger::Trigger;
use super::do_nothing::DoNothing;
use super::gate_behavior::{GateBehavior, OrderPolicy, RunningPolicy};
use super::give_gift_to_hero::GiveGiftToHero;
use super::go_to_potential_job_site::GoToPotentialJobSite;
use super::harvest_farmland::HarvestFarmland;
use super::jump_on_bed::JumpOnBed;
use super::look_and_follow_trading_player_sink::LookAndFollowTradingPlayerSink;
use super::look_at_target_sink::LookAtTargetSink;
use super::move_to_target_sink::MoveToTargetSink;
use super::show_trades_to_player::ShowTradesToPlayer;
use super::sleep_in_bed::SleepInBed;
use super::swim::Swim;
use super::trade_with_villager::TradeWithVillager;
use super::use_bonemeal::UseBonemeal;
use super::villager_make_love::VillagerMakeLove;
use super::villager_panic_trigger::VillagerPanicTrigger;
use super::work_at_poi::WorkAtPoi;
use super::{
    acquire_poi, assign_profession_from_job_site, celebrate_villagers_survived_raid, go_to_closest_village, go_to_wanted_item, inside_brownian_walk, interact_with, interact_with_door,
    locate_hiding_place, move_to_sky_seeing_spot, play_tag_with_other_kids, poi_competitor_scan, react_to_bell, reset_profession, reset_raid_status, ring_bell, run_one,
    set_closest_home_as_walk_target, set_entity_look_target, set_hidden_state, set_look_and_interact, set_raid_status, set_walk_target_away_from, set_walk_target_from_block_memory,
    set_walk_target_from_look_target, socialize_at_bell, stroll_around_poi, stroll_to_poi, stroll_to_poi_list, trigger_gate, update_activity_from_schedule, validate_nearby_poi,
    village_bound_random_stroll, villager_calm_down, wake_up, yield_job_site,
};
use crate::server::level::ServerLevel;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::ai::village::poi::poi_types::PoiType;
use crate::world::entity::mob_category::MobCategory;
use crate::world::entity::npc::villager::villager;
use crate::world::entity::npc::villager::villager_profession::ProfessionId;
use crate::world::entity::{EntityId, entity_event, entity_types};
use crate::world::level::BlockGetter;

const STROLL_SPEED_MODIFIER: f32 = 0.4;

type Package = Vec<(i32, Box<dyn BehaviorControl>)>;

fn pair(priority: i32, behavior: impl BehaviorControl) -> (i32, Box<dyn BehaviorControl>) {
    (priority, Box::new(behavior))
}

fn weighted(behavior: impl BehaviorControl, weight: i32) -> (Box<dyn BehaviorControl>, i32) {
    (Box::new(behavior), weight)
}

fn trigger(behavior: impl Trigger + 'static, weight: i32) -> (Box<dyn Trigger>, i32) {
    (Box::new(behavior), weight)
}

pub fn get_core_package(profession: ProfessionId, speed_modifier: f32) -> Package {
    let held_job_site = profession.value().held_job_site;
    let acquirable_job_site = profession.value().acquirable_job_site;
    vec![
        pair(0, Swim::new(0.8)),
        pair(0, interact_with_door::create()),
        pair(0, LookAtTargetSink::new(45, 90)),
        pair(0, VillagerPanicTrigger::default()),
        pair(0, wake_up::create()),
        pair(0, react_to_bell::create()),
        pair(0, set_raid_status::create()),
        pair(0, validate_nearby_poi::create(move |poi_type| held_job_site.test(poi_type), memory_module_type::JOB_SITE)),
        pair(0, validate_nearby_poi::create(move |poi_type| acquirable_job_site.test(poi_type), memory_module_type::POTENTIAL_JOB_SITE)),
        pair(1, MoveToTargetSink::default()),
        pair(2, poi_competitor_scan::create()),
        pair(3, LookAndFollowTradingPlayerSink::new(speed_modifier)),
        pair(5, go_to_wanted_item::create(speed_modifier, false, 4)),
        pair(
            6,
            acquire_poi::create_validating(move |poi_type| acquirable_job_site.test(poi_type), memory_module_type::JOB_SITE, memory_module_type::POTENTIAL_JOB_SITE, true, None, |_, _| true),
        ),
        pair(7, GoToPotentialJobSite::new(speed_modifier)),
        pair(8, yield_job_site::create(speed_modifier)),
        pair(10, acquire_poi::create(|poi_type| poi_type == PoiType::Home, memory_module_type::HOME, false, Some(entity_event::VILLAGER_HAPPY), validate_bed_poi)),
        pair(10, acquire_poi::create(|poi_type| poi_type == PoiType::Meeting, memory_module_type::MEETING_POINT, true, Some(entity_event::VILLAGER_HAPPY), |_, _| true)),
        pair(10, assign_profession_from_job_site::create()),
        pair(10, reset_profession::create()),
    ]
}

fn validate_bed_poi(level: &mut ServerLevel, block_pos: BlockPos) -> bool {
    let block_state = level.block_state(block_pos);
    block_state.block().is(&block_tags::VILLAGERS_CAN_SLEEP_ON_BED) && !block_state.get(OCCUPIED)
}

pub fn get_work_package(profession: ProfessionId, speed_modifier: f32) -> Package {
    let work_at_poi = if profession == ProfessionId::FARMER { WorkAtPoi::work_at_composter() } else { WorkAtPoi::new() };
    let is_farmer = profession == ProfessionId::FARMER;
    vec![
        get_minimal_look_behavior(),
        pair(
            5,
            run_one::new(vec![
                weighted(work_at_poi, 7),
                weighted(stroll_around_poi::create(memory_module_type::JOB_SITE, STROLL_SPEED_MODIFIER, 4), 2),
                weighted(stroll_to_poi::create(memory_module_type::JOB_SITE, STROLL_SPEED_MODIFIER, 1, 10), 5),
                weighted(stroll_to_poi_list::create(memory_module_type::SECONDARY_JOB_SITE, speed_modifier, 1, 6, memory_module_type::JOB_SITE), 5),
                weighted(HarvestFarmland::default(), if is_farmer { 2 } else { 5 }),
                weighted(UseBonemeal::default(), if is_farmer { 4 } else { 7 }),
            ]),
        ),
        pair(10, ShowTradesToPlayer::new(400, 1600)),
        pair(10, set_look_and_interact::create(&entity_types::PLAYER, 4)),
        pair(2, set_walk_target_from_block_memory::create(memory_module_type::JOB_SITE, speed_modifier, 9, 100, 1200)),
        pair(3, GiveGiftToHero::new(100)),
        pair(99, update_activity_from_schedule::create()),
    ]
}

pub fn get_play_package(speed_modifier: f32) -> Package {
    vec![
        pair(0, MoveToTargetSink::new(80, 120)),
        get_full_look_behavior(),
        pair(5, play_tag_with_other_kids::create()),
        pair(
            5,
            run_one::with_entry_condition(
                vec![(memory_module_type::VISIBLE_VILLAGER_BABIES.erase(), MemoryStatus::ValueAbsent)],
                vec![
                    weighted(interact_with::of(&entity_types::VILLAGER, 8, memory_module_type::INTERACTION_TARGET, speed_modifier, 2), 2),
                    weighted(interact_with::of(&entity_types::CAT, 8, memory_module_type::INTERACTION_TARGET, speed_modifier, 2), 1),
                    weighted(village_bound_random_stroll::create(speed_modifier), 1),
                    weighted(set_walk_target_from_look_target::create(speed_modifier, 2), 1),
                    weighted(JumpOnBed::new(speed_modifier), 2),
                    weighted(DoNothing::new(20, 40), 2),
                ],
            ),
        ),
        pair(99, update_activity_from_schedule::create()),
    ]
}

pub fn get_rest_package(speed_modifier: f32) -> Package {
    vec![
        pair(2, set_walk_target_from_block_memory::create(memory_module_type::HOME, speed_modifier, 1, 150, 1200)),
        pair(3, validate_nearby_poi::create(|poi_type| poi_type == PoiType::Home, memory_module_type::HOME)),
        pair(3, SleepInBed::default()),
        pair(
            5,
            run_one::with_entry_condition(
                vec![(memory_module_type::HOME.erase(), MemoryStatus::ValueAbsent)],
                vec![
                    weighted(set_closest_home_as_walk_target::create(speed_modifier), 1),
                    weighted(inside_brownian_walk::create(speed_modifier), 4),
                    weighted(go_to_closest_village::create(speed_modifier, 4), 2),
                    weighted(DoNothing::new(20, 40), 2),
                ],
            ),
        ),
        get_minimal_look_behavior(),
        pair(99, update_activity_from_schedule::create()),
    ]
}

pub fn get_meet_package(speed_modifier: f32) -> Package {
    vec![
        pair(
            2,
            trigger_gate::trigger_one_shuffled(vec![
                trigger(stroll_around_poi::create(memory_module_type::MEETING_POINT, STROLL_SPEED_MODIFIER, 40), 2),
                trigger(socialize_at_bell::create(), 2),
            ]),
        ),
        pair(10, ShowTradesToPlayer::new(400, 1600)),
        pair(10, set_look_and_interact::create(&entity_types::PLAYER, 4)),
        pair(2, set_walk_target_from_block_memory::create(memory_module_type::MEETING_POINT, speed_modifier, 6, 100, 200)),
        pair(3, GiveGiftToHero::new(100)),
        pair(3, validate_nearby_poi::create(|poi_type| poi_type == PoiType::Meeting, memory_module_type::MEETING_POINT)),
        pair(
            3,
            GateBehavior::new(Vec::new(), vec![memory_module_type::INTERACTION_TARGET.erase()], OrderPolicy::Ordered, RunningPolicy::RunOne, vec![weighted(TradeWithVillager::default(), 1)]),
        ),
        get_full_look_behavior(),
        pair(99, update_activity_from_schedule::create()),
    ]
}

fn can_breed(level: &ServerLevel, entity: EntityId) -> bool {
    level.mob(entity).is_some_and(|mob| mob.abstract_villager().is_some() && villager::can_breed(mob))
}

pub fn get_idle_package(speed_modifier: f32) -> Package {
    vec![
        pair(
            2,
            run_one::new(vec![
                weighted(interact_with::of(&entity_types::VILLAGER, 8, memory_module_type::INTERACTION_TARGET, speed_modifier, 2), 2),
                weighted(interact_with::of_with(&entity_types::VILLAGER, 8, can_breed, can_breed, memory_module_type::BREED_TARGET, speed_modifier, 2), 1),
                weighted(interact_with::of(&entity_types::CAT, 8, memory_module_type::INTERACTION_TARGET, speed_modifier, 2), 1),
                weighted(village_bound_random_stroll::create(speed_modifier), 1),
                weighted(set_walk_target_from_look_target::create(speed_modifier, 2), 1),
                weighted(JumpOnBed::new(speed_modifier), 1),
                weighted(DoNothing::new(30, 60), 1),
            ]),
        ),
        pair(3, GiveGiftToHero::new(100)),
        pair(3, set_look_and_interact::create(&entity_types::PLAYER, 4)),
        pair(3, ShowTradesToPlayer::new(400, 1600)),
        pair(
            3,
            GateBehavior::new(Vec::new(), vec![memory_module_type::INTERACTION_TARGET.erase()], OrderPolicy::Ordered, RunningPolicy::RunOne, vec![weighted(TradeWithVillager::default(), 1)]),
        ),
        pair(
            3,
            GateBehavior::new(Vec::new(), vec![memory_module_type::BREED_TARGET.erase()], OrderPolicy::Ordered, RunningPolicy::RunOne, vec![weighted(VillagerMakeLove::default(), 1)]),
        ),
        get_full_look_behavior(),
        pair(99, update_activity_from_schedule::create()),
    ]
}

pub fn get_panic_package(speed_modifier: f32) -> Package {
    let runaway_speed = speed_modifier * 1.5;
    vec![
        pair(0, villager_calm_down::create()),
        pair(1, set_walk_target_away_from::entity(memory_module_type::NEAREST_HOSTILE, runaway_speed, 6, false)),
        pair(1, set_walk_target_away_from::entity(memory_module_type::HURT_BY_ENTITY, runaway_speed, 6, false)),
        pair(3, village_bound_random_stroll::create_with(runaway_speed, 2, 2)),
        get_minimal_look_behavior(),
    ]
}

pub fn get_pre_raid_package(speed_modifier: f32) -> Package {
    vec![
        pair(0, ring_bell::create()),
        pair(
            0,
            trigger_gate::trigger_one_shuffled(vec![
                trigger(set_walk_target_from_block_memory::create(memory_module_type::MEETING_POINT, speed_modifier * 1.5, 2, 150, 200), 6),
                trigger(village_bound_random_stroll::create(speed_modifier * 1.5), 2),
            ]),
        ),
        get_minimal_look_behavior(),
        pair(99, reset_raid_status::create()),
    ]
}

pub fn get_raid_package(speed_modifier: f32) -> Package {
    vec![
        pair(
            0,
            behavior_builder::sequence(
                behavior_builder::trigger_if(raid_exists_and_not_victory),
                trigger_gate::trigger_one_shuffled(vec![
                    trigger(move_to_sky_seeing_spot::create(speed_modifier), 5),
                    trigger(village_bound_random_stroll::create(speed_modifier * 1.1), 2),
                ]),
            ),
        ),
        pair(0, celebrate_villagers_survived_raid::CelebrateVillagersSurvivedRaid::new(600, 600)),
        pair(2, behavior_builder::sequence(behavior_builder::trigger_if(raid_exists_and_active), locate_hiding_place::create(24, speed_modifier * 1.4, 1))),
        get_minimal_look_behavior(),
        pair(99, reset_raid_status::create()),
    ]
}

pub fn get_hide_package(speed_modifier: f32) -> Package {
    vec![pair(0, set_hidden_state::create(15, 3)), pair(1, locate_hiding_place::create(32, speed_modifier * 1.25, 2)), get_minimal_look_behavior()]
}

fn get_full_look_behavior() -> (i32, Box<dyn BehaviorControl>) {
    pair(
        5,
        run_one::new(vec![
            weighted(set_entity_look_target::create_for_type(&entity_types::CAT, 8.0), 8),
            weighted(set_entity_look_target::create_for_type(&entity_types::VILLAGER, 8.0), 2),
            weighted(set_entity_look_target::create_for_type(&entity_types::PLAYER, 8.0), 2),
            weighted(set_entity_look_target::create_for_category(MobCategory::Creature, 8.0), 1),
            weighted(set_entity_look_target::create_for_category(MobCategory::WaterCreature, 8.0), 1),
            weighted(set_entity_look_target::create_for_category(MobCategory::Axolotls, 8.0), 1),
            weighted(set_entity_look_target::create_for_category(MobCategory::UndergroundWaterCreature, 8.0), 1),
            weighted(set_entity_look_target::create_for_category(MobCategory::WaterAmbient, 8.0), 1),
            weighted(set_entity_look_target::create_for_category(MobCategory::Monster, 8.0), 1),
            weighted(DoNothing::new(30, 60), 2),
        ]),
    )
}

fn get_minimal_look_behavior() -> (i32, Box<dyn BehaviorControl>) {
    pair(
        5,
        run_one::new(vec![
            weighted(set_entity_look_target::create_for_type(&entity_types::VILLAGER, 8.0), 2),
            weighted(set_entity_look_target::create_for_type(&entity_types::PLAYER, 8.0), 2),
            weighted(DoNothing::new(30, 60), 8),
        ]),
    )
}

fn raid_exists_and_active(level: &ServerLevel, body: EntityId) -> bool {
    let pos = level.entity(body).block_position();
    level.get_raid_at(pos).is_some_and(|current_raid| current_raid.is_active() && !current_raid.is_victory() && !current_raid.is_loss())
}

fn raid_exists_and_not_victory(level: &ServerLevel, body: EntityId) -> bool {
    let pos = level.entity(body).block_position();
    level.get_raid_at(pos).is_some_and(|current_raid| current_raid.is_victory())
}
