use wafer_core::Vec3;
use wafer_core::random::RandomSource;
use wafer_data::{EquipmentSlot, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::abstract_horse::{self, AbstractHorse, abstract_horse_defaults};
use crate::server::level::ServerLevel;
use crate::world::InteractionHand;
use crate::world::InteractionResult;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::attributes::{Attribute, SKELETON_HORSE_ATTRIBUTES};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::lightning_bolt::LightningBolt;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, sound_event};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::component::values::ItemEnchantments;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::item::enchantment::enchantment_helper;
use crate::world::item::enchantment::providers::vanilla_enchantment_providers;

const TRAP_MAX_LIFE: i32 = 18000;
const DEFAULT_IS_TRAP: bool = false;
const DEFAULT_TRAP_TIME: i32 = 0;
const LIGHTNING_INVULNERABLE_TICKS: i32 = 60;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(1.3964844, 1.6).with_eye_height(1.52).with_passenger_attachments(&[Vec3::new(0.0, 1.31875f32 as f64, 0.0)]);
const BABY_DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(1.3964844, 1.6).with_eye_height(1.52).with_passenger_attachments(&[Vec3::new(0.0, (1.6f32 - 0.25f32) as f64, 0.0)]);

#[derive(Default)]
pub struct SkeletonHorse {
    pub abstract_horse: AbstractHorse,
    is_trap: bool,
    trap_time: i32,
    trap_goal_removal_pending: bool,
}

fn skeleton_horse(mob: &Mob) -> &SkeletonHorse {
    match &mob.kind {
        MobKind::SkeletonHorse(horse) => horse,
        _ => unreachable!("a skeleton horse"),
    }
}

fn skeleton_horse_mut(mob: &mut Mob) -> &mut SkeletonHorse {
    match &mut mob.kind {
        MobKind::SkeletonHorse(horse) => horse,
        _ => unreachable!("a skeleton horse"),
    }
}

pub fn baby_dimensions() -> EntityDimensions {
    BABY_DIMENSIONS.scale(abstract_horse::BABY_SCALE)
}

pub fn randomize_attributes(mob: &mut Mob, random: &mut (impl RandomSource + ?Sized)) {
    let jump_strength = abstract_horse::generate_jump_strength(|| random.next_double());
    mob.living.attributes.instance(Attribute::JumpStrength).expect("horses have a jump strength").set_base_value(jump_strength);
}

pub fn get_ambient_sound(mob: &Mob) -> &'static str {
    if mob.entity.is_eye_in_fluid(&wafer_data::fluid_tags::WATER) { "minecraft:entity.skeleton_horse.ambient_water" } else { "minecraft:entity.skeleton_horse.ambient" }
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.skeleton_horse.death"
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.skeleton_horse.hurt"
}

pub fn get_water_slow_down() -> f32 {
    0.96
}

