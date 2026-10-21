use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3};
use wafer_data::{items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::stats::stats;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::avoid_entity_goal::{AvoidClass, AvoidEntityGoal};
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::look_at_trading_player_goal::LookAtTradingPlayerGoal;
use crate::world::entity::ai::goal::move_towards_restriction_goal::MoveTowardsRestrictionGoal;
use crate::world::entity::ai::goal::panic_goal::PanicGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::TargetType;
use crate::world::entity::ai::goal::trade_with_player_goal::TradeWithPlayerGoal;
use crate::world::entity::ai::goal::use_item_goal::UseItemGoal;
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::attributes::WANDERING_TRADER_ATTRIBUTES;
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::npc::villager::abstract_villager::AbstractVillager;
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::alchemy::potion_contents::PotionContents;
use crate::world::item::trading::merchant_offer::MerchantOffer;
use crate::world::{InteractionHand, InteractionResult};

const DEFAULT_DESPAWN_DELAY: i32 = 0;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.6, 1.95).with_eye_height(1.62);

#[derive(Default)]
pub struct WanderingTrader {
    pub abstract_villager: AbstractVillager,
    wander_target: Option<BlockPos>,
    despawn_delay: i32,
}

fn wandering_trader(mob: &Mob) -> &WanderingTrader {
    match &mob.kind {
        MobKind::WanderingTrader(trader) => trader,
        _ => unreachable!("a wandering trader"),
    }
}

fn wandering_trader_mut(mob: &mut Mob) -> &mut WanderingTrader {
    match &mut mob.kind {
        MobKind::WanderingTrader(trader) => trader,
        _ => unreachable!("a wandering trader"),
    }
}

fn wants_to_disappear(level: &ServerLevel, mob: EntityId) -> bool {
    level.is_dark_outside() && !level.entity(mob).is_invisible()
}

fn wants_to_reappear(level: &ServerLevel, mob: EntityId) -> bool {
    level.is_bright_outside() && level.entity(mob).is_invisible()
}

impl WanderingTrader {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        data.push(DataValue::Int(0));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:wandering_trader"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&WANDERING_TRADER_ATTRIBUTES), MobKind::WanderingTrader(WanderingTrader::default()));
        AbstractVillager::init(&mut mob);
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::Float(float_goal));
        let invisibility = PotionContents::create_item_stack(items::POTION, "minecraft:invisibility");
        goal_selector.add_goal(0, Goal::UseItem(UseItemGoal::new(invisibility, Some("minecraft:entity.wandering_trader.disappeared"), wants_to_disappear)));
        goal_selector.add_goal(0, Goal::UseItem(UseItemGoal::new(ItemStack::new(items::MILK_BUCKET, 1), Some("minecraft:entity.wandering_trader.reappeared"), wants_to_reappear)));
        goal_selector.add_goal(1, Goal::TradeWithPlayer(TradeWithPlayerGoal));
        goal_selector.add_goal(1, Goal::AvoidEntity(AvoidEntityGoal::of_class(AvoidClass::Zombie, 8.0, 0.5, 0.5)));
        goal_selector.add_goal(1, Goal::AvoidEntity(AvoidEntityGoal::new("minecraft:evoker", 12.0, 0.5, 0.5)));
        goal_selector.add_goal(1, Goal::AvoidEntity(AvoidEntityGoal::new("minecraft:vindicator", 8.0, 0.5, 0.5)));
        goal_selector.add_goal(1, Goal::AvoidEntity(AvoidEntityGoal::new("minecraft:vex", 8.0, 0.5, 0.5)));
        goal_selector.add_goal(1, Goal::AvoidEntity(AvoidEntityGoal::new("minecraft:pillager", 15.0, 0.5, 0.5)));
        goal_selector.add_goal(1, Goal::AvoidEntity(AvoidEntityGoal::new("minecraft:illusioner", 12.0, 0.5, 0.5)));
        goal_selector.add_goal(1, Goal::AvoidEntity(AvoidEntityGoal::new("minecraft:zoglin", 10.0, 0.5, 0.5)));
        goal_selector.add_goal(1, Goal::Panic(PanicGoal::new(0.5)));
        goal_selector.add_goal(1, Goal::LookAtTradingPlayer(LookAtTradingPlayerGoal::new()));
        goal_selector.add_goal(2, Goal::WanderToPosition(WanderToPositionGoal { stop_distance: 2.0, speed_modifier: 0.35 }));
        goal_selector.add_goal(4, Goal::MoveTowardsRestriction(MoveTowardsRestrictionGoal::new(0.35)));
        goal_selector.add_goal(8, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::new(0.35)));
        goal_selector.add_goal(9, Goal::Interact(LookAtPlayerGoal::new(3.0).with_probability(1.0)));
        goal_selector.add_goal(10, Goal::LookAtPlayer(LookAtPlayerGoal::of_type(TargetType::Mob, 8.0)));
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        AbstractVillager::add_additional_save_data(mob, out);
        let trader = wandering_trader(mob);
        out.push(("DespawnDelay".into(), Nbt::Int(trader.despawn_delay)));
        if let Some(wander_target) = trader.wander_target {
            out.push(("wander_target".into(), Nbt::IntArray(vec![wander_target.x, wander_target.y, wander_target.z])));
        }
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        AbstractVillager::read_additional_save_data(mob, input);
        let trader = wandering_trader_mut(mob);
        trader.despawn_delay = input.get("DespawnDelay").and_then(Nbt::as_i64).map_or(DEFAULT_DESPAWN_DELAY, |v| v as i32);
        trader.wander_target = match input.get("wander_target") {
            Some(Nbt::IntArray(v)) if v.len() == 3 => Some(BlockPos::new(v[0], v[1], v[2])),
            _ => None,
        };
        let age = mob.ageable().map_or(0, |ageable| ageable.age);
        mob.set_age(age.max(0));
    }

    pub fn set_despawn_delay(mob: &mut Mob, despawn_delay: i32) {
        wandering_trader_mut(mob).despawn_delay = despawn_delay;
    }

    pub fn set_wander_target(mob: &mut Mob, pos: Option<BlockPos>) {
        wandering_trader_mut(mob).wander_target = pos;
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.wandering_trader.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.wandering_trader.death"
    }

    pub fn get_consume_sound(item_stack: &ItemStack) -> &'static str {
        if item_stack.is(items::MILK_BUCKET) { "minecraft:entity.wandering_trader.drink_milk" } else { "minecraft:entity.wandering_trader.drink_potion" }
    }

    pub fn get_trade_updated_sound(valid_trade: bool) -> &'static str {
        if valid_trade { "minecraft:entity.wandering_trader.yes" } else { "minecraft:entity.wandering_trader.no" }
    }

    pub fn get_notify_trade_sound() -> &'static str {
        "minecraft:entity.wandering_trader.yes"
    }
}

