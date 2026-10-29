use wafer_data::BlockState;
use serde_json::Value;

use super::blockpredicates::block_predicate::BlockSet;
use super::feature::stateproviders::block_state_provider::BlockStateProviderHolder;
use super::parse_block_state;

pub struct GeodeBlockSettings {
    pub filling_provider: BlockStateProviderHolder,
    pub inner_layer_provider: BlockStateProviderHolder,
    pub alternate_inner_layer_provider: BlockStateProviderHolder,
    pub middle_layer_provider: BlockStateProviderHolder,
    pub outer_layer_provider: BlockStateProviderHolder,
    pub inner_placements: Vec<BlockState>,
    pub cannot_replace: BlockSet,
    pub invalid_blocks: BlockSet,
}

impl GeodeBlockSettings {
    pub fn parse(json: &Value) -> Result<GeodeBlockSettings, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| crate::util::codec::no_key(key, json));
        let provider = |key: &str| BlockStateProviderHolder::parse(field(key)?);
        Ok(GeodeBlockSettings {
            filling_provider: provider("filling_provider")?,
            inner_layer_provider: provider("inner_layer_provider")?,
            alternate_inner_layer_provider: provider("alternate_inner_layer_provider")?,
            middle_layer_provider: provider("middle_layer_provider")?,
            outer_layer_provider: provider("outer_layer_provider")?,
            inner_placements: field("inner_placements")?.as_array().ok_or("inner_placements is a list")?.iter().map(parse_block_state).collect::<Result<_, _>>()?,
            cannot_replace: BlockSet::parse(field("cannot_replace")?)?,
            invalid_blocks: BlockSet::parse(field("invalid_blocks")?)?,
        })
    }
}

pub fn codec() -> crate::world::item::component::types::Pair {
    use super::feature::stateproviders::block_state_provider;
    use crate::world::item::component::types::{self, record, req};
    use crate::world::level::block::state::block_state;
    record(vec![
        req("filling_provider", block_state_provider::holder_codec()),
        req("inner_layer_provider", block_state_provider::holder_codec()),
        req("alternate_inner_layer_provider", block_state_provider::holder_codec()),
        req("middle_layer_provider", block_state_provider::holder_codec()),
        req("outer_layer_provider", block_state_provider::holder_codec()),
        req("inner_placements", types::non_empty_list(block_state::codec())),
        req("cannot_replace", types::holder_set("minecraft:block")),
        req("invalid_blocks", types::holder_set("minecraft:block")),
    ])
}
