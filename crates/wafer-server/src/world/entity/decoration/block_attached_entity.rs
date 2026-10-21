use wafer_core::BlockPos;
use wafer_data::game_rule;

use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::{EntityId, LevelEntity, RemovalReason};

const CHECK_INTERVAL: i32 = 100;

pub struct BlockAttachedEntity {
    ticks_since_last_check: i32,
    pub pos: BlockPos,
}

impl BlockAttachedEntity {
    pub fn new(pos: BlockPos) -> BlockAttachedEntity {
        BlockAttachedEntity { ticks_since_last_check: 0, pos }
    }
}

impl ServerLevel {
    fn block_attached_entity_mut(&mut self, id: EntityId) -> &mut BlockAttachedEntity {
        match self.entities.get_mut(&id) {
            Some(LevelEntity::LeashKnot(knot)) => &mut knot.block_attached,
            _ => unreachable!("not a block attached entity"),
        }
    }

    pub fn is_block_attached_entity(&self, id: EntityId) -> bool {
        matches!(self.entities.get(&id), Some(LevelEntity::LeashKnot(_)))
    }

    pub fn block_attached_entity_tick(&mut self, id: EntityId) {
        self.check_below_world(id);
        let block_attached = self.block_attached_entity_mut(id);
        let ticks_since_last_check = block_attached.ticks_since_last_check;
        block_attached.ticks_since_last_check += 1;
        if ticks_since_last_check >= CHECK_INTERVAL {
            self.block_attached_entity_mut(id).ticks_since_last_check = 0;
            if !self.entity(id).is_removed() && !self.block_attached_entity_survives(id) {
                self.discard(id);
                self.block_attached_entity_drop_item(id, None);
            }
        }
    }

    fn block_attached_entity_survives(&mut self, id: EntityId) -> bool {
        match self.entities.get(&id) {
            Some(LevelEntity::LeashKnot(_)) => self.leash_fence_knot_survives(id),
            _ => unreachable!("not a block attached entity"),
        }
    }

    pub fn block_attached_entity_drop_item(&mut self, id: EntityId, caused_by: Option<EntityId>) {
        match self.entities.get(&id) {
            Some(LevelEntity::LeashKnot(_)) => self.leash_fence_knot_drop_item(id, caused_by),
            _ => unreachable!("not a block attached entity"),
        }
    }

    pub fn block_attached_entity_kill(&mut self, id: EntityId, _attributed_to: Option<EntityId>) {
        self.remove(id, RemovalReason::Killed);
    }

    pub fn block_attached_entity_hurt_server(&mut self, id: EntityId, source: DamageSource) -> bool {
        if self.is_invulnerable_to_base(id, &source) {
            return false;
        }
        if !self.game_rules.get_bool(game_rule::MOB_GRIEFING) && source.causing_entity.is_some_and(|entity| self.is_mob(entity)) {
            return false;
        }
        if !self.entity(id).is_removed() {
            self.block_attached_entity_kill(id, source.causing_entity);
            self.mark_hurt(id);
            self.block_attached_entity_drop_item(id, source.causing_entity);
        }
        true
    }

    pub fn block_attached_entity_move(&mut self, id: EntityId, delta: wafer_core::Vec3) {
        if !self.entity(id).is_removed() && delta.length_sqr() > 0.0 {
            self.block_attached_entity_kill(id, None);
            self.block_attached_entity_drop_item(id, None);
        }
    }

    pub fn block_attached_entity_push(&mut self, id: EntityId, xa: f64, ya: f64, za: f64) {
        if !self.entity(id).is_removed() && xa * xa + ya * ya + za * za > 0.0 {
            self.block_attached_entity_kill(id, None);
            self.block_attached_entity_drop_item(id, None);
        }
    }
}
