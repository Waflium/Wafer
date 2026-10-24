use wafer_core::shapes::VoxelShape;
use wafer_core::{AABB, Axis, BlockPos, ChunkPos, Direction, Vec3};
use wafer_data::properties::{CHEST_TYPE, ChestType};
use wafer_data::{BlockState, EquipmentSlot};

use super::behavior::{Behavior, BehaviorState};
use super::behavior_utils;
use super::block_pos_tracker::BlockPosTracker;
use super::position_tracker::PositionTracker;
use crate::core::global_pos::GlobalPos;
use crate::server::level::ServerLevel;
use crate::world::container::ContainerRef;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::item::ItemStack;
use crate::world::level::block::chest_block;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};
use crate::world::level::path_navigation_region::PathNavigationRegion;
use crate::world::phys::shapes::collision_context::CollisionContext;

pub const TARGET_INTERACTION_TIME: i32 = 60;
const VISITED_POSITIONS_MEMORY_TIME: i64 = 6000;
const TRANSPORTED_ITEM_MAX_STACK_SIZE: i32 = 16;
const MAX_VISITED_POSITIONS: usize = 10;
const MAX_UNREACHABLE_POSITIONS: usize = 50;
const PASSENGER_MOB_TARGET_SEARCH_DISTANCE: i32 = 1;
const IDLE_COOLDOWN: i32 = 140;
const CLOSE_ENOUGH_TO_START_QUEUING_DISTANCE: f64 = 3.0;
const CLOSE_ENOUGH_TO_START_INTERACTING_WITH_TARGET_DISTANCE: f64 = 0.5;
const CLOSE_ENOUGH_TO_START_INTERACTING_WITH_TARGET_PATH_END_DISTANCE: f64 = 1.0;
const CLOSE_ENOUGH_TO_CONTINUE_INTERACTING_WITH_TARGET: f64 = 2.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContainerInteractionState {
    PickupItem,
    PickupNoItem,
    PlaceItem,
    PlaceNoItem,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TransportItemState {
    Travelling,
    Queuing,
    Interacting,
}

pub type OnTargetReachedInteraction = Box<dyn Fn(&mut ServerLevel, EntityId, &TransportItemTarget, i32)>;

#[derive(Clone, Copy, Debug)]
pub struct TransportItemTarget {
    pub pos: BlockPos,
    pub container: ContainerRef,
    pub block_entity: u64,
    pub state: BlockState,
}

impl TransportItemTarget {
    pub fn try_create_possible_target(level: &mut ServerLevel, block_pos: BlockPos) -> Option<TransportItemTarget> {
        let block_entity = level.get_block_entity(block_pos)?;
        let (id, block_state) = (block_entity.id(), block_entity.block_state());
        let container = get_block_entity_container(level, block_state, block_pos)?;
        Some(TransportItemTarget { pos: block_pos, container, block_entity: id, state: block_state })
    }
}

fn get_block_entity_container(level: &mut ServerLevel, block_state: BlockState, block_pos: BlockPos) -> Option<ContainerRef> {
    if block_state.block().is_instance_of("ChestBlock") {
        chest_block::get_container(block_state, level, block_pos, false)
    } else {
        Some(ContainerRef::block(level, block_pos))
    }
}

fn bounds(shape: &VoxelShape) -> AABB {
    AABB::new(shape.min(Axis::X), shape.min(Axis::Y), shape.min(Axis::Z), shape.max(Axis::X), shape.max(Axis::Y), shape.max(Axis::Z))
}

pub struct TransportItemsBetweenContainers {
    behavior: BehaviorState,
    speed_modifier: f32,
    horizontal_search_distance: i32,
    vertical_search_distance: i32,
    source_block_type: fn(BlockState) -> bool,
    destination_block_type: fn(BlockState) -> bool,
    should_queue_for_target: fn(&mut ServerLevel, &TransportItemTarget) -> bool,
    on_start_travelling: fn(&mut ServerLevel, EntityId),
    on_target_interaction_actions: Vec<(ContainerInteractionState, OnTargetReachedInteraction)>,
    target: Option<TransportItemTarget>,
    state: TransportItemState,
    interaction_state: Option<ContainerInteractionState>,
    ticks_since_reaching_target: i32,
}

impl TransportItemsBetweenContainers {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        speed_modifier: f32,
        source_block_type: fn(BlockState) -> bool,
        destination_block_type: fn(BlockState) -> bool,
        horizontal_search_distance: i32,
        vertical_search_distance: i32,
        on_target_interaction_actions: Vec<(ContainerInteractionState, OnTargetReachedInteraction)>,
        on_start_travelling: fn(&mut ServerLevel, EntityId),
        should_queue_for_target: fn(&mut ServerLevel, &TransportItemTarget) -> bool,
    ) -> TransportItemsBetweenContainers {
        TransportItemsBetweenContainers {
            behavior: BehaviorState::new(vec![
                (memory_module_type::VISITED_BLOCK_POSITIONS.erase(), MemoryStatus::Registered),
                (memory_module_type::UNREACHABLE_TRANSPORT_BLOCK_POSITIONS.erase(), MemoryStatus::Registered),
                (memory_module_type::TRANSPORT_ITEMS_COOLDOWN_TICKS.erase(), MemoryStatus::ValueAbsent),
                (memory_module_type::IS_PANICKING.erase(), MemoryStatus::ValueAbsent),
            ]),
            speed_modifier,
            horizontal_search_distance,
            vertical_search_distance,
            source_block_type,
            destination_block_type,
            should_queue_for_target,
            on_start_travelling,
            on_target_interaction_actions,
            target: None,
            state: TransportItemState::Travelling,
            interaction_state: None,
            ticks_since_reaching_target: 0,
        }
    }

    fn update_invalid_target(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        if !self.has_valid_target(level, body) {
            self.stop_targeting_current_target(level, body);
            match self.get_transport_target(level, body) {
                Some(target) => {
                    self.target = Some(target);
                    self.on_start_travelling(level, body);
                    self.set_visited_block_pos(level, body, target.pos);
                }
                None => self.enter_cooldown_after_no_matching_target_found(level, body),
            }
            true
        } else {
            false
        }
    }

    fn on_queuing_for_target(&mut self, target: TransportItemTarget, level: &mut ServerLevel, body: EntityId) {
        if !self.is_another_mob_interacting_with_target(target, level) {
            self.resume_travelling(level, body);
        }
    }

    fn on_travel_to_target(&mut self, target: TransportItemTarget, level: &mut ServerLevel, body: EntityId) {
        let center_pos = self.get_center_pos(level, body);
        if self.is_within_target_distance(CLOSE_ENOUGH_TO_START_QUEUING_DISTANCE, target, level, body, center_pos) && self.is_another_mob_interacting_with_target(target, level) {
            self.start_queuing(level, body);
        } else if self.is_within_target_distance(get_interaction_range(level, body), target, level, body, center_pos) {
            self.start_on_reached_target_interaction(target, level, body);
        } else {
            self.walk_towards_target(level, body);
        }
    }

    fn get_center_pos(&self, level: &ServerLevel, body: EntityId) -> Vec3 {
        self.set_middle_y_position(level, body, level.entity(body).position())
    }

    fn on_reached_target(&mut self, target: TransportItemTarget, level: &mut ServerLevel, body: EntityId) {
        let center_pos = self.get_center_pos(level, body);
        if !self.is_within_target_distance(CLOSE_ENOUGH_TO_CONTINUE_INTERACTING_WITH_TARGET, target, level, body, center_pos) {
            self.on_start_travelling(level, body);
        } else {
            self.ticks_since_reaching_target += 1;
            self.on_target_interaction(target, level, body);
            if self.ticks_since_reaching_target >= TARGET_INTERACTION_TIME {
                if is_picking_up_items(level, body) {
                    if matches_getting_items_requirement(level, target.container) {
                        self.pick_up_items(level, body, target.container);
                    } else {
                        self.stop_targeting_current_target(level, body);
                    }
                } else if matches_leaving_items_requirement(level, body, target.container) {
                    self.put_down_item(level, body, target.container);
                } else {
                    self.stop_targeting_current_target(level, body);
                }
                self.on_start_travelling(level, body);
            }
        }
    }

    fn start_queuing(&mut self, level: &mut ServerLevel, body: EntityId) {
        stop_in_place(level, body);
        self.set_transporting_state(TransportItemState::Queuing);
    }

    fn resume_travelling(&mut self, level: &mut ServerLevel, body: EntityId) {
        self.set_transporting_state(TransportItemState::Travelling);
        self.walk_towards_target(level, body);
    }

    fn walk_towards_target(&self, level: &mut ServerLevel, body: EntityId) {
        if let Some(target) = self.target {
            behavior_utils::set_walk_and_look_target_memories_to_pos(level, body, target.pos, self.speed_modifier, 0);
        }
    }

    fn start_on_reached_target_interaction(&mut self, target: TransportItemTarget, level: &mut ServerLevel, body: EntityId) {
        let state = if is_picking_up_items(level, body) {
            if matches_getting_items_requirement(level, target.container) { ContainerInteractionState::PickupItem } else { ContainerInteractionState::PickupNoItem }
        } else if matches_leaving_items_requirement(level, body, target.container) {
            ContainerInteractionState::PlaceItem
        } else {
            ContainerInteractionState::PlaceNoItem
        };
        self.set_interaction_state(state);
        self.set_transporting_state(TransportItemState::Interacting);
    }

    fn on_start_travelling(&mut self, level: &mut ServerLevel, body: EntityId) {
        (self.on_start_travelling)(level, body);
        self.set_transporting_state(TransportItemState::Travelling);
        self.interaction_state = None;
        self.ticks_since_reaching_target = 0;
    }

    fn set_transporting_state(&mut self, state: TransportItemState) {
        self.state = state;
    }

    fn set_interaction_state(&mut self, state: ContainerInteractionState) {
        self.interaction_state = Some(state);
    }

    fn on_target_interaction(&self, target: TransportItemTarget, level: &mut ServerLevel, body: EntityId) {
        level.get_brain_mut(body).set_memory(memory_module_type::LOOK_TARGET, PositionTracker::BlockPos(BlockPosTracker::new(target.pos)));
        stop_in_place(level, body);
        if let Some(interaction_state) = self.interaction_state
            && let Some((_, action)) = self.on_target_interaction_actions.iter().find(|(state, _)| *state == interaction_state)
        {
            action(level, body, &target, self.ticks_since_reaching_target);
        }
    }

    fn get_transport_target(&self, level: &mut ServerLevel, body: EntityId) -> Option<TransportItemTarget> {
        let target_block_search_area = self.get_target_search_area(level, body);
        let body_position = level.entity(body).position();
        let center = level.entity(body).block_position().chunk();
        let radius = self.get_horizontal_search_distance(level, body).div_euclid(16) + 1;
        let mut target = None;
        let mut closest_distance = f32::MAX as f64;
        let mut candidates = Vec::new();
        for z in center.z - radius..=center.z + radius {
            for x in center.x - radius..=center.x + radius {
                let Some(level_chunk) = level.chunks.loaded_chunk(ChunkPos::new(x, z)) else { continue };
                candidates.clear();
                candidates.extend(
                    level_chunk.block_entities.map.keys_in_order().into_iter().filter(|pos| level_chunk.block_entities.map.get(pos).is_some_and(|entity| matches!(entity.kind, BlockEntityKind::Chest(_)))),
                );
                for &pos in &candidates {
                    let distance = pos.dist_to_center_sqr(body_position.x, body_position.y, body_position.z);
                    if distance < closest_distance
                        && let Some(target_valid_to_pick) = self.is_target_valid_to_pick(level, body, pos, &target_block_search_area)
                    {
                        target = Some(target_valid_to_pick);
                        closest_distance = distance;
                    }
                }
            }
        }
        target
    }

    fn is_target_valid_to_pick(&self, level: &mut ServerLevel, body: EntityId, block_pos: BlockPos, target_block_search_area: &AABB) -> Option<TransportItemTarget> {
        let is_within_search_area = target_block_search_area.contains(block_pos.x as f64, block_pos.y as f64, block_pos.z as f64);
        if !is_within_search_area {
            return None;
        }
        let transport_item_target = TransportItemTarget::try_create_possible_target(level, block_pos)?;
        let is_valid_target = self.is_wanted_block(level, body, transport_item_target.state)
            && !self.is_position_already_visited(level, body, transport_item_target)
            && !self.is_container_locked(level, transport_item_target);
        is_valid_target.then_some(transport_item_target)
    }

    fn is_container_locked(&self, level: &mut ServerLevel, transport_item_target: TransportItemTarget) -> bool {
        level.get_block_entity(transport_item_target.pos).and_then(|entity| entity.container_base()).is_some_and(|base| base.is_locked())
    }

    fn has_valid_target(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        let Some(target) = self.target else { return false };
        let target_is_of_valid_type = self.is_wanted_block(level, body, target.state) && self.target_has_not_changed(level, target);
        if target_is_of_valid_type && !self.is_target_blocked(level, target) {
            if self.state != TransportItemState::Travelling {
                return true;
            }
            if self.has_valid_travelling_path(level, target, body) {
                return true;
            }
            self.mark_visited_block_pos_as_unreachable(level, body, target.pos);
        }
        false
    }

    fn has_valid_travelling_path(&self, level: &mut ServerLevel, target: TransportItemTarget, body: EntityId) -> bool {
        let end_node = level.get_navigation(body, |navigation, level| match navigation.get_path() {
            Some(path) => Some(path.get_end_node().map(|node| node.as_block_pos())),
            None => navigation.create_path(level, body, target.pos, 0).map(|path| path.get_end_node().map(|node| node.as_block_pos())),
        });
        let pos_from_which_to_reach_target = self.get_position_to_reach_target_from(level, end_node.flatten(), body);
        let can_reach_target = self.is_within_target_distance(get_interaction_range(level, body), target, level, body, pos_from_which_to_reach_target);
        let has_not_yet_created_path_to_target = end_node.is_none() && !can_reach_target;
        has_not_yet_created_path_to_target || self.target_is_reachable_from_position(level, can_reach_target, pos_from_which_to_reach_target, target, body)
    }

    fn get_position_to_reach_target_from(&self, level: &ServerLevel, end_node: Option<BlockPos>, body: EntityId) -> Vec3 {
        let bottom_center = match end_node {
            None => level.entity(body).position(),
            Some(end_node) => Vec3::at_bottom_center_of(end_node),
        };
        self.set_middle_y_position(level, body, bottom_center)
    }

    fn set_middle_y_position(&self, level: &ServerLevel, body: EntityId, pos: Vec3) -> Vec3 {
        pos.add(0.0, level.entity(body).bounding_box().y_size() / 2.0, 0.0)
    }

    fn is_target_blocked(&self, level: &mut ServerLevel, target: TransportItemTarget) -> bool {
        chest_block::is_chest_blocked_at(level, target.pos)
    }

    fn target_has_not_changed(&self, level: &mut ServerLevel, target: TransportItemTarget) -> bool {
        level.get_block_entity(target.pos).is_some_and(|entity| entity.id() == target.block_entity)
    }

    fn get_connected_targets(&self, target: TransportItemTarget, level: &mut ServerLevel) -> ([TransportItemTarget; 2], usize) {
        if target.state.has(CHEST_TYPE) && target.state.get(CHEST_TYPE) != ChestType::Single {
            let connected_pos = target.pos.relative(chest_block::get_connected_direction(target.state));
            match TransportItemTarget::try_create_possible_target(level, connected_pos) {
                Some(connected_target) => ([target, connected_target], 2),
                None => ([target, target], 1),
            }
        } else {
            ([target, target], 1)
        }
    }

    fn get_target_search_area(&self, level: &ServerLevel, body: EntityId) -> AABB {
        let horizontal_search_distance = self.get_horizontal_search_distance(level, body) as f64;
        AABB::of_block(level.entity(body).block_position()).inflate(horizontal_search_distance, self.get_vertical_search_distance(level, body) as f64, horizontal_search_distance)
    }

    fn get_horizontal_search_distance(&self, level: &ServerLevel, body: EntityId) -> i32 {
        if level.is_passenger(body) { PASSENGER_MOB_TARGET_SEARCH_DISTANCE } else { self.horizontal_search_distance }
    }

    fn get_vertical_search_distance(&self, level: &ServerLevel, body: EntityId) -> i32 {
        if level.is_passenger(body) { PASSENGER_MOB_TARGET_SEARCH_DISTANCE } else { self.vertical_search_distance }
    }

    fn is_position_already_visited(&self, level: &mut ServerLevel, body: EntityId, target: TransportItemTarget) -> bool {
        let (targets, count) = self.get_connected_targets(target, level);
        let dimension = level.dimension;
        let brain = level.get_brain(body);
        let visited_positions = brain.get_memory(memory_module_type::VISITED_BLOCK_POSITIONS);
        let unreachable_positions = brain.get_memory(memory_module_type::UNREACHABLE_TRANSPORT_BLOCK_POSITIONS);
        let contains = |positions: Option<&Vec<GlobalPos>>, pos: BlockPos| positions.is_some_and(|positions| positions.iter().any(|p| p.pos == pos && p.dimension == dimension));
        targets[..count].iter().any(|t| contains(visited_positions, t.pos) || contains(unreachable_positions, t.pos))
    }

    fn set_visited_block_pos(&mut self, level: &mut ServerLevel, body: EntityId, target: BlockPos) {
        let mut visited_positions = get_visited_positions(level, body);
        let global_pos = GlobalPos::of(level.dimension, target);
        if !visited_positions.contains(&global_pos) {
            visited_positions.push(global_pos);
        }
        if visited_positions.len() > MAX_VISITED_POSITIONS {
            self.enter_cooldown_after_no_matching_target_found(level, body);
        } else {
            level.get_brain_mut(body).set_memory_with_expiry(memory_module_type::VISITED_BLOCK_POSITIONS, visited_positions, VISITED_POSITIONS_MEMORY_TIME);
        }
    }

    fn mark_visited_block_pos_as_unreachable(&mut self, level: &mut ServerLevel, body: EntityId, target: BlockPos) {
        let global_pos = GlobalPos::of(level.dimension, target);
        let mut visited_positions = get_visited_positions(level, body);
        visited_positions.retain(|pos| *pos != global_pos);
        let mut unreachable_positions = get_unreachable_positions(level, body);
        if !unreachable_positions.contains(&global_pos) {
            unreachable_positions.push(global_pos);
        }
        if unreachable_positions.len() > MAX_UNREACHABLE_POSITIONS {
            self.enter_cooldown_after_no_matching_target_found(level, body);
        } else {
            let brain = level.get_brain_mut(body);
            brain.set_memory_with_expiry(memory_module_type::VISITED_BLOCK_POSITIONS, visited_positions, VISITED_POSITIONS_MEMORY_TIME);
            brain.set_memory_with_expiry(memory_module_type::UNREACHABLE_TRANSPORT_BLOCK_POSITIONS, unreachable_positions, VISITED_POSITIONS_MEMORY_TIME);
        }
    }

    fn is_wanted_block(&self, level: &ServerLevel, body: EntityId, block: BlockState) -> bool {
        if is_picking_up_items(level, body) { (self.source_block_type)(block) } else { (self.destination_block_type)(block) }
    }

    fn is_within_target_distance(&self, distance: f64, target: TransportItemTarget, level: &ServerLevel, body: EntityId, from_pos: Vec3) -> bool {
        let bounding_box = level.entity(body).bounding_box();
        let moved_bound_box = AABB::of_size(from_pos, bounding_box.x_size(), bounding_box.y_size(), bounding_box.z_size());
        bounds(&PathNavigationRegion::new(level).get_state_collision_shape(target.state, target.pos)).inflate(distance, 0.5, distance).move_to_block(target.pos).intersects(&moved_bound_box)
    }

    fn target_is_reachable_from_position(&self, level: &mut ServerLevel, can_reach_target: bool, pos: Vec3, target: TransportItemTarget, body: EntityId) -> bool {
        can_reach_target && self.can_see_any_target_side(target, level, body, pos)
    }

    fn can_see_any_target_side(&self, target: TransportItemTarget, level: &mut ServerLevel, body: EntityId, eye_position: Vec3) -> bool {
        let center = Vec3::at_center_of(target.pos);
        let collision_context = CollisionContext::of(level, body);
        Direction::VALUES.into_iter().map(|direction| center.add(0.5 * direction.step_x() as f64, 0.5 * direction.step_y() as f64, 0.5 * direction.step_z() as f64)).any(|hit_target| {
            let result = level.clip(&ClipContext { from: eye_position, to: hit_target, block: ClipBlock::Collider, fluid: ClipFluid::None, collision_context });
            !result.miss && result.hit.block_pos == target.pos
        })
    }

    fn is_another_mob_interacting_with_target(&self, target: TransportItemTarget, level: &mut ServerLevel) -> bool {
        let (targets, count) = self.get_connected_targets(target, level);
        targets[..count].iter().any(|t| (self.should_queue_for_target)(level, t))
    }

    fn pick_up_items(&mut self, level: &mut ServerLevel, body: EntityId, container: ContainerRef) {
        let item = pickup_item_from_container(level, container);
        level.living_set_item_slot(body, EquipmentSlot::Mainhand, item);
        level.set_guaranteed_drop(body, EquipmentSlot::Mainhand);
        level.container_set_changed(container);
        self.clear_memories_after_matching_target_found(level, body);
    }

    fn put_down_item(&mut self, level: &mut ServerLevel, body: EntityId, container: ContainerRef) {
        let items_left_after_visiting_chest = add_items_to_container(level, body, container);
        level.container_set_changed(container);
        let left_is_empty = items_left_after_visiting_chest.is_empty();
        level.living_set_item_slot(body, EquipmentSlot::Mainhand, items_left_after_visiting_chest);
        if left_is_empty {
            self.clear_memories_after_matching_target_found(level, body);
        } else {
            self.stop_targeting_current_target(level, body);
        }
    }

    fn stop_targeting_current_target(&mut self, level: &mut ServerLevel, body: EntityId) {
        self.ticks_since_reaching_target = 0;
        self.target = None;
        level.get_navigation(body, |navigation, _| navigation.stop());
        level.get_brain_mut(body).erase_memory(memory_module_type::WALK_TARGET);
    }

    fn clear_memories_after_matching_target_found(&mut self, level: &mut ServerLevel, body: EntityId) {
        self.stop_targeting_current_target(level, body);
        let brain = level.get_brain_mut(body);
        brain.erase_memory(memory_module_type::VISITED_BLOCK_POSITIONS);
        brain.erase_memory(memory_module_type::UNREACHABLE_TRANSPORT_BLOCK_POSITIONS);
    }

    fn enter_cooldown_after_no_matching_target_found(&mut self, level: &mut ServerLevel, body: EntityId) {
        self.stop_targeting_current_target(level, body);
        let brain = level.get_brain_mut(body);
        brain.set_memory(memory_module_type::TRANSPORT_ITEMS_COOLDOWN_TICKS, IDLE_COOLDOWN);
        brain.erase_memory(memory_module_type::VISITED_BLOCK_POSITIONS);
        brain.erase_memory(memory_module_type::UNREACHABLE_TRANSPORT_BLOCK_POSITIONS);
    }
}

