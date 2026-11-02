use wafer_core::BlockPos;
use wafer_data::BlockState;

use super::block_matcher::{BlockMatcher, NO_PREDICATE};
use super::block_scan_utils;
use crate::util::continuation::Continuation;
use crate::world::level::LevelReader;

pub struct BoxBlockMatcher<'l, L: ?Sized, P> {
    level: &'l mut L,
    from: BlockPos,
    to: BlockPos,
    state_predicate: P,
}

impl<'l, L: LevelReader + ?Sized> BoxBlockMatcher<'l, L, fn(BlockState) -> bool> {
    pub fn new(level: &'l mut L, from: BlockPos, to: BlockPos) -> BoxBlockMatcher<'l, L, fn(BlockState) -> bool> {
        BoxBlockMatcher { level, from, to, state_predicate: NO_PREDICATE }
    }
}

impl<'l, L: LevelReader + ?Sized, P: Fn(BlockState) -> bool> BoxBlockMatcher<'l, L, P> {
    pub fn filter_state(self, predicate: impl Fn(BlockState) -> bool) -> BoxBlockMatcher<'l, L, impl Fn(BlockState) -> bool> {
        let state_predicate = self.state_predicate;
        BoxBlockMatcher { level: self.level, from: self.from, to: self.to, state_predicate: move |state| state_predicate(state) && predicate(state) }
    }

    fn any_matched_with(level: &mut L, from: BlockPos, to: BlockPos, state_predicate: &impl Fn(BlockState) -> bool) -> bool {
        block_scan_utils::find_blocks(level, from, to, state_predicate, &mut |_: &mut L, _, _| Continuation::Abort)
    }
}

impl<L: LevelReader + ?Sized, P: Fn(BlockState) -> bool> BlockMatcher<L> for BoxBlockMatcher<'_, L, P> {
    fn at_least_matched(self, n: i32) -> bool {
        let mut count = 0;
        block_scan_utils::find_blocks(self.level, self.from, self.to, &self.state_predicate, &mut |_: &mut L, _, _| {
            count += 1;
            Continuation::abort_if(count >= n)
        })
    }

    fn any_matched(self) -> bool {
        Self::any_matched_with(self.level, self.from, self.to, &self.state_predicate)
    }

    fn all_matched(self) -> bool {
        let state_predicate = &self.state_predicate;
        !Self::any_matched_with(self.level, self.from, self.to, &|state| !state_predicate(state))
    }

    fn for_each(self, mut consumer: impl FnMut(&mut L, BlockPos, BlockState)) {
        block_scan_utils::find_blocks(self.level, self.from, self.to, &self.state_predicate, &mut |level: &mut L, pos, state| {
            consumer(level, pos, state);
            Continuation::Continue
        });
    }

    fn for_each_until(self, mut consumer: impl FnMut(&mut L, BlockPos, BlockState) -> Continuation) -> bool {
        block_scan_utils::find_blocks(self.level, self.from, self.to, &self.state_predicate, &mut consumer)
    }
}
