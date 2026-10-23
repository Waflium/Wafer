use std::sync::LazyLock;

use wafer_core::{BlockPos, ChunkPos, SectionPos};
use wafer_data::{BlockState, DimensionType, blocks};

use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::chunk::{Biome, HeightmapType, LevelChunk};
use crate::world::level::noise_column::NoiseColumn;

pub const HEIGHT: i32 = 70;
pub const BARRIER_HEIGHT: i32 = 60;

static GRID_WIDTH: LazyLock<i32> = LazyLock::new(|| ((BlockState::count() as f32).sqrt()).ceil() as i32);
static GRID_HEIGHT: LazyLock<i32> = LazyLock::new(|| (BlockState::count() as f32 / *GRID_WIDTH as f32).ceil() as i32);

#[derive(Clone, Copy, Default)]
pub struct DebugLevelSource;

impl DebugLevelSource {
    pub fn biome(&self) -> Biome {
        Biome::by_name("minecraft:plains").expect("plains biome")
    }

    pub fn generate(&self, pos: ChunkPos, dimension: &DimensionType) -> LevelChunk {
        let mut chunk = LevelChunk::empty(pos, dimension, self.biome());
        chunk.prime_heightmaps();
        chunk
    }

    pub fn apply_biome_decoration(&self, level: &mut WorldGenRegion) {
        let center_pos = level.get_center();
        for x in 0..16 {
            for z in 0..16 {
                let world_x = SectionPos::section_to_block_coord(center_pos.x) + x;
                let world_z = SectionPos::section_to_block_coord(center_pos.z) + z;
                level.set_block(BlockPos::new(world_x, BARRIER_HEIGHT, world_z), blocks::BARRIER.default_state(), 2);
                let state = get_block_state_for(world_x, world_z);
                level.set_block(BlockPos::new(world_x, HEIGHT, world_z), state, 2);
            }
        }
    }

    pub fn get_base_height(&self, _kind: HeightmapType) -> i32 {
        0
    }

    pub fn get_base_column(&self) -> NoiseColumn {
        NoiseColumn::new(0, Vec::new())
    }

    pub fn min_y(&self) -> i32 {
        0
    }

    pub fn gen_depth(&self) -> i32 {
        384
    }

    pub fn sea_level(&self) -> i32 {
        63
    }
}

pub fn get_block_state_for(mut world_x: i32, mut world_z: i32) -> BlockState {
    let mut state = blocks::AIR.default_state();
    if world_x > 0 && world_z > 0 && world_x % 2 != 0 && world_z % 2 != 0 {
        world_x /= 2;
        world_z /= 2;
        if world_x <= *GRID_WIDTH && world_z <= *GRID_HEIGHT {
            let index = (world_x * *GRID_WIDTH + world_z).abs();
            if let Some(found) = BlockState::by_id(index as u32) {
                state = found;
            }
        }
    }
    state
}

pub fn get_block_state(pos: BlockPos) -> BlockState {
    match pos.y {
        BARRIER_HEIGHT => blocks::BARRIER.default_state(),
        HEIGHT => get_block_state_for(pos.x, pos.z),
        _ => blocks::AIR.default_state(),
    }
}
