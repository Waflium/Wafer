use super::command_frames::{ENTITY_TICK, TICK_CHILDREN_LEVELS};
use super::main_thread_frames::{self, FrameScope};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::mob::MobKind;

const MELEE_ATTACK_GOAL: [&str; 2] = [
    "net.minecraft.world.entity.ai.goal.MeleeAttackGoal.checkAndPerformAttack(MeleeAttackGoal.java:151)",
    "net.minecraft.world.entity.ai.goal.MeleeAttackGoal.tick(MeleeAttackGoal.java:144)",
];
const ZOMBIE_ATTACK_GOAL_TICK: &str = "net.minecraft.world.entity.ai.goal.ZombieAttackGoal.tick(ZombieAttackGoal.java:28)";
const GOAL_SELECTOR_TICK_RUNNING_GOALS: [&str; 2] = [
    "net.minecraft.world.entity.ai.goal.WrappedGoal.tick(WrappedGoal.java:66)",
    "net.minecraft.world.entity.ai.goal.GoalSelector.tickRunningGoals(GoalSelector.java:105)",
];
const GOAL_SELECTOR_TICK: &str = "net.minecraft.world.entity.ai.goal.GoalSelector.tick(GoalSelector.java:96)";
const SERVER_AI_STEP_GOAL_SELECTOR_TICK: &str = "net.minecraft.world.entity.Mob.serverAiStep(Mob.java:800)";
const SERVER_AI_STEP_TICK_RUNNING_GOALS: &str = "net.minecraft.world.entity.Mob.serverAiStep(Mob.java:808)";
const MONSTER_AI_STEP: [&str; 3] = [
    "net.minecraft.world.entity.LivingEntity.aiStep(LivingEntity.java:3187)",
    "net.minecraft.world.entity.Mob.aiStep(Mob.java:513)",
    "net.minecraft.world.entity.monster.Monster.aiStep(Monster.java:44)",
];
const RAIDER_AI_STEP: &str = "net.minecraft.world.entity.raid.Raider.aiStep(Raider.java:117)";
const MOB_TICK: [&str; 2] = ["net.minecraft.world.entity.LivingEntity.tick(LivingEntity.java:2886)", "net.minecraft.world.entity.Mob.tick(Mob.java:379)"];
const ZOMBIE_TICK: &str = "net.minecraft.world.entity.monster.zombie.Zombie.tick(Zombie.java:234)";
const ZOMBIE_VILLAGER_TICK: &str = "net.minecraft.world.entity.monster.zombie.ZombieVillager.tick(ZombieVillager.java:176)";
const SKELETON_TICK: &str = "net.minecraft.world.entity.monster.skeleton.Skeleton.tick(Skeleton.java:54)";

pub fn melee_attack(level: &ServerLevel, mob: EntityId, target: EntityId) -> Option<FrameScope> {
    if !level.is_player(target) {
        return None;
    }
    let entity = level.mob(mob)?;
    let zombie_goal = matches!(entity.kind, MobKind::Zombie(_) | MobKind::Husk(_) | MobKind::ZombieVillager(_));
    let tick_overrides: &[&str] = match entity.kind {
        MobKind::Zombie(_) | MobKind::Husk(_) => &[ZOMBIE_TICK],
        MobKind::ZombieVillager(_) => &[ZOMBIE_TICK, ZOMBIE_VILLAGER_TICK],
        MobKind::Skeleton(_) => &[SKELETON_TICK],
        MobKind::WitherSkeleton(_) | MobKind::Vindicator(_) => &[],
        _ => return None,
    };
    let mut frames: Vec<&'static str> = MELEE_ATTACK_GOAL.to_vec();
    if zombie_goal {
        frames.push(ZOMBIE_ATTACK_GOAL_TICK);
    }
    frames.extend(GOAL_SELECTOR_TICK_RUNNING_GOALS);
    if entity.entity.tick_count.wrapping_add(mob) % 2 != 0 && entity.entity.tick_count > 1 {
        frames.push(SERVER_AI_STEP_TICK_RUNNING_GOALS);
    } else {
        frames.push(GOAL_SELECTOR_TICK);
        frames.push(SERVER_AI_STEP_GOAL_SELECTOR_TICK);
    }
    frames.extend(MONSTER_AI_STEP);
    if matches!(entity.kind, MobKind::Vindicator(_)) {
        frames.push(RAIDER_AI_STEP);
    }
    frames.extend(MOB_TICK);
    frames.extend_from_slice(tick_overrides);
    frames.extend(ENTITY_TICK);
    frames.push(TICK_CHILDREN_LEVELS);
    Some(main_thread_frames::enter(frames))
}
