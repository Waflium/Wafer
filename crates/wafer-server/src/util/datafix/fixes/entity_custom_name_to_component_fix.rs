use std::sync::Arc;

use wafer_protocol::nbt::Nbt;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{OpticFinder, Typed, field_finder, find_field};
use crate::datafixers::types::{Type, const_type};
use crate::util::datafix::extra_data_fix_utils;
use crate::util::datafix::fixes::references::ENTITY;
use crate::util::datafix::legacy_component_data_fix_utils;

pub struct EntityCustomNameToComponentFix {
    base: DataFixBase,
}

impl EntityCustomNameToComponentFix {
    pub fn new(output_schema: &Arc<Schema>) -> EntityCustomNameToComponentFix {
        EntityCustomNameToComponentFix { base: DataFixBase::new(output_schema, true) }
    }
}

impl DataFix for EntityCustomNameToComponentFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let entity_type = self.get_input_schema().get_type(ENTITY);
        let new_entity_type = self.get_output_schema().get_type(ENTITY);
        let id_f = field_finder("id", const_type(ConstType::NamespacedString));
        let custom_name_f = find_field(&entity_type, "CustomName");
        let new_custom_name_type = new_entity_type.find_field_type("CustomName").expect("a CustomName field");
        let target_type = new_entity_type.clone();
        self.fix_type_everywhere_typed_to("EntityCustomNameToComponentFix", entity_type, new_entity_type, move |entity| {
            fix_entity(entity, &target_type, &id_f, &custom_name_f, &new_custom_name_type)
        })
    }
}

fn fix_entity(entity: Typed, new_entity_type: &Arc<Type>, id_f: &OpticFinder, custom_name_f: &OpticFinder, new_custom_name_type: &Arc<Type>) -> Typed {
    let Some(custom_name) = entity.get_optional(custom_name_f) else { return extra_data_fix_utils::cast(new_entity_type, entity) };
    let custom_name = custom_name.as_str().expect("a custom name string").to_string();
    if custom_name.is_empty() {
        return crate::util::write_and_read_typed_or_throw(&entity, new_entity_type, |dynamic| dynamic.remove("CustomName"));
    }
    let id = entity.get_optional(id_f).and_then(|id| id.as_str().map(str::to_string)).unwrap_or_default();
    let component = fix_custom_name(&custom_name, &id);
    entity.set(custom_name_f, crate::util::read_typed_or_throw(new_custom_name_type, component, false))
}

fn fix_custom_name(custom_name: &str, id: &str) -> Dynamic {
    if id == "minecraft:commandblock_minecart" { Dynamic::new(Nbt::String(custom_name.to_string())) } else { legacy_component_data_fix_utils::create_plain_text_component(custom_name) }
}
