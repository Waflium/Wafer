use super::melee_attack_goal::MeleeAttackGoal;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub struct ZombieAttackGoal {
    pub melee_attack_goal: MeleeAttackGoal,
    raise_arm_ticks: i32,
}

impl ZombieAttackGoal {
    pub fn new(speed_modifier: f64, track_target: bool) -> ZombieAttackGoal {
        ZombieAttackGoal { melee_attack_goal: MeleeAttackGoal::new(speed_modifier, track_target), raise_arm_ticks: 0 }
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.melee_attack_goal.start(level, mob);
        self.raise_arm_ticks = 0;
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.melee_attack_goal.stop(level, mob);
        level.mob_mut(mob).expect("a zombie").set_aggressive(false);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.melee_attack_goal.tick(level, mob);
        self.raise_arm_ticks += 1;
        let aggressive = self.raise_arm_ticks >= 5 && self.melee_attack_goal.get_ticks_until_next_attack() < self.melee_attack_goal.get_attack_interval() / 2;
        if let Some(zombie) = level.mob_mut(mob) {
            zombie.set_aggressive(aggressive);
        }
    }
}
