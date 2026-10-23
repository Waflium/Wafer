use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::DATA_COMPONENTS;

pub type ComponentFix = Arc<dyn Fn(Dynamic) -> Option<Dynamic> + Send + Sync>;

pub struct DataComponentRemainderFix {
    base: DataFixBase,
    name: String,
    component_id: String,
    new_component_id: String,
    fix_component: ComponentFix,
}

impl DataComponentRemainderFix {
    pub fn new(output_schema: &Arc<Schema>, name: &str, component_id: &str, fix_component: impl Fn(Dynamic) -> Option<Dynamic> + Send + Sync + 'static) -> DataComponentRemainderFix {
        DataComponentRemainderFix::renaming(output_schema, name, component_id, component_id, fix_component)
    }

    pub fn renaming(
        output_schema: &Arc<Schema>,
        name: &str,
        component_id: &str,
        new_component_id: &str,
        fix_component: impl Fn(Dynamic) -> Option<Dynamic> + Send + Sync + 'static,
    ) -> DataComponentRemainderFix {
        DataComponentRemainderFix {
            base: DataFixBase::new(output_schema, false),
            name: name.to_string(),
            component_id: component_id.to_string(),
            new_component_id: new_component_id.to_string(),
            fix_component: Arc::new(fix_component),
        }
    }
}

impl DataFix for DataComponentRemainderFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let data_components_type = self.get_input_schema().get_type(DATA_COMPONENTS);
        let (component_id, new_component_id, fix_component) = (self.component_id.clone(), self.new_component_id.clone(), self.fix_component.clone());
        self.fix_type_everywhere_typed(&self.name, data_components_type, move |components| {
            components.update_remainder(|remainder| {
                let Some(component) = remainder.get(&component_id).result() else { return remainder };
                let new_component = fix_component(component);
                remainder.remove(&component_id).set_field_if_present(&new_component_id, new_component)
            })
        })
    }
}
