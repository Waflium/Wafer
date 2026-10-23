use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, find_field};
use crate::util::datafix::fixes::references::{ENTITY, ITEM_STACK, PLAYER};

type Renames = Arc<dyn Fn(&str) -> String + Send + Sync>;

pub struct AttributesRenameLegacy {
    base: DataFixBase,
    name: String,
    renames: Renames,
}

impl AttributesRenameLegacy {
    pub fn new(output_schema: &Arc<Schema>, name: &str, renames: impl Fn(&str) -> String + Send + Sync + 'static) -> AttributesRenameLegacy {
        AttributesRenameLegacy { base: DataFixBase::new(output_schema, false), name: name.to_string(), renames: Arc::new(renames) }
    }
}

fn fix_name(name: Dynamic, renames: &Renames) -> Dynamic {
    match name.as_string().into_result() {
        Ok(string) => name.create_string(renames(&string)),
        Err(_) => name,
    }
}

fn fix_item_stack_tag(item_stack: Typed, renames: &Renames) -> Typed {
    item_stack.update_remainder(|tag| {
        tag.update("AttributeModifiers", |modifiers| match modifiers.as_stream_opt().into_result() {
            Ok(entries) => modifiers.create_list(entries.into_iter().map(|modifier| modifier.update("AttributeName", |name| fix_name(name, renames)))),
            Err(_) => modifiers,
        })
    })
}

fn fix_entity(entity: Typed, renames: &Renames) -> Typed {
    entity.update_remainder(|tag| {
        tag.update("Attributes", |attribute_list| match attribute_list.as_stream_opt().into_result() {
            Ok(entries) => attribute_list.create_list(entries.into_iter().map(|attribute| attribute.update("Name", |name| fix_name(name, renames)))),
            Err(_) => attribute_list,
        })
    })
}

impl DataFix for AttributesRenameLegacy {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let item_stack_type = self.get_input_schema().get_type(ITEM_STACK);
        let tag_f = find_field(&item_stack_type, "tag");
        let (item_stack_renames, entity_renames, player_renames) = (self.renames.clone(), self.renames.clone(), self.renames.clone());
        TypeRewriteRule::seq(vec![
            self.fix_type_everywhere_typed(&format!("{} (ItemStack)", self.name), item_stack_type, move |item_stack| {
                item_stack.update_typed(&tag_f, None, |tag| fix_item_stack_tag(tag, &item_stack_renames))
            }),
            self.fix_type_everywhere_typed(&format!("{} (Entity)", self.name), self.get_input_schema().get_type(ENTITY), move |entity| fix_entity(entity, &entity_renames)),
            self.fix_type_everywhere_typed(&format!("{} (Player)", self.name), self.get_input_schema().get_type(PLAYER), move |entity| fix_entity(entity, &player_renames)),
        ])
    }
}
