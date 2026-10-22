use wafer_core::{BlockPos, Vec3};

use super::game_event_listener::GameEventListener;
use crate::server::level::ServerLevel;

#[derive(Clone, Default)]
pub struct EuclideanGameEventListenerRegistry {
    listeners: Vec<GameEventListener>,
    listeners_to_remove: Vec<GameEventListener>,
    listeners_to_add: Vec<GameEventListener>,
    processing: bool,
}

impl EuclideanGameEventListenerRegistry {
    pub fn is_empty(&self) -> bool {
        self.listeners.is_empty()
    }

    pub fn register(&mut self, listener: GameEventListener) {
        if self.processing {
            self.listeners_to_add.push(listener);
        } else {
            self.listeners.push(listener);
        }
    }

    pub fn unregister(&mut self, listener: GameEventListener) -> bool {
        if self.processing {
            if !self.listeners_to_remove.contains(&listener) {
                self.listeners_to_remove.push(listener);
            }
        } else if let Some(index) = self.listeners.iter().position(|&l| l == listener) {
            self.listeners.remove(index);
        }
        self.listeners.is_empty()
    }

    pub fn start_processing(&mut self) -> Vec<GameEventListener> {
        self.processing = true;
        self.listeners.clone()
    }

    pub fn take_pending_removal(&mut self, listener: GameEventListener) -> bool {
        let Some(index) = self.listeners_to_remove.iter().position(|&l| l == listener) else { return false };
        self.listeners_to_remove.swap_remove(index);
        if let Some(index) = self.listeners.iter().position(|&l| l == listener) {
            self.listeners.remove(index);
        }
        true
    }

    pub fn finish_processing(&mut self) {
        self.processing = false;
        self.listeners.append(&mut self.listeners_to_add);
        if !self.listeners_to_remove.is_empty() {
            let to_remove = std::mem::take(&mut self.listeners_to_remove);
            self.listeners.retain(|listener| !to_remove.contains(listener));
        }
    }
}

pub fn get_postable_listener_position(level: &ServerLevel, source_position: Vec3, listener: GameEventListener) -> Option<Vec3> {
    let position = level.game_event_listener_source(listener)?.get_position(level)?;
    let distance_from_origin = BlockPos::containing_vec(position).dist_sqr(BlockPos::containing_vec(source_position));
    let radius = level.game_event_listener_radius(listener);
    let radius_sqr = radius * radius;
    if distance_from_origin > radius_sqr as f64 { None } else { Some(position) }
}