impl SkeletonHorse {
    pub fn create() -> Mob {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:skeleton_horse"), DIMENSIONS, abstract_horse_defaults());
        let mut mob = Mob::new(entity, LivingEntity::new(&SKELETON_HORSE_ATTRIBUTES), MobKind::SkeletonHorse(SkeletonHorse::default()));
        AbstractHorse::init(&mut mob);
        mob
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        AbstractHorse::add_additional_save_data(mob, output);
        let horse = skeleton_horse(mob);
        output.push(("SkeletonTrap".into(), Nbt::Byte(horse.is_trap as i8)));
        output.push(("SkeletonTrapTime".into(), Nbt::Int(horse.trap_time)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        AbstractHorse::read_additional_save_data(mob, input);
        let trap = input.get("SkeletonTrap").and_then(Nbt::as_i64).map_or(DEFAULT_IS_TRAP, |v| v != 0);
        SkeletonHorse::set_trap(mob, trap);
        skeleton_horse_mut(mob).trap_time = input.get("SkeletonTrapTime").and_then(Nbt::as_i64).map_or(DEFAULT_TRAP_TIME, |v| v as i32);
    }

    pub fn is_trap(mob: &Mob) -> bool {
        skeleton_horse(mob).is_trap
    }

    pub fn set_trap(mob: &mut Mob, trap: bool) {
        let horse = skeleton_horse_mut(mob);
        if trap != horse.is_trap {
            horse.is_trap = trap;
            if trap {
                mob.ai.goal_selector.add_goal(1, Goal::SkeletonTrap(SkeletonTrapGoal));
            } else if mob.ai.goal_selector.remove_goal(|goal| matches!(goal, Goal::SkeletonTrap(_))).is_none() {
                skeleton_horse_mut(mob).trap_goal_removal_pending = true;
            }
        }
    }
}

impl ServerLevel {
    pub fn skeleton_horse_ai_step(&mut self, id: EntityId) {
        self.abstract_horse_ai_step(id);
        let Some(mob) = self.mob_mut(id) else { return };
        if std::mem::take(&mut skeleton_horse_mut(mob).trap_goal_removal_pending) {
            mob.ai.goal_selector.remove_goal(|goal| matches!(goal, Goal::SkeletonTrap(_)));
        }
        let persistence_required = mob.is_persistence_required();
        let horse = skeleton_horse_mut(mob);
        if !persistence_required && horse.is_trap {
            let trap_time = horse.trap_time;
            horse.trap_time += 1;
            if trap_time >= TRAP_MAX_LIFE {
                self.discard(id);
            }
        }
    }

    pub fn skeleton_horse_get_swim_sound(&mut self, id: EntityId) -> &'static str {
        if self.entity(id).on_ground {
            if !self.is_vehicle(id) {
                return "minecraft:entity.skeleton_horse.step_water";
            }
            let horse = abstract_horse::abstract_horse_mut(self.mob_mut(id).expect("a skeleton horse"));
            horse.gallop_sound_counter += 1;
            let counter = horse.gallop_sound_counter;
            if counter > 5 && counter % 3 == 0 {
                return "minecraft:entity.skeleton_horse.gallop_water";
            }
            if counter <= 5 {
                return "minecraft:entity.skeleton_horse.step_water";
            }
        }
        "minecraft:entity.skeleton_horse.swim"
    }

    pub fn skeleton_horse_swim_sound_volume(&self, id: EntityId, volume: f32) -> f32 {
        if self.entity(id).on_ground { 0.3 } else { 0.1f32.min(volume * 25.0) }
    }

    pub fn skeleton_horse_play_jump_sound(&mut self, id: EntityId) -> bool {
        if self.entity(id).is_in_water() {
            self.entity_play_sound(id, sound_event("minecraft:entity.skeleton_horse.jump_water"), 0.4, 1.0);
            return true;
        }
        false
    }

    pub fn skeleton_horse_get_breed_offspring(&mut self, _id: EntityId, _partner: EntityId) -> Option<Box<Mob>> {
        Some(Box::new(SkeletonHorse::create()))
    }

    pub fn skeleton_horse_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        if !self.is_tamed(id) { InteractionResult::Pass } else { self.abstract_horse_mob_interact(id, player, hand) }
    }

    pub fn strike_skeleton_trap(&mut self, pos: wafer_core::BlockPos) {
        let mut horse = SkeletonHorse::create();
        SkeletonHorse::set_trap(&mut horse, true);
        horse.set_age(0);
        horse.entity.set_pos(Vec3::new(pos.x as f64, pos.y as f64, pos.z as f64));
        self.add_fresh_entity(LevelEntity::Mob(Box::new(horse)));
    }
}

pub struct SkeletonTrapGoal;

