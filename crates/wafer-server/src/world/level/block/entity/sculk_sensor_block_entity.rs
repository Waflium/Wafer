use wafer_core::{BlockPos, Vec3};
use wafer_data::properties::HORIZONTAL_FACING;
use wafer_protocol::nbt::Nbt;

use super::BlockEntityKind;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::block::sculk_sensor_block;
use crate::world::level::gameevent::game_event::{self, Context, GameEvent};
use crate::world::level::gameevent::vibrations::vibration_system::{self, Data, VibrationUser};
use crate::world::level::signal;
use crate::world::level::storage::tag_value_input;

pub const DEFAULT_LAST_VIBRATION_FREQUENCY: i32 = 0;
pub const LISTENER_RANGE: i32 = 8;
pub const CALIBRATED_LISTENER_RANGE: i32 = 16;

#[derive(Clone, Default)]
pub struct SculkSensorBlockEntity {
    pub vibration_data: Data,
    pub last_vibration_frequency: i32,
}

impl SculkSensorBlockEntity {
    pub fn load_additional(&mut self, input: &Nbt) {
        self.last_vibration_frequency = tag_value_input::get_i64(input, "last_vibration_frequency").map_or(DEFAULT_LAST_VIBRATION_FREQUENCY, |value| value as i32);
        self.vibration_data = Data::load(input.get("listener"));
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("last_vibration_frequency".into(), Nbt::Int(self.last_vibration_frequency)));
        output.push(("listener".into(), self.vibration_data.to_nbt()));
    }
}

impl ServerLevel {
    fn sculk_sensor(&self, pos: BlockPos, id: u64) -> Option<&SculkSensorBlockEntity> {
        match self.chunks.block_entity_ref(pos).filter(|entity| entity.id() == id).map(|entity| &entity.kind) {
            Some(BlockEntityKind::SculkSensor(sensor)) => Some(sensor),
            _ => None,
        }
    }

    pub fn sculk_sensor_vibration_data(&self, pos: BlockPos, id: u64) -> Option<&Data> {
        self.sculk_sensor(pos, id).map(|sensor| &sensor.vibration_data)
    }

    pub fn sculk_sensor_vibration_data_mut(&mut self, pos: BlockPos, id: u64) -> Option<&mut Data> {
        match self.get_block_entity(pos).filter(|entity| entity.id() == id).map(|entity| &mut entity.kind) {
            Some(BlockEntityKind::SculkSensor(sensor)) => Some(&mut sensor.vibration_data),
            _ => None,
        }
    }

    pub fn sculk_sensor_vibration_user(&self, pos: BlockPos) -> Option<VibrationUser> {
        let entity = self.chunks.block_entity_ref(pos)?;
        if !matches!(entity.kind, BlockEntityKind::SculkSensor(_)) {
            return None;
        }
        let id = entity.id();
        Some(if entity.ty() == wafer_data::block_entity_types::CALIBRATED_SCULK_SENSOR { VibrationUser::CalibratedSculkSensor { pos, id } } else { VibrationUser::SculkSensor { pos, id } })
    }

    pub fn sculk_sensor_last_vibration_frequency(&self, pos: BlockPos) -> Option<i32> {
        match self.chunks.block_entity_ref(pos).map(|entity| &entity.kind) {
            Some(BlockEntityKind::SculkSensor(sensor)) => Some(sensor.last_vibration_frequency),
            _ => None,
        }
    }

    pub fn sculk_sensor_can_receive_vibration(&mut self, block_pos: BlockPos, id: u64, pos: BlockPos, event: GameEvent, _context: Context) -> bool {
        if pos == block_pos && (event == game_event::BLOCK_DESTROY || event == game_event::BLOCK_PLACE) {
            return false;
        }
        if vibration_system::get_game_event_frequency(event) == 0 {
            return false;
        }
        self.chunks.block_entity_ref(block_pos).filter(|entity| entity.id() == id).is_some_and(|entity| sculk_sensor_block::can_activate(entity.block_state()))
    }

    pub fn calibrated_sculk_sensor_can_receive_vibration(&mut self, block_pos: BlockPos, id: u64, pos: BlockPos, event: GameEvent, context: Context) -> bool {
        let Some(state) = self.chunks.block_entity_ref(block_pos).filter(|entity| entity.id() == id).map(|entity| entity.block_state()) else { return false };
        let comparison_type = self.calibrated_get_back_signal(block_pos, state);
        if comparison_type != 0 && vibration_system::get_game_event_frequency(event) != comparison_type {
            false
        } else {
            self.sculk_sensor_can_receive_vibration(block_pos, id, pos, event, context)
        }
    }

    fn calibrated_get_back_signal(&mut self, pos: BlockPos, state: wafer_data::BlockState) -> i32 {
        let direction = state.get(HORIZONTAL_FACING).opposite();
        signal::get_signal(self, pos.relative(direction), direction)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sculk_sensor_on_receive_vibration(
        &mut self,
        user: VibrationUser,
        block_pos: BlockPos,
        id: u64,
        _pos: BlockPos,
        event: GameEvent,
        source_entity: Option<EntityId>,
        _projectile_owner: Option<EntityId>,
        receiving_distance: f32,
    ) {
        let Some(state) = self.chunks.block_entity_ref(block_pos).filter(|entity| entity.id() == id).map(|entity| entity.block_state()) else { return };
        if !sculk_sensor_block::can_activate(state) {
            return;
        }
        let event_frequency = vibration_system::get_game_event_frequency(event);
        if let Some(BlockEntityKind::SculkSensor(sensor)) = self.get_block_entity(block_pos).map(|entity| &mut entity.kind) {
            sensor.last_vibration_frequency = event_frequency;
        }
        let calculated_power = vibration_system::get_redstone_strength_for_distance(receiving_distance, self.vibration_user_listener_radius(user));
        if state.block().is_instance_of("SculkSensorBlock") {
            sculk_sensor_block::activate(source_entity, self, block_pos, state, calculated_power, event_frequency);
        }
    }

    pub fn sculk_sensor_step_on_schedule(&mut self, pos: BlockPos, entity: EntityId, on_state: wafer_data::BlockState) {
        let Some(user) = self.sculk_sensor_vibration_user(pos) else { return };
        let can_receive = match user {
            VibrationUser::SculkSensor { pos: block_pos, id } => self.sculk_sensor_can_receive_vibration(block_pos, id, pos, game_event::STEP, Context::of_state(Some(on_state))),
            VibrationUser::CalibratedSculkSensor { pos: block_pos, id } => self.calibrated_sculk_sensor_can_receive_vibration(block_pos, id, pos, game_event::STEP, Context::of_state(Some(on_state))),
            VibrationUser::SculkShrieker { .. } | VibrationUser::Allay(_) | VibrationUser::Warden(_) => false,
        };
        if can_receive {
            let position: Vec3 = self.entity(entity).position();
            self.vibration_listener_force_schedule_vibration(user, game_event::STEP, Context::of(Some(entity)), position);
        }
    }
}
