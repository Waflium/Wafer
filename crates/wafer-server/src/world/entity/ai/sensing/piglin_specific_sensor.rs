use wafer_core::BlockPos;
use wafer_data::{BlockState, block_tags, blocks};

use crate::server::level::ServerLevel;
use crate::world::entity::ai::memory::memory_module_type::{self, Erased, MemoryModuleType};
use crate::world::entity::ai::memory::nearest_visible_living_entities::NearestVisibleLivingEntities;
use crate::world::entity::monster::piglin::piglin_ai;
use crate::world::entity::{EntityId, entity_types};
use crate::world::level::BlockGetter;
use crate::world::level::block::campfire_block;

pub fn requires() -> Vec<MemoryModuleType<Erased>> {
    vec![
        memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES.erase(),
        memory_module_type::NEAREST_LIVING_ENTITIES.erase(),
        memory_module_type::NEAREST_VISIBLE_NEMESIS.erase(),
        memory_module_type::NEAREST_TARGETABLE_PLAYER_NOT_WEARING_GOLD.erase(),
        memory_module_type::NEAREST_PLAYER_HOLDING_WANTED_ITEM.erase(),
        memory_module_type::NEAREST_VISIBLE_HUNTABLE_HOGLIN.erase(),
        memory_module_type::NEAREST_VISIBLE_BABY_HOGLIN.erase(),
        memory_module_type::NEAREST_VISIBLE_ADULT_PIGLINS.erase(),
        memory_module_type::NEARBY_ADULT_PIGLINS.erase(),
        memory_module_type::VISIBLE_ADULT_PIGLIN_COUNT.erase(),
        memory_module_type::VISIBLE_ADULT_HOGLIN_COUNT.erase(),
        memory_module_type::NEAREST_REPELLENT.erase(),
    ]
}

pub fn do_tick(level: &mut ServerLevel, body: EntityId) {
    let nearest_repellent = find_nearest_repellent(level, body);
    level.get_brain_mut(body).set_memory_optional(memory_module_type::NEAREST_REPELLENT, nearest_repellent);
    let mut nemesis = None;
    let mut huntable_hoglin = None;
    let mut baby_hoglin = None;
    let mut baby_piglin = None;
    let mut zombified = None;
    let mut player_not_wearing_gold = None;
    let mut player_holding_wanted_item = None;
    let mut visible_adult_hoglin_count = 0;
    let mut visible_adult_piglins = Vec::new();
    let visible_living_entities = level.get_brain(body).get_memory(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES).cloned().unwrap_or_else(NearestVisibleLivingEntities::empty);
    for entity in visible_living_entities.find_all(level, |_, _| true) {
        let entity_type = level.entity(entity).entity_type;
        if entity_type == entity_types::HOGLIN.id {
            if level.is_baby(entity) && baby_hoglin.is_none() {
                baby_hoglin = Some(entity);
            } else if !level.is_baby(entity) {
                visible_adult_hoglin_count += 1;
                if huntable_hoglin.is_none() && level.hoglin_can_be_hunted(entity) {
                    huntable_hoglin = Some(entity);
                }
            }
        } else if entity_type == entity_types::PIGLIN_BRUTE.id {
            visible_adult_piglins.push(entity);
        } else if entity_type == entity_types::PIGLIN.id {
            if level.is_baby(entity) && baby_piglin.is_none() {
                baby_piglin = Some(entity);
            } else if !level.is_baby(entity) {
                visible_adult_piglins.push(entity);
            }
        } else if level.is_player(entity) {
            if player_not_wearing_gold.is_none() && !piglin_ai::is_wearing_safe_armor(level, entity) && level.can_attack(body, entity) {
                player_not_wearing_gold = Some(entity);
            }
            if player_holding_wanted_item.is_none() && !level.is_spectator(entity) && piglin_ai::is_player_holding_loved_item(level, entity) {
                player_holding_wanted_item = Some(entity);
            }
        } else if nemesis.is_some() || entity_type != entity_types::WITHER_SKELETON.id && entity_type != entity_types::WITHER.id {
            if zombified.is_none() && piglin_ai::is_zombified(level, entity) {
                zombified = Some(entity);
            }
        } else {
            nemesis = Some(entity);
        }
    }
    let adult_piglins = piglin_ai::find_nearby_adult_piglins(level, body);
    let brain = level.get_brain_mut(body);
    brain.set_memory_optional(memory_module_type::NEAREST_VISIBLE_NEMESIS, nemesis);
    brain.set_memory_optional(memory_module_type::NEAREST_VISIBLE_HUNTABLE_HOGLIN, huntable_hoglin);
    brain.set_memory_optional(memory_module_type::NEAREST_VISIBLE_BABY_HOGLIN, baby_hoglin);
    brain.set_memory_optional(memory_module_type::NEAREST_VISIBLE_ZOMBIFIED, zombified);
    brain.set_memory_optional(memory_module_type::NEAREST_TARGETABLE_PLAYER_NOT_WEARING_GOLD, player_not_wearing_gold);
    brain.set_memory_optional(memory_module_type::NEAREST_PLAYER_HOLDING_WANTED_ITEM, player_holding_wanted_item);
    brain.set_memory(memory_module_type::NEARBY_ADULT_PIGLINS, adult_piglins);
    let visible_adult_piglin_count = visible_adult_piglins.len() as i32;
    brain.set_memory(memory_module_type::NEAREST_VISIBLE_ADULT_PIGLINS, visible_adult_piglins);
    brain.set_memory(memory_module_type::VISIBLE_ADULT_PIGLIN_COUNT, visible_adult_piglin_count);
    brain.set_memory(memory_module_type::VISIBLE_ADULT_HOGLIN_COUNT, visible_adult_hoglin_count);
}

fn find_nearest_repellent(level: &mut ServerLevel, body: EntityId) -> Option<BlockPos> {
    let origin = level.entity(body).block_position();
    BlockPos::within_box_by_manhattan_distance(origin, 8, 4, 8).find(|&pos| is_valid_repellent(level.block_state(pos)))
}

fn is_valid_repellent(state: BlockState) -> bool {
    let is_repellent = state.is(&block_tags::PIGLIN_REPELLENTS);
    if is_repellent && state.is_block(blocks::SOUL_CAMPFIRE) { campfire_block::is_lit_campfire(state) } else { is_repellent }
}
