use wafer_core::Vec3;
use wafer_core::random::RandomSource;
use wafer_data::{EquipmentSlot, item_tags, items, registry, registry_id};

use super::bogged::Bogged;
use crate::server::level::ServerLevel;
use crate::util::special_dates;
use crate::world::difficulty::Difficulty;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::avoid_entity_goal::AvoidEntityGoal;
use crate::world::entity::ai::goal::flee_sun_goal::FleeSunGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::melee_attack_goal::MeleeAttackGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::ranged_bow_attack_goal::RangedBowAttackGoal;
use crate::world::entity::ai::goal::restrict_sun_goal::RestrictSunGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::attributes::AttributeSupplier;
use crate::world::entity::data::DataValue;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::monster::monster;
use crate::world::entity::monster::zombie::zombie;
use crate::world::entity::projectile::projectile::Projectile;
use crate::world::entity::projectile::projectile_util;
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, sound_event};
use crate::world::item::ItemStack;

const HARD_ATTACK_INTERVAL: i32 = 20;
const NORMAL_ATTACK_INTERVAL: i32 = 40;
pub const INCREASED_HARD_ATTACK_INTERVAL: i32 = 50;
pub const INCREASED_NORMAL_ATTACK_INTERVAL: i32 = 70;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.6, 1.99).with_eye_height(1.74).with_vehicle_attachment(&[Vec3::new(0.0, 0.7f32 as f64, 0.0)]);

pub struct AbstractSkeletonMeleeGoal {
    pub melee_attack_goal: MeleeAttackGoal,
}

impl AbstractSkeletonMeleeGoal {
    fn new() -> AbstractSkeletonMeleeGoal {
        AbstractSkeletonMeleeGoal { melee_attack_goal: MeleeAttackGoal::new(1.2, false) }
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.melee_attack_goal.stop(level, mob);
        level.mob_mut(mob).expect("a skeleton").set_aggressive(false);
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.melee_attack_goal.start(level, mob);
        level.mob_mut(mob).expect("a skeleton").set_aggressive(true);
    }
}

pub struct AbstractSkeleton {
    bow_goal: Option<RangedBowAttackGoal>,
    melee_goal: Option<AbstractSkeletonMeleeGoal>,
}

impl Default for AbstractSkeleton {
    fn default() -> AbstractSkeleton {
        AbstractSkeleton { bow_goal: Some(RangedBowAttackGoal::new(1.0, 20, 15.0)), melee_goal: Some(AbstractSkeletonMeleeGoal::new()) }
    }
}

pub fn abstract_skeleton_mut(mob: &mut Mob) -> &mut AbstractSkeleton {
    match &mut mob.kind {
        MobKind::Skeleton(skeleton) => &mut skeleton.abstract_skeleton,
        MobKind::Stray(stray) => &mut stray.abstract_skeleton,
        MobKind::Parched(parched) => &mut parched.abstract_skeleton,
        MobKind::WitherSkeleton(wither_skeleton) => &mut wither_skeleton.abstract_skeleton,
        MobKind::Bogged(bogged) => &mut bogged.abstract_skeleton,
        _ => unreachable!("not a skeleton"),
    }
}

