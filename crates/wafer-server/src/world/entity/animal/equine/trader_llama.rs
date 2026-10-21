use wafer_data::registry;
use wafer_protocol::nbt::Nbt;

use super::llama::{self, Llama, llama_mut};
use crate::server::level::ServerLevel;
use crate::world::entity::ageable_mob::AgeableMobGroupData;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::panic_goal::PanicGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::goal::target::target_goal::TargetGoal;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::EntityId;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

const DEFAULT_DESPAWN_DELAY: i32 = 47999;

fn not_a_zombified_piglin(_mob: EntityId, target: EntityId, level: &ServerLevel) -> bool {
    registry::ENTITY_TYPE[level.entity(target).entity_type as usize] != "minecraft:zombified_piglin"
}

pub struct TraderLlama;

impl TraderLlama {
    pub fn create() -> Mob {
        Llama::create_kind("minecraft:trader_llama", MobKind::TraderLlama(Llama { despawn_delay: DEFAULT_DESPAWN_DELAY, ..Llama::default() }))
    }

    pub fn register_goals(ai: &mut MobAi) {
        Llama::register_goals(ai);
        ai.goal_selector.add_goal(1, Goal::Panic(PanicGoal::new(2.0)));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::TraderLlamaDefendWanderingTarget(TraderLlamaDefendWanderingTraderGoal::new()));
        target_selector.add_goal(2, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::with(TargetType::Zombie, 10, true, false, Some(not_a_zombified_piglin))));
        target_selector.add_goal(2, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::AbstractIllager, true)));
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        Llama::add_additional_save_data(mob, out);
        out.push(("DespawnDelay".into(), Nbt::Int(llama::llama(mob).despawn_delay)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        Llama::read_additional_save_data(mob, input);
        llama_mut(mob).despawn_delay = input.get("DespawnDelay").and_then(Nbt::as_i64).map_or(DEFAULT_DESPAWN_DELAY, |v| v as i32);
    }
}

pub fn finalize_spawn(level: &mut (impl ServerLevelAccessor + ?Sized), mob: &mut Mob, spawn_reason: EntitySpawnReason, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
    if spawn_reason == EntitySpawnReason::Event {
        mob.set_age(0);
    }
    let group_data = group_data.or(Some(SpawnGroupData::AgeableMob(AgeableMobGroupData::new(false))));
    llama::finalize_spawn(level, mob, group_data)
}

impl ServerLevel {
    fn is_leashed_to_wandering_trader(&self, id: EntityId) -> bool {
        self.get_leash_holder(id).is_some_and(|holder| self.is_wandering_trader(holder))
    }

    fn is_leashed_to_something_other_than_the_wandering_trader(&self, id: EntityId) -> bool {
        self.is_leashed(id) && !self.is_leashed_to_wandering_trader(id)
    }

    pub fn trader_llama_do_player_ride(&mut self, id: EntityId, player: EntityId) {
        if !self.is_leashed_to_wandering_trader(id) {
            self.do_player_ride(id, player);
        }
    }

    pub fn trader_llama_ai_step(&mut self, id: EntityId) {
        self.abstract_horse_ai_step(id);
        if self.mob(id).is_some() {
            self.trader_llama_maybe_despawn(id);
        }
    }

    fn trader_llama_maybe_despawn(&mut self, id: EntityId) {
        if !self.trader_llama_can_despawn(id) {
            return;
        }
        let despawn_delay = match self.get_leash_holder(id).filter(|&holder| self.is_wandering_trader(holder)) {
            Some(trader) => self.wandering_trader_get_despawn_delay(trader) - 1,
            None => llama::llama(self.mob(id).expect("a trader llama")).despawn_delay - 1,
        };
        llama_mut(self.mob_mut(id).expect("a trader llama")).despawn_delay = despawn_delay;
        if despawn_delay <= 0 {
            self.remove_leash(id);
            self.discard(id);
        }
    }

    fn trader_llama_can_despawn(&self, id: EntityId) -> bool {
        let mob = self.mob(id).expect("a trader llama");
        !self.is_tamed(id) && !self.is_leashed_to_something_other_than_the_wandering_trader(id) && !self.has_exactly_one_player_passenger(id) && !mob.is_age_locked() && !mob.is_persistence_required()
    }
}

pub struct TraderLlamaDefendWanderingTraderGoal {
    pub target_goal: TargetGoal,
    owner_last_hurt_by: Option<EntityId>,
    timestamp: i32,
}

impl TraderLlamaDefendWanderingTraderGoal {
    fn new() -> TraderLlamaDefendWanderingTraderGoal {
        TraderLlamaDefendWanderingTraderGoal { target_goal: TargetGoal::new(false, false), owner_last_hurt_by: None, timestamp: 0 }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if !level.is_leashed(mob) {
            return false;
        }
        let Some(owner) = level.get_leash_holder(mob).filter(|&holder| level.is_wandering_trader(holder)) else { return false };
        self.owner_last_hurt_by = level.get_last_hurt_by_mob(owner);
        let time_stamp = level.living(owner).last_hurt_by_mob_timestamp;
        time_stamp != self.timestamp && self.target_goal.can_attack(level, mob, self.owner_last_hurt_by, &TargetingConditions::for_combat())
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.set_target(mob, self.owner_last_hurt_by);
        if let Some(wandering_trader) = level.get_leash_holder(mob).filter(|&holder| level.is_wandering_trader(holder)) {
            self.timestamp = level.living(wandering_trader).last_hurt_by_mob_timestamp;
        }
        self.target_goal.start();
    }
}
