use super::abstract_cube_mob::{self, AbstractCubeMob, CubeMobAttackGoal};
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::ageable_mob::AgeableMobGroupData;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::attributes::{Attribute, SLIME_ATTRIBUTES};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{EntityDimensions, EntityId};
use crate::server::level::ServerLevel;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.52, 0.52).with_eye_height(0.325);

#[derive(Default)]
pub struct Slime {
    pub abstract_cube_mob: AbstractCubeMob,
}

pub fn is_within_four_blocks_vertically(mob: EntityId, target: EntityId, level: &ServerLevel) -> bool {
    (level.entity(target).y() - level.entity(mob).y()).abs() <= 4.0
}

impl Slime {
    pub fn create() -> Mob {
        AbstractCubeMob::create_with("minecraft:slime", DIMENSIONS, &SLIME_ATTRIBUTES, MobKind::Slime(Slime::default()))
    }

    pub fn register_goals(ai: &mut MobAi) {
        AbstractCubeMob::register_goals(ai);
        Slime::add_behaviour_goals(ai);
        Slime::add_targeting_goals(ai);
    }

    pub fn add_behaviour_goals(ai: &mut MobAi) {
        ai.goal_selector.add_goal(2, Goal::CubeMobAttack(CubeMobAttackGoal::default()));
    }

    pub fn add_targeting_goals(ai: &mut MobAi) {
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::with(TargetType::Player, 10, true, false, Some(is_within_four_blocks_vertically))));
        target_selector.add_goal(3, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::IronGolem, true)));
    }

    pub fn get_hurt_sound(mob: &Mob) -> &'static str {
        if mob.is_tiny() { "minecraft:entity.slime.hurt_small" } else { "minecraft:entity.slime.hurt" }
    }

    pub fn get_death_sound(mob: &Mob) -> &'static str {
        if mob.is_tiny() { "minecraft:entity.slime.death_small" } else { "minecraft:entity.slime.death" }
    }

    pub fn get_particle_type() -> Option<&'static str> {
        Some("minecraft:item_slime")
    }

    pub fn get_squish_sound(mob: &Mob) -> &'static str {
        if mob.is_tiny() { "minecraft:entity.slime.squish_small" } else { "minecraft:entity.slime.squish" }
    }

    pub fn get_jump_sound(mob: &Mob) -> &'static str {
        if mob.is_tiny() { "minecraft:entity.slime.jump_small" } else { "minecraft:entity.slime.jump" }
    }
}

impl Mob {
    pub fn slime_set_size(&mut self, size: i32, update_health: bool) -> bool {
        let changed = self.abstract_cube_mob_set_size(size, update_health);
        let actual_size = abstract_cube_mob::get_size(&self.entity);
        self.living.attributes.instance(Attribute::AttackDamage).expect("slimes have an attack damage").set_base_value(actual_size as f64);
        self.xp_reward = actual_size;
        changed
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
