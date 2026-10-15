use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

#[derive(Default)]
pub struct OcelotAttackGoal {
    target: Option<EntityId>,
    attack_time: i32,
}

impl OcelotAttackGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let Some(best_target) = level.get_target(mob) else { return false };
        self.target = Some(best_target);
        true
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let Some(target) = self.target.filter(|&target| level.entity_ref(target).is_some() && level.is_alive(target)) else { return false };
        if level.entity(mob).distance_to_sqr(level.entity(target)) > 225.0 {
            false
        } else {
            !level.mob(mob).expect("a mob").ai.navigation.is_done() || self.can_use(level, mob)
        }
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.target = None;
        level.mob_mut(mob).expect("a mob").ai.navigation.stop();
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let target = self.target.expect("an attack target");
        level.set_look_at_entity(mob, target, 30.0, 30.0);
        let bb_width = level.entity(mob).dimensions.width;
        let melee_radius_sqr = (bb_width * 2.0 * (bb_width * 2.0)) as f64;
        let t = level.entity(target);
        let dist_sqr = level.entity(mob).distance_to_sqr_xyz(t.x(), t.y(), t.z());
        let mut speed_modifier = 0.8;
        if dist_sqr > melee_radius_sqr && dist_sqr < 16.0 {
            speed_modifier = 1.33;
        } else if dist_sqr < 225.0 {
            speed_modifier = 0.6;
        }
        level.get_navigation(mob, |navigation, level| navigation.move_to_entity(level, mob, target, speed_modifier));
        self.attack_time = (self.attack_time - 1).max(0);
        if dist_sqr <= melee_radius_sqr && self.attack_time <= 0 {
            self.attack_time = 20;
            level.do_hurt_target(mob, target);
        }
    }
}
