use wafer_core::Vec3;
use wafer_data::{Tag, fluid_tags};

use super::abstract_cube_mob::{self, AbstractCubeMob, CubeMobAttackGoal};
use super::slime::is_within_four_blocks_vertically;
use crate::server::level::ServerLevel;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::ageable_mob::AgeableMobGroupData;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::attributes::{Attribute, MAGMA_CUBE_ATTRIBUTES};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{EntityDimensions, EntityId};
use crate::world::level::server_level_accessor::ServerLevelAccessor;

pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.52, 0.52).with_eye_height(0.325);

#[derive(Default)]
pub struct MagmaCube {
    pub abstract_cube_mob: AbstractCubeMob,
}

impl MagmaCube {
    pub fn create() -> Mob {
        AbstractCubeMob::create_with("minecraft:magma_cube", DIMENSIONS, &MAGMA_CUBE_ATTRIBUTES, MobKind::MagmaCube(MagmaCube::default()))
    }

    pub fn register_goals(ai: &mut MobAi) {
        AbstractCubeMob::register_goals(ai);
        MagmaCube::add_behaviour_goals(ai);
        MagmaCube::add_targeting_goals(ai);
    }

    pub fn add_behaviour_goals(ai: &mut MobAi) {
        ai.goal_selector.add_goal(2, Goal::CubeMobAttack(CubeMobAttackGoal::default()));
    }

    pub fn add_targeting_goals(ai: &mut MobAi) {
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::with(TargetType::Player, 10, true, false, Some(is_within_four_blocks_vertically))));
        target_selector.add_goal(3, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::IronGolem, true)));
    }

    pub fn get_particle_type() -> Option<&'static str> {
        Some("minecraft:flame")
    }

    pub fn get_hurt_sound(mob: &Mob) -> &'static str {
        if mob.is_tiny() { "minecraft:entity.magma_cube.hurt_small" } else { "minecraft:entity.magma_cube.hurt" }
    }

    pub fn get_death_sound(mob: &Mob) -> &'static str {
        if mob.is_tiny() { "minecraft:entity.magma_cube.death_small" } else { "minecraft:entity.magma_cube.death" }
    }

    pub fn get_squish_sound(mob: &Mob) -> &'static str {
        if mob.is_tiny() { "minecraft:entity.magma_cube.squish_small" } else { "minecraft:entity.magma_cube.squish" }
    }

    pub fn get_jump_sound() -> &'static str {
        "minecraft:entity.magma_cube.jump"
    }

    pub fn get_light_level_dependent_magic_value() -> f32 {
        1.0
    }
}

impl Mob {
    pub fn magma_cube_set_size(&mut self, size: i32, update_health: bool) -> bool {
        let changed = self.abstract_cube_mob_set_size(size, update_health);
        let actual_size = abstract_cube_mob::get_size(&self.entity);
        let attributes = &mut self.living.attributes;
        attributes.instance(Attribute::AttackDamage).expect("magma cubes have an attack damage").set_base_value(actual_size as f64);
        attributes.instance(Attribute::Armor).expect("magma cubes have armor").set_base_value((size * 3) as f64);
        self.xp_reward = actual_size;
        changed
    }

    pub fn magma_cube_get_jump_delay(&mut self) -> i32 {
        self.abstract_cube_mob_get_jump_delay() * 4
    }
}

impl ServerLevel {
    pub fn magma_cube_jump_from_ground(&mut self, id: EntityId) {
        let size_jump_boost_power = self.mob(id).expect("a magma cube").get_size() as f32 * 0.1;
        let jump_power = self.get_jump_power(id) + size_jump_boost_power;
        let e = self.entity_mut(id);
        let movement = e.delta_movement;
        e.set_delta_movement(Vec3::new(movement.x, jump_power as f64, movement.z));
        e.needs_sync = true;
    }

    pub fn magma_cube_jump_in_liquid(&mut self, id: EntityId, ty: &Tag) {
        if ty.name == fluid_tags::LAVA.name {
            let size = self.mob(id).expect("a magma cube").get_size();
            let e = self.entity_mut(id);
            let movement = e.delta_movement;
            e.set_delta_movement(Vec3::new(movement.x, (0.22f32 + size as f32 * 0.05f32) as f64, movement.z));
            e.needs_sync = true;
        } else {
            self.mob_jump_in_liquid(id, ty);
        }
    }

    pub fn magma_cube_can_deal_damage(&self, id: EntityId) -> bool {
        self.is_effective_ai(id)
    }

    pub fn magma_cube_get_attack_damage(&self, id: EntityId) -> f32 {
        self.abstract_cube_mob_get_attack_damage(id) + 2.0
    }
}

pub fn finalize_spawn(
    level: &mut (impl ServerLevelAccessor + ?Sized),
    mob: &mut Mob,
    difficulty: &DifficultyInstance,
    spawn_reason: EntitySpawnReason,
    group_data: Option<SpawnGroupData>,
) -> Option<SpawnGroupData> {
    let group_data = group_data.or(Some(SpawnGroupData::AgeableMob(AgeableMobGroupData::new(false))));
    abstract_cube_mob::finalize_spawn(level, mob, difficulty, spawn_reason, group_data)
}
