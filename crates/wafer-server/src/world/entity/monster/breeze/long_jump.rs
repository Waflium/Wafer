use wafer_core::{BlockPos, Direction, Vec3};
use wafer_data::{blocks, fluid_tags, mob_effects};

use super::breeze_util;
use crate::commands::source::Anchor;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::ai::behavior::behavior::{Behavior, BehaviorState};
use crate::world::entity::ai::behavior::long_jump_util;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::data::Pose;
use crate::world::entity::{EntityId, entity_types, sound_event};
use crate::world::level::BlockGetter;
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};
use crate::world::phys::shapes::collision_context::CollisionContext;

const REQUIRED_AIR_BLOCKS_ABOVE: i32 = 4;
const JUMP_COOLDOWN_TICKS: i64 = 10;
const JUMP_COOLDOWN_WHEN_HURT_TICKS: i64 = 2;
const INHALING_DURATION_TICKS: i64 = 10;
const MAX_JUMP_VELOCITY_MULTIPLIER: f32 = 0.058333334;
const ALLOWED_ANGLES: [i32; 5] = [40, 55, 60, 75, 80];

pub struct LongJump {
    state: BehaviorState,
}

impl Default for LongJump {
    fn default() -> LongJump {
        let entry_condition = vec![
            (memory_module_type::ATTACK_TARGET.erase(), MemoryStatus::ValuePresent),
            (memory_module_type::BREEZE_JUMP_COOLDOWN.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::BREEZE_JUMP_INHALING.erase(), MemoryStatus::Registered),
            (memory_module_type::BREEZE_JUMP_TARGET.erase(), MemoryStatus::Registered),
            (memory_module_type::BREEZE_SHOOT.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::BREEZE_LEAVING_WATER.erase(), MemoryStatus::Registered),
        ];
        LongJump { state: BehaviorState::with_time_out(entry_condition, 200) }
    }
}

pub fn can_run(level: &mut ServerLevel, breeze: EntityId) -> bool {
    let e = level.entity(breeze);
    if !e.on_ground && !e.is_in_water() {
        return false;
    }
    if e.fluid_height(&fluid_tags::ENTITY_FLOATABLE) > e.fluid_jump_threshold() || e.is_in_lava() {
        return false;
    }
    if level.get_brain(breeze).check_memory(memory_module_type::BREEZE_JUMP_TARGET.erase(), MemoryStatus::ValuePresent) {
        return true;
    }
    let Some(attack_target) = level.get_brain(breeze).get_memory(memory_module_type::ATTACK_TARGET).copied() else { return false };
    if out_of_aggro_range(level, breeze, attack_target) {
        level.get_brain_mut(breeze).erase_memory(memory_module_type::ATTACK_TARGET);
        return false;
    }
    if too_close_for_jump(level, breeze, attack_target) {
        return false;
    }
    if !can_jump_from_current_position(level, breeze) {
        return false;
    }
    let point = breeze_util::random_point_behind_target(level, attack_target, breeze);
    let Some(target_pos) = snap_to_surface(level, breeze, point) else { return false };
    let bs = level.block_state(target_pos.below());
    if entity_types::BREEZE.is_block_dangerous(bs) {
        return false;
    }
    if !breeze_util::has_line_of_sight(level, breeze, Vec3::at_center_of(target_pos)) && !breeze_util::has_line_of_sight(level, breeze, Vec3::at_center_of(target_pos.relative_n(Direction::Up, 4))) {
        return false;
    }
    level.get_brain_mut(breeze).set_memory(memory_module_type::BREEZE_JUMP_TARGET, target_pos);
    true
}

fn is_finished_inhaling(level: &ServerLevel, breeze: EntityId) -> bool {
    level.get_brain(breeze).get_memory(memory_module_type::BREEZE_JUMP_INHALING).is_none() && level.entity(breeze).pose() == Pose::Inhaling
}

fn is_finished_jumping(level: &ServerLevel, breeze: EntityId) -> bool {
    let e = level.entity(breeze);
    let is_jumping = e.pose() == Pose::LongJumping;
    let landed_on_ground = e.on_ground;
    let landed_in_water = e.is_in_water() && level.get_brain(breeze).check_memory(memory_module_type::BREEZE_LEAVING_WATER.erase(), MemoryStatus::ValueAbsent);
    is_jumping && (landed_on_ground || landed_in_water)
}

fn snap_to_surface(level: &mut ServerLevel, entity: EntityId, target: Vec3) -> Option<BlockPos> {
    let collision_context = CollisionContext::of(level, entity);
    let collision_below = ClipContext { from: target, to: target.add(0.0, -10.0, 0.0), block: ClipBlock::Collider, fluid: ClipFluid::None, collision_context };
    let surface_below = level.clip(&collision_below);
    if !surface_below.miss {
        return Some(BlockPos::containing_vec(surface_below.hit.location).above());
    }
    let collision_above = ClipContext { from: target, to: target.add(0.0, 10.0, 0.0), block: ClipBlock::Collider, fluid: ClipFluid::None, collision_context };
    let surface_above = level.clip(&collision_above);
    (!surface_above.miss).then(|| BlockPos::containing_vec(surface_above.hit.location).above())
}

fn out_of_aggro_range(level: &ServerLevel, breeze: EntityId, attack_target: EntityId) -> bool {
    let range = level.attribute_value(breeze, Attribute::FollowRange);
    !(level.entity(attack_target).distance_to_sqr(level.entity(breeze)) < range * range)
}

fn too_close_for_jump(level: &ServerLevel, breeze: EntityId, attack_target: EntityId) -> bool {
    level.entity(attack_target).distance_to(level.entity(breeze)) - 4.0 <= 0.0
}

