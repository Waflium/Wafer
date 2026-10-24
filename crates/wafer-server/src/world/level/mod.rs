pub mod base_command_block;
pub mod base_spawner;
#[cfg(test)]
mod base_spawner_tests;
pub mod biome;
pub mod block;
pub mod blockscan;
pub mod border;
pub mod chunk;
pub mod data_pack_config;
pub mod dimension;
pub mod gameevent;
pub mod gamerules;
pub mod clip;
pub mod custom_spawner;
pub mod entity;
pub mod light_layer;
pub mod server_explosion;
pub mod collision;
pub mod levelgen;
pub mod lighting;
pub mod local_mob_cap_calculator;
pub mod material;
pub mod moon_phase;
pub mod natural_spawner;
pub mod noise_column;
pub mod path_navigation_region;
pub mod pathfinder;
pub mod portal;
pub mod potential_calculator;
pub mod saveddata;
pub mod redstone;
pub mod server_level_accessor;
pub mod signal;
pub mod spawn_data;
pub mod storage;
pub mod structure_manager;
pub mod timers;
pub mod validation;
pub mod world_data_configuration;
pub mod world_gen_level;

use std::borrow::Cow;

use wafer_core::random::RandomSource;
use wafer_core::shapes::{self, VoxelShape};
use wafer_core::{BlockPos, Direction};
use wafer_data::{Block, BlockState, Fluid, FluidState, SupportType};

use crate::world::phys::shapes::collision_context::CollisionContext;
use crate::world::ticks::TickPriority;

pub fn is_in_spawnable_bounds(pos: BlockPos) -> bool {
    (-20_000_000..20_000_000).contains(&pos.y) && (-30_000_000..30_000_000).contains(&pos.x) && (-30_000_000..30_000_000).contains(&pos.z)
}

pub trait BlockGetter {
    fn block_state(&mut self, pos: BlockPos) -> BlockState;

    fn get_min_y(&self) -> i32;

    fn fluid_state(&mut self, pos: BlockPos) -> FluidState {
        self.block_state(pos).fluid_state()
    }

    fn get_block_support_shape(&mut self, state: BlockState, pos: BlockPos) -> Cow<'static, VoxelShape> {
        state.get_block_support_shape(pos)
    }

    fn get_state_collision_shape(&mut self, state: BlockState, pos: BlockPos) -> Cow<'static, VoxelShape> {
        state.get_collision_shape(pos)
    }

    fn get_state_collision_shape_in_context(&mut self, state: BlockState, pos: BlockPos, context: &CollisionContext) -> Cow<'static, VoxelShape> {
        context.get_moved_collision_shape(state, pos)
    }

    fn is_shulker_box_closed(&mut self, _pos: BlockPos) -> bool {
        true
    }

    fn is_collision_shape_full_block_at(&mut self, pos: BlockPos, state: BlockState) -> bool {
        if state.has_dynamic_shape() {
            let shape = self.get_state_collision_shape(state, pos);
            shapes::is_shape_full_block(&shape)
        } else {
            state.is_collision_shape_full_block()
        }
    }

    fn is_redstone_conductor_at(&mut self, pos: BlockPos, state: BlockState) -> bool {
        state.is_redstone_conductor() && (!state.has_dynamic_shape() || self.is_collision_shape_full_block_at(pos, state))
    }

    fn is_suffocating_at(&mut self, pos: BlockPos, state: BlockState) -> bool {
        if !state.has_dynamic_shape() {
            state.is_suffocating()
        } else if state.block().is_instance_of("ShulkerBoxBlock") {
            self.is_shulker_box_closed(pos)
        } else if state.is_suffocating() {
            self.is_collision_shape_full_block_at(pos, state)
        } else {
            false
        }
    }

    fn is_face_sturdy_at(&mut self, pos: BlockPos, state: BlockState, direction: Direction, support: SupportType) -> bool {
        if state.has_dynamic_shape() {
            let support_shape = self.get_block_support_shape(state, pos);
            block::support_type::is_supporting(&support_shape, direction, support)
        } else {
            state.is_face_sturdy_cached(direction, support)
        }
    }

    fn is_face_sturdy_full_at(&mut self, pos: BlockPos, state: BlockState, direction: Direction) -> bool {
        self.is_face_sturdy_at(pos, state, direction, SupportType::Full)
    }
}

pub trait LevelReader: BlockGetter {
    fn raw_brightness(&mut self, pos: BlockPos, darkening: i32) -> i32;

    fn get_brightness(&mut self, layer: light_layer::LightLayer, pos: BlockPos) -> i32;

    fn get_sea_level(&self) -> i32;

    fn is_inside_build_height(&self, y: i32) -> bool;

    fn is_fast_lava(&self) -> bool;

    fn get_control_input_signal(&mut self, pos: BlockPos, direction: Direction, only_diodes: bool) -> i32;

    fn get_block_entity(&mut self, pos: BlockPos) -> Option<&mut block::entity::BlockEntity>;

    fn is_water_at(&mut self, pos: BlockPos) -> bool {
        self.fluid_state(pos).is(&wafer_data::fluid_tags::WATER)
    }
}

pub trait ScheduledTickAccess {
    fn schedule_tick_with_priority(&mut self, pos: BlockPos, block: Block, delay: i32, priority: TickPriority);

    fn schedule_fluid_tick(&mut self, pos: BlockPos, fluid: Fluid, delay: i32);

    fn has_scheduled_tick(&self, pos: BlockPos, block: Block) -> bool;

    fn schedule_tick(&mut self, pos: BlockPos, block: Block, delay: i32) {
        self.schedule_tick_with_priority(pos, block, delay, TickPriority::NORMAL);
    }
}

pub trait LevelWriter {
    fn set_block(&mut self, pos: BlockPos, state: BlockState, flags: i32) -> bool;
}

pub trait LevelAccessor: LevelReader + ScheduledTickAccess + LevelWriter {
    fn get_random(&mut self) -> &mut dyn RandomSource;

    fn mark_pos_for_post_processing(&mut self, pos: BlockPos);

    fn play_sound(&mut self, _pos: BlockPos, _sound: i32, _source: crate::server::level::SoundSource, _volume: f32, _pitch: f32) {}

    fn game_event(&mut self, _game_event: gameevent::game_event::GameEvent, _position: wafer_core::Vec3, _context: gameevent::game_event::Context) {}

    fn as_server_level(&mut self) -> Option<&mut crate::server::level::ServerLevel> {
        None
    }
}

pub mod update_flags {
    pub const UPDATE_NEIGHBORS: i32 = 1;
    pub const UPDATE_CLIENTS: i32 = 2;
    pub const UPDATE_INVISIBLE: i32 = 4;
    pub const UPDATE_IMMEDIATE: i32 = 8;
    pub const UPDATE_KNOWN_SHAPE: i32 = 16;
    pub const UPDATE_SUPPRESS_DROPS: i32 = 32;
    pub const UPDATE_MOVE_BY_PISTON: i32 = 64;
    pub const UPDATE_SKIP_SHAPE_UPDATE_ON_WIRE: i32 = 128;
    pub const UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS: i32 = 256;
    pub const UPDATE_SKIP_ON_PLACE: i32 = 512;
    pub const UPDATE_NONE: i32 = UPDATE_INVISIBLE | UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS;
    pub const UPDATE_ALL: i32 = UPDATE_NEIGHBORS | UPDATE_CLIENTS;
    pub const UPDATE_ALL_IMMEDIATE: i32 = UPDATE_ALL | UPDATE_IMMEDIATE;
    pub const UPDATE_LIMIT: i32 = 512;
}
