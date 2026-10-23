use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::abstract_uuid_fix::replace_uuid_least_most;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct GossipUUIDFix;

impl GossipUUIDFix {
    pub fn new(output_schema: &Arc<Schema>, entity_name: &str) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, false, &format!("Gossip for for {entity_name}"), ENTITY, entity_name, |entity| {
            entity.update_remainder(|tag| {
                tag.update("Gossips", |gossips| match gossips.as_stream_opt().into_result() {
                    Ok(stream) => gossips.create_list(stream.into_iter().map(|gossip| replace_uuid_least_most(&gossip, "Target", "Target").unwrap_or(gossip))),
                    Err(_) => gossips,
                })
            })
        })
    }
}
