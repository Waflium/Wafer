use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::find_field;
use crate::util::datafix::fixes::references::WORLD_GEN_SETTINGS;

pub struct StructureSettingsFlattenFix {
    base: DataFixBase,
}

impl StructureSettingsFlattenFix {
    pub fn new(output_schema: &Arc<Schema>) -> StructureSettingsFlattenFix {
        StructureSettingsFlattenFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for StructureSettingsFlattenFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let world_gen_settings_type = self.get_input_schema().get_type(WORLD_GEN_SETTINGS);
        let dimensions = find_field(&world_gen_settings_type, "dimensions");
        self.fix_type_everywhere_typed("StructureSettingsFlatten", world_gen_settings_type, move |input| {
            input.update_typed(&dimensions, None, |typed| {
                crate::util::write_and_read_typed_or_throw(&typed, &dimensions.ty(), |serialized| serialized.update_map_values(fix_dimension))
            })
        })
    }
}

fn fix_dimension(entry: (Dynamic, Dynamic)) -> (Dynamic, Dynamic) {
    let (key, dimension) = entry;
    (key, dimension.update("generator", |generator| generator.update("settings", |settings| settings.update("structures", fix_structures))))
}

fn fix_structures(input: Dynamic) -> Dynamic {
    let structures = input.get("structures").or_else_empty_map().update_map_values(|(key, structure)| {
        let random_spread = structure.create_string("minecraft:random_spread");
        (key, structure.set("type", random_spread))
    });
    match input.get("stronghold").result() {
        Some(stronghold) => {
            let concentric_rings = input.create_string("minecraft:concentric_rings");
            structures.set("minecraft:stronghold", stronghold.set("type", concentric_rings))
        }
        None => structures,
    }
}