impl AbstractSkeleton {
    pub fn create_with(entity_type: &str, attributes: &'static AttributeSupplier, data: Vec<DataValue>, kind: MobKind) -> Mob {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, entity_type), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(attributes), kind);
        mob.xp_reward = monster::XP_REWARD;
        if let Some(melee_goal) = abstract_skeleton_mut(&mut mob).melee_goal.take() {
            mob.ai.goal_selector.add_goal(4, Goal::AbstractSkeletonMelee(melee_goal));
        }
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(2, Goal::RestrictSun(RestrictSunGoal));
        goal_selector.add_goal(3, Goal::FleeSun(FleeSunGoal::new(1.0)));
        goal_selector.add_goal(3, Goal::AvoidEntity(AvoidEntityGoal::new("minecraft:wolf", 6.0, 1.0, 1.2)));
        goal_selector.add_goal(5, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::new(1.0)));
        goal_selector.add_goal(6, Goal::LookAtPlayer(LookAtPlayerGoal::new(8.0)));
        goal_selector.add_goal(6, Goal::RandomLookAround(RandomLookAroundGoal::default()));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::HurtByTarget(HurtByTargetGoal::new(&[])));
        target_selector.add_goal(2, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::Player, true)));
        target_selector.add_goal(3, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::IronGolem, true)));
        target_selector.add_goal(3, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::with(TargetType::Turtle, 10, true, false, Some(zombie::baby_on_land_selector))));
    }

    pub fn get_hard_attack_interval(mob: &Mob) -> i32 {
        match mob.kind {
            MobKind::Bogged(_) => Bogged::get_hard_attack_interval(),
            MobKind::Parched(_) => super::parched::Parched::get_hard_attack_interval(),
            _ => HARD_ATTACK_INTERVAL,
        }
    }

    pub fn get_attack_interval(mob: &Mob) -> i32 {
        match mob.kind {
            MobKind::Bogged(_) => Bogged::get_attack_interval(),
            MobKind::Parched(_) => super::parched::Parched::get_attack_interval(),
            _ => NORMAL_ATTACK_INTERVAL,
        }
    }
}

impl ServerLevel {
    pub fn abstract_skeleton_ride_tick(&mut self, id: EntityId) {
        self.entity_ride_tick(id);
        if let Some(vehicle) = self.controlled_vehicle(id).filter(|&vehicle| self.is_mob(vehicle)) {
            let y_body_rot = self.living(vehicle).y_body_rot;
            self.living_mut(id).y_body_rot = y_body_rot;
        }
    }

