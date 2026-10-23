use std::sync::Arc;

use wafer_protocol::nbt::Nbt;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::{Dynamic, put};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, find_field, type_finder};
use crate::datafixers::types::{field, named, optional};
use crate::util::datafix::fixes::references::CHUNK;

pub struct ChunkRenamesFix {
    base: DataFixBase,
}

impl ChunkRenamesFix {
    pub fn new(output_schema: &Arc<Schema>) -> ChunkRenamesFix {
        ChunkRenamesFix { base: DataFixBase::new(output_schema, true) }
    }
}

impl DataFix for ChunkRenamesFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let chunk_type = self.get_input_schema().get_type(CHUNK);
        let level_finder = find_field(&chunk_type, "Level");
        let structure_finder = find_field(&level_finder.ty(), "Structures");
        let new_chunk_type = self.get_output_schema().get_type(CHUNK);
        let new_structures_type = new_chunk_type.find_field_type("structures").expect("a structures field");
        self.fix_type_everywhere_typed_to("Chunk Renames; purge Level-tag", chunk_type, new_chunk_type, move |chunk| {
            let level = chunk.get_typed(&level_finder);
            let level_remainder = level.get_remainder();
            let chunk_typed = append_chunk_name(level);
            let chunk_typed = chunk_typed.set_remainder(merge_remainders(&chunk, level_remainder));
            let chunk_typed = rename_field(chunk_typed, "TileEntities", "block_entities");
            let chunk_typed = rename_field(chunk_typed, "TileTicks", "block_ticks");
            let chunk_typed = rename_field(chunk_typed, "Entities", "entities");
            let chunk_typed = rename_field(chunk_typed, "Sections", "sections");
            let chunk_typed = chunk_typed.update_typed(&structure_finder, Some(&new_structures_type), |structure| rename_field(structure, "Starts", "starts"));
            let chunk_typed = rename_field(chunk_typed, "Structures", "structures");
            chunk_typed.update_remainder(|remainder| remainder.remove("Level"))
        })
    }
}

fn rename_field(input: Typed, old_name: &str, new_name: &str) -> Typed {
    let field_type = input.ty.find_field_type(old_name).unwrap_or_else(|| panic!("Field not found: {old_name}"));
    let old_type = optional(field(old_name, field_type.clone()));
    let new_type = optional(field(new_name, field_type));
    input.update_to(&type_finder(old_type), &new_type, |value| value).update_remainder(|tag| tag.remove(old_name))
}

fn append_chunk_name(input: Typed) -> Typed {
    Typed::new(named("chunk", input.ty), input.value)
}

fn merge_remainders(chunk: &Typed, level_remainder: Dynamic) -> Dynamic {
    let chunk_remainder = chunk.get_remainder();
    let Some(Nbt::Compound(level_entries)) = &level_remainder.value else { return level_remainder };
    match chunk_remainder.value {
        None => Dynamic::new(Nbt::Compound(level_entries.clone())),
        Some(Nbt::Compound(mut entries)) => {
            for (key, value) in level_entries {
                put(&mut entries, key, value.clone());
            }
            Dynamic::new(Nbt::Compound(entries))
        }
        Some(_) => level_remainder,
    }
}
