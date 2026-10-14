use wafer_core::BlockPos;
use wafer_data::BlockState;
use wafer_protocol::nbt::Nbt;

use super::data_component_matchers::DataComponentMatchers;
use super::nbt_predicate::NbtPredicate;
use super::state_properties_predicate::StatePropertiesPredicate;
use crate::core::holder_set::HolderSet;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::level::BlockGetter;
use crate::world::level::block::entity::BlockEntity;
use crate::world::level::block::state::pattern::block_in_world::BlockInWorld;

#[derive(Debug)]
pub struct BlockPredicate {
    blocks: Option<HolderSet>,
    properties: Option<StatePropertiesPredicate>,
    nbt: Option<NbtPredicate>,
    components: DataComponentMatchers,
}

impl BlockPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<BlockPredicate> {
        codec::map(tag)?;
        Ok(BlockPredicate {
            blocks: codec::decode_optional(tag, "blocks", |t| HolderSet::decode("minecraft:block", t))?,
            properties: codec::decode_optional(tag, "state", StatePropertiesPredicate::decode)?,
            nbt: codec::decode_optional(tag, "nbt", NbtPredicate::decode)?,
            components: DataComponentMatchers::decode(tag)?,
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, pos: BlockPos) -> bool {
        if !level.is_loaded(pos) {
            return false;
        }
        if !self.matches_state(level.block_state(pos)) {
            return false;
        }
        if self.will_match_block_entity() {
            let block_entity = level.get_block_entity(pos);
            if !self.matches_block_entity(block_entity.as_deref()) {
                return false;
            }
        }
        true
    }

    pub fn will_match_block_entity(&self) -> bool {
        self.nbt.is_some() || !self.components.is_empty()
    }

    pub fn matches_block_in_world(&self, block_in_world: &mut BlockInWorld) -> bool {
        if !block_in_world.get_state().is_some_and(|state| self.matches_state(state)) {
            return false;
        }
        self.nbt.as_ref().is_none_or(|nbt| matches_block_entity_data(block_in_world.get_entity().as_deref(), nbt))
    }

    pub fn matches_state(&self, state: BlockState) -> bool {
        if self.blocks.as_ref().is_some_and(|blocks| !blocks.contains(state.block().0 as i32)) {
            return false;
        }
        self.properties.as_ref().is_none_or(|properties| properties.matches_block(state))
    }

    pub fn matches_block_entity(&self, block_entity: Option<&BlockEntity>) -> bool {
        if let Some(nbt) = &self.nbt
            && !matches_block_entity_data(block_entity, nbt)
        {
            return false;
        }
        self.components.is_empty() || block_entity.is_some_and(|entity| self.components.test(entity.collect_components().as_slice()))
    }
}

fn matches_block_entity_data(entity: Option<&BlockEntity>, nbt: &NbtPredicate) -> bool {
    entity.is_some_and(|entity| nbt.matches(Some(&entity.save_with_full_metadata())))
}