    pub fn abstract_skeleton_populate_default_equipment_slots(&mut self, id: EntityId, difficulty: &DifficultyInstance) {
        self.mob_populate_default_equipment_slots(id, difficulty);
        self.living_set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::new(items::BOW, 1));
    }

    pub fn abstract_skeleton_finalize_spawn(&mut self, id: EntityId, difficulty: &DifficultyInstance, spawn_reason: EntitySpawnReason, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
        let group_data = self.mob_finalize_spawn(id, difficulty, spawn_reason, group_data);
        self.populate_default_equipment_slots(id, difficulty);
        self.populate_default_equipment_enchantments(id, difficulty);
        self.reassess_weapon_goal(id);
        let can_pick_up_loot = self.random.next_float() < 0.55 * difficulty.get_special_multiplier();
        self.mob_mut(id).expect("a skeleton").can_pick_up_loot = can_pick_up_loot;
        if self.item_by_slot_ref(id, EquipmentSlot::Head).is_none() && special_dates::is_halloween() && self.random.next_float() < 0.25 {
            let item = if self.random.next_float() < 0.1 { items::JACK_O_LANTERN } else { items::CARVED_PUMPKIN };
            self.living_set_item_slot(id, EquipmentSlot::Head, ItemStack::new(item, 1));
            self.mob_mut(id).expect("a skeleton").set_drop_chance(EquipmentSlot::Head, 0.0);
        }
        group_data
    }

    pub fn reassess_weapon_goal(&mut self, id: EntityId) {
        self.with_goal_selector(id, |goal_selector, level| {
            for goal in goal_selector.remove_all_goals(level, id, |goal| matches!(goal, Goal::AbstractSkeletonMelee(_))) {
                if let Goal::AbstractSkeletonMelee(melee_goal) = goal {
                    abstract_skeleton_mut(level.mob_mut(id).expect("a skeleton")).melee_goal = Some(melee_goal);
                }
            }
            for goal in goal_selector.remove_all_goals(level, id, |goal| matches!(goal, Goal::RangedBowAttack(_))) {
                if let Goal::RangedBowAttack(bow_goal) = goal {
                    abstract_skeleton_mut(level.mob_mut(id).expect("a skeleton")).bow_goal = Some(bow_goal);
                }
            }
            let hand = projectile_util::get_weapon_holding_hand(level, id, items::BOW);
            let used_weapon_is_bow = level.get_item_in_hand(id, hand).is_some_and(|used_weapon| used_weapon.is(items::BOW));
            let difficulty = level.difficulty;
            let mob = level.mob_mut(id).expect("a skeleton");
            if used_weapon_is_bow {
                let mut min_attack_interval = AbstractSkeleton::get_hard_attack_interval(mob);
                if difficulty != Difficulty::Hard {
                    min_attack_interval = AbstractSkeleton::get_attack_interval(mob);
                }
                if let Some(mut bow_goal) = abstract_skeleton_mut(mob).bow_goal.take() {
                    bow_goal.set_min_attack_interval(min_attack_interval);
                    goal_selector.add_goal(4, Goal::RangedBowAttack(bow_goal));
                }
            } else if let Some(melee_goal) = abstract_skeleton_mut(mob).melee_goal.take() {
                goal_selector.add_goal(4, Goal::AbstractSkeletonMelee(melee_goal));
            }
        });
    }

    pub fn reassess_weapon_goal_detached(&mut self, entity: LevelEntity) -> LevelEntity {
        if !matches!(&entity, LevelEntity::Mob(mob) if mob.is_abstract_skeleton()) {
            return entity;
        }
        let id = entity.entity().id;
        self.entities.insert(id, entity);
        self.reassess_weapon_goal(id);
        self.entities.shift_remove(&id).expect("the loaded skeleton")
    }

    pub fn abstract_skeleton_perform_ranged_attack(&mut self, id: EntityId, target: EntityId, power: f32) {
        let hand = projectile_util::get_weapon_holding_hand(self, id, items::BOW);
        let bow_item = self.get_item_in_hand(id, hand).unwrap_or_else(ItemStack::empty);
        let projectile = self.monster_get_projectile(id, &bow_item).stack;
        let arrow = self.get_arrow(id, &projectile, power, Some(&bow_item));
        let (e, t) = (self.entity(id), self.entity(target));
        let xd = t.x() - e.x();
        let yd = t.get_y(0.3333333333333333) - arrow.entity.y();
        let zd = t.z() - e.z();
        let distance_to_target = (xd * xd + zd * zd).sqrt();
        let uncertainty = self.ranged_attack_uncertainty(id);
        self.spawn_projectile_using_shoot(arrow, &projectile, xd, yd + distance_to_target * 0.2f32 as f64, zd, 1.6, uncertainty);
        let pitch = 1.0 / (self.entity_mut(id).random.next_float() * 0.4 + 0.8);
        self.entity_play_sound(id, sound_event("minecraft:entity.skeleton.shoot"), 1.0, pitch);
    }

    pub fn get_arrow(&mut self, id: EntityId, projectile: &ItemStack, power: f32, firing_weapon: Option<&ItemStack>) -> Projectile {
        match self.mob(id).expect("a skeleton").kind {
            MobKind::Stray(_) => self.stray_get_arrow(id, projectile, power, firing_weapon),
            MobKind::Parched(_) => self.parched_get_arrow(id, projectile, power, firing_weapon),
            MobKind::WitherSkeleton(_) => self.wither_skeleton_get_arrow(id, projectile, power, firing_weapon),
            MobKind::Bogged(_) => self.bogged_get_arrow(id, projectile, power, firing_weapon),
            _ => self.abstract_skeleton_get_arrow(id, projectile, power, firing_weapon),
        }
    }

    pub fn abstract_skeleton_get_arrow(&mut self, id: EntityId, projectile: &ItemStack, power: f32, firing_weapon: Option<&ItemStack>) -> Projectile {
        projectile_util::get_mob_arrow(self, id, projectile, power, firing_weapon)
    }

    pub fn abstract_skeleton_on_equip_item(&mut self, id: EntityId, slot: EquipmentSlot, old_stack: &ItemStack, stack: &ItemStack) {
        self.living_entity_on_equip_item(id, slot, old_stack, stack);
        self.reassess_weapon_goal(id);
    }

    pub fn abstract_skeleton_wants_to_pick_up(&self, id: EntityId, item_stack: &ItemStack) -> bool {
        if item_stack.get_item().is(&item_tags::SPEARS) { false } else { self.mob_wants_to_pick_up(id, item_stack) }
    }
}