fn get_visited_positions(level: &ServerLevel, body: EntityId) -> Vec<GlobalPos> {
    level.get_brain(body).get_memory(memory_module_type::VISITED_BLOCK_POSITIONS).cloned().unwrap_or_default()
}

fn get_unreachable_positions(level: &ServerLevel, body: EntityId) -> Vec<GlobalPos> {
    level.get_brain(body).get_memory(memory_module_type::UNREACHABLE_TRANSPORT_BLOCK_POSITIONS).cloned().unwrap_or_default()
}

fn has_finished_path(level: &ServerLevel, body: EntityId) -> bool {
    level.mob(body).is_some_and(|mob| mob.ai.navigation.get_path().is_some_and(|path| path.is_done()))
}

fn get_interaction_range(level: &ServerLevel, body: EntityId) -> f64 {
    if has_finished_path(level, body) { CLOSE_ENOUGH_TO_START_INTERACTING_WITH_TARGET_PATH_END_DISTANCE } else { CLOSE_ENOUGH_TO_START_INTERACTING_WITH_TARGET_DISTANCE }
}

fn main_hand_item(level: &ServerLevel, body: EntityId) -> ItemStack {
    level.item_by_slot(body, EquipmentSlot::Mainhand).unwrap_or_else(ItemStack::empty)
}

