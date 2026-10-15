use wafer_data::EquipmentSlot;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, Erased, MemoryModuleType};
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::attributes::Attribute;
use crate::world::item::ItemStack;

pub fn for_animal(level: &ServerLevel, mob: EntityId, item_stack: &ItemStack) -> bool {
    level.mob(mob).and_then(|mob| mob.animal()).is_some() && level.is_food(mob, item_stack)
}

pub fn requires() -> Vec<MemoryModuleType<Erased>> {
    vec![memory_module_type::TEMPTING_PLAYER.erase()]
}

pub fn do_tick(level: &mut ServerLevel, body: EntityId, temptations: fn(&ServerLevel, EntityId, &ItemStack) -> bool) {
    let targeting = TargetingConditions::for_non_combat().ignore_line_of_sight().range(level.attribute_value(body, Attribute::TemptRange) as f32 as f64);
    let position = level.entity(body).position();
    let mut players: Vec<EntityId> = Vec::new();
    for player in level.level_players() {
        if !level.is_spectator(player)
            && targeting.test(level, Some(body), player)
            && player_holding_temptation(level, body, player, temptations)
            && !level.has_passenger(body, player)
        {
            players.push(player);
        }
    }
    players.sort_by(|&a, &b| position.distance_to_sqr(level.entity(a).position()).total_cmp(&position.distance_to_sqr(level.entity(b).position())));
    let brain = level.get_brain_mut(body);
    match players.first() {
        Some(&player) => brain.set_memory(memory_module_type::TEMPTING_PLAYER, player),
        None => brain.erase_memory(memory_module_type::TEMPTING_PLAYER),
    }
}

fn player_holding_temptation(level: &ServerLevel, mob: EntityId, player: EntityId, temptations: fn(&ServerLevel, EntityId, &ItemStack) -> bool) -> bool {
    let main_hand = level.item_by_slot(player, EquipmentSlot::Mainhand).unwrap_or_else(ItemStack::empty);
    let offhand = level.item_by_slot(player, EquipmentSlot::Offhand).unwrap_or_else(ItemStack::empty);
    temptations(level, mob, &main_hand) || temptations(level, mob, &offhand)
}
