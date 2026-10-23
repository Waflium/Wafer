use std::collections::VecDeque;

use indexmap::IndexMap;
use wafer_core::{BlockPos, Direction};
use wafer_data::properties::POWER;
use wafer_data::{Block, BlockState};

use super::orientation::{Orientation, SideBias};
use super::redstone_wire_evaluator::{get_block_signal, get_incoming_wire_signal, get_wire_signal};
use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;
use crate::world::level::block::redstone_wire_block;
use crate::world::level::update_flags::{UPDATE_CLIENTS, UPDATE_SKIP_SHAPE_UPDATE_ON_WIRE};

pub struct ExperimentalRedstoneWireEvaluator {
    wire_block: Block,
    wires_to_turn_off: VecDeque<BlockPos>,
    wires_to_turn_on: VecDeque<BlockPos>,
    updated_wires: IndexMap<BlockPos, i32>,
}

impl ExperimentalRedstoneWireEvaluator {
    pub fn new(wire_block: Block) -> ExperimentalRedstoneWireEvaluator {
        ExperimentalRedstoneWireEvaluator { wire_block, wires_to_turn_off: VecDeque::new(), wires_to_turn_on: VecDeque::new(), updated_wires: IndexMap::new() }
    }

    pub fn update_power_strength(&mut self, level: &mut ServerLevel, initial_pos: BlockPos, orientation: Option<Orientation>, shape_update_wires_around_initial_position: bool) {
        let initial_orientation = get_initial_orientation(level, orientation);
        self.calculate_current_changes(level, initial_pos, initial_orientation);
        let mut removed = Vec::new();
        for (index, (&pos, &packed)) in self.updated_wires.iter().enumerate() {
            let new_level = unpack_power(packed);
            let state = level.block_state(pos);
            if state.is_block(self.wire_block) && state.get(POWER) != new_level {
                let mut update_flags = UPDATE_CLIENTS;
                if !shape_update_wires_around_initial_position || index != 0 {
                    update_flags |= UPDATE_SKIP_SHAPE_UPDATE_ON_WIRE;
                }
                level.set_block(pos, state.set(POWER, new_level), update_flags);
            } else {
                removed.push(pos);
            }
        }
        for pos in removed {
            self.updated_wires.shift_remove(&pos);
        }
        self.cause_neighbor_updates(level);
    }

    fn cause_neighbor_updates(&self, level: &mut ServerLevel) {
        for (&wire_pos, &packed) in &self.updated_wires {
            let orientation = unpack_orientation(packed);
            let state = level.block_state(wire_pos);
            for &neighbor_direction in orientation.get_directions() {
                if is_connected(state, neighbor_direction) {
                    let neighbor_pos = wire_pos.relative(neighbor_direction);
                    let neighbor_state = level.block_state(neighbor_pos);
                    let neighbor_orientation = orientation.with_front_preserve_up(neighbor_direction);
                    level.neighbor_changed_with_state(neighbor_state, neighbor_pos, self.wire_block, Some(neighbor_orientation), false);
                    if neighbor_state.is_redstone_conductor() {
                        for &direction in neighbor_orientation.get_directions() {
                            if direction != neighbor_direction.opposite() {
                                level.neighbor_changed(neighbor_pos.relative(direction), self.wire_block, Some(neighbor_orientation.with_front_preserve_up(direction)));
                            }
                        }
                    }
                }
            }
        }
    }

    fn calculate_current_changes(&mut self, level: &mut ServerLevel, initial_position: BlockPos, initial_orientation: Orientation) {
        let initial_state = level.block_state(initial_position);
        if initial_state.is_block(self.wire_block) {
            self.set_power(initial_position, initial_state.get(POWER), initial_orientation);
            self.wires_to_turn_off.push_back(initial_position);
        } else {
            self.propagate_change_to_neighbors(level, initial_position, 0, initial_orientation, true);
        }
        while let Some(pos) = self.wires_to_turn_off.pop_front() {
            let packed = self.updated_wires.get(&pos).copied().unwrap_or(0);
            let orientation = unpack_orientation(packed);
            let old_power = unpack_power(packed);
            let block_power = get_block_signal(level, pos);
            let wire_power = self.get_incoming_wire_signal(level, pos);
            let new_power = block_power.max(wire_power);
            let power_to_set = if new_power < old_power {
                if block_power > 0 && !self.wires_to_turn_on.contains(&pos) {
                    self.wires_to_turn_on.push_back(pos);
                }
                0
            } else {
                new_power
            };
            if power_to_set != old_power {
                self.set_power(pos, power_to_set, orientation);
            }
            self.propagate_change_to_neighbors(level, pos, power_to_set, orientation, old_power > new_power);
        }
        while let Some(pos) = self.wires_to_turn_on.pop_front() {
            let packed = self.updated_wires.get(&pos).copied().unwrap_or(0);
            let old_power = unpack_power(packed);
            let block_power = get_block_signal(level, pos);
            let wire_power = self.get_incoming_wire_signal(level, pos);
            let new_power = block_power.max(wire_power);
            let orientation = unpack_orientation(packed);
            if new_power > old_power {
                self.set_power(pos, new_power, orientation);
            } else if new_power < old_power {
                panic!("Turning off wire while trying to turn it on. Should not happen.");
            }
            self.propagate_change_to_neighbors(level, pos, new_power, orientation, false);
        }
    }

