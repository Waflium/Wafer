use wafer_core::BlockPos;
use wafer_core::random::RandomSource;

use super::declarative::behavior_builder;
use super::declarative::memory_accessor::MemoryAccessor;
use super::entity_tracker::EntityTracker;
use super::one_shot::OneShot;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_value::EntityValue;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::entity::ai::util::land_random_pos;

const MAX_FLEE_XZ_DIST: i32 = 20;
const MAX_FLEE_Y_DIST: i32 = 8;
const FLEE_SPEED_MODIFIER: f32 = 0.6;
const CHASE_SPEED_MODIFIER: f32 = 0.6;
const MAX_CHASERS_PER_TARGET: i32 = 5;
const AVERAGE_WAIT_TIME_BETWEEN_RUNS: i32 = 10;

pub fn create() -> OneShot {
    behavior_builder::create(|i| {
        let babies = i.present(memory_module_type::VISIBLE_VILLAGER_BABIES);
        let walk_target = i.absent(memory_module_type::WALK_TARGET);
        let look_target = i.registered(memory_module_type::LOOK_TARGET);
        let interaction_target = i.registered(memory_module_type::INTERACTION_TARGET);
        move |level: &mut ServerLevel, me: EntityId, _timestamp: i64| {
            if level.random.next_int_bounded(AVERAGE_WAIT_TIME_BETWEEN_RUNS) != 0 {
                return false;
            }
            let friends_nearby = babies.get(level, me);
            let other_kid_chasing_me = friends_nearby.iter().any(|&friend| is_friend_chasing_me(level, me, friend));
            if !other_kid_chasing_me {
                match find_someone_being_chased(level, &friends_nearby) {
                    Some(other_kid_being_chased) => chase_kid(level, me, interaction_target, look_target, other_kid_being_chased),
                    None => {
                        if let Some(&entity) = friends_nearby.first() {
                            chase_kid(level, me, interaction_target, look_target, entity);
                        }
                    }
                }
                true
            } else {
                for _ in 0..10 {
                    let pos = land_random_pos::get_pos(level, me, MAX_FLEE_XZ_DIST, MAX_FLEE_Y_DIST);
                    if let Some(pos) = pos
                        && level.is_village(BlockPos::containing_vec(pos))
                    {
                        walk_target.set(level, me, WalkTarget::of_vec(pos, FLEE_SPEED_MODIFIER, 0));
                        break;
                    }
                }
                true
            }
        }
    })
}

fn chase_kid<F1, F2>(level: &mut ServerLevel, me: EntityId, interaction_target: MemoryAccessor<F1, EntityValue>, look_target: MemoryAccessor<F2, PositionTracker>, kid_to_chase: EntityId) {
    interaction_target.set(level, me, kid_to_chase);
    let look = PositionTracker::Entity(EntityTracker::new(level, kid_to_chase, true));
    look_target.set(level, me, look);
    let walk = WalkTarget::new(PositionTracker::Entity(EntityTracker::new(level, kid_to_chase, false)), CHASE_SPEED_MODIFIER, 1);
    level.get_brain_mut(me).set_memory(memory_module_type::WALK_TARGET, walk);
}

fn find_someone_being_chased(level: &ServerLevel, friends_nearby: &[EntityId]) -> Option<EntityId> {
    let mut chased_kids = check_how_many_chasers_each_friend_has(level, friends_nearby);
    chased_kids.sort_by_key(|&(_, count)| count);
    chased_kids.into_iter().find(|&(_, count)| count > 0 && count <= MAX_CHASERS_PER_TARGET).map(|(kid, _)| kid)
}

fn check_how_many_chasers_each_friend_has(level: &ServerLevel, friends_nearby: &[EntityId]) -> Vec<(EntityId, i32)> {
    let mut chased_kids: Vec<(EntityId, i32)> = Vec::new();
    for &chaser in friends_nearby.iter().filter(|&&friend| is_chasing_someone(level, friend)) {
        let chased = who_are_you_chasing(level, chaser);
        match chased_kids.iter_mut().find(|(kid, _)| *kid == chased) {
            Some((_, count)) => *count += 1,
            None => chased_kids.push((chased, 1)),
        }
    }
    chased_kids
}

fn who_are_you_chasing(level: &ServerLevel, friend: EntityId) -> EntityId {
    *level.get_brain(friend).get_memory(memory_module_type::INTERACTION_TARGET).expect("a chased kid")
}

fn is_chasing_someone(level: &ServerLevel, friend: EntityId) -> bool {
    level.get_brain_if_present(friend).is_some_and(|brain| brain.get_memory_internal(memory_module_type::INTERACTION_TARGET).flatten().is_some())
}

fn is_friend_chasing_me(level: &ServerLevel, me: EntityId, friend: EntityId) -> bool {
    level.get_brain_if_present(friend).is_some_and(|brain| brain.get_memory_internal(memory_module_type::INTERACTION_TARGET).flatten() == Some(&me))
}
