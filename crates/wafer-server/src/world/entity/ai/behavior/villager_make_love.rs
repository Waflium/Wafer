use wafer_core::BlockPos;
use wafer_core::random::RandomSource;

use super::behavior::{Behavior, BehaviorState};
use super::behavior_utils;
use crate::core::global_pos::GlobalPos;
use crate::server::level::ServerLevel;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::ai::village::poi::poi_types::PoiType;
use crate::world::entity::npc::villager::villager;
use crate::world::entity::{EntityId, LevelEntity, entity_event, entity_types};
use crate::world::level::server_level_accessor::ServerLevelAccessor;

pub struct VillagerMakeLove {
    behavior: BehaviorState,
    birth_timestamp: i64,
}

impl Default for VillagerMakeLove {
    fn default() -> VillagerMakeLove {
        VillagerMakeLove {
            behavior: BehaviorState::with_durations(
                vec![(memory_module_type::BREED_TARGET.erase(), MemoryStatus::ValuePresent), (memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES.erase(), MemoryStatus::ValuePresent)],
                350,
                350,
            ),
            birth_timestamp: 0,
        }
    }
}

fn can_breed(level: &ServerLevel, entity: EntityId) -> bool {
    level.mob(entity).is_some_and(|mob| mob.abstract_villager().is_some() && villager::can_breed(mob))
}

impl VillagerMakeLove {
    fn try_to_give_birth(&self, level: &mut ServerLevel, body: EntityId, target: EntityId) {
        match self.take_vacant_bed(level, body) {
            None => {
                level.broadcast_entity_event(target, entity_event::VILLAGER_ANGRY);
                level.broadcast_entity_event(body, entity_event::VILLAGER_ANGRY);
            }
            Some(childs_bed) => match self.breed(level, body, target) {
                Some(child) => self.give_bed_to_child(level, child, childs_bed),
                None => {
                    level.chunks.poi_manager.release(childs_bed);
                }
            },
        }
    }

    fn is_breeding_possible(&self, level: &mut ServerLevel, my_body: EntityId) -> bool {
        let breed_target = level
            .get_brain(my_body)
            .get_memory(memory_module_type::BREED_TARGET)
            .copied()
            .filter(|&entity| level.entity_ref(entity).is_some_and(|e| e.entity_type == entity_types::VILLAGER.id));
        match breed_target {
            None => false,
            Some(breed_target) => {
                behavior_utils::target_is_valid(level, my_body, memory_module_type::BREED_TARGET, &entity_types::VILLAGER) && can_breed(level, my_body) && can_breed(level, breed_target)
            }
        }
    }

    fn take_vacant_bed(&self, level: &mut ServerLevel, body: EntityId) -> Option<BlockPos> {
        let center = level.entity(body).block_position();
        let candidates = level.chunks.poi_manager.get_in_range(&|poi_type| poi_type == PoiType::Home, center, 48, crate::world::entity::ai::village::poi::poi_manager::Occupancy::HasSpace);
        let found = candidates.into_iter().find(|record| can_reach(level, body, record.pos, record.poi_type))?;
        level.chunks.poi_manager.take(&|poi_type| poi_type == PoiType::Home, |_, pos| pos == found.pos, found.pos, 0)
    }

    fn breed(&self, level: &mut ServerLevel, source: EntityId, target: EntityId) -> Option<EntityId> {
        let mut child = level.villager_get_breed_offspring(source, target)?;
        level.set_age(source, 6000);
        level.set_age(target, 6000);
        if child.set_age(-24000) {
            child.refresh_dimensions();
        }
        let e = level.entity(source);
        let (x, y, z) = (e.x(), e.y(), e.z());
        child.entity.snap_to(x, y, z, 0.0, 0.0);
        let id = child.entity.id;
        level.add_fresh_entity_with_passengers(LevelEntity::Mob(child));
        level.villager_age_boundary_reached(id);
        level.broadcast_entity_event(id, entity_event::LOVE_HEARTS);
        Some(id)
    }

    fn give_bed_to_child(&self, level: &mut ServerLevel, child: EntityId, bed_pos: BlockPos) {
        let global_bed_pos = GlobalPos::of(level.dimension, bed_pos);
        level.get_brain_mut(child).set_memory(memory_module_type::HOME, global_bed_pos);
    }
}

fn can_reach(level: &mut ServerLevel, body: EntityId, poi_pos: BlockPos, poi_type: PoiType) -> bool {
    let path = level.get_navigation(body, |navigation, level| navigation.create_path(level, body, poi_pos, poi_type.valid_range()));
    path.is_some_and(|path| path.can_reach())
}

impl Behavior for VillagerMakeLove {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        self.is_breeding_possible(level, body)
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) -> bool {
        timestamp <= self.birth_timestamp && self.is_breeding_possible(level, body)
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        let breed_target = *level.get_brain(body).get_memory(memory_module_type::BREED_TARGET).expect("a breed target");
        behavior_utils::lock_gaze_and_walk_to_each_other(level, body, breed_target, 0.5, 2);
        level.broadcast_entity_event(breed_target, entity_event::IN_LOVE_HEARTS);
        level.broadcast_entity_event(body, entity_event::IN_LOVE_HEARTS);
        let duration = 275 + level.entity_mut(body).random.next_int_bounded(50);
        self.birth_timestamp = timestamp + duration as i64;
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        let target = *level.get_brain(body).get_memory(memory_module_type::BREED_TARGET).expect("a breed target");
        if level.entity(body).position().distance_to_sqr(level.entity(target).position()) > 5.0 {
            return;
        }
        behavior_utils::lock_gaze_and_walk_to_each_other(level, body, target, 0.5, 2);
        if timestamp >= self.birth_timestamp {
            level.eat_and_digest_food(body);
            level.eat_and_digest_food(target);
            self.try_to_give_birth(level, body, target);
        } else if level.entity_mut(body).random.next_int_bounded(35) == 0 {
            level.broadcast_entity_event(target, entity_event::LOVE_HEARTS);
            level.broadcast_entity_event(body, entity_event::LOVE_HEARTS);
        }
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        level.get_brain_mut(body).erase_memory(memory_module_type::BREED_TARGET);
    }
}
