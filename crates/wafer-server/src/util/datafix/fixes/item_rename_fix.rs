use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::types::{const_type, equals, named};
use crate::util::datafix::fixes::references::ITEM_NAME;

pub struct ItemRenameFix {
    base: DataFixBase,
    name: String,
    fix_item: Arc<dyn Fn(&str) -> String + Send + Sync>,
}

impl ItemRenameFix {
    pub fn create(output_schema: &Arc<Schema>, name: &str, fix_item: impl Fn(&str) -> String + Send + Sync + 'static) -> ItemRenameFix {
        ItemRenameFix { base: DataFixBase::new(output_schema, false), name: name.to_string(), fix_item: Arc::new(fix_item) }
    }
}

impl DataFix for ItemRenameFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let item_name_type = named(ITEM_NAME.type_name(), const_type(ConstType::NamespacedString));
        if !equals(&self.get_input_schema().get_type(ITEM_NAME), &item_name_type, false) {
            panic!("item name type is not what was expected.");
        }
        let fix_item = self.fix_item.clone();
        self.fix_type_everywhere(&self.name, item_name_type, move |input| input.map_string(|item| fix_item(item)))
    }
}
