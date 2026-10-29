use wafer_data::BlockState;
use serde_json::Value;

use crate::world::level::levelgen::parse_block_state;
use crate::world::level::levelgen::structure::templatesystem::rule_test::RuleTest;

pub struct BlockReplacement {
    pub target: RuleTest,
    pub state: BlockState,
}

impl BlockReplacement {
    pub fn parse(json: &Value) -> Result<BlockReplacement, String> {
        Ok(BlockReplacement {
            target: RuleTest::parse(json.get("target").ok_or("missing target")?)?,
            state: parse_block_state(json.get("state").ok_or("missing state")?)?,
        })
    }
}

pub fn codec() -> crate::world::item::component::types::Pair {
    use crate::world::item::component::types::{record, req};
    use crate::world::level::block::state::block_state;
    use crate::world::level::levelgen::structure::templatesystem::rule_test;
    record(vec![req("target", rule_test::codec()), req("state", block_state::codec())])
}
