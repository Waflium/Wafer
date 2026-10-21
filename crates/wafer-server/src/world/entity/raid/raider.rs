use std::collections::HashMap;

use wafer_core::random::RandomSource;
use wafer_data::{EquipmentSlot, registry};
use wafer_protocol::nbt::Nbt;

use super::raid::{self, Raid};
use super::raids;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::goal::{Goal, reduced_tick_delay};
use crate::world::entity::attributes::Attribute;
use crate::world::entity::data::DataValue;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::mob::Mob;
use crate::world::entity::monster::patrolling_monster::{self, PatrollingMonster};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{EntityId, LevelEntity, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::pathfinder::path::Path;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

pub const IS_CELEBRATING: usize = 16;
const DEFAULT_WAVE: i32 = 0;
const DEFAULT_CAN_JOIN_RAID: bool = false;
pub const RAIDER_TYPES: &[&str] = &["minecraft:evoker", "minecraft:illusioner", "minecraft:pillager", "minecraft:ravager", "minecraft:vindicator", "minecraft:witch"];

pub struct Raider {
    pub patrolling_monster: PatrollingMonster,
    raid: Option<Raid>,
    wave: i32,
    can_join_raid: bool,
    ticks_outside_raid: i32,
}

impl Default for Raider {
    fn default() -> Raider {
        Raider { patrolling_monster: PatrollingMonster::default(), raid: None, wave: DEFAULT_WAVE, can_join_raid: DEFAULT_CAN_JOIN_RAID, ticks_outside_raid: 0 }
    }
}

impl Raider {
    pub fn register_goals(ai: &mut MobAi) {
        PatrollingMonster::register_goals(ai);
        ai.goal_selector.add_goal(1, Goal::ObtainRaidLeaderBanner(ObtainRaidLeaderBannerGoal::default()));
        ai.goal_selector.add_goal(3, Goal::PathfindToRaid(PathfindToRaidGoal));
        ai.goal_selector.add_goal(4, Goal::RaiderMoveThroughVillage(RaiderMoveThroughVillageGoal::new()));
        ai.goal_selector.add_goal(5, Goal::RaiderCelebration(RaiderCelebration));
    }

    pub fn can_join_raid(&self) -> bool {
        self.can_join_raid
    }

    pub fn set_can_join_raid(&mut self, can_join_raid: bool) {
        self.can_join_raid = can_join_raid;
    }

    pub fn get_current_raid(&self) -> Option<&Raid> {
        self.raid.as_ref()
    }

    pub fn has_active_raid(&self) -> bool {
        self.get_current_raid().is_some_and(Raid::is_active)
    }

    pub fn set_wave(&mut self, wave: i32) {
        self.wave = wave;
    }

    pub fn get_wave(&self) -> i32 {
        self.wave
    }

    pub fn get_ticks_outside_raid(&self) -> i32 {
        self.ticks_outside_raid
    }

    pub fn set_ticks_outside_raid(&mut self, ticks_outside_raid: i32) {
        self.ticks_outside_raid = ticks_outside_raid;
    }

    pub fn add_additional_save_data(&self, out: &mut Vec<(String, Nbt)>) {
        self.patrolling_monster.add_additional_save_data(out);
        out.push(("Wave".into(), Nbt::Int(self.wave)));
        out.push(("CanJoinRaid".into(), Nbt::Byte(self.can_join_raid as i8)));
        if let Some(raid) = self.raid.as_ref() {
            match *raid {}
        }
    }

    pub fn read_additional_save_data(&mut self, input: &Nbt) {
        self.patrolling_monster.read_additional_save_data(input);
        self.wave = input.get("Wave").and_then(Nbt::as_i64).map_or(DEFAULT_WAVE, |wave| wave as i32);
        self.can_join_raid = input.get("CanJoinRaid").and_then(Nbt::as_i64).map_or(DEFAULT_CAN_JOIN_RAID, |v| v != 0);
    }

    pub fn remove_when_far_away(&self, dist_sqr: f64) -> bool {
        if self.get_current_raid().is_none() { self.patrolling_monster.remove_when_far_away(dist_sqr) } else { false }
    }
}

pub fn set_celebrating(mob: &mut Mob, celebrating: bool) {
    mob.entity.data.set(IS_CELEBRATING, DataValue::Boolean(celebrating));
}

fn allowed_items(level: &ServerLevel, item: EntityId) -> bool {
    let Some(LevelEntity::Item(item_entity)) = level.entities.get(&item) else { return false };
    !item_entity.has_pickup_delay() && level.is_alive(item) && ItemStack::matches(&item_entity.item, &raid::get_ominous_banner_instance())
}

pub fn finalize_spawn(
    level: &mut (impl ServerLevelAccessor + ?Sized),
    mob: &mut Mob,
    difficulty: &DifficultyInstance,
    spawn_reason: EntitySpawnReason,
    group_data: Option<SpawnGroupData>,
) -> Option<SpawnGroupData> {
    let is_witch = registry::ENTITY_TYPE[mob.entity.entity_type as usize] == "minecraft:witch";
    mob.raider_mut().expect("a raider").set_can_join_raid(!is_witch || spawn_reason != EntitySpawnReason::Natural);
    patrolling_monster::finalize_spawn(level, mob, difficulty, spawn_reason, group_data)
}

impl ServerLevel {
    pub fn raider(&self, id: EntityId) -> &Raider {
        self.mob(id).and_then(Mob::raider).expect("a raider")
    }

    pub fn raider_mut(&mut self, id: EntityId) -> &mut Raider {
        self.mob_mut(id).and_then(Mob::raider_mut).expect("a raider")
    }

    pub fn raider_ai_step(&mut self, id: EntityId) {
        if self.is_alive(id) {
            let can_join_raid = self.raider(id).can_join_raid();
            if can_join_raid {
                match self.raider(id).get_current_raid() {
                    None => {
                        if self.game_time % 20 == 0 {
                            let pos = self.entity(id).block_position();
                            if let Some(nearby_raid) = self.get_raid_at(pos)
                                && raids::can_join_raid(self, id)
                            {
                                match *nearby_raid {}
                            }
                        }
                    }
                    Some(current_raid) => match *current_raid {},
                }
            }
        }
        self.monster_ai_step(id);
    }

    pub fn raider_update_no_action_time(&mut self, id: EntityId) {
        self.living_mut(id).no_action_time += 2;
    }

    pub fn raider_die(&mut self, id: EntityId, source: DamageSource) {
        if let Some(raid_when_killed) = self.raider(id).get_current_raid() {
            match *raid_when_killed {}
        }
        self.living_entity_die(id, source);
    }

    pub fn raider_can_join_patrol(&self, id: EntityId) -> bool {
        !self.raider(id).has_active_raid()
    }

    pub fn is_captain(&self, id: EntityId) -> bool {
        let banner = self.item_by_slot(id, EquipmentSlot::Head);
        let has_captain_banner = banner.is_some_and(|banner| ItemStack::matches(&banner, &raid::get_ominous_banner_instance()));
        let patrol_leader = self.raider(id).patrolling_monster.is_patrol_leader();
        has_captain_banner && patrol_leader
    }

    pub fn has_raid(&self, id: EntityId) -> bool {
        self.raider(id).get_current_raid().is_some() || self.get_raid_at(self.entity(id).block_position()).is_some()
    }

    pub fn raider_pick_up_item(&mut self, id: EntityId, entity: EntityId) {
        if self.raider(id).has_active_raid() {
            let raid = self.raider(id).get_current_raid().expect("an active raid");
            match *raid {}
        }
        self.mob_pick_up_item(id, entity);
    }

    pub fn raider_requires_custom_persistence(&self, id: EntityId) -> bool {
        self.mob_requires_custom_persistence(id) || self.raider(id).get_current_raid().is_some()
    }

    pub fn raider_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        if self.raider(id).has_active_raid() {
            let raid = self.raider(id).get_current_raid().expect("an active raid");
            match *raid {}
        }
        self.living_entity_hurt_server(id, source, damage)
    }

    pub fn get_celebrate_sound(&self, id: EntityId) -> &'static str {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(crate::world::entity::mob::MobKind::Witch(_)) => crate::world::entity::monster::witch::Witch::get_celebrate_sound(),
            Some(crate::world::entity::mob::MobKind::Vindicator(_)) => crate::world::entity::monster::illager::vindicator::Vindicator::get_celebrate_sound(),
            Some(crate::world::entity::mob::MobKind::Pillager(_)) => crate::world::entity::monster::illager::pillager::Pillager::get_celebrate_sound(),
            Some(crate::world::entity::mob::MobKind::Evoker(_)) => crate::world::entity::monster::illager::evoker::Evoker::get_celebrate_sound(),
            _ => unreachable!("not a raider"),
        }
    }
}

