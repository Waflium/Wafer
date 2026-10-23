use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::CHUNK;

const NUM_SECTIONS: usize = 16;

pub struct ChunkToProtochunkFix {
    base: DataFixBase,
}

impl ChunkToProtochunkFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> ChunkToProtochunkFix {
        ChunkToProtochunkFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for ChunkToProtochunkFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.write_fix_and_read("ChunkToProtoChunkFix", self.get_input_schema().get_type(CHUNK), self.get_output_schema().get_type(CHUNK), |chunk| {
            chunk.update("Level", fix_chunk_data)
        })
    }
}

fn fix_chunk_data(tag: Dynamic) -> Dynamic {
    let terrain_populated = tag.get("TerrainPopulated").as_boolean_or(false);
    let light_populated = tag.get("LightPopulated").as_number().into_result().is_err() || tag.get("LightPopulated").as_boolean_or(false);
    let status = if terrain_populated {
        if light_populated { "mobs_spawned" } else { "decorated" }
    } else {
        "carved"
    };
    let status = tag.create_string(status);
    let has_legacy_structure_data = tag.create_boolean(true);
    repack_ticks(repack_biomes(tag)).set("Status", status).set("hasLegacyStructureData", has_legacy_structure_data)
}

fn repack_biomes(tag: Dynamic) -> Dynamic {
    tag.update("Biomes", |biomes| match biomes.as_byte_buffer_opt().into_result() {
        Ok(buffer) => {
            let mut new_biomes = [0i32; 256];
            for (i, new_biome) in new_biomes.iter_mut().enumerate() {
                if i < buffer.len() {
                    *new_biome = buffer[i] as i32 & 255;
                }
            }
            biomes.create_int_list(new_biomes)
        }
        Err(_) => biomes,
    })
}

fn repack_ticks(tag: Dynamic) -> Dynamic {
    match tag.get("TileTicks").as_stream_opt().into_result() {
        Ok(ticks) => {
            let mut to_be_ticked_tag: Vec<Vec<i16>> = vec![Vec::new(); NUM_SECTIONS];
            for pending_tick_tag in ticks {
                let x = pending_tick_tag.get("x").as_int(0);
                let y = pending_tick_tag.get("y").as_int(0);
                let z = pending_tick_tag.get("z").as_int(0);
                let packed_offset = pack_offset_coordinates(x, y, z);
                let section = y >> 4;
                if section < 0 || section >= NUM_SECTIONS as i32 {
                    panic!("Index {section} out of bounds for length {NUM_SECTIONS}");
                }
                to_be_ticked_tag[section as usize].push(packed_offset);
            }
            let sections: Vec<Dynamic> = to_be_ticked_tag.into_iter().map(|list| tag.create_list(list.into_iter().map(|value| tag.create_short(value)))).collect();
            let to_be_ticked = tag.create_list(sections);
            tag.remove("TileTicks").set("ToBeTicked", to_be_ticked)
        }
        Err(_) => tag,
    }
}

fn pack_offset_coordinates(x: i32, y: i32, z: i32) -> i16 {
    (x & 15 | (y & 15) << 4 | (z & 15) << 8) as i16
}
