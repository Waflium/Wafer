use wafer_core::{BlockPos, Vec3};
use wafer_data::EquipmentSlot;

use super::entity_tracker::EntityTracker;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::entity::ai::util::default_random_pos;
use crate::world::item::{ItemStack, projectile_weapon_item};
use crate::world::level::BlockGetter;

pub fn lock_gaze_and_walk_to_each_other(level: &mut ServerLevel, entity1: EntityId, entity2: EntityId, speed_modifier: f32, close_enough_distance: i32) {
    look_at_each_other(level, entity1, entity2);
    set_walk_and_look_target_memories_to_each_other(level, entity1, entity2, speed_modifier, close_enough_distance);
}

pub fn target_is_valid(level: &mut ServerLevel, body: EntityId, memory: crate::world::entity::ai::memory::memory_module_type::MemoryModuleType<crate::world::entity::ai::memory::memory_value::EntityValue>, target_type: &crate::world::entity::entity_type::EntityType) -> bool {
    let Some(entity) = level.get_brain(body).get_memory(memory).copied() else { return false };
    level.entity_ref(entity).is_some_and(|e| e.entity_type == target_type.id) && level.is_alive(entity) && entity_is_visible(level, body, entity)
}

pub fn throw_item(level: &mut ServerLevel, thrower: EntityId, item: crate::world::item::ItemStack, target_pos: Vec3) {
    throw_item_with(level, thrower, item, target_pos, Vec3::new(0.3f32 as f64, 0.3f32 as f64, 0.3f32 as f64), 0.3);
}

pub fn throw_item_with(level: &mut ServerLevel, thrower: EntityId, item: crate::world::item::ItemStack, target_pos: Vec3, throw_velocity: Vec3, hand_y_distance_from_eye: f32) {
    let e = level.entity(thrower);
    let y_hand_pos = e.eye_y() - hand_y_distance_from_eye as f64;
    let (x, z, position, uuid) = (e.x(), e.z(), e.position(), e.uuid);
    let mut item_entity = crate::world::entity::ItemEntity::new(Vec3::new(x, y_hand_pos, z), item);
    item_entity.thrower = Some(uuid);
    let throw_vector = target_pos.subtract(position).normalize();
    let throw_vector = Vec3::new(throw_vector.x * throw_velocity.x, throw_vector.y * throw_velocity.y, throw_vector.z * throw_velocity.z);
    item_entity.entity.set_delta_movement(throw_vector);
    item_entity.set_default_pickup_delay();
    level.add_fresh_entity(crate::world::entity::LevelEntity::Item(item_entity));
}

pub fn entity_is_visible(level: &mut ServerLevel, body: EntityId, target_entity: EntityId) -> bool {
    match level.get_brain(body).get_memory(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES).cloned() {
        Some(visible_entities) => visible_entities.contains(level, target_entity),
        None => false,
    }
}

fn look_at_each_other(level: &mut ServerLevel, entity1: EntityId, entity2: EntityId) {
    look_at_entity(level, entity1, entity2);
    look_at_entity(level, entity2, entity1);
}

pub fn look_at_entity(level: &mut ServerLevel, looker: EntityId, target_entity: EntityId) {
    let tracker = EntityTracker::new(level, target_entity, true);
    level.get_brain_mut(looker).set_memory(memory_module_type::LOOK_TARGET, PositionTracker::Entity(tracker));
}

fn set_walk_and_look_target_memories_to_each_other(level: &mut ServerLevel, entity1: EntityId, entity2: EntityId, speed_modifier: f32, close_enough_distance: i32) {
    set_walk_and_look_target_memories_to_entity(level, entity1, entity2, speed_modifier, close_enough_distance);
    set_walk_and_look_target_memories_to_entity(level, entity2, entity1, speed_modifier, close_enough_distance);
}

pub fn set_walk_and_look_target_memories_to_entity(level: &mut ServerLevel, walker: EntityId, target_entity: EntityId, speed_modifier: f32, close_enough_distance: i32) {
    let tracker = EntityTracker::new(level, target_entity, true);
    set_walk_and_look_target_memories(level, walker, PositionTracker::Entity(tracker), speed_modifier, close_enough_distance);
}

pub fn set_walk_and_look_target_memories_to_pos(level: &mut ServerLevel, walker: EntityId, target_pos: BlockPos, speed_modifier: f32, close_enough_distance: i32) {
    set_walk_and_look_target_memories(level, walker, PositionTracker::BlockPos(super::block_pos_tracker::BlockPosTracker::new(target_pos)), speed_modifier, close_enough_distance);
}

