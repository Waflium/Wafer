use wafer_core::{BlockPos, Direction, Vec3, mth};

use super::behavior::{Behavior, BehaviorState};
use super::entity_tracker::EntityTracker;
use super::position_tracker::PositionTracker;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::ai::util::goal_utils;
use crate::world::entity::{EntityId, sound_event};
use crate::world::level::pathfinder::path::Path;

pub const TIME_OUT_DURATION: i32 = 160;

#[derive(Clone, Copy, Debug)]
pub struct RamCandidate {
    start_position: BlockPos,
    target_position: BlockPos,
    target: EntityId,
}

impl RamCandidate {
    pub fn get_start_position(&self) -> BlockPos {
        self.start_position
    }

    pub fn get_target_position(&self) -> BlockPos {
        self.target_position
    }

    pub fn get_target(&self) -> EntityId {
        self.target
    }
}

pub struct PrepareRamNearestTarget {
    behavior: BehaviorState,
    get_cooldown_on_fail: fn(&ServerLevel, EntityId) -> i32,
    min_ram_distance: i32,
    max_ram_distance: i32,
    walk_speed: f32,
    ram_targeting: TargetingConditions<'static>,
    ram_prepare_time: i32,
    get_prepare_ram_sound: fn(&ServerLevel, EntityId) -> &'static str,
    reached_ram_position_timestamp: Option<i64>,
    ram_candidate: Option<RamCandidate>,
}

impl PrepareRamNearestTarget {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        get_cooldown_on_fail: fn(&ServerLevel, EntityId) -> i32,
        min_ram_distance: i32,
        max_ram_distance: i32,
        walk_speed: f32,
        ram_targeting: TargetingConditions<'static>,
        ram_prepare_time: i32,
        get_prepare_ram_sound: fn(&ServerLevel, EntityId) -> &'static str,
    ) -> PrepareRamNearestTarget {
        PrepareRamNearestTarget {
            behavior: BehaviorState::with_time_out(
                vec![
                    (memory_module_type::LOOK_TARGET.erase(), MemoryStatus::Registered),
                    (memory_module_type::RAM_COOLDOWN_TICKS.erase(), MemoryStatus::ValueAbsent),
                    (memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES.erase(), MemoryStatus::ValuePresent),
                    (memory_module_type::RAM_TARGET.erase(), MemoryStatus::ValueAbsent),
                ],
                TIME_OUT_DURATION,
            ),
            get_cooldown_on_fail,
            min_ram_distance,
            max_ram_distance,
            walk_speed,
            ram_targeting,
            ram_prepare_time,
            get_prepare_ram_sound,
            reached_ram_position_timestamp: None,
            ram_candidate: None,
        }
    }

    fn get_edge_of_block(start_ram_pos: BlockPos, target_pos: BlockPos) -> Vec3 {
        let offset_distance = 0.5;
        let x_offset = offset_distance * mth::sign((target_pos.x - start_ram_pos.x) as f64) as f64;
        let z_offset = offset_distance * mth::sign((target_pos.z - start_ram_pos.z) as f64) as f64;
        Vec3::at_bottom_center_of(target_pos).add(x_offset, 0.0, z_offset)
    }

    fn calculate_ramming_start_position(&self, level: &mut ServerLevel, body: EntityId, ramable_target: EntityId) -> Option<BlockPos> {
        let target_pos = level.entity(ramable_target).block_position();
        if !is_walkable_block(level, body, target_pos) {
            return None;
        }
        let mut possible_ram_positions = Vec::new();
        for direction in Direction::PLANE_HORIZONTAL {
            let mut walkable_pos_furthest_away_from_target = target_pos;
            for _ in 0..self.max_ram_distance {
                walkable_pos_furthest_away_from_target = walkable_pos_furthest_away_from_target.relative(direction);
                if !is_walkable_block(level, body, walkable_pos_furthest_away_from_target) {
                    walkable_pos_furthest_away_from_target = walkable_pos_furthest_away_from_target.relative(direction.opposite());
                    break;
                }
            }
            if walkable_pos_furthest_away_from_target.dist_manhattan(target_pos) >= self.min_ram_distance {
                possible_ram_positions.push(walkable_pos_furthest_away_from_target);
            }
        }
        let body_pos = level.entity(body).block_position();
        possible_ram_positions.sort_by(|&a, &b| body_pos.dist_sqr(a).total_cmp(&body_pos.dist_sqr(b)));
        possible_ram_positions.into_iter().find(|&pos| {
            let path = level.get_navigation(body, |navigation, level| navigation.create_path(level, body, pos, 0));
            path.as_ref().is_some_and(Path::can_reach)
        })
    }

    fn choose_ram_position(&mut self, level: &mut ServerLevel, body: EntityId, ramable_target: EntityId) {
        self.reached_ram_position_timestamp = None;
        let target_position = level.entity(ramable_target).block_position();
        self.ram_candidate = self.calculate_ramming_start_position(level, body, ramable_target).map(|pos| RamCandidate { start_position: pos, target_position, target: ramable_target });
    }
}

