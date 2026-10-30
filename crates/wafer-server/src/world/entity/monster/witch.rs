use std::borrow::Cow;

use wafer_core::Vec3;
use wafer_core::random::RandomSource;
use wafer_data::{EquipmentSlot, damage_type_tags, fluid_tags, items, mob_effects, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::ranged_attack_goal::RangedAttackGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::TargetType;
use crate::world::entity::ai::goal::target::nearest_attackable_witch_target_goal::NearestAttackableWitchTargetGoal;
use crate::world::entity::ai::goal::target::nearest_healable_raider_target_goal::NearestHealableRaiderTargetGoal;
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::attributes::{Attribute, AttributeModifier, Operation, WITCH_ATTRIBUTES};
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::monster::monster;
use crate::world::entity::projectile::throwableitemprojectile::thrown_splash_potion::ThrownSplashPotion;
use crate::world::entity::raid::raider::{RAIDER_TYPES, Raider};
use crate::world::entity::{Entity, EntityDimensions, EntityId, entity_event, sound_event};
use crate::world::item::alchemy::potion_contents::PotionContents;
use crate::world::item::component as dc;
use crate::world::item::{self, ItemStack};

pub const DATA_USING_ITEM: usize = 17;
const SPEED_MODIFIER_DRINKING_ID: &str = "minecraft:drinking";
const SPEED_MODIFIER_DRINKING: AttributeModifier = AttributeModifier { id: Cow::Borrowed(SPEED_MODIFIER_DRINKING_ID), amount: -0.25, operation: Operation::AddValue };
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.6, 1.95).with_eye_height(1.62).with_passenger_attachments(&[Vec3::new(0.0, 2.2625f32 as f64, 0.0)]);

#[derive(Default)]
pub struct Witch {
    pub raider: Raider,
    using_time: i32,
}

fn witch_mut(mob: &mut Mob) -> &mut Witch {
    match &mut mob.kind {
        MobKind::Witch(witch) => witch,
        _ => unreachable!("not a witch"),
    }
}

pub fn create_item_stack(item: wafer_data::Item, potion: &str) -> ItemStack {
    let mut item_stack = ItemStack::new(item, 1);
    item_stack.set(dc::POTION_CONTENTS, Some(Nbt::Compound(vec![("potion".into(), Nbt::String(potion.into()))].into())));
    item_stack
}

impl Witch {
    pub fn create() -> Mob {
        let mut data = mob_defaults();
        data.extend([DataValue::Boolean(false), DataValue::Boolean(false)]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:witch"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&WITCH_ATTRIBUTES), MobKind::Witch(Witch::default()));
        mob.xp_reward = monster::XP_REWARD;
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        Raider::register_goals(ai);
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(1, Goal::Float(float_goal));
        goal_selector.add_goal(2, Goal::RangedAttack(RangedAttackGoal::new(1.0, 60, 10.0)));
        goal_selector.add_goal(2, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::new(1.0)));
        goal_selector.add_goal(3, Goal::LookAtPlayer(LookAtPlayerGoal::new(8.0)));
        goal_selector.add_goal(3, Goal::RandomLookAround(RandomLookAroundGoal::default()));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::HurtByTarget(HurtByTargetGoal::new(RAIDER_TYPES)));
        target_selector.add_goal(2, Goal::NearestHealableRaiderTarget(NearestHealableRaiderTargetGoal::new(TargetType::Raider, true, Some(heal_raiders_selector))));
        target_selector.add_goal(3, Goal::NearestAttackableWitchTarget(NearestAttackableWitchTargetGoal::new(TargetType::Player, 10, true, false, None)));
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.witch.ambient"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.witch.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.witch.death"
    }

    pub fn get_celebrate_sound() -> &'static str {
        "minecraft:entity.witch.celebrate"
    }

    pub fn set_using_item(mob: &mut Mob, using: bool) {
        mob.entity.data.set(DATA_USING_ITEM, DataValue::Boolean(using));
    }

    pub fn is_drinking_potion(mob: &Mob) -> bool {
        matches!(mob.entity.data.get(DATA_USING_ITEM), DataValue::Boolean(true))
    }

    pub fn can_be_leader() -> bool {
        false
    }

    fn heal_raiders_goal(mob: &mut Mob) -> &mut NearestHealableRaiderTargetGoal {
        mob.ai.target_selector.get_available_goals_mut().iter_mut().find_map(|goal| match goal.get_goal_mut() {
            Goal::NearestHealableRaiderTarget(goal) => Some(goal),
            _ => None,
        })
        .expect("the heal raiders goal")
    }

    fn attack_players_goal(mob: &mut Mob) -> &mut NearestAttackableWitchTargetGoal {
        mob.ai.target_selector.get_available_goals_mut().iter_mut().find_map(|goal| match goal.get_goal_mut() {
            Goal::NearestAttackableWitchTarget(goal) => Some(goal),
            _ => None,
        })
        .expect("the attack players goal")
    }
}

