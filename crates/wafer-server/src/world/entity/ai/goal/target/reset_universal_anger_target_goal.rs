use wafer_core::AABB;
use wafer_data::game_rule;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::attributes::Attribute;

const ALERT_RANGE_Y: f64 = 10.0;

pub struct ResetUniversalAngerTargetGoal {
    alert_others_of_same_type: bool,
    last_hurt_by_player_timestamp: i32,
}

impl ResetUniversalAngerTargetGoal {
    pub fn new(alert_others_of_same_type: bool) -> ResetUniversalAngerTargetGoal {
        ResetUniversalAngerTargetGoal { alert_others_of_same_type, last_hurt_by_player_timestamp: 0 }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        level.game_rules.get_bool(game_rule::UNIVERSAL_ANGER) && self.was_hurt_by_player(level, mob)
    }

    fn was_hurt_by_player(&self, level: &ServerLevel, mob: EntityId) -> bool {
        level.get_last_hurt_by_mob(mob).is_some_and(|last_hurt_by_mob| level.is_player(last_hurt_by_mob))
            && level.living(mob).last_hurt_by_mob_timestamp > self.last_hurt_by_player_timestamp
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.last_hurt_by_player_timestamp = level.living(mob).last_hurt_by_mob_timestamp;
        level.forget_current_target_and_refresh_universal_anger(mob);
        if self.alert_others_of_same_type {
            for other_mob in self.get_nearby_mobs_of_same_type(level, mob) {
                if other_mob != mob {
                    level.forget_current_target_and_refresh_universal_anger(other_mob);
                }
            }
        }
    }

    fn get_nearby_mobs_of_same_type(&self, level: &ServerLevel, mob: EntityId) -> Vec<EntityId> {
        let within = level.attribute_value(mob, Attribute::FollowRange);
        let search_aabb = AABB::unit_cube_from_lower_corner(level.entity(mob).position()).inflate(within, ALERT_RANGE_Y, within);
        level.get_entities_of_mob_class(mob, &search_aabb).into_iter().filter(|&other| !level.is_spectator(other)).collect()
    }
}