#[derive(Default)]
pub struct ObtainRaidLeaderBannerGoal {
    unreachable_banner_cache: HashMap<EntityId, i64>,
    path_to_banner: Option<Path>,
    pursued_banner_item_entity: Option<EntityId>,
}

impl ObtainRaidLeaderBannerGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if self.cannot_pick_up_banner(level, mob) {
            return false;
        }
        let mut temp_cache = HashMap::new();
        let follow_range = level.attribute_value(mob, Attribute::FollowRange);
        let area = level.entity(mob).bounding_box().inflate(follow_range, 8.0, follow_range);
        let banners: Vec<EntityId> = level.items_in(&area, None).into_iter().filter(|&banner| allowed_items(level, banner)).collect();
        for banner in banners {
            let unreachable_until_time = self.unreachable_banner_cache.get(&banner).copied().unwrap_or(i64::MIN);
            if level.game_time < unreachable_until_time {
                temp_cache.insert(banner, unreachable_until_time);
            } else {
                let path = level.get_navigation(mob, |navigation, level| navigation.create_path_to_entity(level, mob, banner, 1));
                if let Some(path) = path.filter(Path::can_reach) {
                    self.path_to_banner = Some(path);
                    self.pursued_banner_item_entity = Some(banner);
                    return true;
                }
                temp_cache.insert(banner, level.game_time + 600);
            }
        }
        self.unreachable_banner_cache = temp_cache;
        false
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let (Some(pursued_banner_item_entity), Some(_)) = (self.pursued_banner_item_entity, self.path_to_banner.as_ref()) else { return false };
        if level.entity_ref(pursued_banner_item_entity).is_none_or(|entity| entity.is_removed()) {
            return false;
        }
        if level.mob(mob).expect("a raider").ai.navigation.get_path().is_none_or(Path::is_done) { false } else { !self.cannot_pick_up_banner(level, mob) }
    }

    fn cannot_pick_up_banner(&self, level: &ServerLevel, mob: EntityId) -> bool {
        let raider = level.raider(mob);
        if !raider.has_active_raid() {
            return true;
        }
        let raid = raider.get_current_raid().expect("an active raid");
        if raid.is_over() {
            return true;
        }
        if !level.mob(mob).expect("a raider").can_be_leader() {
            return true;
        }
        if level.item_by_slot(mob, EquipmentSlot::Head).is_some_and(|head| ItemStack::matches(&head, &raid::get_ominous_banner_instance())) {
            return true;
        }
        let leader = raid.get_leader(raider.get_wave());
        leader.is_some_and(|leader| level.is_alive(leader))
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let path_to_banner = self.path_to_banner.clone();
        level.get_navigation(mob, |navigation, level| navigation.move_to_path(level, mob, path_to_banner, 1.15f32 as f64));
    }

    pub fn stop(&mut self) {
        self.path_to_banner = None;
        self.pursued_banner_item_entity = None;
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if let Some(pursued_banner_item_entity) = self.pursued_banner_item_entity
            && level.entity(pursued_banner_item_entity).distance_to_sqr(level.entity(mob)) < 1.414 * 1.414
        {
            level.pick_up_item(mob, pursued_banner_item_entity);
        }
    }
}

