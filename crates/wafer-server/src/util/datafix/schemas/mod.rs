pub mod namespaced_schema;
pub mod v99;
pub mod v100;

use crate::datafixers::schemas::schema::SchemaClass;

pub static SAME: SchemaClass = SchemaClass { namespaced: false, register_types: None, register_entities: None, register_block_entities: None };