fn heal_raiders_selector(_mob: EntityId, target: EntityId, level: &ServerLevel) -> bool {
    registry::ENTITY_TYPE[level.entity(target).entity_type as usize] != "minecraft:witch"
}

impl ServerLevel {
    pub fn witch_ai_step(&mut self, id: EntityId) {
        if self.is_alive(id) {
            let mob = self.mob_mut(id).expect("a witch");
            let heal_raiders_goal = Witch::heal_raiders_goal(mob);
            heal_raiders_goal.decrement_cooldown();
            let can_attack = heal_raiders_goal.get_cooldown() <= 0;
            Witch::attack_players_goal(mob).set_can_attack(can_attack);
            if Witch::is_drinking_potion(mob) {
                let witch = witch_mut(mob);
                let using_time = witch.using_time;
                witch.using_time -= 1;
                if using_time <= 0 {
                    Witch::set_using_item(mob, false);
                    let item_stack = self.item_by_slot(id, EquipmentSlot::Mainhand).unwrap_or_else(ItemStack::empty);
                    self.living_set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::empty());
                    if item_stack.is(items::POTION)
                        && let Some(potion) = item_stack.get(dc::POTION_CONTENTS)
                    {
                        let duration_scale = item_stack.get(dc::POTION_DURATION_SCALE).and_then(Nbt::as_f64).map_or(1.0, |scale| scale as f32);
                        for effect in PotionContents::new(potion).for_each_effect(duration_scale) {
                            self.add_effect(id, effect, None);
                        }
                    }
                    self.entity_game_event(id, crate::world::level::gameevent::game_event::DRINK);
                    if let Some(speed) = self.living_mut(id).attributes.instance(Attribute::MovementSpeed) {
                        speed.remove_modifier(SPEED_MODIFIER_DRINKING_ID);
                    }
                }
            } else {
                let potion = self.witch_choose_potion(id);
                if let Some(potion) = potion {
                    self.living_set_item_slot(id, EquipmentSlot::Mainhand, create_item_stack(items::POTION, potion));
                    let use_duration = self.item_by_slot(id, EquipmentSlot::Mainhand).map_or(0, |stack| item::get_use_duration(&stack));
                    let mob = self.mob_mut(id).expect("a witch");
                    witch_mut(mob).using_time = use_duration;
                    Witch::set_using_item(mob, true);
                    if !mob.entity.is_silent() {
                        let pitch = 0.8 + mob.entity.random.next_float() * 0.4;
                        let (x, y, z) = (mob.entity.x(), mob.entity.y(), mob.entity.z());
                        let source = self.sound_source(id);
                        self.play_sound_at(None, x, y, z, sound_event("minecraft:entity.witch.drink"), source, 1.0, pitch);
                    }
                    if let Some(speed) = self.living_mut(id).attributes.instance(Attribute::MovementSpeed) {
                        speed.remove_modifier(SPEED_MODIFIER_DRINKING_ID);
                        speed.add_transient_modifier(SPEED_MODIFIER_DRINKING);
                    }
                }
            }
            if self.entity_mut(id).random.next_float() < 7.5E-4 {
                self.broadcast_entity_event(id, entity_event::WITCH_HAT_MAGIC);
            }
        }
        self.raider_ai_step(id);
    }

    fn witch_choose_potion(&mut self, id: EntityId) -> Option<&'static str> {
        if self.entity_mut(id).random.next_float() < 0.15 && self.entity(id).is_eye_in_fluid(&fluid_tags::WATER) && !self.has_effect(id, mob_effects::WATER_BREATHING) {
            return Some("minecraft:water_breathing");
        }
        if self.entity_mut(id).random.next_float() < 0.15
            && (self.is_on_fire(id) || self.get_last_damage_source(id).is_some_and(|source| source.is(&damage_type_tags::IS_FIRE)))
            && !self.has_effect(id, mob_effects::FIRE_RESISTANCE)
        {
            return Some("minecraft:fire_resistance");
        }
        if self.entity_mut(id).random.next_float() < 0.05 && self.health(id) < self.max_health(id) {
            return Some("minecraft:healing");
        }
        if self.entity_mut(id).random.next_float() < 0.5
            && let Some(target) = self.get_target(id)
            && !self.has_effect(id, mob_effects::SPEED)
            && self.entity(target).distance_to_sqr(self.entity(id)) > 121.0
        {
            return Some("minecraft:swiftness");
        }
        None
    }

    pub fn witch_get_damage_after_magic_absorb(&mut self, id: EntityId, source: &DamageSource, damage: f32) -> f32 {
        let mut damage = self.living_entity_damage_after_magic_absorb(id, source, damage);
        if source.causing_entity == Some(id) {
            damage = 0.0;
        }
        if source.is(&damage_type_tags::WITCH_RESISTANT_TO) {
            damage *= 0.15;
        }
        damage
    }

    pub fn witch_perform_ranged_attack(&mut self, id: EntityId, target: EntityId, _power: f32) {
        if Witch::is_drinking_potion(self.mob(id).expect("a witch")) {
            return;
        }
        let (e, t) = (self.entity(id), self.entity(target));
        let target_movement = t.delta_movement;
        let xd = t.x() + target_movement.x - e.x();
        let yd = t.eye_y() - 1.1f32 as f64 - e.y();
        let zd = t.z() + target_movement.z - e.z();
        let dist = (xd * xd + zd * zd).sqrt();
        let mut potion = "minecraft:harming";
        if self.mob(target).is_some_and(|mob| mob.raider().is_some()) {
            potion = if self.health(target) <= 4.0 { "minecraft:healing" } else { "minecraft:regeneration" };
            self.set_target(id, None);
        } else if dist >= 8.0 && !self.has_effect(target, mob_effects::SLOWNESS) {
            potion = "minecraft:slowness";
        } else if self.health(target) >= 8.0 && !self.has_effect(target, mob_effects::POISON) {
            potion = "minecraft:poison";
        } else if dist <= 3.0 && !self.has_effect(target, mob_effects::WEAKNESS) && self.entity_mut(id).random.next_float() < 0.25 {
            potion = "minecraft:weakness";
        }
        let item_stack = create_item_stack(items::SPLASH_POTION, potion);
        let projectile = ThrownSplashPotion::from_owner(self, id, &item_stack);
        let uncertainty = self.ranged_attack_uncertainty(id);
        self.spawn_projectile_using_shoot(projectile, &item_stack, xd, yd + dist * 0.2, zd, if dist <= 2.0 { 0.45 } else { 0.75 }, uncertainty);
        if !self.entity(id).is_silent() {
            let pitch = 0.8 + self.entity_mut(id).random.next_float() * 0.4;
            let e = self.entity(id);
            let (x, y, z) = (e.x(), e.y(), e.z());
            let source = self.sound_source(id);
            self.play_sound_at(None, x, y, z, sound_event("minecraft:entity.witch.throw"), source, 1.0, pitch);
        }
    }

    pub fn witch_ranged_attack_uncertainty(&self) -> f32 {
        8.0
    }
}
