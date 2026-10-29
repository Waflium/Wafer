use std::sync::Arc;

use wafer_core::{BlockPos, ChunkPos};

use crate::server::level::ServerLevel;
use crate::server::level::chunk_level::ChunkStatus;
use crate::world::level::levelgen::structure::structure::StructureHolder;
use crate::world::level::levelgen::structure::structure_start::StructureStart;

impl ServerLevel {
    pub fn get_structure_at(&mut self, block_pos: BlockPos, structure: StructureHolder) -> Option<Arc<StructureStart>> {
        let chunk_pos = ChunkPos::new(block_pos.x >> 4, block_pos.z >> 4);
        self.get_chunk_structures_at(chunk_pos, ChunkStatus::StructureReferences);
        self.chunks.starts_for_structure(chunk_pos, structure).into_iter().find(|start| start.get_bounding_box().is_inside(block_pos))
    }

    pub fn get_structure_with_piece_at(&mut self, pos: BlockPos, predicate: &dyn Fn(StructureHolder) -> bool) -> Option<Arc<StructureStart>> {
        let chunk_pos = ChunkPos::new(pos.x >> 4, pos.z >> 4);
        self.get_chunk_structures_at(chunk_pos, ChunkStatus::StructureReferences);
        self.chunks.starts_for_structure_matching(chunk_pos, predicate).into_iter().find(|start| self.structure_has_piece_at(pos, start))
    }

    pub fn structure_has_piece_at(&self, block_pos: BlockPos, structure_start: &StructureStart) -> bool {
        structure_start.piece_container.lock().unwrap().is_inside_piece(block_pos)
    }

    pub fn get_all_structures_at(&mut self, pos: BlockPos) -> Vec<StructureHolder> {
        let chunk_pos = ChunkPos::new(pos.x >> 4, pos.z >> 4);
        self.get_chunk_structures_at(chunk_pos, ChunkStatus::StructureReferences).references.keys().copied().collect()
    }

    pub fn fill_starts_for_structure(&mut self, pos: BlockPos, structure: StructureHolder, consumer: &mut impl FnMut(&StructureStart)) {
        let chunk_pos = ChunkPos::new(pos.x >> 4, pos.z >> 4);
        for start in self.chunks.starts_for_structure(chunk_pos, structure) {
            consumer(&start);
        }
    }
}
