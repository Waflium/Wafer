use super::target_goal::TargetGoal;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub struct DefendVillageTargetGoal {
    pub target_goal: TargetGoal,
    potential_target: Option<EntityId>,
}

impl DefendVillageTargetGoal {
    pub fn new() -> DefendVillageTargetGoal {
        DefendVillageTargetGoal { target_goal: TargetGoal::new(false, true), potential_target: None }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, golem: EntityId) -> bool {
        let grow = level.entity(golem).bounding_box().inflate(10.0, 8.0, 10.0);
        let attack_targeting = TargetingConditions::for_combat().range(64.0);
        let villagers: Vec<EntityId> = level
            .get_living_entities(&grow, |level, entity| level.is_villager(entity))
            .into_iter()
            .filter(|&villager| attack_targeting.test(level, Some(golem), villager))
            .collect();
        let players: Vec<EntityId> = level
            .level_players()
            .into_iter()
            .filter(|&player| {
                let e = level.entity(player);
                grow.contains(e.x(), e.y(), e.z()) && attack_targeting.test(level, Some(golem), player)
            })
            .collect();
        for &villager in &villagers {
            for &player in &players {
                let reputation = level.get_player_reputation(villager, player);
                if reputation <= -100 {
                    self.potential_target = Some(player);
                }
            }
        }
        let Some(potential_target) = self.potential_target else { return false };
        !(level.is_player(potential_target) && (level.is_spectator(potential_target) || level.player(potential_target).game_mode.game_type.is_creative()))
    }

    pub fn start(&mut self, level: &mut ServerLevel, golem: EntityId) {
        level.set_target(golem, self.potential_target);
        self.target_goal.start();
    }
}
