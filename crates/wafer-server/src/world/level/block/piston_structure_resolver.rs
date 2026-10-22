use wafer_core::{BlockPos, Direction};
use wafer_data::{BlockState, PushReaction, blocks};

use super::piston_base_block;
use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;

pub const MAX_PUSH_DEPTH: usize = 12;

pub struct PistonStructureResolver {
    piston_pos: BlockPos,
    extending: bool,
    start_pos: BlockPos,
    push_direction: Direction,
    to_push: Vec<BlockPos>,
    to_destroy: Vec<BlockPos>,
    piston_direction: Direction,
}

fn is_sticky(state: BlockState) -> bool {
    state.is_block(blocks::SLIME_BLOCK) || state.is_block(blocks::HONEY_BLOCK)
}

fn can_stick_to_each_other(state1: BlockState, state2: BlockState) -> bool {
    if state1.is_block(blocks::HONEY_BLOCK) && state2.is_block(blocks::SLIME_BLOCK) {
        false
    } else if state1.is_block(blocks::SLIME_BLOCK) && state2.is_block(blocks::HONEY_BLOCK) {
        false
    } else {
        is_sticky(state1) || is_sticky(state2)
    }
}

impl PistonStructureResolver {
    pub fn new(piston_pos: BlockPos, direction: Direction, extending: bool) -> PistonStructureResolver {
        let (push_direction, start_pos) = if extending { (direction, piston_pos.relative(direction)) } else { (direction.opposite(), piston_pos.relative_n(direction, 2)) };
        PistonStructureResolver { piston_pos, extending, start_pos, push_direction, to_push: Vec::new(), to_destroy: Vec::new(), piston_direction: direction }
    }

    pub fn resolve(&mut self, level: &mut ServerLevel) -> bool {
        self.to_push.clear();
        self.to_destroy.clear();
        let next_state = level.block_state(self.start_pos);
        if !piston_base_block::is_pushable(next_state, level, self.start_pos, self.push_direction, false, self.piston_direction) {
            if self.extending && next_state.push_reaction() == PushReaction::Popped {
                self.to_destroy.push(self.start_pos);
                return true;
            }
            return false;
        }
        if !self.add_block_line(level, self.start_pos, self.push_direction) {
            return false;
        }
        let mut i = 0;
        while i < self.to_push.len() {
            let pos = self.to_push[i];
            if is_sticky(level.block_state(pos)) && !self.add_branching_blocks(level, pos) {
                return false;
            }
            i += 1;
        }
        true
    }

    fn add_block_line(&mut self, level: &mut ServerLevel, start: BlockPos, direction: Direction) -> bool {
        let mut next_state = level.block_state(start);
        if next_state.is_air() {
            return true;
        }
        if !piston_base_block::is_pushable(next_state, level, start, self.push_direction, false, direction) {
            return true;
        }
        if start == self.piston_pos {
            return true;
        }
        if self.to_push.contains(&start) {
            return true;
        }
        let mut block_count = 1;
        if block_count + self.to_push.len() > MAX_PUSH_DEPTH {
            return false;
        }
        while is_sticky(next_state) {
            let pos = start.relative_n(self.push_direction.opposite(), block_count as i32);
            let previous_state = next_state;
            next_state = level.block_state(pos);
            if next_state.is_air()
                || !can_stick_to_each_other(previous_state, next_state)
                || !piston_base_block::is_pushable(next_state, level, pos, self.push_direction, false, self.push_direction.opposite())
                || pos == self.piston_pos
            {
                break;
            }
            block_count += 1;
            if block_count + self.to_push.len() > MAX_PUSH_DEPTH {
                return false;
            }
        }
        let mut blocks_added = 0;
        for i in (0..block_count).rev() {
            self.to_push.push(start.relative_n(self.push_direction.opposite(), i as i32));
            blocks_added += 1;
        }
        let mut i = 1;
        loop {
            let pos = start.relative_n(self.push_direction, i);
            if let Some(collision_pos) = self.to_push.iter().position(|&p| p == pos) {
                self.reorder_list_at_collision(blocks_added, collision_pos);
                for j in 0..=collision_pos + blocks_added {
                    let block_pos = self.to_push[j];
                    if is_sticky(level.block_state(block_pos)) && !self.add_branching_blocks(level, block_pos) {
                        return false;
                    }
                }
                return true;
            }
            next_state = level.block_state(pos);
            if next_state.is_air() {
                return true;
            }
            if !piston_base_block::is_pushable(next_state, level, pos, self.push_direction, true, self.push_direction) || pos == self.piston_pos {
                return false;
            }
            if next_state.push_reaction() == PushReaction::Popped {
                self.to_destroy.push(pos);
                return true;
            }
            if self.to_push.len() >= MAX_PUSH_DEPTH {
                return false;
            }
            self.to_push.push(pos);
            blocks_added += 1;
            i += 1;
        }
    }

    fn reorder_list_at_collision(&mut self, blocks_added: usize, collision_pos: usize) {
        let len = self.to_push.len();
        let head = self.to_push[0..collision_pos].to_vec();
        let last_line_added = self.to_push[len - blocks_added..len].to_vec();
        let collision_to_line = self.to_push[collision_pos..len - blocks_added].to_vec();
        self.to_push.clear();
        self.to_push.extend(head);
        self.to_push.extend(last_line_added);
        self.to_push.extend(collision_to_line);
    }

    fn add_branching_blocks(&mut self, level: &mut ServerLevel, from_pos: BlockPos) -> bool {
        let from_state = level.block_state(from_pos);
        for direction in Direction::VALUES {
            if direction.axis() != self.push_direction.axis() {
                let neighbour_pos = from_pos.relative(direction);
                let neighbour_state = level.block_state(neighbour_pos);
                if can_stick_to_each_other(neighbour_state, from_state) && !self.add_block_line(level, neighbour_pos, direction) {
                    return false;
                }
            }
        }
        true
    }

    pub fn get_to_push(&self) -> &[BlockPos] {
        &self.to_push
    }

    pub fn get_to_destroy(&self) -> &[BlockPos] {
        &self.to_destroy
    }
}
