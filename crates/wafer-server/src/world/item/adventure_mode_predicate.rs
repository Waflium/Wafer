use wafer_protocol::nbt::Nbt;

use crate::advancements::predicates::block_predicate::BlockPredicate;
use crate::util::codec::DataResult;
use crate::world::level::block::state::pattern::block_in_world::BlockInWorld;

pub struct AdventureModePredicate {
    predicates: Vec<BlockPredicate>,
}

impl AdventureModePredicate {
    pub fn decode(tag: &Nbt) -> DataResult<AdventureModePredicate> {
        let predicates = match tag {
            Nbt::List(items) => items.iter().map(BlockPredicate::decode).collect::<DataResult<Vec<_>>>()?,
            _ => vec![BlockPredicate::decode(tag)?],
        };
        Ok(AdventureModePredicate { predicates })
    }

    pub fn test(&self, block_in_world: &mut BlockInWorld) -> bool {
        self.predicates.iter().any(|predicate| predicate.matches_block_in_world(block_in_world))
    }
}
