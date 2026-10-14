use wafer_core::AABB;
use wafer_data::{game_rule, registry};

use super::target_goal::{TargetGoal, get_follow_distance};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;

const HURT_BY_TARGETING: TargetingConditions<'static> = TargetingConditions::for_combat().ignore_line_of_sight().ignore_invisibility_testing();
const ALERT_RANGE_Y: f64 = 10.0;

pub struct HurtByTargetGoal {
    pub target_goal: TargetGoal,
    alert_same_type: bool,
    timestamp: i32,
    to_ignore_damage: &'static [&'static str],
    to_ignore_alert: Option<&'static [&'static str]>,
}

impl HurtByTargetGoal {
    pub fn new(ignore_damage_from_these_types: &'static [&'static str]) -> HurtByTargetGoal {
        HurtByTargetGoal { target_goal: TargetGoal::new(true, false), alert_same_type: false, timestamp: 0, to_ignore_damage: ignore_damage_from_these_types, to_ignore_alert: None }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let timestamp = level.living(mob).last_hurt_by_mob_timestamp;
        let last_hurt_by_mob = level.get_last_hurt_by_mob(mob);
        let Some(last_hurt_by_mob) = last_hurt_by_mob.filter(|_| timestamp != self.timestamp) else { return false };
        if level.is_player(last_hurt_by_mob) && level.game_rules.get_bool(game_rule::UNIVERSAL_ANGER) {
            return false;
        }
        let last_hurt_by_type = registry::ENTITY_TYPE[level.entity(last_hurt_by_mob).entity_type as usize];
        if self.to_ignore_damage.contains(&last_hurt_by_type) {
            return false;
        }
        self.target_goal.can_attack(level, mob, Some(last_hurt_by_mob), &HURT_BY_TARGETING)
    }

    pub fn set_alert_others(mut self, except_these_types: &'static [&'static str]) -> HurtByTargetGoal {
        self.alert_same_type = true;
        self.to_ignore_alert = Some(except_these_types);
        self
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let last_hurt_by_mob = level.get_last_hurt_by_mob(mob);
        level.set_target(mob, last_hurt_by_mob);
        self.target_goal.target_mob = level.get_target(mob);
        self.timestamp = level.living(mob).last_hurt_by_mob_timestamp;
        self.target_goal.unseen_memory_ticks = 300;
        if self.alert_same_type {
            self.alert_others(level, mob);
        }
        self.target_goal.start();
    }

    fn alert_others(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let within = get_follow_distance(level, mob);
        let search_aabb = AABB::unit_cube_from_lower_corner(level.entity(mob).position()).inflate(within, ALERT_RANGE_Y, within);
        let nearby = level.get_entities_of_mob_class(mob, &search_aabb);
        let hurt_by_mob = level.get_last_hurt_by_mob(mob);
        for other in nearby {
            if mob == other || level.get_target(other).is_some() {
                continue;
            }
            if level.is_tamable_animal(mob) && level.get_owner(mob) != level.get_owner(other) {
                continue;
            }
            if hurt_by_mob.is_some_and(|hurt_by_mob| level.is_allied_to(other, hurt_by_mob)) {
                continue;
            }
            if let Some(to_ignore_alert) = self.to_ignore_alert
                && to_ignore_alert.contains(&registry::ENTITY_TYPE[level.entity(other).entity_type as usize])
            {
                continue;
            }
            if let Some(hurt_by_mob) = hurt_by_mob {
                self.alert_other(level, other, hurt_by_mob);
            } else {
                level.set_target(other, None);
            }
        }
    }

    fn alert_other(&self, level: &mut ServerLevel, other: EntityId, hurt_by_mob: EntityId) {
        level.set_target(other, Some(hurt_by_mob));
    }
}
