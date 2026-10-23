use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::types::{const_type, equals, named};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::{ENTITY, ENTITY_NAME};

pub type Rename = Arc<dyn Fn(&str) -> String + Send + Sync>;

pub struct SimplestEntityRenameFix {
    base: DataFixBase,
    name: String,
    rename: Rename,
}

impl SimplestEntityRenameFix {
    pub fn new(name: &str, output_schema: &Arc<Schema>, changes_type: bool, rename: impl Fn(&str) -> String + Send + Sync + 'static) -> SimplestEntityRenameFix {
        SimplestEntityRenameFix { base: DataFixBase::new(output_schema, changes_type), name: name.to_string(), rename: Arc::new(rename) }
    }
}

impl DataFix for SimplestEntityRenameFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let old_type = self.get_input_schema().find_choice_type(ENTITY);
        let new_type = self.get_output_schema().find_choice_type(ENTITY);
        let entity_name_type = named(ENTITY_NAME.type_name(), const_type(ConstType::NamespacedString));
        if !equals(&self.get_output_schema().get_type(ENTITY_NAME), &entity_name_type, false) {
            panic!("Entity name type is not what was expected.");
        }
        let rename = self.rename.clone();
        let (old_choices, new_choices) = (old_type.clone(), new_type.clone());
        let entity_rule = self.fix_type_everywhere_to(&self.name, old_type, new_type, move |input| match input {
            Value::Choice(old_name, value) => {
                let new_name = rename(&old_name);
                let old_entity_type = old_choices.choices().get(&old_name).cloned();
                let new_entity_type = new_choices.choices().get(&new_name).cloned();
                match (new_entity_type, old_entity_type) {
                    (Some(new_entity_type), Some(old_entity_type)) if equals(&new_entity_type, &old_entity_type, true) => Value::Choice(new_name, value),
                    (new_entity_type, old_entity_type) => panic!("Dynamic type check failed: {new_entity_type:?} not equal to {old_entity_type:?}"),
                }
            }
            other => other,
        });
        let rename = self.rename.clone();
        let name_rule = self.fix_type_everywhere(&format!("{} for entity name", self.name), entity_name_type, move |input| input.map_string(|name| rename(name)));
        TypeRewriteRule::seq(vec![entity_rule, name_rule])
    }
}