pub fn set_walk_and_look_target_memories(level: &mut ServerLevel, walker: EntityId, target: PositionTracker, speed_modifier: f32, close_enough_distance: i32) {
    let walk_target = WalkTarget::new(target.clone(), speed_modifier, close_enough_distance);
    let brain = level.get_brain_mut(walker);
    brain.set_memory(memory_module_type::LOOK_TARGET, target);
    brain.set_memory(memory_module_type::WALK_TARGET, walk_target);
}

pub fn is_within_attack_range(level: &ServerLevel, body: EntityId, target: EntityId, projectile_attack_range_margin: i32) -> bool {
    let main_hand_item = level.item_by_slot(body, EquipmentSlot::Mainhand).unwrap_or_else(ItemStack::empty);
    if projectile_weapon_item::is_projectile_weapon(main_hand_item.get_item()) && level.can_use_non_melee_weapon(body, &main_hand_item) {
        let max_allowed_distance = (projectile_weapon_item::get_default_projectile_range(main_hand_item.get_item()) - projectile_attack_range_margin) as f64;
        level.entity(body).distance_to_sqr(level.entity(target)) < max_allowed_distance * max_allowed_distance
    } else {
        level.is_within_melee_attack_range(body, target)
    }
}

pub fn is_breeding(level: &ServerLevel, body: EntityId) -> bool {
    level.get_brain(body).has_memory_value(memory_module_type::BREED_TARGET)
}

pub fn find_section_closest_to_village(level: &ServerLevel, center: BlockPos, radius: i32) -> BlockPos {
    let dist_to_village = level.sections_to_village(center);
    let mut closest: Option<(BlockPos, i32)> = None;
    for z in center.z - radius..=center.z + radius {
        for y in center.y - radius..=center.y + radius {
            for x in center.x - radius..=center.x + radius {
                let section_pos = BlockPos::new(x, y, z);
                let sections_to_village = level.sections_to_village(section_pos);
                if sections_to_village < dist_to_village && closest.is_none_or(|(_, best)| sections_to_village < best) {
                    closest = Some((section_pos, sections_to_village));
                }
            }
        }
    }
    closest.map_or(center, |(section_pos, _)| section_pos)
}

pub fn get_random_swimmable_pos(level: &mut ServerLevel, body: EntityId, max_horizontal_distance: i32, max_vertical_distance: i32) -> Option<Vec3> {
    let mut target_pos = default_random_pos::get_pos(level, body, max_horizontal_distance, max_vertical_distance);
    let mut count = 0;
    while let Some(pos) = target_pos {
        if level.block_state(BlockPos::containing_vec(pos)).is_pathfindable_water() {
            break;
        }
        let within_attempts = count < 10;
        count += 1;
        if !within_attempts {
            break;
        }
        target_pos = default_random_pos::get_pos(level, body, max_horizontal_distance, max_vertical_distance);
    }
    target_pos
}

pub fn is_other_target_much_further_away_than_current_attack_target(level: &ServerLevel, body: EntityId, other_target: EntityId, how_much_further_away: f64) -> bool {
    let Some(current_target) = level.get_brain(body).get_memory(crate::world::entity::ai::memory::memory_module_type::ATTACK_TARGET).copied() else { return false };
    let body_position = level.entity(body).position();
    let dist_sqr_to_current_target = body_position.distance_to_sqr(level.entity(current_target).position());
    let dist_sqr_to_other_target = body_position.distance_to_sqr(level.entity(other_target).position());
    dist_sqr_to_other_target > dist_sqr_to_current_target + how_much_further_away * how_much_further_away
}

pub fn get_nearest_target(level: &ServerLevel, body: EntityId, target1: Option<EntityId>, target2: EntityId) -> EntityId {
    match target1 {
        None => target2,
        Some(target1) => get_target_nearest_me(level, body, target1, target2),
    }
}

pub fn get_target_nearest_me(level: &ServerLevel, body: EntityId, target1: EntityId, target2: EntityId) -> EntityId {
    let body_position = level.entity(body).position();
    let (pos1, pos2) = (level.entity(target1).position(), level.entity(target2).position());
    if body_position.distance_to_sqr(pos1) < body_position.distance_to_sqr(pos2) { target1 } else { target2 }
}
