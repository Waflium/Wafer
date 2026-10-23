use std::sync::Arc;

use wafer_core::fastutil::IntOpenHashSet;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, field_finder, find_field, remainder_finder, type_finder};
use crate::datafixers::types::Type;
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::add_new_choices::AddNewChoices;
use crate::util::datafix::fixes::leaves_fix::{self, Section};
use crate::util::datafix::fixes::references::{BLOCK_ENTITY, CHUNK};

const SIZE: i32 = 4096;
const SIZE_BITS: i32 = 12;

pub struct TrappedChestBlockEntityFix {
    base: DataFixBase,
}

impl TrappedChestBlockEntityFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> TrappedChestBlockEntityFix {
        TrappedChestBlockEntityFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for TrappedChestBlockEntityFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let chunk_type = self.get_output_schema().get_type(CHUNK);
        let level_type = chunk_type.find_field_type("Level").expect("a Level field");
        let Some(tile_entity_list_type) = level_type.find_field_type("TileEntities").filter(|ty| matches!(&**ty, Type::List(_))) else {
            panic!("Tile entity type is not a list type.")
        };
        let tile_entities_f = field_finder("TileEntities", tile_entity_list_type);
        let chunk_type_1 = self.get_input_schema().get_type(CHUNK);
        let level_finder = find_field(&chunk_type_1, "Level");
        let sections_finder = find_field(&level_finder.ty(), "Sections");
        let sections_type = sections_finder.ty();
        let Type::List(section_type) = &*sections_type else { panic!("Expecting sections to be a list.") };
        let section_finder = type_finder(section_type.clone());
        let input_schema = self.get_input_schema().clone();
        let tile_entity_choice_type = input_schema.find_choice_type(BLOCK_ENTITY);
        TypeRewriteRule::seq(vec![
            AddNewChoices::new(self.get_output_schema(), "AddTrappedChestFix", BLOCK_ENTITY).make_rule(),
            self.fix_type_everywhere_typed("Trapped Chest fix", chunk_type_1, move |chunk| {
                chunk.update_typed(&level_finder, None, |level| {
                    let Some(sections) = level.get_optional_typed(&sections_finder) else { return level };
                    let section_list = sections.get_all_typed(&section_finder);
                    let mut chest_locations = IntOpenHashSet::new();
                    for section in &section_list {
                        let trapped_chest_section = TrappedChestSection::new(section, &input_schema);
                        if !trapped_chest_section.is_skippable() {
                            for i in 0..SIZE {
                                let block = trapped_chest_section.get_block(i);
                                if trapped_chest_section.is_trapped_chest(block) {
                                    chest_locations.add(trapped_chest_section.get_index() << SIZE_BITS | i);
                                }
                            }
                        }
                    }
                    let level_tag = level.get_remainder();
                    let chunk_x = level_tag.get("xPos").as_int(0);
                    let chunk_z = level_tag.get("zPos").as_int(0);
                    let tile_entity_finder = type_finder(tile_entity_choice_type.clone());
                    level.update_typed(&tile_entities_f, None, |tile_entities| {
                        tile_entities.update_typed(&tile_entity_finder, None, |tile_entity| {
                            let tag = match tile_entity.get_or_create_typed(&remainder_finder()).value {
                                Value::Dynamic(tag) => tag,
                                other => panic!("Unexpected remainder {other:?}"),
                            };
                            let x = tag.get("x").as_int(0).wrapping_sub(chunk_x << 4);
                            let y = tag.get("y").as_int(0);
                            let z = tag.get("z").as_int(0).wrapping_sub(chunk_z << 4);
                            if chest_locations.contains(leaves_fix::get_index(x, y, z)) {
                                tile_entity.update(&tile_entity_finder, |value| match value {
                                    Value::Choice(key, inner) => {
                                        if key != "minecraft:chest" {
                                            warn!("Block Entity was expected to be a chest");
                                        }
                                        Value::Choice("minecraft:trapped_chest".to_string(), inner)
                                    }
                                    other => other,
                                })
                            } else {
                                tile_entity
                            }
                        })
                    })
                })
            }),
        ])
    }
}

pub struct TrappedChestSection {
    section: Section,
    chest_ids: IntOpenHashSet,
}

impl TrappedChestSection {
    pub fn new(section: &Typed, input_schema: &Schema) -> TrappedChestSection {
        let mut chest_ids = IntOpenHashSet::new();
        let section = Section::new(section, input_schema, |palette| {
            for (i, palette_tag) in palette.iter().enumerate() {
                let block_name = palette_tag.get("Name").as_string_or("");
                if block_name == "minecraft:trapped_chest" {
                    chest_ids.add(i as i32);
                }
            }
            chest_ids.is_empty()
        });
        TrappedChestSection { section, chest_ids }
    }

    pub fn is_trapped_chest(&self, block: i32) -> bool {
        self.chest_ids.contains(block)
    }

    pub fn is_skippable(&self) -> bool {
        self.section.is_skippable()
    }

    pub fn get_block(&self, pos: i32) -> i32 {
        self.section.get_block(pos)
    }

    pub fn get_index(&self) -> i32 {
        self.section.get_index()
    }
}
