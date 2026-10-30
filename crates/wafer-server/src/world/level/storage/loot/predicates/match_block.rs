use wafer_protocol::nbt::Nbt;

use crate::advancements::predicates::block_predicate::BlockPredicate;
use crate::util::codec::{self, DecodeResult};
use crate::world::item::component::codec::Wrap;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::parameters::loot_context_params::LootContextParam;

pub struct MatchBlock {
    predicate: BlockPredicate,
}

impl MatchBlock {
    pub fn decode(tag: &Nbt) -> DecodeResult<MatchBlock> {
        codec::wrapped(Wrap::MapXmap, || BlockPredicate::decode_map(tag)).map(|predicate| MatchBlock { predicate })
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        let Some(block_state) = context.params.block_state else { return false };
        self.predicate.matches_state(block_state) && self.predicate.matches_block_entity(context.params.block_entity)
    }
}

impl MatchBlock {
    pub fn get_referenced_context_params(&self) -> Vec<LootContextParam> {
        if self.predicate.will_match_block_entity() { vec![LootContextParam::BlockState, LootContextParam::BlockEntity] } else { vec![LootContextParam::BlockState] }
    }
}