fn is_walkable_block(level: &ServerLevel, body: EntityId, target_pos: BlockPos) -> bool {
    level.mob(body).expect("a mob").ai.navigation.is_stable_destination(level, body, target_pos) && !goal_utils::has_malus(level, body, target_pos)
}

impl Behavior for PrepareRamNearestTarget {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let Some(living_entities) = level.get_brain(body).get_memory(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES).cloned() else { return };
        let ram_targeting = self.ram_targeting;
        if let Some(living_entity) = living_entities.find_closest(level, |level, entity| ram_targeting.test(level, Some(body), entity)) {
            self.choose_ram_position(level, body, living_entity);
        }
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        if !level.get_brain(body).has_memory_value(memory_module_type::RAM_TARGET) {
            level.broadcast_entity_event(body, 59);
            let cooldown = (self.get_cooldown_on_fail)(level, body);
            level.get_brain_mut(body).set_memory(memory_module_type::RAM_COOLDOWN_TICKS, cooldown);
        }
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, _body: EntityId, _timestamp: i64) -> bool {
        self.ram_candidate.is_some_and(|ram_candidate| level.is_alive(ram_candidate.get_target()))
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        let Some(ram_candidate) = self.ram_candidate else { return };
        let walk_target = WalkTarget::of_block_pos(ram_candidate.get_start_position(), self.walk_speed, 0);
        let look_target = PositionTracker::Entity(EntityTracker::new(level, ram_candidate.get_target(), true));
        let brain = level.get_brain_mut(body);
        brain.set_memory(memory_module_type::WALK_TARGET, walk_target);
        brain.set_memory(memory_module_type::LOOK_TARGET, look_target);
        let did_target_move = level.entity(ram_candidate.get_target()).block_position() != ram_candidate.get_target_position();
        if did_target_move {
            level.broadcast_entity_event(body, 59);
            level.get_navigation(body, |navigation, _| navigation.stop());
            self.choose_ram_position(level, body, ram_candidate.target);
        } else {
            let start_ram_pos = level.entity(body).block_position();
            if start_ram_pos == ram_candidate.get_start_position() {
                level.broadcast_entity_event(body, 58);
                let reached_ram_position_timestamp = *self.reached_ram_position_timestamp.get_or_insert(timestamp);
                if timestamp - reached_ram_position_timestamp >= self.ram_prepare_time as i64 {
                    let ram_target = PrepareRamNearestTarget::get_edge_of_block(start_ram_pos, ram_candidate.get_target_position());
                    level.get_brain_mut(body).set_memory(memory_module_type::RAM_TARGET, ram_target);
                    let pitch = level.voice_pitch(body);
                    let sound = (self.get_prepare_ram_sound)(level, body);
                    level.play_sound_from_entity(None, body, sound_event(sound), SoundSource::Neutral, 1.0, pitch);
                    self.ram_candidate = None;
                }
            }
        }
    }
}
