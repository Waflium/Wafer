use std::collections::HashMap;
use std::fmt::Write;

use wafer_core::{BlockPos, ChunkPos, Direction};
use wafer_data::{BlockState, blocks};

use super::block_matcher::BlockMatcher;
use crate::util::continuation::Continuation;
use crate::world::level::block::entity::BlockEntity;
use crate::world::level::chunk::{Biome, LevelChunk};
use crate::world::level::light_layer::LightLayer;
use crate::world::level::{BlockGetter, LevelReader};

const MIN_Y: i32 = -64;
const HEIGHT: i32 = 384;

struct PatternLevel {
    chunks: HashMap<(i32, i32), LevelChunk>,
    out: String,
}

fn pattern(x: i32, y: i32, z: i32) -> BlockState {
    let band = (-64..=-50).contains(&y) || (0..=10).contains(&y) || (310..=319).contains(&y);
    if !band {
        return blocks::AIR.default_state();
    }
    match (x * 7 + y * 13 + z * 29).rem_euclid(11) {
        0..=2 if (x >> 4).rem_euclid(2) == 1 && (0..=10).contains(&y) => blocks::DIRT.default_state(),
        0..=2 => blocks::STONE.default_state(),
        3 => blocks::WATER.default_state(),
        4 => blocks::VINE.default_state(),
        _ => blocks::AIR.default_state(),
    }
}

impl PatternLevel {
    fn new() -> PatternLevel {
        PatternLevel { chunks: HashMap::new(), out: String::new() }
    }

    fn header(&mut self, name: &str) {
        writeln!(self.out, "# {name}").unwrap();
    }

    fn line(&mut self, line: String) {
        self.out.push_str(&line);
        self.out.push('\n');
    }
}

fn found(level: &mut PatternLevel, pos: BlockPos, state: BlockState) {
    level.line(format!("found {} {} {} {}", pos.x, pos.y, pos.z, state.block().name().trim_start_matches("minecraft:")));
}

fn pos(pos: Option<BlockPos>) -> String {
    pos.map_or("none".to_string(), |p| format!("{} {} {}", p.x, p.y, p.z))
}

impl BlockGetter for PatternLevel {
    fn block_state(&mut self, _pos: BlockPos) -> BlockState {
        unreachable!()
    }

    fn get_min_y(&self) -> i32 {
        MIN_Y
    }
}

impl LevelReader for PatternLevel {
    fn raw_brightness(&mut self, _pos: BlockPos, _darkening: i32) -> i32 {
        unreachable!()
    }

    fn get_brightness(&mut self, _layer: LightLayer, _pos: BlockPos) -> i32 {
        unreachable!()
    }

    fn get_sea_level(&self) -> i32 {
        63
    }

    fn is_inside_build_height(&self, y: i32) -> bool {
        (MIN_Y..MIN_Y + HEIGHT).contains(&y)
    }

    fn is_fast_lava(&self) -> bool {
        false
    }

    fn get_control_input_signal(&mut self, _pos: BlockPos, _direction: Direction, _only_diodes: bool) -> i32 {
        unreachable!()
    }

    fn get_block_entity(&mut self, _pos: BlockPos) -> Option<&mut BlockEntity> {
        None
    }

    fn get_height_of_level(&self) -> i32 {
        HEIGHT
    }

    fn get_chunk(&mut self, chunk_x: i32, chunk_z: i32) -> &LevelChunk {
        writeln!(self.out, "getChunk {chunk_x} {chunk_z}").unwrap();
        self.chunks.entry((chunk_x, chunk_z)).or_insert_with(|| {
            let dimension = &wafer_data::DIMENSION_TYPES[0];
            let mut chunk = LevelChunk::empty(ChunkPos::new(chunk_x, chunk_z), dimension, Biome::by_name("minecraft:plains").expect("plains"));
            for (index, section) in chunk.sections.iter_mut().enumerate() {
                let section_y = (MIN_Y >> 4) + index as i32;
                for y in 0..16 {
                    for z in 0..16 {
                        for x in 0..16 {
                            let state = pattern((chunk_x << 4) + x, (section_y << 4) + y, (chunk_z << 4) + z);
                            if !state.is_air() {
                                section.set_block_state(x as usize, y as usize, z as usize, state);
                            }
                        }
                    }
                }
            }
            chunk
        })
    }

    fn loaded_chunk(&self, chunk_x: i32, chunk_z: i32) -> &LevelChunk {
        &self.chunks[&(chunk_x, chunk_z)]
    }
}

fn is(block: wafer_data::Block) -> impl Fn(BlockState) -> bool {
    move |state| state.is_block(block)
}