impl SkeletonTrapGoal {
    pub fn can_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        let e = level.entity(mob);
        level.has_nearby_alive_player(e.x(), e.y(), e.z(), 10.0)
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let pos = level.entity(mob).block_position();
        let difficulty = level.current_difficulty_at(pos);
        let horse = level.mob_mut(mob).expect("a skeleton horse");
        SkeletonHorse::set_trap(horse, false);
        abstract_horse::set_tamed(&mut horse.entity, true);
        horse.set_age(0);
        let e = level.entity(mob);
        let (x, y, z) = (e.x(), e.y(), e.z());
        let mut bolt = LightningBolt::new(Vec3::ZERO);
        bolt.entity.snap_to(x, y, z, 0.0, 0.0);
        bolt.visual_only = true;
        level.add_fresh_entity(LevelEntity::LightningBolt(bolt));
        let skeleton = self.create_skeleton(level, &difficulty, mob);
        level.start_riding(skeleton, mob, false, true);
        let skeleton_entity = level.entities.shift_remove(&skeleton).expect("the trap skeleton");
        level.add_fresh_entity_with_passengers(skeleton_entity);
        for _ in 0..3 {
            let other_horse = self.create_horse(level, &difficulty, mob);
            let other_skeleton = self.create_skeleton(level, &difficulty, other_horse);
            level.start_riding(other_skeleton, other_horse, false, true);
            let random = &mut level.entity_mut(mob).random;
            let push_x = random.triangle(0.0, 1.1485);
            let push_z = random.triangle(0.0, 1.1485);
            level.entity_push(other_horse, push_x, 0.0, push_z);
            let other_horse_entity = level.entities.shift_remove(&other_horse).expect("the trap horse");
            level.add_fresh_entity_with_passengers(other_horse_entity);
        }
    }

    fn create_horse(&self, level: &mut ServerLevel, difficulty: &DifficultyInstance, horse: EntityId) -> EntityId {
        let other_horse = SkeletonHorse::create();
        let id = other_horse.entity.id;
        level.entities.insert(id, LevelEntity::Mob(Box::new(other_horse)));
        level.finalize_spawn(id, difficulty, EntitySpawnReason::Triggered, None);
        let position = level.entity(horse).position();
        let other = level.mob_mut(id).expect("a skeleton horse");
        other.entity.set_pos(position);
        other.entity.invulnerable_time = LIGHTNING_INVULNERABLE_TICKS;
        other.persistence_required = true;
        abstract_horse::set_tamed(&mut other.entity, true);
        other.set_age(0);
        id
    }

    fn create_skeleton(&self, level: &mut ServerLevel, difficulty: &DifficultyInstance, horse: EntityId) -> EntityId {
        let skeleton = crate::world::entity::monster::skeleton::skeleton::Skeleton::create();
        let id = skeleton.entity.id;
        level.entities.insert(id, LevelEntity::Mob(Box::new(skeleton)));
        level.finalize_spawn(id, difficulty, EntitySpawnReason::Triggered, None);
        let position = level.entity(horse).position();
        let mob = level.mob_mut(id).expect("a skeleton");
        mob.entity.set_pos(position);
        mob.entity.invulnerable_time = LIGHTNING_INVULNERABLE_TICKS;
        mob.persistence_required = true;
        if level.item_by_slot(id, EquipmentSlot::Head).is_none() {
            level.living_set_item_slot(id, EquipmentSlot::Head, ItemStack::new(items::IRON_HELMET, 1));
        }
        self.enchant(level, id, EquipmentSlot::Mainhand, difficulty);
        self.enchant(level, id, EquipmentSlot::Head, difficulty);
        id
    }

    fn enchant(&self, level: &mut ServerLevel, skeleton: EntityId, slot: EquipmentSlot, difficulty: &DifficultyInstance) {
        let mut stack = level.item_by_slot(skeleton, slot).unwrap_or_else(ItemStack::empty);
        stack.set(dc::ENCHANTMENTS, Some(ItemEnchantments::default().to_nbt()));
        let mut random = std::mem::replace(&mut level.entity_mut(skeleton).random, wafer_core::random::LegacyRandomSource::new(0));
        enchantment_helper::enchant_item_from_provider(&mut stack, vanilla_enchantment_providers::MOB_SPAWN_EQUIPMENT, difficulty, &mut random);
        level.entity_mut(skeleton).random = random;
        level.living_set_item_slot(skeleton, slot, stack);
    }
}
