use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::DEBUG_PROFILE;

pub struct DebugProfileLookingAtSplitFix {
    base: DataFixBase,
}

impl DebugProfileLookingAtSplitFix {
    pub fn new(output_schema: &Arc<Schema>) -> DebugProfileLookingAtSplitFix {
        DebugProfileLookingAtSplitFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for DebugProfileLookingAtSplitFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("DebugProfileLookingAtSplitFix", self.get_input_schema().get_type(DEBUG_PROFILE), |input| {
            input.update_remainder(|tag| tag.update("custom", update_options))
        })
    }
}

fn update_options(custom: Dynamic) -> Dynamic {
    let map = custom.get_map_values();
    if !map.is_success() {
        return custom;
    }
    let mut new_options: Vec<(Dynamic, Dynamic)> = Vec::new();
    for (key, value) in map.value.unwrap_or_default() {
        match renamed_key(&key) {
            Some(new_key) => {
                if !new_options.iter().any(|(existing, _)| *existing == new_key) {
                    new_options.push((new_key, value));
                }
            }
            None => match new_options.iter_mut().find(|(existing, _)| *existing == key) {
                Some((_, slot)) => *slot = value,
                None => new_options.push((key, value)),
            },
        }
    }
    custom.create_map(new_options)
}

fn renamed_key(key_dynamic: &Dynamic) -> Option<Dynamic> {
    let key = key_dynamic.as_string().into_result().ok()?;
    let renamed = match key.as_str() {
        "minecraft:looking_at_block" => "minecraft:looking_at_block_state",
        "minecraft:looking_at_fluid" => "minecraft:looking_at_fluid_state",
        _ => return None,
    };
    Some(key_dynamic.create_string(renamed))
}
