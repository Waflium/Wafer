use std::collections::VecDeque;

use super::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::projectile::throwableitemprojectile::thrown_enderpearl::PendingEnderPearlTeleport;
use crate::world::level::block::portal::PendingPortalTeleport;

type Resume = Box<dyn FnOnce(&mut ServerLevel)>;

pub enum PendingServerWork {
    MinecartCommand(EntityId),
    PortalTeleport(PendingPortalTeleport),
    EnderPearlTeleport(PendingEnderPearlTeleport),
}

#[derive(Clone, Copy)]
pub enum TickRoot {
    Entity(EntityId),
    Player(EntityId),
}

#[derive(Default)]
pub struct TickContinuation {
    root: Option<TickRoot>,
    frames: VecDeque<Resume>,
}

impl TickContinuation {
    pub fn is_suspended(&self) -> bool {
        !self.frames.is_empty()
    }

    pub fn root_id(&self) -> Option<EntityId> {
        match self.root {
            Some(TickRoot::Entity(id) | TickRoot::Player(id)) => Some(id),
            None => None,
        }
    }

    pub fn take_from(&mut self, other: &mut TickContinuation) {
        self.root = other.root.take();
        self.frames = std::mem::take(&mut other.frames);
    }

    pub fn clear(&mut self) {
        self.root = None;
        self.frames.clear();
    }
}

impl ServerLevel {
    pub fn tick_suspended(&self) -> bool {
        self.pending_server_work.is_some()
    }

    pub fn suspend_tick(&mut self, resume: impl FnOnce(&mut ServerLevel) + 'static) {
        self.tick_continuation.frames.push_back(Box::new(resume));
    }

    pub fn then_tick(&mut self, resume: impl FnOnce(&mut ServerLevel) + 'static) {
        if self.tick_suspended() {
            self.suspend_tick(resume);
        } else {
            resume(self);
        }
    }

    pub(crate) fn tick_root_entity(&mut self, id: EntityId) {
        self.guard_entity_tick(id, |level| level.tick_non_passenger(id));
        if self.tick_suspended() {
            self.tick_continuation.root = Some(TickRoot::Entity(id));
        }
    }

    pub(crate) fn tick_root_player(&mut self, id: EntityId) {
        self.server_player_do_tick(id);
        if self.tick_suspended() {
            self.tick_continuation.root = Some(TickRoot::Player(id));
        }
    }

    pub fn drop_pending_server_work(&mut self) {
        while self.pending_server_work.take().is_some() {
            self.resume_entity_tick();
        }
    }

    pub(crate) fn resume_entity_tick(&mut self) {
        let root = self.tick_continuation.root;
        let resume = |level: &mut ServerLevel| {
            let mut remaining = std::mem::take(&mut level.tick_continuation.frames);
            while let Some(resume) = remaining.pop_front() {
                resume(level);
                if level.tick_suspended() {
                    level.tick_continuation.frames.append(&mut remaining);
                    return;
                }
            }
        };
        match root {
            Some(TickRoot::Entity(id)) => self.guard_entity_tick(id, resume),
            _ => resume(self),
        }
        if !self.tick_suspended() {
            self.tick_continuation.root = None;
        }
    }
}

impl crate::server::Server {
    pub fn run_pending_server_work(&mut self, mut index: usize) {
        while let Some(work) = self.levels[index].pending_server_work.take() {
            match work {
                PendingServerWork::MinecartCommand(entity) => self.run_pending_minecart_command(index, entity),
                PendingServerWork::PortalTeleport(pending) => self.handle_pending_portal_teleport(index, pending),
                PendingServerWork::EnderPearlTeleport(pending) => self.handle_pending_ender_pearl_teleport(index, pending),
            }
            index = self.follow_tick_continuation(index);
            super::lent_levels::LentLevels::lend(&mut self.levels, index).level().resume_entity_tick();
        }
    }

    fn follow_tick_continuation(&mut self, index: usize) -> usize {
        let Some(root) = self.levels[index].tick_continuation.root_id() else { return index };
        if !self.has_player(root) {
            return index;
        }
        let Some(target) = self.level_of(root) else { return index };
        if target != index {
            let [from, to] = self.levels.get_disjoint_mut([index, target]).expect("two different levels");
            to.tick_continuation.take_from(&mut from.tick_continuation);
        }
        target
    }
}
