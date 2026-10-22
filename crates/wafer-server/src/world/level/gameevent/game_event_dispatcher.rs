use wafer_core::{BlockPos, ChunkPos, SectionPos, Vec3};

use super::euclidean_game_event_listener_registry::get_postable_listener_position;
use super::game_event::{Context, GameEvent};
use super::game_event_listener::{DeliveryMode, GameEventListener};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

impl ServerLevel {
    pub fn game_event(&mut self, game_event: GameEvent, position: Vec3, context: Context) {
        let radius = game_event.notification_radius();
        let center = BlockPos::containing_vec(position);
        let section_min_x = SectionPos::block_to_section_coord(center.x - radius);
        let section_min_y = SectionPos::block_to_section_coord(center.y - radius);
        let section_min_z = SectionPos::block_to_section_coord(center.z - radius);
        let section_max_x = SectionPos::block_to_section_coord(center.x + radius);
        let section_max_y = SectionPos::block_to_section_coord(center.y + radius);
        let section_max_z = SectionPos::block_to_section_coord(center.z + radius);
        let mut to_handle_by_distance: Vec<(f64, GameEventListener)> = Vec::new();
        for chunk_x in section_min_x..=section_max_x {
            for chunk_z in section_min_z..=section_max_z {
                let chunk_pos = ChunkPos::new(chunk_x, chunk_z);
                if self.chunks.loaded_chunk(chunk_pos).is_none() {
                    continue;
                }
                for section in section_min_y..=section_max_y {
                    self.visit_in_range_listeners(chunk_pos, section, game_event, position, context, &mut to_handle_by_distance);
                }
            }
        }
        if !to_handle_by_distance.is_empty() {
            to_handle_by_distance.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (_, listener) in to_handle_by_distance {
                self.handle_game_event(listener, game_event, context, position);
            }
        }
    }

    pub fn game_event_from_entity(&mut self, source_entity: Option<EntityId>, game_event: GameEvent, position: Vec3) {
        self.game_event(game_event, position, Context::of(source_entity));
    }

    pub fn game_event_at(&mut self, source_entity: Option<EntityId>, game_event: GameEvent, pos: BlockPos) {
        self.game_event(game_event, Vec3::at_center_of(pos), Context::of(source_entity));
    }

    pub fn game_event_at_with(&mut self, game_event: GameEvent, pos: BlockPos, context: Context) {
        self.game_event(game_event, Vec3::at_center_of(pos), context);
    }

    pub fn entity_game_event(&mut self, id: EntityId, game_event: GameEvent) {
        self.entity_game_event_by(id, game_event, Some(id));
    }

    pub fn entity_game_event_by(&mut self, id: EntityId, game_event: GameEvent, source_entity: Option<EntityId>) {
        let position = self.entity(id).position();
        self.game_event_from_entity(source_entity, game_event, position);
    }

    fn visit_in_range_listeners(
        &mut self,
        chunk_pos: ChunkPos,
        section: i32,
        game_event: GameEvent,
        source_position: Vec3,
        context: Context,
        to_handle_by_distance: &mut Vec<(f64, GameEventListener)>,
    ) {
        let Some(registry) = self.chunks.loaded_chunk_mut(chunk_pos).and_then(|chunk| chunk.game_event_listener_registry_sections.get_mut(&section)) else { return };
        let listeners = registry.start_processing();
        for listener in listeners {
            let pending_removal = self
                .chunks
                .loaded_chunk_mut(chunk_pos)
                .and_then(|chunk| chunk.game_event_listener_registry_sections.get_mut(&section))
                .is_some_and(|registry| registry.take_pending_removal(listener));
            if pending_removal {
                continue;
            }
            let Some(position) = get_postable_listener_position(self, source_position, listener) else { continue };
            if self.game_event_listener_delivery_mode(listener) == DeliveryMode::ByDistance {
                to_handle_by_distance.push((source_position.distance_to_sqr(position), listener));
            } else {
                self.handle_game_event(listener, game_event, context, source_position);
            }
        }
        if let Some(registry) = self.chunks.loaded_chunk_mut(chunk_pos).and_then(|chunk| chunk.game_event_listener_registry_sections.get_mut(&section)) {
            registry.finish_processing();
        }
    }
}
