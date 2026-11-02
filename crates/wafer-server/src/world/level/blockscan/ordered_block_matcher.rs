use wafer_core::BlockPos;
use wafer_data::BlockState;

use super::block_matcher::{BlockMatcher, NO_PREDICATE};
use super::block_scan_utils;
use crate::util::continuation::Continuation;
use crate::world::level::LevelReader;

pub struct OrderedBlockMatcher<'l, L: ?Sized, I, P> {
    level: &'l mut L,
    positions: I,
    state_predicate: P,
}

impl<'l, L: LevelReader + ?Sized, I: Iterator<Item = BlockPos>> OrderedBlockMatcher<'l, L, I, fn(BlockState) -> bool> {
    pub fn new(level: &'l mut L, positions: I) -> OrderedBlockMatcher<'l, L, I, fn(BlockState) -> bool> {
        OrderedBlockMatcher { level, positions, state_predicate: NO_PREDICATE }
    }
}

impl<'l, L: LevelReader + ?Sized, I: Iterator<Item = BlockPos>, P: Fn(BlockState) -> bool> OrderedBlockMatcher<'l, L, I, P> {
    pub fn filter_pos(self, predicate: impl FnMut(&BlockPos) -> bool) -> OrderedBlockMatcher<'l, L, impl Iterator<Item = BlockPos>, P> {
        OrderedBlockMatcher { level: self.level, positions: self.positions.filter(predicate), state_predicate: self.state_predicate }
    }

    pub fn filter_state(self, predicate: impl Fn(BlockState) -> bool) -> OrderedBlockMatcher<'l, L, I, impl Fn(BlockState) -> bool> {
        let state_predicate = self.state_predicate;
        OrderedBlockMatcher { level: self.level, positions: self.positions, state_predicate: move |state| state_predicate(state) && predicate(state) }
    }

    fn any_matched_with(level: &mut L, positions: I, state_predicate: &impl Fn(BlockState) -> bool) -> bool {
        block_scan_utils::find_blocks_with_cache(level, positions, state_predicate, &mut |_: &mut L, _, _| Continuation::Abort)
    }

    pub fn find_first(self) -> Option<BlockPos> {
        let mut result = None;
        block_scan_utils::find_blocks_with_cache(self.level, self.positions, &self.state_predicate, &mut |_: &mut L, pos, _| {
            result = Some(pos);
            Continuation::Abort
        });
        result
    }

    pub fn find_first_matching(self, mut predicate: impl FnMut(&mut L, BlockPos, BlockState) -> bool) -> Option<BlockPos> {
        let mut result = None;
        block_scan_utils::find_blocks_with_cache(self.level, self.positions, &self.state_predicate, &mut |level: &mut L, pos, state| {
            if predicate(level, pos, state) {
                result = Some(pos);
                Continuation::Abort
            } else {
                Continuation::Continue
            }
        });
        result
    }
}

impl<L: LevelReader + ?Sized, I: Iterator<Item = BlockPos>, P: Fn(BlockState) -> bool> BlockMatcher<L> for OrderedBlockMatcher<'_, L, I, P> {
    fn at_least_matched(self, n: i32) -> bool {
        let mut count = 0;
        block_scan_utils::find_blocks_with_cache(self.level, self.positions, &self.state_predicate, &mut |_: &mut L, _, _| {
            count += 1;
            Continuation::abort_if(count >= n)
        })
    }

    fn any_matched(self) -> bool {
        Self::any_matched_with(self.level, self.positions, &self.state_predicate)
    }

    fn all_matched(self) -> bool {
        let state_predicate = &self.state_predicate;
        !Self::any_matched_with(self.level, self.positions, &|state| !state_predicate(state))
    }

    fn for_each(self, mut consumer: impl FnMut(&mut L, BlockPos, BlockState)) {
        block_scan_utils::find_blocks_with_cache(self.level, self.positions, &self.state_predicate, &mut |level: &mut L, pos, state| {
            consumer(level, pos, state);
            Continuation::Continue
        });
    }

    fn for_each_until(self, mut consumer: impl FnMut(&mut L, BlockPos, BlockState) -> Continuation) -> bool {
        block_scan_utils::find_blocks_with_cache(self.level, self.positions, &self.state_predicate, &mut consumer)
    }
}