fn is_picking_up_items(level: &ServerLevel, body: EntityId) -> bool {
    level.item_by_slot_ref(body, EquipmentSlot::Mainhand).is_none_or(ItemStack::is_empty)
}

fn matches_getting_items_requirement(level: &mut ServerLevel, container: ContainerRef) -> bool {
    !level.container_is_empty(container)
}

fn matches_leaving_items_requirement(level: &mut ServerLevel, body: EntityId, container: ContainerRef) -> bool {
    level.container_is_empty(container) || has_item_matching_hand_item(level, body, container)
}

fn has_item_matching_hand_item(level: &mut ServerLevel, body: EntityId, container: ContainerRef) -> bool {
    let main_hand_item = main_hand_item(level, body);
    (0..level.container_size(container)).any(|slot| ItemStack::is_same_item(&level.get_item(container, slot), &main_hand_item))
}

fn pickup_item_from_container(level: &mut ServerLevel, container: ContainerRef) -> ItemStack {
    for slot in 0..level.container_size(container) {
        let item_stack = level.get_item(container, slot);
        if !item_stack.is_empty() {
            let item_count = item_stack.count.min(TRANSPORTED_ITEM_MAX_STACK_SIZE);
            return level.remove_item(container, slot, item_count);
        }
    }
    ItemStack::empty()
}

