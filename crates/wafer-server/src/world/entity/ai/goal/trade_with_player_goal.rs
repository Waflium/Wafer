use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub struct TradeWithPlayerGoal;

impl TradeWithPlayerGoal {
    pub fn can_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        if !level.is_alive(mob) {
            return false;
        }
        let e = level.entity(mob);
        if e.is_in_water() {
            return false;
        }
        if !e.on_ground {
            return false;
        }
        if level.living(mob).hurt_time > 0 {
            return false;
        }
        let Some(trader) = level.get_trading_player(mob) else { return false };
        !(level.entity(mob).distance_to_sqr(level.entity(trader)) > 16.0)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.mob_mut(mob).expect("an abstract villager").ai.navigation.stop();
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.set_trading_player(mob, None);
    }
}
