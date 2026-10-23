use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::types::{const_type, equals, named};
use crate::util::datafix::fixes::references::{BLOCK_NAME, BLOCK_STATE, FLAT_BLOCK_STATE};

pub type Renamer = Arc<dyn Fn(&str) -> String + Send + Sync>;

pub struct LegacyBlockRenameFix {
    base: DataFixBase,
    name: String,
    rename_block: Renamer,
}

impl LegacyBlockRenameFix {
    pub fn create(output_schema: &Arc<Schema>, name: &str, renamer: impl Fn(&str) -> String + Send + Sync + 'static) -> LegacyBlockRenameFix {
        LegacyBlockRenameFix { base: DataFixBase::new(output_schema, false), name: name.to_string(), rename_block: Arc::new(renamer) }
    }
}

impl DataFix for LegacyBlockRenameFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let block_type = self.get_input_schema().get_type(BLOCK_NAME);
        let expected_type = named(BLOCK_NAME.type_name(), const_type(ConstType::NamespacedString));
        if !equals(&block_type, &expected_type, false) {
            panic!("block type is not what was expected.");
        }
        let rename = self.rename_block.clone();
        let block_rule = self.fix_type_everywhere(&format!("{} for block", self.name), expected_type, move |input| input.map_string(|block| rename(block)));
        let rename = self.rename_block.clone();
        let block_state_rule = self.fix_type_everywhere_typed(&format!("{} for block_state", self.name), self.get_input_schema().get_type(BLOCK_STATE), move |input| {
            input.update_remainder(|tag| fix_block_state(tag, &rename))
        });
        let rename = self.rename_block.clone();
        let flat_block_state_rule =
            self.fix_type_everywhere_typed(&format!("{} for flat_block_state", self.name), self.get_input_schema().get_type(FLAT_BLOCK_STATE), move |input| {
                input.update_remainder(|tag| match tag.as_string().into_result() {
                    Ok(string) => tag.create_string(fix_flat_block_state(&string, &rename)),
                    Err(_) => tag,
                })
            });
        TypeRewriteRule::seq(vec![block_rule, block_state_rule, flat_block_state_rule])
    }
}

fn fix_block_state(tag: Dynamic, rename: &Renamer) -> Dynamic {
    match tag.get("Name").as_string().into_result() {
        Ok(name) => {
            let renamed = tag.create_string(rename(&name));
            tag.set("Name", renamed)
        }
        Err(_) => tag,
    }
}

fn fix_flat_block_state(string: &str, rename: &Renamer) -> String {
    let start_properties = string.find('[');
    let start_nbt = string.find('{');
    let mut end = string.len();
    if let Some(start) = start_properties.filter(|&start| start > 0) {
        end = start;
    }
    if let Some(start) = start_nbt.filter(|&start| start > 0) {
        end = end.min(start);
    }
    format!("{}{}", rename(&string[..end]), &string[end..])
}
