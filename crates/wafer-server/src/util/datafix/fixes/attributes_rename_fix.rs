use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::util::datafix::extra_data_fix_utils;
use crate::util::datafix::fixes::references::{DATA_COMPONENTS, ENTITY, PLAYER};

pub type Renames = Arc<dyn Fn(&str) -> String + Send + Sync>;

pub struct AttributesRenameFix {
    base: DataFixBase,
    name: String,
    renames: Renames,
    old_data_component_format: bool,
}

impl AttributesRenameFix {
    pub fn new(output_schema: &Arc<Schema>, name: &str, renames: impl Fn(&str) -> String + Send + Sync + 'static) -> AttributesRenameFix {
        AttributesRenameFix::with_format(output_schema, name, renames, false)
    }

    pub fn with_format(output_schema: &Arc<Schema>, name: &str, renames: impl Fn(&str) -> String + Send + Sync + 'static, old_data_component_format: bool) -> AttributesRenameFix {
        AttributesRenameFix { base: DataFixBase::new(output_schema, false), name: name.to_string(), renames: Arc::new(renames), old_data_component_format }
    }
}

impl DataFix for AttributesRenameFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let renames = self.renames.clone();
        let old_format = self.old_data_component_format;
        let components_rule = self.fix_type_everywhere_typed(&format!("{} (Components)", self.name), self.get_input_schema().get_type(DATA_COMPONENTS), move |components| {
            if old_format { fix_data_components_old(components, &renames) } else { fix_data_components(components, &renames) }
        });
        let renames = self.renames.clone();
        let entity_rule = self.fix_type_everywhere_typed(&format!("{} (Entity)", self.name), self.get_input_schema().get_type(ENTITY), move |entity| fix_entity(entity, &renames));
        let renames = self.renames.clone();
        let player_rule = self.fix_type_everywhere_typed(&format!("{} (Player)", self.name), self.get_input_schema().get_type(PLAYER), move |entity| fix_entity(entity, &renames));
        TypeRewriteRule::seq(vec![components_rule, entity_rule, player_rule])
    }
}

fn map_list(list: Dynamic, f: impl Fn(Dynamic) -> Dynamic) -> Dynamic {
    match list.as_stream_opt().into_result() {
        Ok(values) => list.create_list(values.into_iter().map(f)),
        Err(_) => list,
    }
}

fn fix_data_components(components: Typed, renames: &Renames) -> Typed {
    components.update_remainder(|component_data| component_data.update("minecraft:attribute_modifiers", |modifiers| map_list(modifiers, |modifier| fix_type_field(modifier, renames))))
}

fn fix_data_components_old(components: Typed, renames: &Renames) -> Typed {
    components.update_remainder(|component_data| {
        component_data.update("minecraft:attribute_modifiers", |attribute_modifiers| {
            attribute_modifiers.update("modifiers", |modifiers| map_list(modifiers, |modifier| fix_type_field(modifier, renames)))
        })
    })
}

fn fix_entity(entity: Typed, renames: &Renames) -> Typed {
    entity.update_remainder(|tag| tag.update("attributes", |attribute_list| map_list(attribute_list, |attribute| fix_id_field(attribute, renames))))
}

fn fix_id_field(dynamic: Dynamic, renames: &Renames) -> Dynamic {
    extra_data_fix_utils::fix_string_field(dynamic, "id", |id| renames(id))
}

fn fix_type_field(dynamic: Dynamic, renames: &Renames) -> Dynamic {
    extra_data_fix_utils::fix_string_field(dynamic, "type", |id| renames(id))
}