pub struct RaiderCelebration;

impl RaiderCelebration {
    pub fn can_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        let current_raid = level.raider(mob).get_current_raid();
        level.is_alive(mob) && level.get_target(mob).is_none() && current_raid.is_some_and(Raid::is_loss)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        set_celebrating(level.mob_mut(mob).expect("a raider"), true);
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        set_celebrating(level.mob_mut(mob).expect("a raider"), false);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let is_silent = level.entity(mob).is_silent();
        if !is_silent && level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(100)) == 0 {
            let sound = level.get_celebrate_sound(mob);
            level.make_sound(mob, sound_event(sound));
        }
        if !level.is_passenger(mob) && level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(50)) == 0 {
            level.mob_mut(mob).expect("a raider").ai.jump_control.jump();
        }
    }
}

pub struct HoldGroundAttackGoal {
    hostile_radius_sqr: f32,
}

impl HoldGroundAttackGoal {
    pub fn new(hostile_radius: f32) -> HoldGroundAttackGoal {
        HoldGroundAttackGoal { hostile_radius_sqr: hostile_radius * hostile_radius }
    }

    fn shout_targeting() -> crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions<'static> {
        crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions::for_non_combat().range(8.0).ignore_line_of_sight().ignore_invisibility_testing()
    }

    fn nearby_raiders(level: &mut ServerLevel, mob: EntityId) -> Vec<EntityId> {
        let area = level.entity(mob).bounding_box().inflate(8.0, 8.0, 8.0);
        let candidates: Vec<EntityId> = level.get_entities(None, &area).into_iter().filter(|&entity| level.mob(entity).and_then(Mob::raider).is_some()).collect();
        let shout_targeting = HoldGroundAttackGoal::shout_targeting();
        candidates.into_iter().filter(|&entity| shout_targeting.test(level, Some(mob), entity)).collect()
    }

    pub fn can_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        let last_hurt_by_mob = level.get_last_hurt_by_mob(mob);
        let raider = level.raider(mob);
        raider.get_current_raid().is_none()
            && raider.patrolling_monster.is_patrolling()
            && level.get_target(mob).is_some()
            && !level.mob(mob).expect("a raider").is_aggressive()
            && last_hurt_by_mob.is_none_or(|entity| !level.is_player(entity))
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.mob_mut(mob).expect("a raider").ai.navigation.stop();
        let target = level.get_target(mob);
        for entity in HoldGroundAttackGoal::nearby_raiders(level, mob) {
            level.set_target(entity, target);
        }
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if let Some(target) = level.get_target(mob) {
            for entity in HoldGroundAttackGoal::nearby_raiders(level, mob) {
                level.set_target(entity, Some(target));
                level.mob_mut(entity).expect("a raider").set_aggressive(true);
            }
            level.mob_mut(mob).expect("a raider").set_aggressive(true);
        }
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let Some(target) = level.get_target(mob) else { return };
        if level.entity(mob).distance_to_sqr(level.entity(target)) > self.hostile_radius_sqr as f64 {
            level.set_look_at_entity(mob, target, 30.0, 30.0);
            if level.entity_mut(mob).random.next_int_bounded(50) == 0 {
                level.play_ambient_sound(mob);
            }
        } else {
            level.mob_mut(mob).expect("a raider").set_aggressive(true);
        }
    }
}

#[derive(Default)]
pub struct PathfindToRaidGoal;

impl PathfindToRaidGoal {
    pub fn can_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        if level.get_target(mob).is_some() || level.has_controlling_passenger(mob) {
            return false;
        }
        match level.raider(mob).get_current_raid() {
            Some(raid) => match *raid {},
            None => false,
        }
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        match level.raider(mob).get_current_raid() {
            Some(raid) => match *raid {},
            None => false,
        }
    }
}

pub struct RaiderMoveThroughVillageGoal {
    visited: Vec<wafer_core::BlockPos>,
}

impl RaiderMoveThroughVillageGoal {
    pub fn new() -> RaiderMoveThroughVillageGoal {
        RaiderMoveThroughVillageGoal { visited: Vec::new() }
    }

    fn update_visited(&mut self) {
        if self.visited.len() > 2 {
            self.visited.remove(0);
        }
    }

    pub fn can_use(&mut self, level: &ServerLevel, mob: EntityId) -> bool {
        self.update_visited();
        match level.raider(mob).get_current_raid() {
            Some(raid) => match *raid {},
            None => false,
        }
    }
}
