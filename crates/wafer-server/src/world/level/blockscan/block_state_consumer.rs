use wafer_core::BlockPos;
use wafer_data::BlockState;

use crate::util::continuation::Continuation;

pub trait BlockStateConsumer<L: ?Sized>: FnMut(&mut L, BlockPos, BlockState) -> Continuation {}

impl<L: ?Sized, F: FnMut(&mut L, BlockPos, BlockState) -> Continuation> BlockStateConsumer<L> for F {}
