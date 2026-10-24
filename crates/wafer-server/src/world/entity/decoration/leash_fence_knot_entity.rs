use wafer_core::{AABB, BlockPos, Vec3};
use wafer_data::{block_tags, items, registry, registry_id};

use super::block_attached_entity::{self, BlockAttachedEntity, BlockAttachedKind};
use crate::server::level::ServerLevel;
use crate::world::entity::data::entity_defaults;
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, sound_event};
use crate::world::level::BlockGetter;
use crate::world::{InteractionHand, InteractionResult};

pub const OFFSET_Y: f64 = 0.375;

pub struct LeashFenceKnotEntity {
    pub entity: Entity,
}

impl LeashFenceKnotEntity {
    pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.375, 0.5).with_eye_height(0.0625);

    pub fn new(pos: BlockPos) -> LeashFenceKnotEntity {
        let mut entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:leash_knot"), Self::DIMENSIONS, entity_defaults());
        entity.block_attached = Some(BlockAttachedEntity::new(BlockAttachedKind::LeashKnot, pos));
        block_attached_entity::set_pos(&mut entity, pos.x as f64, pos.y as f64, pos.z as f64);
        LeashFenceKnotEntity { entity }
    }

    pub fn get_or_create_knot(level: &mut ServerLevel, pos: BlockPos) -> EntityId {
        LeashFenceKnotEntity::get_knot(level, pos).unwrap_or_else(|| LeashFenceKnotEntity::create_knot(level, pos))
    }

    pub fn get_knot(level: &ServerLevel, pos: BlockPos) -> Option<EntityId> {
        let (x, y, z) = (pos.x as f64, pos.y as f64, pos.z as f64);
        let area = AABB::new(x - 1.0, y - 1.0, z - 1.0, x + 1.0, y + 1.0, z + 1.0);
        level.get_entities(None, &area).into_iter().find(|&id| matches!(level.find_entity(id), Some(LevelEntity::LeashKnot(knot)) if knot.entity.block_attached_pos() == pos))
    }

    pub fn create_knot(level: &mut ServerLevel, pos: BlockPos) -> EntityId {
        let knot = LeashFenceKnotEntity::new(pos);
        let id = knot.entity.id;
        level.add_fresh_entity(LevelEntity::LeashKnot(knot));
        id
    }
}

pub fn recalculate_bounding_box(entity: &mut Entity, pos: BlockPos) {
    entity.set_pos_raw(Vec3::new(pos.x as f64 + 0.5, pos.y as f64 + OFFSET_Y, pos.z as f64 + 0.5));
    let half_width = LeashFenceKnotEntity::DIMENSIONS.width as f64 / 2.0;
    let height = LeashFenceKnotEntity::DIMENSIONS.height as f64;
    let (x, y, z) = (entity.x(), entity.y(), entity.z());
    entity.set_bounding_box(AABB::new(x - half_width, y, z - half_width, x + half_width, y + height, z + half_width));
}

impl ServerLevel {
    pub fn tick_leash_fence_knot(&mut self, id: EntityId) {
        self.block_attached_entity_tick(id);
    }

    pub fn leash_fence_knot_drop_item(&mut self, id: EntityId, _caused_by: Option<EntityId>) {
        self.entity_play_sound(id, sound_event("minecraft:item.lead.untied"), 1.0, 1.0);
    }

    pub fn leash_fence_knot_survives(&mut self, id: EntityId) -> bool {
        let Some(LevelEntity::LeashKnot(knot)) = self.find_entity(id) else { return false };
        let pos = knot.entity.block_attached_pos();
        self.block_state(pos).is(&block_tags::FENCES)
    }

    pub fn leash_fence_knot_play_placement_sound(&mut self, id: EntityId) {
        self.entity_play_sound(id, sound_event("minecraft:item.lead.tied"), 1.0, 1.0);
    }

    pub fn leash_fence_knot_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand, location: Vec3) -> InteractionResult {
        if self.player(player).item_in_hand(hand).is_some_and(|stack| stack.is(items::SHEARS)) {
            let result = self.entity_interact(id, player, hand, location);
            if let InteractionResult::Success { was_item_interaction: true, .. } = result {
                return result;
            }
        }
        let mut attached_mob = false;
        for leashable in self.leashable_leashed_to(player) {
            if self.can_have_a_leash_attached_to(leashable, id) {
                self.set_leashed_to(leashable, id, true);
                attached_mob = true;
            }
        }
        let mut any_dropped = false;
        if !attached_mob && !self.player(player).is_secondary_use_active() {
            for mob in self.leashable_leashed_to(id) {
                if self.can_have_a_leash_attached_to(mob, player) {
                    self.set_leashed_to(mob, player, true);
                    any_dropped = true;
                }
            }
        }
        if !attached_mob && !any_dropped {
            return self.entity_interact(id, player, hand, location);
        }
        self.entity_game_event_by(id, crate::world::level::gameevent::game_event::BLOCK_ATTACH, Some(player));
        self.entity_play_sound(id, sound_event("minecraft:item.lead.tied"), 1.0, 1.0);
        InteractionResult::SUCCESS
    }
}

pub fn bind_player_mobs(level: &mut ServerLevel, player: EntityId, pos: BlockPos) -> InteractionResult {
    let entities_to_leash = level.leashable_in_area(Vec3::at_center_of(pos), |level, leashable| level.get_leash_holder(leashable) == Some(player));
    if entities_to_leash.is_empty() {
        return InteractionResult::Pass;
    }
    let existing_knot = LeashFenceKnotEntity::get_knot(level, pos);
    let active_knot = existing_knot.unwrap_or_else(|| LeashFenceKnotEntity::create_knot(level, pos));
    let mut any_leashed = false;
    for leashable in entities_to_leash {
        if level.can_have_a_leash_attached_to(leashable, active_knot) {
            level.set_leashed_to(leashable, active_knot, true);
            any_leashed = true;
        }
    }
    if any_leashed {
        level.leash_fence_knot_play_placement_sound(active_knot);
        level.game_event_at_with(crate::world::level::gameevent::game_event::BLOCK_ATTACH, pos, crate::world::level::gameevent::game_event::Context::of(Some(player)));
        return InteractionResult::SUCCESS_SERVER;
    }
    if existing_knot.is_none() {
        level.discard(active_knot);
    }
    InteractionResult::Pass
}
