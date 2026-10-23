use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::TypeReference;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;

pub struct EntityFallDistanceFloatToDoubleFix {
    base: DataFixBase,
    ty: TypeReference,
}

impl EntityFallDistanceFloatToDoubleFix {
    pub fn new(output_schema: &Arc<Schema>, ty: TypeReference) -> EntityFallDistanceFloatToDoubleFix {
        EntityFallDistanceFloatToDoubleFix { base: DataFixBase::new(output_schema, false), ty }
    }
}

impl DataFix for EntityFallDistanceFloatToDoubleFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed(&format!("EntityFallDistanceFloatToDoubleFixFor{}", self.ty.type_name()), self.get_output_schema().get_type(self.ty), fix_entity)
    }
}

fn fix_entity(entity: Typed) -> Typed {
    entity.update_remainder(|remainder| remainder.rename_and_fix_field("FallDistance", "fall_distance", |fall_distance| fall_distance.create_double(fall_distance.as_float(0.0) as f64)))
}