fn add_items_to_container(level: &mut ServerLevel, body: EntityId, container: ContainerRef) -> ItemStack {
    let mut item_stack = main_hand_item(level, body);
    for slot in 0..level.container_size(container) {
        let mut container_item_stack = level.get_item(container, slot);
        if container_item_stack.is_empty() {
            level.set_item(container, slot, item_stack);
            return ItemStack::empty();
        }
        let max_stack_size = container_item_stack.max_stack_size();
        if ItemStack::is_same_item_same_components(&container_item_stack, &item_stack) && container_item_stack.count < max_stack_size {
            let count_that_can_be_added = max_stack_size - container_item_stack.count;
            let count_to_add = count_that_can_be_added.min(item_stack.count);
            container_item_stack.count += count_to_add;
            item_stack.count -= count_that_can_be_added;
            level.set_item(container, slot, container_item_stack);
            if item_stack.is_empty() {
                return ItemStack::empty();
            }
        }
    }
    item_stack
}

fn stop_in_place(level: &mut ServerLevel, body: EntityId) {
    level.get_navigation(body, |navigation, _| navigation.stop());
    let mob = level.mob_mut(body).expect("a mob");
    mob.living.xxa = 0.0;
    mob.living.yya = 0.0;
    mob.set_speed(0.0);
    let delta_movement = mob.entity.delta_movement;
    mob.entity.set_delta_movement(Vec3::new(0.0, delta_movement.y, 0.0));
}

impl Behavior for TransportItemsBetweenContainers {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        level.get_navigation(body, |navigation, _| navigation.can_path_to_targets_below_surface = true);
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        !level.is_leashed(body)
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        !level.get_brain(body).has_memory_value(memory_module_type::TRANSPORT_ITEMS_COOLDOWN_TICKS) && !level.is_panicking(body) && !level.is_leashed(body)
    }

    fn timed_out(&self, _timestamp: i64) -> bool {
        false
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        let updated_invalid_target = self.update_invalid_target(level, body);
        let Some(target) = self.target else {
            self.stop(level, body, timestamp);
            return;
        };
        if !updated_invalid_target {
            if self.state == TransportItemState::Queuing {
                self.on_queuing_for_target(target, level, body);
            }
            if self.state == TransportItemState::Travelling {
                self.on_travel_to_target(target, level, body);
            }
            if self.state == TransportItemState::Interacting {
                self.on_reached_target(target, level, body);
            }
        }
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        self.on_start_travelling(level, body);
        level.get_navigation(body, |navigation, _| navigation.can_path_to_targets_below_surface = false);
    }
}
