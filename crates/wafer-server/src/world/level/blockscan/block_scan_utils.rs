use wafer_core::BlockPos;
use wafer_data::{BlockState, blocks};

use super::block_state_consumer::BlockStateConsumer;
use super::filtered_section_cache::FilteredSectionCache;
use crate::world::level::LevelReader;

pub fn find_blocks<L: LevelReader + ?Sized>(level: &mut L, from: BlockPos, to: BlockPos, predicate: &impl Fn(BlockState) -> bool, consumer: &mut impl BlockStateConsumer<L>) -> bool {
    let min_x = from.x.min(to.x);
    let min_y = from.y.min(to.y);
    let min_z = from.z.min(to.z);
    let max_x = from.x.max(to.x);
    let max_y = from.y.max(to.y);
    let max_z = from.z.max(to.z);
    section_based_scan(level, min_x, min_y, min_z, max_x, max_y, max_z, predicate, consumer)
}

#[allow(clippy::too_many_arguments)]
fn section_based_scan<L: LevelReader + ?Sized>(
    level: &mut L,
    min_x: i32,
    min_y: i32,
    min_z: i32,
    max_x: i32,
    max_y: i32,
    max_z: i32,
    predicate: &impl Fn(BlockState) -> bool,
    consumer: &mut impl BlockStateConsumer<L>,
) -> bool {
    let min_section_x = min_x >> 4;
    let min_section_y = level.get_min_section_y().max(min_y >> 4);
    let min_section_z = min_z >> 4;
    let max_section_x = max_x >> 4;
    let max_section_y = level.get_max_section_y().min(max_y >> 4);
    let max_section_z = max_z >> 4;
    for x in min_section_x..=max_section_x {
        for z in min_section_z..=max_section_z {
            level.get_chunk(x, z);
            for y in min_section_y..=max_section_y {
                let chunk = level.loaded_chunk(x, z);
                let section = chunk.get_section_index_from_section_y(y);
                if chunk.get_section(section).maybe_has(predicate) {
                    let section_origin_x = x << 4;
                    let section_origin_y = y << 4;
                    let section_origin_z = z << 4;
                    let from_x = section_origin_x.max(min_x) & 15;
                    let from_y = section_origin_y.max(min_y) & 15;
                    let from_z = section_origin_z.max(min_z) & 15;
                    let to_x = (section_origin_x + 15).min(max_x) & 15;
                    let to_y = (section_origin_y + 15).min(max_y) & 15;
                    let to_z = (section_origin_z + 15).min(max_z) & 15;
                    let origin = BlockPos::new(section_origin_x, section_origin_y, section_origin_z);
                    if find_blocks_in_section(level, x, z, section, origin, from_x, from_y, from_z, to_x, to_y, to_z, predicate, consumer) {
                        return true;
                    }
                }
            }
        }
    }
    let below_world = min_y < level.get_min_y();
    let above_world = max_y > level.get_max_y();
    let air = blocks::AIR.default_state();
    if (below_world || above_world) && predicate(air) {
        if below_world {
            for pos in BlockPos::between_closed(BlockPos::new(min_x, min_y, min_z), BlockPos::new(max_x, level.get_min_y() - 1, max_z)) {
                if consumer(level, pos, air).should_abort() {
                    return true;
                }
            }
        }
        if above_world {
            for pos in BlockPos::between_closed(BlockPos::new(min_x, level.get_max_y() + 1, min_z), BlockPos::new(max_x, max_y, max_z)) {
                if consumer(level, pos, air).should_abort() {
                    return true;
                }
            }
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
pub fn find_blocks_in_section<L: LevelReader + ?Sized>(
    level: &mut L,
    chunk_x: i32,
    chunk_z: i32,
    section: usize,
    origin: BlockPos,
    from_x: i32,
    from_y: i32,
    from_z: i32,
    to_x: i32,
    to_y: i32,
    to_z: i32,
    predicate: &impl Fn(BlockState) -> bool,
    consumer: &mut impl BlockStateConsumer<L>,
) -> bool {
    for y in from_y..=to_y {
        for z in from_z..=to_z {
            let mut x = from_x;
            while x <= to_x {
                let chunk_section = level.loaded_chunk(chunk_x, chunk_z).get_section(section);
                let Some(found_x) = (x..=to_x).find(|&x| predicate(chunk_section.block_state(x as usize, y as usize, z as usize))) else { break };
                let block_state = chunk_section.block_state(found_x as usize, y as usize, z as usize);
                if consumer(level, origin.offset(found_x, y, z), block_state).should_abort() {
                    return true;
                }
                x = found_x + 1;
            }
        }
    }
    false
}

pub fn find_blocks_with_cache<L: LevelReader + ?Sized>(
    level: &mut L,
    positions: impl Iterator<Item = BlockPos>,
    predicate: &impl Fn(BlockState) -> bool,
    consumer: &mut impl BlockStateConsumer<L>,
) -> bool {
    let mut section_cache = FilteredSectionCache::new(predicate);
    for pos in positions {
        if let Some(state) = section_cache.get_block_state(level, pos)
            && consumer(level, pos, state).should_abort()
        {
            return true;
        }
    }
    false
}
