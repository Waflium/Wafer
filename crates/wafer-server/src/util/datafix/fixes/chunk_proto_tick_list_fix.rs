use std::cell::OnceCell;
use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, find_field, type_finder};
use crate::datafixers::types::Type;
use crate::util::datafix::fixes::chunk_height_and_biome_fix;
use crate::util::datafix::fixes::references::CHUNK;

const SECTION_WIDTH: i32 = 16;
const ALWAYS_WATERLOGGED: [&str; 5] = ["minecraft:bubble_column", "minecraft:kelp", "minecraft:kelp_plant", "minecraft:seagrass", "minecraft:tall_seagrass"];

pub struct ChunkProtoTickListFix {
    base: DataFixBase,
}

impl ChunkProtoTickListFix {
    pub fn new(output_schema: &Arc<Schema>) -> ChunkProtoTickListFix {
        ChunkProtoTickListFix { base: DataFixBase::new(output_schema, false) }
    }
}

struct LazyPalettedContainer {
    block_container: Typed,
    palette_finder: crate::datafixers::typed::OpticFinder,
    container: OnceCell<PoorMansPalettedContainer>,
}

impl LazyPalettedContainer {
    fn get(&self) -> &PoorMansPalettedContainer {
        self.container.get_or_init(|| {
            let palette = self
                .block_container
                .get_optional_typed(&self.palette_finder)
                .map(|palette| palette.write().into_result().ok().map(|written| written.as_list(|dynamic| dynamic)).unwrap_or_default())
                .unwrap_or_default();
            let data = self.block_container.get_remainder().get("data").as_long_stream();
            PoorMansPalettedContainer::new(palette, data)
        })
    }
}

impl DataFix for ChunkProtoTickListFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let chunk_type = self.get_input_schema().get_type(CHUNK);
        let level_finder = find_field(&chunk_type, "Level");
        let sections_finder = find_field(&level_finder.ty(), "Sections");
        let section_type = match &*sections_finder.ty() {
            Type::List(element) => element.clone(),
            other => panic!("Not a list type: {other:?}"),
        };
        let section_finder = type_finder(section_type);
        let block_state_container_finder = find_field(&section_finder.ty(), "block_states");
        let biome_container_finder = find_field(&section_finder.ty(), "biomes");
        let block_state_palette_finder = find_field(&block_state_container_finder.ty(), "palette");
        let tile_tick_finder = find_field(&level_finder.ty(), "TileTicks");
        self.fix_type_everywhere_typed("ChunkProtoTickListFix", chunk_type, move |chunk| {
            chunk.update_typed(&level_finder, None, |level| {
                let level = level.update_remainder(|tag| match tag.get("LiquidTicks").result() {
                    Some(liquid_ticks) => tag.set("fluid_ticks", liquid_ticks).remove("LiquidTicks"),
                    None => tag,
                });
                let chunk_tag = level.get_remainder();
                let mut lowest_y: i32 = 0;
                let mut paletted_containers: Vec<(i32, LazyPalettedContainer)> = Vec::new();
                if let Some(sections) = level.get_optional_typed(&sections_finder) {
                    for section in sections.get_all_typed(&section_finder) {
                        let section_remainder = section.get_remainder();
                        let section_y = section_remainder.get("Y").as_int(i32::MAX);
                        if section_y != i32::MAX {
                            if section.get_optional_typed(&biome_container_finder).is_some() {
                                lowest_y = section_y.min(lowest_y);
                            }
                            if let Some(block_container) = section.get_optional_typed(&block_state_container_finder) {
                                let container = LazyPalettedContainer { block_container, palette_finder: block_state_palette_finder.clone(), container: OnceCell::new() };
                                match paletted_containers.iter_mut().find(|(key, _)| *key == section_y) {
                                    Some((_, slot)) => *slot = container,
                                    None => paletted_containers.push((section_y, container)),
                                }
                            }
                        }
                    }
                }
                let section_min_y = lowest_y as i8;
                let level = level.update_remainder(|remainder| remainder.update("yPos", |y| y.create_byte(section_min_y)));
                if level.get_optional_typed(&tile_tick_finder).is_none() && chunk_tag.get("fluid_ticks").result().is_none() {
                    let section_x = chunk_tag.get("xPos").as_int(0);
                    let section_z = chunk_tag.get("zPos").as_int(0);
                    let fluid_ticks = make_tick_list(&chunk_tag, &paletted_containers, section_min_y, section_x, section_z, "LiquidsToBeTicked", get_liquid);
                    let block_ticks = make_tick_list(&chunk_tag, &paletted_containers, section_min_y, section_x, section_z, "ToBeTicked", get_block);
                    let mut level = level;
                    if let Ok((parsed_block_ticks, _)) = Typed::read_typed(&tile_tick_finder.ty(), block_ticks).into_result() {
                        level = level.set(&tile_tick_finder, parsed_block_ticks);
                    }
                    let mut fluid_ticks = Some(fluid_ticks);
                    level.update_remainder(|remainder| remainder.remove("ToBeTicked").remove("LiquidsToBeTicked").set("fluid_ticks", fluid_ticks.take().expect("a single remainder")))
                } else {
                    level
                }
            })
        })
    }
}

