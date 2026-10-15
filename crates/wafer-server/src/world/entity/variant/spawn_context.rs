use wafer_core::BlockPos;

use crate::world::level::chunk::Biome;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

pub struct SpawnContext<'a, L: ServerLevelAccessor + ?Sized> {
    pub pos: BlockPos,
    pub level: &'a mut L,
    pub biome: Biome,
}

impl<'a, L: ServerLevelAccessor + ?Sized> SpawnContext<'a, L> {
    pub fn create(level: &'a mut L, pos: BlockPos) -> SpawnContext<'a, L> {
        let biome = level.get_biome(pos);
        SpawnContext { pos, level, biome }
    }
}
