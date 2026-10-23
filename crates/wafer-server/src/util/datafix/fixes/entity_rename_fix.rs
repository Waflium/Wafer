use std::sync::{Arc, Mutex};

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::datafixers::types::{Type, equals};
use crate::datafixers::value::Value;
use crate::util::datafix::extra_data_fix_utils;
use crate::util::datafix::fixes::references::ENTITY;

pub type EntityRename = Arc<dyn Fn(&str, Typed) -> (String, Typed) + Send + Sync>;

pub struct EntityRenameFix {
    base: DataFixBase,
    pub name: String,
    fix: EntityRename,
}

impl EntityRenameFix {
    pub fn new(name: &str, output_schema: &Arc<Schema>, changes_type: bool, fix: impl Fn(&str, Typed) -> (String, Typed) + Send + Sync + 'static) -> EntityRenameFix {
        EntityRenameFix { base: DataFixBase::new(output_schema, changes_type), name: name.to_string(), fix: Arc::new(fix) }
    }
}

impl DataFix for EntityRenameFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let old_type = self.get_input_schema().find_choice_type(ENTITY);
        let new_type = self.get_output_schema().find_choice_type(ENTITY);
        let patched_input_types: Arc<Mutex<Vec<(String, Arc<Type>)>>> = Arc::new(Mutex::new(Vec::new()));
        let (old_choices, new_choices) = (old_type.clone(), new_type.clone());
        let fix = self.fix.clone();
        self.fix_type_everywhere_to(&self.name, old_type, new_type, move |input| match input {
            Value::Choice(old_name, value) => {
                let old_entity_type = {
                    let mut patched = patched_input_types.lock().unwrap();
                    match patched.iter().find(|(name, _)| *name == old_name) {
                        Some((_, ty)) => ty.clone(),
                        None => {
                            let ty = old_choices.choices().get(&old_name).expect("an entity type");
                            let ty = extra_data_fix_utils::patch_sub_type(ty, &old_choices, &new_choices);
                            patched.push((old_name.clone(), ty.clone()));
                            ty
                        }
                    }
                };
                let (new_name, new_entity) = fix(&old_name, Typed::new(old_entity_type, *value));
                let expected_type = new_choices.choices().get(&new_name).cloned().expect("an entity type");
                if !equals(&expected_type, &new_entity.ty, true) {
                    panic!("Dynamic type check failed: {expected_type:?} not equal to {:?}", new_entity.ty);
                }
                Value::Choice(new_name, Box::new(new_entity.value))
            }
            other => other,
        })
    }
}
