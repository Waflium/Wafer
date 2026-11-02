use super::main_thread_frames::{self, FrameScope};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::mob::MobKind;

const MELEE_ATTACK_GOAL: [&str; 2] = [
    "net.minecraft.world.entity.ai.goal.MeleeAttackGoal.checkAndPerformAttack(MeleeAttackGoal.java:151)",
    "net.minecraft.world.entity.ai.goal.MeleeAttackGoal.tick(MeleeAttackGoal.java:144)",
];
const ZOMBIE_MELEE_ATTACK_GOAL: [&str; 3] = [
    "net.minecraft.world.entity.ai.goal.MeleeAttackGoal.checkAndPerformAttack(MeleeAttackGoal.java:151)",
    "net.minecraft.world.entity.ai.goal.MeleeAttackGoal.tick(MeleeAttackGoal.java:144)",
    "net.minecraft.world.entity.ai.goal.ZombieAttackGoal.tick(ZombieAttackGoal.java:28)",
];

pub fn melee_attack(level: &ServerLevel, mob: EntityId) -> FrameScope {
    let zombie_goal = level.mob(mob).is_some_and(|entity| matches!(entity.kind, MobKind::Zombie(_) | MobKind::Husk(_) | MobKind::ZombieVillager(_) | MobKind::Drowned(_) | MobKind::ZombifiedPiglin(_)));
    main_thread_frames::enter_static(if zombie_goal { &ZOMBIE_MELEE_ATTACK_GOAL } else { &MELEE_ATTACK_GOAL })
}
