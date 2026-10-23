use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, field_finder, find_field};
use crate::datafixers::types::{Type, const_type, named};
use crate::util::datafix::fixes::references::{ITEM_NAME, ITEM_STACK};

pub type TagFix = Arc<dyn Fn(Typed) -> Typed + Send + Sync>;

pub struct ItemStackTagFix {
    base: DataFixBase,
    name: String,
    id_filter: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    fix_item_stack_tag: TagFix,
}

impl ItemStackTagFix {
    pub fn new(
        output_schema: &Arc<Schema>,
        name: &str,
        id_filter: impl Fn(&str) -> bool + Send + Sync + 'static,
        fix_item_stack_tag: impl Fn(Typed) -> Typed + Send + Sync + 'static,
    ) -> ItemStackTagFix {
        ItemStackTagFix { base: DataFixBase::new(output_schema, false), name: name.to_string(), id_filter: Arc::new(id_filter), fix_item_stack_tag: Arc::new(fix_item_stack_tag) }
    }

    pub fn remainder(
        output_schema: &Arc<Schema>,
        name: &str,
        id_filter: impl Fn(&str) -> bool + Send + Sync + 'static,
        fix_item_stack_tag: impl Fn(Dynamic) -> Dynamic + Send + Sync + 'static,
    ) -> ItemStackTagFix {
        let fix_item_stack_tag = Arc::new(fix_item_stack_tag);
        ItemStackTagFix::new(output_schema, name, id_filter, move |tag| tag.update_remainder(|tag| fix_item_stack_tag(tag)))
    }

    pub fn create_fixer(item_stack_type: &Arc<Type>, id_filter: impl Fn(&str) -> bool + Send + Sync + 'static, fixer: impl Fn(Typed) -> Typed + Send + Sync + 'static) -> impl Fn(Typed) -> Typed + Send + Sync + 'static {
        let id_finder = field_finder("id", named(ITEM_NAME.type_name(), const_type(ConstType::NamespacedString)));
        let tag_finder = find_field(item_stack_type, "tag");
        move |input: Typed| {
            let id = input.get_optional(&id_finder);
            if id.as_ref().and_then(|id| id.as_str()).is_some_and(|id| id_filter(id)) { input.update_typed(&tag_finder, None, |tag| fixer(tag)) } else { input }
        }
    }
}

impl DataFix for ItemStackTagFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let item_stack_type = self.get_input_schema().get_type(ITEM_STACK);
        let (id_filter, fix_item_stack_tag) = (self.id_filter.clone(), self.fix_item_stack_tag.clone());
        let fixer = ItemStackTagFix::create_fixer(&item_stack_type, move |id| id_filter(id), move |tag| fix_item_stack_tag(tag));
        self.fix_type_everywhere_typed(&self.name, item_stack_type, fixer)
    }
}
