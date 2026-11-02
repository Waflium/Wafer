use wafer_core::BlockPos;
use wafer_data::BlockState;

use crate::util::continuation::Continuation;

pub const NO_PREDICATE: fn(BlockState) -> bool = |_| true;

pub trait BlockMatcher<L: ?Sized>: Sized {
    fn at_least_matched(self, n: i32) -> bool;

    fn at_most_matched(self, n: i32) -> bool {
        !self.at_least_matched(n + 1)
    }

    fn any_matched(self) -> bool;

    fn none_matched(self) -> bool {
        !self.any_matched()
    }

    fn all_matched(self) -> bool;

    fn for_each(self, consumer: impl FnMut(&mut L, BlockPos, BlockState));

    fn for_each_until(self, consumer: impl FnMut(&mut L, BlockPos, BlockState) -> Continuation) -> bool;
}