fn make_tick_list(
    tag: &Dynamic,
    paletted_containers: &[(i32, LazyPalettedContainer)],
    section_min_y: i8,
    section_x: i32,
    section_z: i32,
    proto_tick_list_tag: &str,
    type_getter: fn(Option<&Dynamic>) -> String,
) -> Dynamic {
    let ticks_per_section = tag.get(proto_tick_list_tag).as_list(|dynamic| dynamic);
    let mut new_tick_list = Vec::new();
    for (section_y_index, ticks) in ticks_per_section.iter().enumerate() {
        let section_y = section_y_index as i32 + section_min_y as i32;
        let container = paletted_containers.iter().find(|(key, _)| *key == section_y).map(|(_, container)| container);
        for pos in ticks.as_stream().into_iter().map(|pos| pos.as_short(-1) as i32).filter(|pos| *pos > 0) {
            new_tick_list.push(create_tick(tag, container, section_x, section_y, section_z, pos, type_getter));
        }
    }
    tag.create_list(new_tick_list)
}

fn get_block(block_state: Option<&Dynamic>) -> String {
    match block_state {
        Some(block_state) => block_state.get("Name").as_string_or("minecraft:air"),
        None => "minecraft:air".to_string(),
    }
}

fn get_liquid(block_state: Option<&Dynamic>) -> String {
    let Some(block_state) = block_state else { return "minecraft:empty".to_string() };
    let block = block_state.get("Name").as_string_or("");
    if block == "minecraft:water" {
        if block_state.get("Properties").get("level").as_int(0) == 0 { "minecraft:water" } else { "minecraft:flowing_water" }.to_string()
    } else if block == "minecraft:lava" {
        if block_state.get("Properties").get("level").as_int(0) == 0 { "minecraft:lava" } else { "minecraft:flowing_lava" }.to_string()
    } else if !ALWAYS_WATERLOGGED.contains(&block.as_str()) && !block_state.get("Properties").get("waterlogged").as_boolean_or(false) {
        "minecraft:empty".to_string()
    } else {
        "minecraft:water".to_string()
    }
}

fn create_tick(
    tag: &Dynamic,
    container: Option<&LazyPalettedContainer>,
    section_x: i32,
    section_y: i32,
    section_z: i32,
    pos: i32,
    type_getter: fn(Option<&Dynamic>) -> String,
) -> Dynamic {
    let relative_x = pos & 15;
    let relative_y = ((pos as u32) >> 4) as i32 & 15;
    let relative_z = ((pos as u32) >> 8) as i32 & 15;
    let ty = type_getter(container.and_then(|container| container.get().get(relative_x, relative_y, relative_z)));
    let entries = [
        ("i", tag.create_string(ty)),
        ("x", tag.create_int(section_x.wrapping_mul(SECTION_WIDTH).wrapping_add(relative_x))),
        ("y", tag.create_int(section_y.wrapping_mul(SECTION_WIDTH).wrapping_add(relative_y))),
        ("z", tag.create_int(section_z.wrapping_mul(SECTION_WIDTH).wrapping_add(relative_z))),
        ("t", tag.create_int(0)),
        ("p", tag.create_int(0)),
    ];
    tag.create_map(entries.into_iter().map(|(key, value)| (tag.create_string(key), value)))
}

pub struct PoorMansPalettedContainer {
    palette: Vec<Dynamic>,
    data: Vec<i64>,
    bits: i32,
    mask: i64,
    values_per_long: i32,
}

impl PoorMansPalettedContainer {
    pub fn new(palette: Vec<Dynamic>, data: Vec<i64>) -> PoorMansPalettedContainer {
        let bits = 4.max(chunk_height_and_biome_fix::ceillog2(palette.len() as i32));
        PoorMansPalettedContainer { palette, data, bits, mask: (1i64 << bits) - 1, values_per_long: (64 / bits) as u16 as i32 }
    }

    pub fn get(&self, x: i32, y: i32, z: i32) -> Option<&Dynamic> {
        let entry_count = self.palette.len();
        if entry_count < 1 {
            None
        } else if entry_count == 1 {
            self.palette.first()
        } else {
            let index = self.get_index(x, y, z);
            let cell_index = index / self.values_per_long;
            if cell_index >= 0 && (cell_index as usize) < self.data.len() {
                let cell_value = self.data[cell_index as usize];
                let bit_index = (index - cell_index * self.values_per_long) * self.bits;
                let palette_index = (cell_value >> bit_index & self.mask) as i32;
                if palette_index >= 0 && (palette_index as usize) < entry_count { self.palette.get(palette_index as usize) } else { None }
            } else {
                None
            }
        }
    }

    fn get_index(&self, x: i32, y: i32, z: i32) -> i32 {
        (y << 4 | z) << 4 | x
    }
}