    fn set_power(&mut self, pos: BlockPos, new_power: i32, orientation: Orientation) {
        let packed = self.updated_wires.entry(pos).or_insert(pack_orientation_and_power(orientation, new_power));
        *packed = pack_orientation_and_power(unpack_orientation(*packed), new_power);
    }

    fn propagate_change_to_neighbors(&mut self, level: &mut ServerLevel, pos: BlockPos, new_power: i32, orientation: Orientation, allow_turning_off: bool) {
        for &direction_horizontal in orientation.get_horizontal_directions() {
            let offset_pos = pos.relative(direction_horizontal);
            self.enqueue_neighbor_wire(level, offset_pos, new_power, orientation.with_front(direction_horizontal), allow_turning_off);
        }
        for &direction_vertical in orientation.get_vertical_directions() {
            let offset_pos = pos.relative(direction_vertical);
            let solid_block = level.block_state(offset_pos).is_redstone_conductor();
            for &direction_horizontal in orientation.get_horizontal_directions() {
                let neighbor = pos.relative(direction_horizontal);
                if direction_vertical == Direction::Up && !solid_block {
                    let neighbor_wire = offset_pos.relative(direction_horizontal);
                    self.enqueue_neighbor_wire(level, neighbor_wire, new_power, orientation.with_front(direction_horizontal), allow_turning_off);
                } else if direction_vertical == Direction::Down && !level.block_state(neighbor).is_redstone_conductor() {
                    let neighbor_wire = offset_pos.relative(direction_horizontal);
                    self.enqueue_neighbor_wire(level, neighbor_wire, new_power, orientation.with_front(direction_horizontal), allow_turning_off);
                }
            }
        }
    }

    fn enqueue_neighbor_wire(&mut self, level: &mut ServerLevel, pos: BlockPos, new_from_power: i32, orientation: Orientation, allow_turning_off: bool) {
        let state = level.block_state(pos);
        if state.is_block(self.wire_block) {
            let to_power = self.get_wire_signal(pos, state);
            if to_power < new_from_power - 1 && !self.wires_to_turn_on.contains(&pos) {
                self.wires_to_turn_on.push_back(pos);
                self.set_power(pos, to_power, orientation);
            }
            if allow_turning_off && to_power > new_from_power && !self.wires_to_turn_off.contains(&pos) {
                self.wires_to_turn_off.push_back(pos);
                self.set_power(pos, to_power, orientation);
            }
        }
    }

    fn get_wire_signal(&self, pos: BlockPos, state: BlockState) -> i32 {
        match self.updated_wires.get(&pos) {
            Some(&packed) => unpack_power(packed),
            None => get_wire_signal(self.wire_block, state),
        }
    }

    fn get_incoming_wire_signal(&self, level: &mut ServerLevel, pos: BlockPos) -> i32 {
        get_incoming_wire_signal(level, pos, &|pos, state| self.get_wire_signal(pos, state))
    }
}

fn is_connected(state: BlockState, direction: Direction) -> bool {
    if direction.axis().is_vertical() { direction == Direction::Down } else { state.get(redstone_wire_block::property(direction)) != wafer_data::properties::RedstoneSide::None }
}

fn get_initial_orientation(level: &mut ServerLevel, incoming_origination: Option<Orientation>) -> Orientation {
    let orientation = incoming_origination.unwrap_or_else(|| Orientation::random(&mut level.random));
    orientation.with_up(Direction::Up).with_side_bias(SideBias::Left)
}

fn pack_orientation_and_power(orientation: Orientation, power: i32) -> i32 {
    orientation.get_index() << 4 | power
}

fn unpack_orientation(packed: i32) -> Orientation {
    Orientation::from_index(packed >> 4)
}

fn unpack_power(packed: i32) -> i32 {
    packed & 15
}
