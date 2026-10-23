use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::find_field;
use crate::util::datafix::fixes::references::ITEM_STACK;
use crate::util::datafix::schemas::namespaced_schema::ensure_namespaced;

pub struct RenameEnchantmentsFix {
    base: DataFixBase,
    name: String,
    renames: Arc<Vec<(String, String)>>,
}

impl RenameEnchantmentsFix {
    pub fn new(output_schema: &Arc<Schema>, name: &str, renames: &[(&str, &str)]) -> RenameEnchantmentsFix {
        RenameEnchantmentsFix {
            base: DataFixBase::new(output_schema, false),
            name: name.to_string(),
            renames: Arc::new(renames.iter().map(|(from, to)| (from.to_string(), to.to_string())).collect()),
        }
    }
}

fn fix_tag(tag: Dynamic, renames: &[(String, String)]) -> Dynamic {
    let tag = fix_enchantment_list(tag, "Enchantments", renames);
    fix_enchantment_list(tag, "StoredEnchantments", renames)
}

fn fix_enchantment_list(item_stack: Dynamic, field: &str, renames: &[(String, String)]) -> Dynamic {
    item_stack.update(field, |tag| match tag.as_stream_opt().into_result() {
        Ok(elements) => {
            let renamed = elements.into_iter().map(|element| {
                element.update("id", |id| match id.as_string().into_result() {
                    Ok(string_id) => {
                        let namespaced = ensure_namespaced(&string_id);
                        let renamed = renames.iter().find(|(from, _)| *from == namespaced).map_or(string_id, |(_, to)| to.clone());
                        id.create_string(renamed)
                    }
                    Err(_) => id,
                })
            });
            tag.create_list(renamed)
        }
        Err(_) => tag,
    })
}

impl DataFix for RenameEnchantmentsFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let item = self.get_input_schema().get_type(ITEM_STACK);
        let tag_finder = find_field(&item, "tag");
        let renames = self.renames.clone();
        self.fix_type_everywhere_typed(&self.name, item, move |input| input.update_typed(&tag_finder, None, |tag| tag.update_remainder(|tag| fix_tag(tag, &renames))))
    }
}
