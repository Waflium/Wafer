use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::SAVED_DATA_TICKETS;

pub struct ForcedChunkToTicketFix {
    base: DataFixBase,
}

impl ForcedChunkToTicketFix {
    pub fn new(output_schema: &Arc<Schema>) -> ForcedChunkToTicketFix {
        ForcedChunkToTicketFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for ForcedChunkToTicketFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("ForcedChunkToTicketFix", self.get_input_schema().get_type(SAVED_DATA_TICKETS), |input| {
            input.update_remainder(|remainder| {
                let empty_map = remainder.empty_map();
                remainder.update("data", |data| {
                    data.rename_and_fix_field("Forced", "tickets", |forced_chunks| {
                        forced_chunks.create_list(forced_chunks.as_long_stream().into_iter().map(|l| {
                            empty_map
                                .empty_map()
                                .set("type", empty_map.create_string("minecraft:forced"))
                                .set("level", empty_map.create_int(31))
                                .set("ticks_left", empty_map.create_long(0))
                                .set("chunk_pos", empty_map.create_long(l))
                        }))
                    })
                })
            })
        })
    }
}
