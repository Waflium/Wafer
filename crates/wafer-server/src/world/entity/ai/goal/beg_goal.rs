use wafer_core::random::RandomSource;
use wafer_data::items;

use super::goal::reduced_tick_delay;
use crate::server::level::ServerLevel;
use crate::world::InteractionHand;
use crate::world::entity::EntityId;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;

pub struct BegGoal {
    player: Option<EntityId>,
    look_distance: f32,
    look_time: i32,
}

impl BegGoal {
    pub fn new(look_distance: f32) -> BegGoal {
        BegGoal { player: None, look_distance, look_time: 0 }
    }

    fn beg_targeting(&self) -> TargetingConditions<'static> {
        TargetingConditions::for_non_combat().range(self.look_distance as f64)
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let e = level.entity(mob);
        let (x, y, z) = (e.x(), e.y(), e.z());
        self.player = level.get_nearest_player(&self.beg_targeting(), Some(mob), x, y, z);
        self.player.is_some_and(|player| self.player_holding_interesting(level, mob, player))
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let player = self.player.expect("a player");
        if !level.is_alive(player) {
            return false;
        }
        if level.entity(mob).distance_to_sqr(level.entity(player)) > (self.look_distance * self.look_distance) as f64 {
            return false;
        }
        self.look_time > 0 && self.player_holding_interesting(level, mob, player)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.set_is_interested(mob, true);
        self.look_time = reduced_tick_delay(40 + level.entity_mut(mob).random.next_int_bounded(40));
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.set_is_interested(mob, false);
        self.player = None;
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let player = self.player.expect("a player");
        let p = level.entity(player);
        let (x, y, z) = (p.x(), p.eye_y(), p.z());
        let wolf = level.mob_mut(mob).expect("a wolf");
        let max_head_x_rot = wolf.get_max_head_x_rot() as f32;
        wolf.ai.look_control.set_look_at(x, y, z, 10.0, max_head_x_rot);
        self.look_time -= 1;
    }

    fn player_holding_interesting(&self, level: &ServerLevel, mob: EntityId, player: EntityId) -> bool {
        [InteractionHand::MainHand, InteractionHand::OffHand].into_iter().any(|hand| {
            let item_stack = level.player(player).item_in_hand(hand).unwrap_or_else(crate::world::item::ItemStack::empty);
            item_stack.is(items::BONE) || level.is_food(mob, &item_stack)
        })
    }
}
