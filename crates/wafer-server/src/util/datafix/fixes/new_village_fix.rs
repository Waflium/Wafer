use std::sync::Arc;

use wafer_protocol::nbt::Nbt;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{find_field, type_finder};
use crate::datafixers::types::{compound_list, const_type};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::{CHUNK, STRUCTURE_FEATURE};
use crate::util::datafix::schemas::namespaced_schema::ensure_namespaced;

pub struct NewVillageFix {
    base: DataFixBase,
}

impl NewVillageFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> NewVillageFix {
        NewVillageFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for NewVillageFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let starts_type = compound_list(const_type(ConstType::String), self.get_input_schema().get_type(STRUCTURE_FEATURE));
        let chunk_type = self.get_input_schema().get_type(CHUNK);
        let structure_type = self.get_input_schema().get_type(STRUCTURE_FEATURE);
        let level_finder = find_field(&chunk_type, "Level");
        let structures_finder = find_field(&level_finder.ty(), "Structures");
        let starts_finder = find_field(&structures_finder.ty(), "Starts");
        let list_finder = type_finder(starts_type);
        TypeRewriteRule::seq(vec![
            self.fix_type_everywhere_typed("NewVillageFix", chunk_type, move |input| {
                input.update_typed(&level_finder, None, |level| {
                    level.update_typed(&structures_finder, None, |structures| {
                        structures
                            .update_typed(&starts_finder, None, |starts| {
                                starts.update(&list_finder, |list| match list {
                                    Value::CompoundList(entries) => Value::CompoundList(
                                        entries
                                            .into_iter()
                                            .filter(|(name, _)| name.as_str() != Some("Village"))
                                            .map(|(name, value)| match name.as_str() {
                                                Some("New_Village") => (Value::Prim(Nbt::String("Village".to_string())), value),
                                                _ => (name, value),
                                            })
                                            .collect(),
                                    ),
                                    other => other,
                                })
                            })
                            .update_remainder(|tag| {
                                tag.update("References", |references| {
                                    let fixed = match references.get("New_Village").result() {
                                        Some(village) => references.remove("New_Village").set("Village", village),
                                        None => references,
                                    };
                                    fixed.remove("Village")
                                })
                            })
                    })
                })
            }),
            self.fix_type_everywhere_typed("NewVillageStartFix", structure_type, |input| {
                input.update_remainder(|tag| {
                    tag.update("id", |id| if ensure_namespaced(&id.as_string_or("")) == "minecraft:new_village" { id.create_string("minecraft:village") } else { id })
                })
            }),
        ])
    }
}
