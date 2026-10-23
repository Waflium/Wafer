use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::STRUCTURE_FEATURE;

pub struct StructureReferenceCountFix {
    base: DataFixBase,
}

impl StructureReferenceCountFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> StructureReferenceCountFix {
        StructureReferenceCountFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for StructureReferenceCountFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let structure_info = self.get_input_schema().get_type(STRUCTURE_FEATURE);
        self.fix_type_everywhere_typed("Structure Reference Fix", structure_info, |input| input.update_remainder(set_count_to_at_least_one))
    }
}

fn set_count_to_at_least_one(structure_tag: Dynamic) -> Dynamic {
    structure_tag.update("references", |references| {
        let count = references.as_number().into_result().ok().map(|number| number.int_value()).filter(|number| *number > 0).unwrap_or(1);
        references.create_int(count)
    })
}