impl ServerLevel {
    pub fn is_wandering_trader(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::WanderingTrader(_)))
    }

    pub fn wandering_trader_get_despawn_delay(&self, id: EntityId) -> i32 {
        wandering_trader(self.mob(id).expect("a wandering trader")).despawn_delay
    }

    pub fn wandering_trader_get_ambient_sound(&self, id: EntityId) -> &'static str {
        if self.is_trading(id) { "minecraft:entity.wandering_trader.trade" } else { "minecraft:entity.wandering_trader.ambient" }
    }

    pub fn wandering_trader_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if !item_stack.is(items::VILLAGER_SPAWN_EGG) && self.is_alive(id) && !self.is_trading(id) && !self.is_baby(id) {
            if hand == InteractionHand::MainHand {
                self.award_stat(player, stats::TALKED_TO_VILLAGER, 1);
            }
            if self.get_offers(id).is_empty() {
                return InteractionResult::CONSUME;
            }
            self.set_trading_player(id, Some(player));
            self.open_trading_screen(id, player, 1);
            return InteractionResult::SUCCESS_SERVER;
        }
        self.ageable_mob_mob_interact(id, player, hand)
    }

    pub fn wandering_trader_update_trades(&mut self, id: EntityId) {
        self.add_offers_from_trade_set(id, "minecraft:wandering_trader/buying");
        self.add_offers_from_trade_set(id, "minecraft:wandering_trader/uncommon");
        self.add_offers_from_trade_set(id, "minecraft:wandering_trader/common");
    }

    pub fn wandering_trader_reward_trade_xp(&mut self, id: EntityId, offer: &MerchantOffer) {
        if offer.should_reward_exp() {
            let pop_xp = 3 + self.entity_mut(id).random.next_int_bounded(4);
            let e = self.entity(id);
            let position = Vec3::new(e.x(), e.y() + 0.5, e.z());
            let orb = self.new_experience_orb(position, Vec3::ZERO, pop_xp);
            self.add_fresh_entity(LevelEntity::ExperienceOrb(orb));
        }
    }

    pub fn wandering_trader_ai_step(&mut self, id: EntityId) {
        self.ageable_mob_ai_step(id);
        if self.mob(id).is_none() {
            return;
        }
        let trading = self.is_trading(id);
        let trader = wandering_trader_mut(self.mob_mut(id).expect("a wandering trader"));
        if trader.despawn_delay > 0 && !trading {
            trader.despawn_delay -= 1;
            if trader.despawn_delay == 0 {
                self.discard(id);
            }
        }
    }
}

pub struct WanderToPositionGoal {
    stop_distance: f64,
    speed_modifier: f64,
}

impl WanderToPositionGoal {
    fn is_too_far_away(level: &ServerLevel, mob: EntityId, pos: BlockPos, distance: f64) -> bool {
        !pos.closer_to_center_than(level.entity(mob).position(), distance)
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let trader = level.mob_mut(mob).expect("a wandering trader");
        WanderingTrader::set_wander_target(trader, None);
        trader.ai.navigation.stop();
    }

    pub fn can_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        wandering_trader(level.mob(mob).expect("a wandering trader")).wander_target.is_some_and(|pos| Self::is_too_far_away(level, mob, pos, self.stop_distance))
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let trader = level.mob(mob).expect("a wandering trader");
        let Some(wander_position) = wandering_trader(trader).wander_target else { return };
        if !trader.ai.navigation.is_done() {
            return;
        }
        let speed_modifier = self.speed_modifier;
        let (x, y, z) = if Self::is_too_far_away(level, mob, wander_position, 10.0) {
            let e = level.entity(mob);
            let dir = Vec3::new(wander_position.x as f64 - e.x(), wander_position.y as f64 - e.y(), wander_position.z as f64 - e.z()).normalize();
            let target_pos = dir.scale(10.0).add(e.x(), e.y(), e.z());
            (target_pos.x, target_pos.y, target_pos.z)
        } else {
            (wander_position.x as f64, wander_position.y as f64, wander_position.z as f64)
        };
        level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, x, y, z, speed_modifier));
    }
}