fn can_jump_from_current_position(level: &mut ServerLevel, breeze: EntityId) -> bool {
    let current_pos = level.entity(breeze).block_position();
    if level.block_state(current_pos).is_block(blocks::HONEY_BLOCK) {
        return false;
    }
    for i in 1..=REQUIRED_AIR_BLOCKS_ABOVE {
        let offset_pos = current_pos.relative_n(Direction::Up, i);
        let state = level.block_state(offset_pos);
        if !state.is_air() && !state.fluid_state().is(&fluid_tags::WATER) {
            return false;
        }
    }
    true
}

fn calculate_optimal_jump_vector(level: &mut ServerLevel, body: EntityId, target_pos: Vec3) -> Option<Vec3> {
    let mut angles = ALLOWED_ANGLES.to_vec();
    crate::util::shuffle(&mut angles, &mut level.entity_mut(body).random);
    for angle in angles {
        let max_jump_velocity = MAX_JUMP_VELOCITY_MULTIPLIER * level.attribute_value(body, Attribute::FollowRange) as f32;
        let Some(velocity_vector) = long_jump_util::calculate_jump_vector_for_angle(level, body, target_pos, max_jump_velocity, angle, false) else { continue };
        if level.has_effect(body, mob_effects::JUMP_BOOST) {
            let jump_effect_amplifier = velocity_vector.normalize().y * level.jump_boost_power(body) as f64;
            return Some(velocity_vector.add(0.0, jump_effect_amplifier, 0.0));
        }
        return Some(velocity_vector);
    }
    None
}

impl Behavior for LongJump {
    fn behavior(&self) -> &BehaviorState {
        &self.state
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.state
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, breeze: EntityId) -> bool {
        can_run(level, breeze)
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, breeze: EntityId, _timestamp: i64) -> bool {
        level.entity(breeze).pose() != Pose::Standing && !level.get_brain(breeze).has_memory_value(memory_module_type::BREEZE_JUMP_COOLDOWN)
    }

    fn start(&mut self, level: &mut ServerLevel, breeze: EntityId, _timestamp: i64) {
        if level.get_brain(breeze).check_memory(memory_module_type::BREEZE_JUMP_INHALING.erase(), MemoryStatus::ValueAbsent) {
            level.get_brain_mut(breeze).set_memory_with_expiry(memory_module_type::BREEZE_JUMP_INHALING, (), INHALING_DURATION_TICKS);
        }
        level.set_entity_pose(breeze, Pose::Inhaling);
        level.play_sound_from_entity(None, breeze, sound_event("minecraft:entity.breeze.charge"), SoundSource::Hostile, 1.0, 1.0);
        if let Some(target_pos) = level.get_brain(breeze).get_memory(memory_module_type::BREEZE_JUMP_TARGET).copied() {
            level.entity_look_at(breeze, Anchor::Eyes, Vec3::at_center_of(target_pos));
        }
    }

    fn tick(&mut self, level: &mut ServerLevel, breeze: EntityId, _timestamp: i64) {
        let in_water = level.entity(breeze).is_in_water();
        if !in_water && level.get_brain(breeze).check_memory(memory_module_type::BREEZE_LEAVING_WATER.erase(), MemoryStatus::ValuePresent) {
            level.get_brain_mut(breeze).erase_memory(memory_module_type::BREEZE_LEAVING_WATER);
        }
        if is_finished_inhaling(level, breeze) {
            let target_pos = level.get_brain(breeze).get_memory(memory_module_type::BREEZE_JUMP_TARGET).copied();
            let Some(velocity_vector) = target_pos.and_then(|target_pos| calculate_optimal_jump_vector(level, breeze, Vec3::at_bottom_center_of(target_pos))) else {
                level.set_entity_pose(breeze, Pose::Standing);
                return;
            };
            if in_water {
                level.get_brain_mut(breeze).set_memory(memory_module_type::BREEZE_LEAVING_WATER, ());
            }
            level.entity_play_sound(breeze, sound_event("minecraft:entity.breeze.jump"), 1.0, 1.0);
            level.set_entity_pose(breeze, Pose::LongJumping);
            let y_body_rot = level.living(breeze).y_body_rot;
            level.entity_mut(breeze).set_y_rot(y_body_rot);
            level.living_mut(breeze).discard_friction = true;
            level.entity_mut(breeze).set_delta_movement(velocity_vector);
        } else if is_finished_jumping(level, breeze) {
            level.entity_play_sound(breeze, sound_event("minecraft:entity.breeze.land"), 1.0, 1.0);
            level.set_entity_pose(breeze, Pose::Standing);
            level.living_mut(breeze).discard_friction = false;
            let was_hurt = level.get_brain(breeze).has_memory_value(memory_module_type::HURT_BY);
            let brain = level.get_brain_mut(breeze);
            brain.set_memory_with_expiry(memory_module_type::BREEZE_JUMP_COOLDOWN, (), if was_hurt { JUMP_COOLDOWN_WHEN_HURT_TICKS } else { JUMP_COOLDOWN_TICKS });
            brain.set_memory_with_expiry(memory_module_type::BREEZE_SHOOT, (), 100);
        }
    }

    fn stop(&mut self, level: &mut ServerLevel, breeze: EntityId, _timestamp: i64) {
        let pose = level.entity(breeze).pose();
        if pose == Pose::LongJumping || pose == Pose::Inhaling {
            level.set_entity_pose(breeze, Pose::Standing);
        }
        let brain = level.get_brain_mut(breeze);
        brain.erase_memory(memory_module_type::BREEZE_JUMP_TARGET);
        brain.erase_memory(memory_module_type::BREEZE_JUMP_INHALING);
        brain.erase_memory(memory_module_type::BREEZE_LEAVING_WATER);
    }
}
