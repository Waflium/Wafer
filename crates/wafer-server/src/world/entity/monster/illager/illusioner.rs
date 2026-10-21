use wafer_core::Vec3;
use wafer_core::random::RandomSource;
use wafer_data::{EquipmentSlot, items, mob_effects, registry, registry_id};

use super::spellcaster_illager::{IllagerSpell, SpellcasterCastingSpellGoal, SpellcasterIllager, SpellcasterUseSpellGoal};
use crate::server::level::ServerLevel;
use crate::world::difficulty::Difficulty;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::avoid_entity_goal::AvoidEntityGoal;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::random_stroll_goal::RandomStrollGoal;
use crate::world::entity::ai::goal::ranged_bow_attack_goal::RangedBowAttackGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::attributes::ILLUSIONER_ATTRIBUTES;
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::projectile::projectile_util;
use crate::world::entity::raid::raider::{self, RAIDER_TYPES};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, sound_event};
use crate::world::item::ItemStack;

const XP_REWARD: i32 = 5;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.6, 1.95)
    .with_passenger_attachments(&[Vec3::new(0.0, 2.0, 0.0)])
    .with_vehicle_attachment(&[Vec3::new(0.0, 0.6f32 as f64, 0.0)]);

#[derive(Default)]
pub struct Illusioner {
    pub spellcaster_illager: SpellcasterIllager,
}

impl Illusioner {
    pub fn create() -> Mob {
        let mut data = mob_defaults();
        data.push(DataValue::Boolean(false));
        data.push(DataValue::Byte(0));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:illusioner"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&ILLUSIONER_ATTRIBUTES), MobKind::Illusioner(Illusioner::default()));
        mob.xp_reward = XP_REWARD;
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        crate::world::entity::raid::raider::Raider::register_goals(ai);
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::Float(float_goal));
        goal_selector.add_goal(1, Goal::SpellcasterCastingSpell(SpellcasterCastingSpellGoal));
        goal_selector.add_goal(3, Goal::AvoidEntity(AvoidEntityGoal::new("minecraft:creaking", 8.0, 1.0, 1.2)));
        goal_selector.add_goal(4, Goal::IllusionerMirrorSpell(IllusionerMirrorSpellGoal::default()));
        goal_selector.add_goal(5, Goal::IllusionerBlindnessSpell(IllusionerBlindnessSpellGoal::default()));
        goal_selector.add_goal(6, Goal::RangedBowAttack(RangedBowAttackGoal::new(0.5, 20, 15.0)));
        goal_selector.add_goal(8, Goal::RandomStroll(RandomStrollGoal::new(0.6)));
        goal_selector.add_goal(9, Goal::LookAtPlayer(LookAtPlayerGoal::new(3.0).with_probability(1.0)));
        goal_selector.add_goal(10, Goal::LookAtPlayer(LookAtPlayerGoal::of_type(TargetType::Mob, 8.0)));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::HurtByTarget(HurtByTargetGoal::new(RAIDER_TYPES).set_alert_others(&[])));
        for (priority, target_type, must_see) in [(2, TargetType::Player, true), (3, TargetType::AbstractVillager, false), (3, TargetType::IronGolem, false)] {
            let mut goal = NearestAttackableTargetGoal::new(target_type, must_see);
            goal.target_goal.set_unseen_memory_ticks(300);
            target_selector.add_goal(priority, Goal::NearestAttackableTarget(goal));
        }
    }

    pub fn get_celebrate_sound() -> &'static str {
        "minecraft:entity.illusioner.ambient"
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.illusioner.ambient"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.illusioner.death"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.illusioner.hurt"
    }

    pub fn get_casting_sound_event() -> &'static str {
        "minecraft:entity.illusioner.cast_spell"
    }
}

impl ServerLevel {
    pub fn illusioner_finalize_spawn(&mut self, id: EntityId, difficulty: &DifficultyInstance, spawn_reason: EntitySpawnReason, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
        self.living_set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::new(items::BOW, 1));
        let (index, _, entity) = self.entities.shift_remove_full(&id).expect("the illusioner to finalize");
        let LevelEntity::Mob(mut mob) = entity else { unreachable!("a mob") };
        let spawn_group_data = raider::finalize_spawn(self, &mut mob, difficulty, spawn_reason, group_data);
        self.entities.shift_insert(index, id, LevelEntity::Mob(mob));
        spawn_group_data
    }

    pub fn illusioner_perform_ranged_attack(&mut self, id: EntityId, target: EntityId, power: f32) {
        let hand = projectile_util::get_weapon_holding_hand(self, id, items::BOW);
        let bow_item = self.get_item_in_hand(id, hand).unwrap_or_else(ItemStack::empty);
        let projectile = self.monster_get_projectile(id, &bow_item).stack;
        let arrow = projectile_util::get_mob_arrow(self, id, &projectile, power, Some(&bow_item));
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
}

#[derive(Default)]
pub struct IllusionerBlindnessSpellGoal {
    pub use_spell_goal: SpellcasterUseSpellGoal,
    last_target_id: EntityId,
}

impl IllusionerBlindnessSpellGoal {
    const CASTING_TIME: i32 = 20;
    const CASTING_INTERVAL: i32 = 180;

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if !self.use_spell_goal.can_use(level, mob) {
            return false;
        }
        let Some(target) = level.get_target(mob) else { return false };
        if target == self.last_target_id {
            return false;
        }
        let pos = level.entity(mob).block_position();
        level.current_difficulty_at(pos).is_harder_than(Difficulty::Normal as i32 as f32)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.use_spell_goal.start(level, mob, 20, Self::CASTING_TIME, Self::CASTING_INTERVAL, Some("minecraft:entity.illusioner.prepare_blindness"), IllagerSpell::Blindness);
        if let Some(target) = level.get_target(mob) {
            self.last_target_id = target;
        }
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if self.use_spell_goal.tick_warmup() {
            if let Some(target) = level.get_target(mob) {
                level.add_effect(target, MobEffectInstance::new(mob_effects::BLINDNESS, 400, 0), Some(mob));
            }
            SpellcasterUseSpellGoal::play_casting_sound(level, mob);
        }
    }
}

#[derive(Default)]
pub struct IllusionerMirrorSpellGoal {
    pub use_spell_goal: SpellcasterUseSpellGoal,
}

impl IllusionerMirrorSpellGoal {
    const CASTING_TIME: i32 = 20;
    const CASTING_INTERVAL: i32 = 340;

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.use_spell_goal.can_use(level, mob) && !level.has_effect(mob, mob_effects::INVISIBILITY)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.use_spell_goal.start(level, mob, 20, Self::CASTING_TIME, Self::CASTING_INTERVAL, Some("minecraft:entity.illusioner.prepare_mirror"), IllagerSpell::Disappear);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if self.use_spell_goal.tick_warmup() {
            level.add_effect(mob, MobEffectInstance::new(mob_effects::INVISIBILITY, 1200, 0), None);
            SpellcasterUseSpellGoal::play_casting_sound(level, mob);
        }
    }
}
