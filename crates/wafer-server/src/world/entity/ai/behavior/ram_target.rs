use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::{block_tags, damage_types, mob_effects};

use super::behavior::{Behavior, BehaviorState};
use crate::server::level::{ServerLevel, SoundSource};
use crate::util::valueproviders::IntProvider;
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::{EntityId, sound_event};
use crate::world::level::BlockGetter;

pub const TIME_OUT_DURATION: i32 = 200;
pub const RAM_SPEED_FORCE_FACTOR: f32 = 1.65;

pub struct RamTarget {
    behavior: BehaviorState,
    get_time_between_rams: fn(&ServerLevel, EntityId) -> IntProvider,
    ram_targeting: TargetingConditions<'static>,
    speed: f32,
    get_knockback_force: fn(&ServerLevel, EntityId) -> f64,
    ram_direction: Vec3,
    get_impact_sound: fn(&ServerLevel, EntityId) -> &'static str,
    get_horn_break_sound: fn(&ServerLevel, EntityId) -> &'static str,
}

impl RamTarget {
    pub fn new(
        get_time_between_rams: fn(&ServerLevel, EntityId) -> IntProvider,
        ram_targeting: TargetingConditions<'static>,
        speed: f32,
        get_knockback_force: fn(&ServerLevel, EntityId) -> f64,
        get_impact_sound: fn(&ServerLevel, EntityId) -> &'static str,
        get_horn_break_sound: fn(&ServerLevel, EntityId) -> &'static str,
    ) -> RamTarget {
        RamTarget {
            behavior: BehaviorState::with_time_out(
                vec![(memory_module_type::RAM_COOLDOWN_TICKS.erase(), MemoryStatus::ValueAbsent), (memory_module_type::RAM_TARGET.erase(), MemoryStatus::ValuePresent)],
                TIME_OUT_DURATION,
            ),
            get_time_between_rams,
            ram_targeting,
            speed,
            get_knockback_force,
            ram_direction: Vec3::ZERO,
            get_impact_sound,
            get_horn_break_sound,
        }
    }

    fn has_rammed_horn_breaking_block(&self, level: &mut ServerLevel, body: EntityId) -> bool {
        let e = level.entity(body);
        let horizontal_movement_normalized = e.delta_movement.multiply(1.0, 0.0, 1.0).normalize();
        let facing_block_position = BlockPos::containing_vec(e.position().add_vec(horizontal_movement_normalized));
        level.block_state(facing_block_position).is(&block_tags::SNAPS_GOAT_HORN) || level.block_state(facing_block_position.above()).is(&block_tags::SNAPS_GOAT_HORN)
    }

    fn finish_ram(&self, level: &mut ServerLevel, body: EntityId) {
        level.broadcast_entity_event(body, 59);
        let time_between_rams = (self.get_time_between_rams)(level, body).sample(&mut level.random);
        let brain = level.get_brain_mut(body);
        brain.set_memory(memory_module_type::RAM_COOLDOWN_TICKS, time_between_rams);
        brain.erase_memory(memory_module_type::RAM_TARGET);
    }
}

impl Behavior for RamTarget {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        level.get_brain(body).has_memory_value(memory_module_type::RAM_TARGET)
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        level.get_brain(body).has_memory_value(memory_module_type::RAM_TARGET)
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let cur_pos = level.entity(body).block_position();
        let ram_target_pos = *level.get_brain(body).get_memory(memory_module_type::RAM_TARGET).expect("a ram target");
        self.ram_direction = Vec3::new(cur_pos.x as f64 - ram_target_pos.x, 0.0, cur_pos.z as f64 - ram_target_pos.z).normalize();
        level.get_brain_mut(body).set_memory(memory_module_type::WALK_TARGET, crate::world::entity::ai::memory::walk_target::WalkTarget::of_vec(ram_target_pos, self.speed, 0));
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let bounding_box = level.entity(body).bounding_box();
        let ram_targeting = self.ram_targeting;
        let nearby_entities: Vec<EntityId> = level.get_living_entities(&bounding_box, |_, _| true).into_iter().filter(|&entity| ram_targeting.test(level, Some(body), entity)).collect();
        if let Some(&ram_target) = nearby_entities.first() {
            let damage_source = DamageSource::entity(damage_types::MOB_ATTACK_NO_AGGRO, body);
            let damage = level.attribute_value(body, Attribute::AttackDamage) as f32;
            if level.hurt_server(ram_target, damage_source, damage) {
                level.do_post_attack_effects(ram_target, &damage_source);
            }
            let living = level.living(body);
            let movement_speed_level = living.effect(mob_effects::SPEED).map_or(0, |effect| effect.amplifier + 1);
            let movement_slowdown_level = living.effect(mob_effects::SLOWNESS).map_or(0, |effect| effect.amplifier + 1);
            let speed_boost_power = 0.25 * (movement_speed_level - movement_slowdown_level) as f32;
            let speed_factor = mth::clamp_f(living.speed * RAM_SPEED_FORCE_FACTOR, 0.2, 3.0) + speed_boost_power;
            let source = DamageSource::entity(damage_types::MOB_ATTACK, body);
            let blocked_damage = level.apply_item_blocking(ram_target, &source, damage);
            let blocking_factor = if blocked_damage > 0.0 { 0.5 } else { 1.0 };
            let knockback_force = (self.get_knockback_force)(level, body);
            level.knockback(ram_target, (blocking_factor * speed_factor) as f64 * knockback_force, self.ram_direction.x, self.ram_direction.z, &source, damage, false);
            self.finish_ram(level, body);
            let sound = (self.get_impact_sound)(level, body);
            level.play_sound_from_entity(None, body, sound_event(sound), SoundSource::Neutral, 1.0, 1.0);
        } else if self.has_rammed_horn_breaking_block(level, body) {
            let sound = (self.get_impact_sound)(level, body);
            level.play_sound_from_entity(None, body, sound_event(sound), SoundSource::Neutral, 1.0, 1.0);
            let dropped = level.goat_drop_horn(body);
            if dropped {
                let sound = (self.get_horn_break_sound)(level, body);
                level.play_sound_from_entity(None, body, sound_event(sound), SoundSource::Neutral, 1.0, 1.0);
            }
            self.finish_ram(level, body);
        } else {
            let brain = level.get_brain(body);
            let walk_target = brain.get_memory(memory_module_type::WALK_TARGET).cloned();
            let ram_target = brain.get_memory(memory_module_type::RAM_TARGET).copied();
            let lost_or_reached_target = match (walk_target, ram_target) {
                (Some(walk_target), Some(ram_target)) => walk_target.get_target().current_position(level).closer_than(ram_target, 0.25),
                _ => true,
            };
            if lost_or_reached_target {
                self.finish_ram(level, body);
            }
        }
    }
}
