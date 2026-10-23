use std::sync::Arc;

use wafer_protocol::nbt::Nbt;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::types::{const_type, equals, named, or};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::block_state_data;
use crate::util::datafix::fixes::references::BLOCK_NAME;
use crate::util::datafix::schemas::namespaced_schema::ensure_namespaced;

pub struct BlockNameFlatteningFix {
    base: DataFixBase,
}

impl BlockNameFlatteningFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> BlockNameFlatteningFix {
        BlockNameFlatteningFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for BlockNameFlatteningFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let block_type = self.get_input_schema().get_type(BLOCK_NAME);
        let new_block_type = self.get_output_schema().get_type(BLOCK_NAME);
        let expected_block_type = named(BLOCK_NAME.type_name(), or(const_type(ConstType::Int), const_type(ConstType::NamespacedString)));
        let expected_new_block_type = named(BLOCK_NAME.type_name(), const_type(ConstType::NamespacedString));
        if equals(&block_type, &expected_block_type, false) && equals(&new_block_type, &expected_new_block_type, false) {
            self.fix_type_everywhere_to("BlockNameFlatteningFix", expected_block_type, expected_new_block_type, |block| match block {
                Value::Left(id) => match *id {
                    Value::Prim(Nbt::Int(id)) => Value::Prim(Nbt::String(block_state_data::upgrade_block_id(id))),
                    other => other,
                },
                Value::Right(name) => match *name {
                    Value::Prim(Nbt::String(name)) => Value::Prim(Nbt::String(block_state_data::upgrade_block(&ensure_namespaced(&name)))),
                    other => other,
                },
                other => other,
            })
        } else {
            panic!("Expected and actual types don't match.");
        }
    }
}
