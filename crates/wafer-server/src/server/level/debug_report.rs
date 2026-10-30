use std::io::{self, Write};

use super::ServerLevel;
use crate::util::csv_output;
use crate::world::entity::mob_category::MobCategory;

impl ServerLevel {
    pub fn write_debug_stats(&mut self, output: &mut impl Write) -> io::Result<()> {
        writeln!(output, "spawning_chunks: {}", self.chunks.distance_manager.natural_spawn_chunk_count())?;
        if let Some(last_spawn_state) = &self.chunks.last_spawn_state {
            for category in MobCategory::VALUES {
                let count = last_spawn_state.get_mob_category_counts()[category as usize];
                if count > 0 {
                    writeln!(output, "spawn_count.{}: {count}", category.name())?;
                }
            }
        }
        writeln!(output, "entities: {}", self.entity_manager.gather_stats())?;
        writeln!(output, "block_entity_tickers: {}", self.chunks.block_entity_tickers.list.len())?;
        writeln!(output, "block_ticks: {}", self.chunks.block_ticks.count())?;
        writeln!(output, "fluid_ticks: {}", self.chunks.fluid_ticks.count())?;
        writeln!(output, "distance_manager: {}", self.chunks.distance_manager.get_debug_status())?;
        writeln!(output, "pending_tasks: {}", self.chunks.pending_tasks_count())
    }

    pub fn dump_chunks(&self, output: &mut impl Write) -> io::Result<()> {
        self.chunks.dump_chunks(output, |pos| self.any_player_close_enough_for_spawning(pos))
    }

    pub fn dump_entity_sections(&self, output: &mut impl Write) -> io::Result<()> {
        self.entity_manager.dump_sections(output)
    }

    pub fn dump_block_entity_tickers(&self, output: &mut impl Write) -> io::Result<()> {
        let mut csv_output = csv_output::builder().add_column("x").add_column("y").add_column("z").add_column("type").build(output)?;
        for &(_, pos) in &self.chunks.block_entity_tickers.list {
            let ty = self.chunks.loaded_chunk(pos.chunk()).and_then(|chunk| chunk.block_entities.map.get(&pos)).map_or("<null>", |entity| entity.ty().name());
            csv_output.write_row(&[Some(pos.x.to_string()), Some(pos.y.to_string()), Some(pos.z.to_string()), Some(ty.to_string())])?;
        }
        Ok(())
    }
}
