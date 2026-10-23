use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::SAVED_DATA_TICKETS;

const CHUNK_COORD_BITS: u32 = 32;
const CHUNK_COORD_MASK: u64 = 4294967295;

pub struct ChunkTicketUnpackPosFix {
    base: DataFixBase,
}

impl ChunkTicketUnpackPosFix {
    pub fn new(output_schema: &Arc<Schema>) -> ChunkTicketUnpackPosFix {
        ChunkTicketUnpackPosFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for ChunkTicketUnpackPosFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("ChunkTicketUnpackPosFix", self.get_input_schema().get_type(SAVED_DATA_TICKETS), |input| {
            input.update_remainder(|remainder| {
                remainder.update("data", |data| {
                    data.update("tickets", |tickets| {
                        tickets.create_list(tickets.as_stream().into_iter().map(|ticket| {
                            ticket.update("chunk_pos", |chunk_pos| {
                                let key = chunk_pos.as_long(0) as u64;
                                let x = (key & CHUNK_COORD_MASK) as i32;
                                let z = ((key >> CHUNK_COORD_BITS) & CHUNK_COORD_MASK) as i32;
                                chunk_pos.create_int_list([x, z])
                            })
                        }))
                    })
                })
            })
        })
    }
}
