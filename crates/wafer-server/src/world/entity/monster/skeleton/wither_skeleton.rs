use wafer_core::Vec3;
use wafer_data::{EquipmentSlot, item_tags, items, mob_effects};

use super::abstract_skeleton::AbstractSkeleton;
use crate::server::level::ServerLevel;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::attributes::{Attribute, SKELETON_ATTRIBUTES};
use crate::world::entity::data::mob_defaults;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::projectile::projectile::Projectile;
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{EntityDimensions, EntityId};
use crate::world::item::ItemStack;
use crate::world::level::pathfinder::path_type::PathType;

pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.7, 2.4).with_eye_height(2.1).with_vehicle_attachment(&[Vec3::new(0.0, 0.875f32 as f64, 0.0)]);

#[derive(Default)]
pub struct WitherSkeleton {
    pub abstract_skeleton: AbstractSkeleton,
}

impl WitherSkeleton {
    pub fn create() -> Mob {
        let mut mob = AbstractSkeleton::create_with("minecraft:wither_skeleton", &SKELETON_ATTRIBUTES, mob_defaults(), MobKind::WitherSkeleton(WitherSkeleton::default()));
        mob.entity.dimensions = DIMENSIONS;
        mob.refresh_dimensions();
        mob.set_pathfinding_malus(PathType::Lava, 8.0);
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        ai.target_selector.add_goal(3, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::AbstractPiglin, true)));
        AbstractSkeleton::register_goals(ai);
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.wither_skeleton.ambient"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.wither_skeleton.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.wither_skeleton.death"
    }

    pub fn get_step_sound() -> &'static str {
        "minecraft:entity.wither_skeleton.step"
    }
}

impl ServerLevel {
    pub fn wither_skeleton_can_hold_item(&self, id: EntityId, item_stack: &ItemStack) -> bool {
        !item_stack.get_item().is(&item_tags::WITHER_SKELETON_DISLIKED_WEAPONS) && self.mob_can_hold_item(id, item_stack)
    }

    pub fn wither_skeleton_populate_default_equipment_slots(&mut self, id: EntityId) {
        self.living_set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::new(items::STONE_SWORD, 1));
    }

    pub fn wither_skeleton_finalize_spawn(&mut self, id: EntityId, difficulty: &DifficultyInstance, spawn_reason: EntitySpawnReason, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
        let spawn_group_data = self.abstract_skeleton_finalize_spawn(id, difficulty, spawn_reason, group_data);
        self.mob_mut(id).expect("a wither skeleton").living.attributes.instance(Attribute::AttackDamage).expect("wither skeletons have attack damage").set_base_value(4.0);
        self.reassess_weapon_goal(id);
        spawn_group_data
    }

    pub fn wither_skeleton_do_hurt_target(&mut self, id: EntityId, target: EntityId) -> bool {
        if !self.mob_do_hurt_target(id, target) {
            return false;
        }
        if self.is_living(target) {
            self.add_effect(target, MobEffectInstance::new(mob_effects::WITHER, 200, 0), Some(id));
        }
        true
    }

    pub fn wither_skeleton_get_arrow(&mut self, id: EntityId, projectile: &ItemStack, power: f32, firing_weapon: Option<&ItemStack>) -> Projectile {
        let mut arrow = self.abstract_skeleton_get_arrow(id, projectile, power, firing_weapon);
        arrow.entity.ignite_for_ticks(wafer_core::mth::floor_f(100.0 * 20.0));
        arrow
    }

    pub fn wither_skeleton_can_be_affected(&self, id: EntityId, effect: &MobEffectInstance) -> bool {
        if effect.effect == mob_effects::WITHER { false } else { self.living_entity_can_be_affected(id, effect) }
    }
}
