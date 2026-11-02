use wafer_core::BlockPos;
use wafer_data::{BlockState, blocks};

use crate::world::level::LevelReader;

pub struct FilteredSectionCache<P> {
    section_cache: [Option<CacheEntry>; 8],
    state_predicate: P,
}

#[derive(Clone, Copy)]
struct CacheEntry {
    chunk_section: Option<usize>,
    section_x: i32,
    section_y: i32,
    section_z: i32,
}

impl<P: Fn(BlockState) -> bool> FilteredSectionCache<P> {
    pub fn new(state_predicate: P) -> FilteredSectionCache<P> {
        FilteredSectionCache { section_cache: [None; 8], state_predicate }
    }

    pub fn get_block_state<L: LevelReader + ?Sized>(&mut self, level: &mut L, pos: BlockPos) -> Option<BlockState> {
        let section_x = pos.x >> 4;
        let section_y = pos.y >> 4;
        let section_z = pos.z >> 4;
        if section_y >= level.get_min_section_y() && section_y <= level.get_max_section_y() {
            let section = self.get_or_load(level, section_x, section_y, section_z)?;
            let relative_x = (pos.x & 15) as usize;
            let relative_y = (pos.y & 15) as usize;
            let relative_z = (pos.z & 15) as usize;
            let block_state = level.loaded_chunk(section_x, section_z).get_section(section).block_state(relative_x, relative_y, relative_z);
            (self.state_predicate)(block_state).then_some(block_state)
        } else {
            let air = blocks::AIR.default_state();
            (self.state_predicate)(air).then_some(air)
        }
    }

    fn get_or_load<L: LevelReader + ?Sized>(&mut self, level: &mut L, section_x: i32, section_y: i32, section_z: i32) -> Option<usize> {
        let hash = Self::get_hash(section_x, section_y, section_z);
        let cache_entry = match self.section_cache[hash] {
            Some(cache_entry) if cache_entry.section_x == section_x && cache_entry.section_y == section_y && cache_entry.section_z == section_z => cache_entry,
            _ => {
                let chunk = level.get_chunk(section_x, section_z);
                let section = chunk.get_section_index_from_section_y(section_y);
                let chunk_section = chunk.get_section(section).maybe_has(&self.state_predicate).then_some(section);
                let cache_entry = CacheEntry { chunk_section, section_x, section_y, section_z };
                self.section_cache[hash] = Some(cache_entry);
                cache_entry
            }
        };
        cache_entry.chunk_section
    }

    fn get_hash(section_x: i32, section_y: i32, section_z: i32) -> usize {
        let relative_x = section_x & 1;
        let relative_y = section_y & 1;
        let relative_z = section_z & 1;
        ((((relative_z << 1) + relative_y) << 1) + relative_x) as usize
    }
}