#[test]
fn block_scans_match_vanilla() {
    let level = &mut PatternLevel::new();

    level.header("box stone for_each");
    level.find_blocks_in(BlockPos::new(-20, -70, -5), BlockPos::new(20, -50, 18)).filter_state(is(blocks::STONE)).for_each(found);

    level.header("box air below world for_each");
    level.find_blocks_in(BlockPos::new(17, -60, 17), BlockPos::new(-3, -68, 14)).filter_state(|state| state.is_air()).for_each(found);

    level.header("box air above world for_each_until");
    let mut count = 0;
    let result = level.find_blocks_in(BlockPos::new(5, 315, -2), BlockPos::new(7, 322, 1)).filter_state(|state| state.is_air()).for_each_until(|level, pos, state| {
        found(level, pos, state);
        count += 1;
        Continuation::abort_if(count >= 40)
    });
    level.line(format!("result {result}"));

    level.header("box fully below world for_each");
    level.find_blocks_in(BlockPos::new(0, -80, 0), BlockPos::new(1, -79, 1)).for_each(found);

    level.header("box terminal results");
    let (from, to) = (BlockPos::new(-6, -2, 9), BlockPos::new(6, 4, 21));
    let result = level.find_blocks_in(from, to).filter_state(is(blocks::STONE)).any_matched();
    level.line(format!("any stone {result}"));
    let result = level.find_blocks_in(from, to).filter_state(is(blocks::LAVA)).any_matched();
    level.line(format!("any lava {result}"));
    let result = level.find_blocks_in(from, to).filter_state(is(blocks::LAVA)).none_matched();
    level.line(format!("none lava {result}"));
    let result = level.find_blocks_in(from, to).filter_state(|state| state.is_air()).all_matched();
    level.line(format!("all air {result}"));
    let result = level.find_blocks_in(from, to).filter_state(|state| !state.is_block(blocks::LAVA)).all_matched();
    level.line(format!("all not lava {result}"));
    let result = level.find_blocks_in(from, to).filter_state(is(blocks::VINE)).at_most_matched(4);
    level.line(format!("at most 4 vine {result}"));
    let result = level.find_blocks_in(from, to).filter_state(is(blocks::VINE)).at_most_matched(400);
    level.line(format!("at most 400 vine {result}"));
    let result = level.find_blocks_in(from, to).filter_state(is(blocks::WATER)).at_least_matched(3);
    level.line(format!("at least 3 water {result}"));
    let result = level.find_blocks_in(from, to).filter_state(is(blocks::WATER)).at_least_matched(9999);
    level.line(format!("at least 9999 water {result}"));
    let result = level.find_blocks_in(BlockPos::new(0, 320, 0), BlockPos::new(3, 330, 3)).filter_state(|state| state.is_air()).all_matched();
    level.line(format!("all air high {result}"));
    let result = level.find_blocks_in(BlockPos::new(30, 20, 30), BlockPos::new(33, 9, 33)).filter_state(|state| !state.fluid_state().is_empty()).any_matched();
    level.line(format!("liquid {result}"));

    level.header("ordered water box manhattan for_each");
    level.find_blocks_in_box_by_manhattan_distance(BlockPos::new(15, 0, 15), 5, 1).filter_state(is(blocks::WATER)).for_each(found);

    level.header("ordered stone box manhattan find_first");
    let result = level.find_blocks_in_box_by_manhattan_distance(BlockPos::new(-1, -63, 0), 8, 4).filter_state(is(blocks::STONE)).find_first();
    level.line(format!("result {}", pos(result)));

    level.header("ordered manhattan air top for_each");
    let top = BlockPos::new(31, 318, -1);
    level.find_blocks_in_manhattan(top, 4).filter_pos(|pos| *pos != top).filter_state(|state| state.is_air()).for_each(found);

    level.header("ordered find_first matching");
    let origin = BlockPos::new(16, 5, -17);
    let mut seen = 0;
    let result = level.find_blocks_in_box_by_manhattan_distance(origin, 6, 6).filter_pos(|pos| pos.differs_horizontally(origin)).filter_state(is(blocks::VINE)).find_first_matching(|level, pos, state| {
        found(level, pos, state);
        seen += 1;
        seen == 7
    });
    level.line(format!("result {}", pos(result)));

    level.header("ordered terminal results");
    let result = level.find_blocks_in_box_by_manhattan_distance(BlockPos::new(40, 100, 40), 3, 3).filter_state(|state| state.is_air()).all_matched();
    level.line(format!("all air {result}"));
    let result = level.find_blocks_in_box_by_manhattan_distance(BlockPos::new(40, -60, 40), 3, 3).filter_state(|state| state.is_air()).all_matched();
    level.line(format!("all air low {result}"));
    let result = level.find_blocks_in_box_by_manhattan_distance(BlockPos::new(-30, 3, 2), 3, 1).filter_state(is(blocks::STONE)).at_most_matched(2);
    level.line(format!("at most 2 stone {result}"));
    let result = level.find_blocks_in_manhattan(BlockPos::new(7, -64, 7), 3).filter_state(is(blocks::LAVA)).none_matched();
    level.line(format!("none lava {result}"));
    let result = level.find_blocks_in_manhattan(BlockPos::new(-7, 5, 33), 4).filter_state(is(blocks::WATER)).for_each_until(|level, pos, state| {
        found(level, pos, state);
        Continuation::abort_if(pos.y > 5)
    });
    level.line(format!("until {result}"));

    let expected = include_str!("fixtures/block_scan_oracle.txt");
    for (index, (actual, expected)) in level.out.lines().zip(expected.lines()).enumerate() {
        assert_eq!(actual, expected, "line {}", index + 1);
    }
    assert_eq!(level.out.lines().count(), expected.lines().count());
}

#[test]
fn consumers_see_blocks_changed_during_the_scan() {
    let level = &mut PatternLevel::new();
    level.get_chunk(0, 0);
    let section = level.chunks.get_mut(&(0, 0)).unwrap().sections.get_mut(5).unwrap();
    for x in 0..16 {
        section.set_block_state(x, 0, 0, blocks::GLASS.default_state());
    }
    let mut seen = Vec::new();
    level.find_blocks_in(BlockPos::new(0, 16, 0), BlockPos::new(15, 16, 0)).filter_state(is(blocks::GLASS)).for_each(|level, pos, _| {
        seen.push(pos.x);
        if pos.x == 3 {
            level.chunks.get_mut(&(0, 0)).unwrap().sections[5].set_block_state(4, 0, 0, blocks::AIR.default_state());
        }
    });
    assert_eq!(seen, [0, 1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
}
