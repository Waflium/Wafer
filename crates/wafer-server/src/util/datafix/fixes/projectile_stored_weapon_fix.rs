use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::named_choice;
use crate::util::datafix::extra_data_fix_utils::{self, TypedFilter};
use crate::util::datafix::fixes::references::ENTITY;

pub struct ProjectileStoredWeaponFix {
    base: DataFixBase,
}

impl ProjectileStoredWeaponFix {
    pub fn new(output_schema: &Arc<Schema>) -> ProjectileStoredWeaponFix {
        ProjectileStoredWeaponFix { base: DataFixBase::new(output_schema, true) }
    }

    fn fix_choice(&self, entity_name: &str) -> TypedFilter {
        let input_entity_choice_type = self.get_input_schema().get_choice_type(ENTITY, entity_name);
        let output_entity_choice_type = self.get_output_schema().get_choice_type(ENTITY, entity_name);
        let entity_f = named_choice(entity_name, input_entity_choice_type);
        Box::new(move |input| {
            input.update_typed(&entity_f, Some(&output_entity_choice_type), |typed| crate::util::write_and_read_typed_or_throw(&typed, &output_entity_choice_type, |dynamic| dynamic))
        })
    }
}

impl DataFix for ProjectileStoredWeaponFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let input_entity_type = self.get_input_schema().get_type(ENTITY);
        let output_entity_type = self.get_output_schema().get_type(ENTITY);
        self.fix_type_everywhere_typed_to(
            "Fix Arrow stored weapon",
            input_entity_type,
            output_entity_type,
            extra_data_fix_utils::chain_all_filters(vec![self.fix_choice("minecraft:arrow"), self.fix_choice("minecraft:spectral_arrow")]),
        )
    }
}
