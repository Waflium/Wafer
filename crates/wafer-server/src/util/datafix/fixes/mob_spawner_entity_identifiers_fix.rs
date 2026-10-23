use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::util::datafix::fixes::references::UNTAGGED_SPAWNER;

pub struct MobSpawnerEntityIdentifiersFix {
    base: DataFixBase,
}

impl MobSpawnerEntityIdentifiersFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> MobSpawnerEntityIdentifiersFix {
        MobSpawnerEntityIdentifiersFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

fn fix(mut input: Dynamic) -> Dynamic {
    if input.get("id").as_string_or("") != "MobSpawner" {
        return input;
    }
    if let Ok(entity_id) = input.get("EntityId").as_string().into_result() {
        let spawn_data = input.get("SpawnData").result().unwrap_or_else(|| input.empty_map());
        let id = spawn_data.create_string(if entity_id.is_empty() { "Pig".to_string() } else { entity_id });
        let spawn_data = spawn_data.set("id", id);
        input = input.set("SpawnData", spawn_data);
        input = input.remove("EntityId");
    }
    if let Ok(spawn_potentials) = input.get("SpawnPotentials").as_stream_opt().into_result() {
        let potentials = input.create_list(spawn_potentials.into_iter().map(|spawn_potential| match spawn_potential.get("Type").as_string().into_result() {
            Ok(ty) => {
                let spawn_data = spawn_potential.get("Properties").result().unwrap_or_else(|| spawn_potential.empty_map());
                let id = spawn_potential.create_string(ty);
                let spawn_data = spawn_data.set("id", id);
                spawn_potential.set("Entity", spawn_data).remove("Type").remove("Properties")
            }
            Err(_) => spawn_potential,
        }));
        input = input.set("SpawnPotentials", potentials);
    }
    input
}

impl DataFix for MobSpawnerEntityIdentifiersFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let new_type = self.get_output_schema().get_type(UNTAGGED_SPAWNER);
        let read_type = new_type.clone();
        self.fix_type_everywhere_typed_to("MobSpawnerEntityIdentifiersFix", self.get_input_schema().get_type(UNTAGGED_SPAWNER), new_type, move |input| {
            let tag = input.get_remainder();
            let id = tag.create_string("MobSpawner");
            let tag = tag.set("id", id);
            let fixed = Typed::read_typed(&read_type, fix(tag));
            if fixed.is_success() { fixed.value.expect("a value").0 } else { input }
        })
    }
}
