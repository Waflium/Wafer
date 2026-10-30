use wafer_core::BlockPos;
use wafer_data::{BlockState, block_tags};

use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::core::global_pos::GlobalPos;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::level::BlockGetter;
use crate::world::level::block::door_block;
use crate::world::level::pathfinder::node::Node;

const COOLDOWN_BEFORE_RERUNNING_IN_SAME_NODE: i32 = 20;
const SKIP_CLOSING_DOOR_IF_FURTHER_AWAY_THAN: f64 = 3.0;
const MAX_DISTANCE_TO_HOLD_DOOR_OPEN_FOR_OTHER_MOBS: f64 = 2.0;

fn is_mob_interactable_door(state: BlockState) -> bool {
    state.block().is(&block_tags::MOB_INTERACTABLE_DOORS) && state.block().is_instance_of("DoorBlock")
}

pub fn create() -> OneShot {
    let mut last_checked_node: Option<Node> = None;
    let mut remaining_cooldown = 0;
    behavior_builder::create(|i| {
        let path_memory = i.present(memory_module_type::PATH);
        let doors_memory = i.registered(memory_module_type::DOORS_TO_CLOSE);
        let nearest_entities = i.registered(memory_module_type::NEAREST_LIVING_ENTITIES);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let path = path_memory.get(level, body);
            let mut doors = doors_memory.try_get(level, body);
            if path.not_started() || path.is_done() {
                return false;
            }
            if last_checked_node == Some(path.get_next_node()) {
                remaining_cooldown = COOLDOWN_BEFORE_RERUNNING_IN_SAME_NODE;
            } else {
                remaining_cooldown -= 1;
                if remaining_cooldown > 0 {
                    return false;
                }
            }
            last_checked_node = Some(path.get_next_node());
            let from_node = path.get_previous_node();
            let to_node = path.get_next_node();
            let from_pos = from_node.expect("a previous node").as_block_pos();
            let from_state = level.block_state(from_pos);
            if is_mob_interactable_door(from_state) {
                if !door_block::is_open(from_state) {
                    door_block::set_open(Some(body), level, from_state, from_pos, true);
                }
                doors = Some(remember_door_to_close(level, body, doors, from_pos));
            }
            let to_pos = to_node.as_block_pos();
            let to_state = level.block_state(to_pos);
            if is_mob_interactable_door(to_state) && !door_block::is_open(to_state) {
                door_block::set_open(Some(body), level, to_state, to_pos, true);
                doors = Some(remember_door_to_close(level, body, doors, to_pos));
            }
            if let Some(mut door_set) = doors {
                let nearest = nearest_entities.try_get(level, body);
                close_doors_that_i_have_opened_or_passed_through(level, body, from_node.as_ref(), Some(&to_node), &mut door_set, nearest.as_deref());
                level.get_brain_mut(body).set_memory(memory_module_type::DOORS_TO_CLOSE, door_set);
            }
            true
        }
    })
}

pub fn close_doors_that_i_have_opened_or_passed_through(
    level: &mut ServerLevel,
    body: EntityId,
    moving_from_node: Option<&Node>,
    moving_to_node: Option<&Node>,
    doors: &mut Vec<GlobalPos>,
    nearest_entities: Option<&[EntityId]>,
) {
    let mut index = 0;
    while index < doors.len() {
        let door_global_pos = doors[index].clone();
        let door_pos = door_global_pos.pos;
        if moving_from_node.is_some_and(|node| node.as_block_pos() == door_pos) || moving_to_node.is_some_and(|node| node.as_block_pos() == door_pos) {
            index += 1;
            continue;
        }
        if !is_door_too_far_away(level, body, &door_global_pos) {
            let state = level.block_state(door_pos);
            if is_mob_interactable_door(state) && door_block::is_open(state) && !are_other_mobs_coming_through_door(level, body, door_pos, nearest_entities) {
                door_block::set_open(Some(body), level, state, door_pos, false);
            }
        }
        doors.remove(index);
    }
}

fn are_other_mobs_coming_through_door(level: &ServerLevel, body: EntityId, door_pos: BlockPos, nearest_entities: Option<&[EntityId]>) -> bool {
    let Some(nearest_entities) = nearest_entities else { return false };
    let body_type = level.entity(body).entity_type;
    nearest_entities
        .iter()
        .filter(|&&other_mob| level.entity_ref_or_tombstone(other_mob).is_some_and(|e| e.entity_type == body_type))
        .filter(|&&other_mob| door_pos.closer_to_center_than(level.entity(other_mob).position(), MAX_DISTANCE_TO_HOLD_DOOR_OPEN_FOR_OTHER_MOBS))
        .any(|&other_mob| {
            let brain = level.get_brain_if_present(other_mob).or_else(|| level.mob_or_tombstone(other_mob).map(|mob| &mob.living.brain));
            brain.is_some_and(|brain| is_mob_coming_through_door(brain, door_pos))
        })
}

fn is_mob_coming_through_door(other_brain: &crate::world::entity::ai::brain::Brain, door_pos: BlockPos) -> bool {
    if !other_brain.has_memory_value(memory_module_type::PATH) {
        return false;
    }
    let path = other_brain.get_memory(memory_module_type::PATH).expect("a path");
    if path.is_done() {
        return false;
    }
    let Some(moving_from_node) = path.get_previous_node() else { return false };
    let moving_to_node = path.get_next_node();
    door_pos == moving_from_node.as_block_pos() || door_pos == moving_to_node.as_block_pos()
}

fn is_door_too_far_away(level: &ServerLevel, body: EntityId, door_global_pos: &GlobalPos) -> bool {
    door_global_pos.dimension != level.dimension || !door_global_pos.pos.closer_to_center_than(level.entity(body).position(), SKIP_CLOSING_DOOR_IF_FURTHER_AWAY_THAN)
}

fn remember_door_to_close(level: &mut ServerLevel, body: EntityId, doors: Option<Vec<GlobalPos>>, door_pos: BlockPos) -> Vec<GlobalPos> {
    let global_door_pos = GlobalPos::of(level.dimension, door_pos);
    match doors {
        Some(mut set) => {
            if !set.contains(&global_door_pos) {
                set.push(global_door_pos);
            }
            set
        }
        None => {
            let set = vec![global_door_pos];
            level.get_brain_mut(body).set_memory(memory_module_type::DOORS_TO_CLOSE, set.clone());
            set
        }
    }
}
