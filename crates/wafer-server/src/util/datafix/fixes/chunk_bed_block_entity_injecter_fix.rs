use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, find_field};
use crate::datafixers::types::Type;
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::{BLOCK_ENTITY, CHUNK};

pub struct ChunkBedBlockEntityInjecterFix {
    base: DataFixBase,
}

impl ChunkBedBlockEntityInjecterFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> ChunkBedBlockEntityInjecterFix {
        ChunkBedBlockEntityInjecterFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for ChunkBedBlockEntityInjecterFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let chunk_type = self.get_output_schema().get_type(CHUNK);
        let level_finder = find_field(&chunk_type, "Level");
        let tile_entities_finder = find_field(&level_finder.ty(), "TileEntities");
        let tile_entity_list_type = tile_entities_finder.ty();
        let tile_entity_type = match &*tile_entity_list_type {
            Type::List(element) => element.clone(),
            _ => panic!("Tile entity type is not a list type."),
        };
        let bed_id = 416;
        TypeRewriteRule::seq(vec![
            self.fix_type_everywhere_to(
                "InjectBedBlockEntityType",
                self.get_input_schema().find_choice_type(BLOCK_ENTITY),
                self.get_output_schema().find_choice_type(BLOCK_ENTITY),
                |value| value,
            ),
            self.fix_type_everywhere_typed("BedBlockEntityInjecter", self.get_output_schema().get_type(CHUNK), move |input| {
                let level = input.get_typed(&level_finder);
                let level_tag = level.get_remainder();
                let chunk_x = level_tag.get("xPos").as_int(0);
                let chunk_z = level_tag.get("zPos").as_int(0);
                let mut tile_entities = match level.get_or_create_typed(&tile_entities_finder).value {
                    Value::List(tile_entities) => tile_entities,
                    other => panic!("Unexpected tile entities {other:?}"),
                };
                for section_tag in level_tag.get("Sections").as_list(|section| section) {
                    let pos = section_tag.get("Y").as_int(0);
                    for (index, block) in section_tag.get("Blocks").as_int_stream().into_iter().enumerate() {
                        if bed_id == (block & 0xFF) << 4 {
                            let p = index as i32;
                            let xx = p & 15;
                            let yy = p >> 8 & 15;
                            let zz = p >> 4 & 15;
                            let bed_tag = section_tag.create_map([
                                (section_tag.create_string("id"), section_tag.create_string("minecraft:bed")),
                                (section_tag.create_string("x"), section_tag.create_int(xx + (chunk_x << 4))),
                                (section_tag.create_string("y"), section_tag.create_int(yy + (pos << 4))),
                                (section_tag.create_string("z"), section_tag.create_int(zz + (chunk_z << 4))),
                                (section_tag.create_string("color"), section_tag.create_short(14)),
                            ]);
                            match Typed::read_typed(&tile_entity_type, bed_tag).into_result() {
                                Ok((bed, _)) => tile_entities.push(bed.value),
                                Err(_) => panic!("Could not parse newly created bed block entity."),
                            }
                        }
                    }
                }
                if !tile_entities.is_empty() {
                    input.set(&level_finder, level.set(&tile_entities_finder, Typed::new(tile_entity_list_type.clone(), Value::List(tile_entities))))
                } else {
                    input
                }
            }),
        ])
    }
}
