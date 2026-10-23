use wafer_core::{BlockPos, Vec3};
use wafer_data::game_rule;
use wafer_protocol::nbt::Nbt;

use super::{cushion, hanging_entity, item_frame, leash_fence_knot_entity, painting};
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::{Entity, EntityId, LevelEntity, RemovalReason};
use crate::world::level::storage::tag_value_input;

const CHECK_INTERVAL: i32 = 100;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlockAttachedKind {
    LeashKnot,
    Painting,
    ItemFrame,
    Cushion,
}

#[derive(Clone, Copy, Debug)]
pub struct BlockAttachedEntity {
    pub kind: BlockAttachedKind,
    ticks_since_last_check: i32,
    pub pos: BlockPos,
}

impl BlockAttachedEntity {
    pub fn new(kind: BlockAttachedKind, pos: BlockPos) -> BlockAttachedEntity {
        BlockAttachedEntity { kind, ticks_since_last_check: 0, pos }
    }
}

pub fn recalculate_bounding_box(entity: &mut Entity) {
    let Some(block_attached) = entity.block_attached else { return };
    match block_attached.kind {
        BlockAttachedKind::LeashKnot => leash_fence_knot_entity::recalculate_bounding_box(entity, block_attached.pos),
        BlockAttachedKind::Painting => hanging_entity::recalculate_bounding_box(entity, block_attached.pos, painting::calculate_bounding_box),
        BlockAttachedKind::ItemFrame => hanging_entity::recalculate_bounding_box(entity, block_attached.pos, item_frame::calculate_bounding_box),
        BlockAttachedKind::Cushion => cushion::recalculate_bounding_box(entity),
    }
}

pub fn set_pos(entity: &mut Entity, x: f64, y: f64, z: f64) {
    if entity.block_attached.is_some_and(|block_attached| block_attached.kind == BlockAttachedKind::Cushion) {
        cushion::set_pos(entity, x, y, z);
    } else {
        block_attached_entity_set_pos(entity, x, y, z);
    }
}

pub fn block_attached_entity_set_pos(entity: &mut Entity, x: f64, y: f64, z: f64) {
    if let Some(block_attached) = &mut entity.block_attached {
        block_attached.pos = BlockPos::containing(x, y, z);
    }
    recalculate_bounding_box(entity);
    entity.needs_sync = true;
}

pub fn add_additional_save_data(entity: &Entity, output: &mut Vec<(String, Nbt)>) {
    let pos = entity.block_attached_pos();
    output.push(("block_pos".into(), Nbt::IntArray(vec![pos.x, pos.y, pos.z])));
}

pub fn read_additional_save_data(entity: &mut Entity, input: &Nbt) {
    let stored_pos = tag_value_input::read(input, "block_pos", crate::util::extra_codecs::block_pos);
    match stored_pos {
        Some(stored_pos) if stored_pos.closer_than(entity.block_position(), 16.0) => {
            if let Some(block_attached) = &mut entity.block_attached {
                block_attached.pos = stored_pos;
            }
        }
        _ => error!("Block-attached entity at invalid position: {}", stored_pos.map_or_else(|| "null".to_string(), |pos| format!("BlockPos{{x={}, y={}, z={}}}", pos.x, pos.y, pos.z))),
    }
}

impl ServerLevel {
    pub fn is_block_attached_entity(&self, id: EntityId) -> bool {
        self.entity_ref(id).is_some_and(|entity| entity.block_attached.is_some())
    }

    pub fn block_attached_entity_tick(&mut self, id: EntityId) {
        self.check_below_world(id);
        let block_attached = self.entity_mut(id).block_attached.as_mut().expect("a block attached entity");
        let ticks_since_last_check = block_attached.ticks_since_last_check;
        block_attached.ticks_since_last_check += 1;
        if ticks_since_last_check >= CHECK_INTERVAL {
            self.entity_mut(id).block_attached.as_mut().expect("a block attached entity").ticks_since_last_check = 0;
            self.block_attached_entity_tick_at_check_interval(id);
            if !self.entity(id).is_removed() && !self.block_attached_entity_survives(id) {
                self.discard(id);
                self.block_attached_entity_drop_item(id, None);
            }
        }
    }

    fn block_attached_entity_tick_at_check_interval(&mut self, id: EntityId) {
        if let Some(LevelEntity::Cushion(_)) = self.entities.get(&id) {
            self.cushion_tick_at_check_interval(id);
        }
    }

    fn block_attached_entity_survives(&mut self, id: EntityId) -> bool {
        match self.entities.get(&id) {
            Some(LevelEntity::LeashKnot(_)) => self.leash_fence_knot_survives(id),
            Some(LevelEntity::Painting(_)) => self.hanging_entity_survives(id),
            Some(LevelEntity::ItemFrame(_)) => self.item_frame_survives(id),
            Some(LevelEntity::Cushion(_)) => self.cushion_survives(id),
            _ => unreachable!("not a block attached entity"),
        }
    }

    pub fn block_attached_entity_drop_item(&mut self, id: EntityId, caused_by: Option<EntityId>) {
        match self.entities.get(&id) {
            Some(LevelEntity::LeashKnot(_)) => self.leash_fence_knot_drop_item(id, caused_by),
            Some(LevelEntity::Painting(_)) => self.painting_drop_item(id, caused_by),
            Some(LevelEntity::ItemFrame(_)) => self.item_frame_drop_item(id, caused_by),
            Some(LevelEntity::Cushion(_)) => self.cushion_drop_item(id, caused_by),
            _ => unreachable!("not a block attached entity"),
        }
    }

    pub fn block_attached_entity_kill(&mut self, id: EntityId, attributed_to: Option<EntityId>) {
        if let Some(LevelEntity::ItemFrame(_)) = self.entities.get(&id) {
            self.item_frame_on_killed(id);
        }
        self.remove(id, RemovalReason::Killed);
        self.entity_game_event_by(id, crate::world::level::gameevent::game_event::ENTITY_DIE, Some(attributed_to.unwrap_or(id)));
    }

    pub fn block_attached_entity_skip_attack_interaction(&mut self, id: EntityId, source: EntityId) -> bool {
        if !self.is_player(source) {
            return false;
        }
        let pos = self.entity(id).block_attached_pos();
        if !self.may_interact(source, pos) {
            true
        } else {
            let damage_source = DamageSource { causing_entity: Some(source), direct_entity: Some(source), ..DamageSource::new(wafer_data::damage_types::PLAYER_ATTACK) };
            self.hurt_server(id, damage_source, 0.0)
        }
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

    pub fn block_attached_entity_move(&mut self, id: EntityId, delta: Vec3) {
        if self.item_frame_is_fixed(id) {
            return;
        }
        if !self.entity(id).is_removed() && delta.length_sqr() > 0.0 {
            self.block_attached_entity_kill(id, None);
            self.block_attached_entity_drop_item(id, None);
        }
    }

    pub fn block_attached_entity_push(&mut self, id: EntityId, xa: f64, ya: f64, za: f64) {
        if self.item_frame_is_fixed(id) {
            return;
        }
        if !self.entity(id).is_removed() && xa * xa + ya * ya + za * za > 0.0 {
            self.block_attached_entity_kill(id, None);
            self.block_attached_entity_drop_item(id, None);
        }
    }
}
