use super::look_at_player_goal::LookAtPlayerGoal;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub struct LookAtTradingPlayerGoal {
    pub look_at_player_goal: LookAtPlayerGoal,
}

impl LookAtTradingPlayerGoal {
    pub fn new() -> LookAtTradingPlayerGoal {
        LookAtTradingPlayerGoal { look_at_player_goal: LookAtPlayerGoal::new(8.0) }
    }

    pub fn can_use(&mut self, level: &ServerLevel, mob: EntityId) -> bool {
        if level.is_trading(mob) {
            self.look_at_player_goal.look_at = level.get_trading_player(mob);
            true
        } else {
            false
        }
    }
}
