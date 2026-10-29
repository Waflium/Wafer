use std::collections::VecDeque;

use super::ServerLevel;
use crate::world::entity::EntityId;

type Resume = Box<dyn FnOnce(&mut ServerLevel)>;

#[derive(Default)]
pub struct TickContinuation {
    root: Option<EntityId>,
    frames: VecDeque<Resume>,
}

impl TickContinuation {
    pub fn is_suspended(&self) -> bool {
        !self.frames.is_empty()
    }
}

impl ServerLevel {
    pub fn tick_suspended(&self) -> bool {
        self.pending_minecart_command.is_some()
    }

    pub fn suspend_tick(&mut self, resume: impl FnOnce(&mut ServerLevel) + 'static) {
        self.tick_continuation.frames.push_back(Box::new(resume));
    }

    pub(crate) fn tick_root_entity(&mut self, id: EntityId) {
        self.guard_entity_tick(id, |level| level.tick_non_passenger(id));
        if self.tick_suspended() {
            self.tick_continuation.root = Some(id);
        }
    }

    pub(crate) fn resume_entity_tick(&mut self) {
        let Some(root) = self.tick_continuation.root else { return };
        self.guard_entity_tick(root, |level| {
            let mut remaining = std::mem::take(&mut level.tick_continuation.frames);
            while let Some(resume) = remaining.pop_front() {
                resume(level);
                if level.tick_suspended() {
                    level.tick_continuation.frames.append(&mut remaining);
                    return;
                }
            }
        });
        if !self.tick_suspended() {
            self.tick_continuation.root = None;
        }
    }
}
