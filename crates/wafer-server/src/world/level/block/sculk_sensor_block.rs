use std::sync::LazyLock;

use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Direction};
use wafer_data::properties::{POWER, SCULK_SENSOR_PHASE, SculkSensorPhase, WATERLOGGED};
use wafer_data::{Block, BlockState, Fluid, block_tags, registry, registry_id};

use super::drop_experience_block::XpRange;
use super::note_block;
use super::simple_waterlogged_block::schedule_water_tick;
use super::BlockBehaviour;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::gameevent::game_event::{self, Context};
use crate::world::level::gameevent::vibrations::vibration_system;
use crate::world::level::update_flags::{UPDATE_ALL, UPDATE_CLIENTS, UPDATE_KNOWN_SHAPE};
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};

pub const ACTIVE_TICKS: i32 = 30;
pub const COOLDOWN_TICKS: i32 = 10;
pub const CALIBRATED_ACTIVE_TICKS: i32 = 10;

static RESONANCE_PITCH_BEND: LazyLock<[f32; 16]> = LazyLock::new(|| {
    let tone_map = [0, 0, 2, 4, 6, 7, 9, 10, 12, 14, 15, 18, 19, 21, 22, 24];
    tone_map.map(note_block::get_pitch_from_note)
});

pub struct SculkSensorBlock;

pub fn get_phase(state: BlockState) -> SculkSensorPhase {
    state.get(SCULK_SENSOR_PHASE)
}

pub fn can_activate(state: BlockState) -> bool {
    get_phase(state) == SculkSensorPhase::Inactive
}

fn get_active_ticks(state: BlockState) -> i32 {
    if state.block().is_instance_of("CalibratedSculkSensorBlock") { CALIBRATED_ACTIVE_TICKS } else { ACTIVE_TICKS }
}

fn update_neighbours(level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
    let block = state.block();
    level.update_neighbors_at(pos, block);
    level.update_neighbors_at(pos.below(), block);
}

pub fn deactivate(level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
    level.set_block(pos, state.set(SCULK_SENSOR_PHASE, SculkSensorPhase::Cooldown).set(POWER, 0), UPDATE_ALL);
    level.schedule_tick(pos, state.block(), COOLDOWN_TICKS);
    update_neighbours(level, pos, state);
}

pub fn activate(source_entity: Option<EntityId>, level: &mut ServerLevel, pos: BlockPos, state: BlockState, calculated_power: i32, vibration_frequency: i32) {
    level.set_block(pos, state.set(SCULK_SENSOR_PHASE, SculkSensorPhase::Active).set(POWER, calculated_power), UPDATE_ALL);
    level.schedule_tick(pos, state.block(), get_active_ticks(state));
    update_neighbours(level, pos, state);
    try_resonate_vibration(source_entity, level, pos, vibration_frequency);
    level.game_event_at(source_entity, game_event::SCULK_SENSOR_TENDRILS_CLICKING, pos);
    if !state.get(WATERLOGGED) {
        let pitch = level.random.next_float() * 0.2 + 0.8;
        level.play_sound_at(None, pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5, registry_id(registry::SOUND_EVENT, "minecraft:block.sculk_sensor.clicking"), SoundSource::Blocks, 1.0, pitch);
    }
}

pub fn try_resonate_vibration(source_entity: Option<EntityId>, level: &mut ServerLevel, pos: BlockPos, vibration_frequency: i32) {
    for direction in Direction::VALUES {
        let relative_pos = pos.relative(direction);
        let block_state = level.block_state(relative_pos);
        if block_state.is(&block_tags::VIBRATION_RESONATORS) {
            level.game_event_at_with(vibration_system::get_resonance_event_by_frequency(vibration_frequency), relative_pos, Context::of_entity_and_state(source_entity, Some(block_state)));
            let pitch = RESONANCE_PITCH_BEND[vibration_frequency as usize];
            level.play_sound(None, relative_pos, registry_id(registry::SOUND_EVENT, "minecraft:block.amethyst_block.resonate"), SoundSource::Blocks, 1.0, pitch);
        }
    }
}

impl BlockBehaviour for SculkSensorBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let replaced_fluid_state = level.fluid_state(ctx.clicked_pos());
        Some(block.default_state().set(WATERLOGGED, replaced_fluid_state.fluid == Fluid::Water))
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if get_phase(state) != SculkSensorPhase::Active {
            if get_phase(state) == SculkSensorPhase::Cooldown {
                level.set_block(pos, state.set(SCULK_SENSOR_PHASE, SculkSensorPhase::Inactive), UPDATE_ALL);
                if !state.get(WATERLOGGED) {
                    let pitch = level.random.next_float() * 0.2 + 0.8;
                    level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:block.sculk_sensor.clicking_stop"), SoundSource::Blocks, 1.0, pitch);
                }
            }
        } else {
            deactivate(level, pos, state);
        }
    }

    fn step_on(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, entity: EntityId) {
        if can_activate(state) && level.entity(entity).entity_type != registry_id(registry::ENTITY_TYPE, "minecraft:warden") {
            level.sculk_sensor_step_on_schedule(pos, entity, state);
        }
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, _moved_by_piston: bool) {
        if !old_state.is_block(state.block()) && state.get(POWER) > 0 && !level.has_scheduled_tick(pos, state.block()) {
            level.set_block(pos, state.set(POWER, 0), UPDATE_CLIENTS | UPDATE_KNOWN_SHAPE);
        }
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _moved_by_piston: bool) {
        if get_phase(state) == SculkSensorPhase::Active {
            update_neighbours(level, pos, state);
        }
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        schedule_water_tick(state, level, pos);
        state
    }

    fn own_signal(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos) -> i32 {
        state.get(POWER)
    }

    fn get_direct_signal(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos, direction: Direction) -> i32 {
        if direction == Direction::Up { super::of(state).get_signal(state, level, pos, direction) } else { 0 }
    }

    fn get_analog_output_signal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _direction: Direction) -> i32 {
        match level.sculk_sensor_last_vibration_frequency(pos) {
            Some(last_vibration_frequency) if get_phase(state) == SculkSensorPhase::Active => last_vibration_frequency,
            _ => 0,
        }
    }

    fn spawn_after_break(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, tool: Option<ItemStack>, drop_experience: bool) {
        if drop_experience {
            level.try_drop_experience(pos, tool.as_ref(), XpRange::Constant(5));
        }
    }
}
