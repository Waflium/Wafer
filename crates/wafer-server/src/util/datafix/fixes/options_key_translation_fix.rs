use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::OPTIONS;

pub struct OptionsKeyTranslationFix {
    base: DataFixBase,
}

impl OptionsKeyTranslationFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> OptionsKeyTranslationFix {
        OptionsKeyTranslationFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

fn substring(input: &str, begin_index: usize) -> String {
    let units: Vec<u16> = input.encode_utf16().collect();
    if begin_index > units.len() {
        panic!("Range [{}, {}) out of bounds for length {}", begin_index, units.len(), units.len());
    }
    String::from_utf16_lossy(&units[begin_index..])
}

impl DataFix for OptionsKeyTranslationFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("OptionsKeyTranslationFix", self.get_input_schema().get_type(OPTIONS), |input| {
            input.update_remainder(|tag| match tag.get_map_values().into_result() {
                Ok(map) => {
                    let entries: Vec<_> = map
                        .into_iter()
                        .map(|(key, value)| {
                            if key.as_string_or("").starts_with("key_") {
                                let old_value = value.as_string_or("");
                                if !old_value.starts_with("key.mouse") && !old_value.starts_with("scancode.") {
                                    let name = tag.create_string(format!("key.keyboard.{}", substring(&old_value, "key.".len())));
                                    return (key, name);
                                }
                            }
                            (key, value)
                        })
                        .collect();
                    tag.create_map(entries)
                }
                Err(_) => tag,
            })
        })
    }
}
