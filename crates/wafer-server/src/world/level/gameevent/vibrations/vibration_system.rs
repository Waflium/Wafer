use std::sync::LazyLock;

use wafer_core::{BlockPos, ChunkPos, Direction, Vec3, mth};
use wafer_data::{BlockState, Tag, block_tags, game_event_tags};
use wafer_protocol::nbt::Nbt;

use super::vibration_info::VibrationInfo;
use super::vibration_selector::VibrationSelector;
use crate::advancements::triggers::criteria_triggers;
use crate::server::level::ServerLevel;
use crate::util::codec::DecodeResult;
use crate::util::extra_codecs;
use crate::world::entity::EntityId;
use crate::world::item::component::codec::{Codec, Stream};
use crate::world::item::component::types::{Pair, always, codec_float_range, fixed, float, lenient_opt, non_negative_int, nbt, record, req, var_int};
use crate::world::level::storage::uuid_ints;
use crate::world::level::gameevent::game_event::{self, Context, GameEvent, RESONATE_1};
use crate::world::level::gameevent::position_source::PositionSource;

pub const NO_VIBRATION_FREQUENCY: i32 = 0;

pub fn get_game_event_frequency(event: GameEvent) -> i32 {
    match event {
        game_event::STEP | game_event::SWIM | game_event::FLAP => 1,
        game_event::PROJECTILE_LAND | game_event::HIT_GROUND | game_event::SPLASH | game_event::BOUNCE => 2,
        game_event::ITEM_INTERACT_FINISH | game_event::PROJECTILE_SHOOT | game_event::INSTRUMENT_PLAY => 3,
        game_event::ENTITY_ACTION | game_event::ELYTRA_GLIDE | game_event::UNEQUIP => 4,
        game_event::ENTITY_DISMOUNT | game_event::EQUIP => 5,
        game_event::ENTITY_INTERACT | game_event::SHEAR | game_event::ENTITY_MOUNT => 6,
        game_event::ENTITY_DAMAGE => 7,
        game_event::DRINK | game_event::EAT => 8,
        game_event::CONTAINER_CLOSE | game_event::BLOCK_CLOSE | game_event::BLOCK_DEACTIVATE | game_event::BLOCK_DETACH => 9,
        game_event::CONTAINER_OPEN | game_event::BLOCK_OPEN | game_event::BLOCK_ACTIVATE | game_event::BLOCK_ATTACH | game_event::PRIME_FUSE | game_event::NOTE_BLOCK_PLAY => 10,
        game_event::BLOCK_CHANGE => 11,
        game_event::BLOCK_DESTROY | game_event::FLUID_PICKUP => 12,
        game_event::BLOCK_PLACE | game_event::FLUID_PLACE => 13,
        game_event::ENTITY_PLACE | game_event::LIGHTNING_STRIKE | game_event::TELEPORT => 14,
        game_event::ENTITY_DIE | game_event::EXPLODE => 15,
        GameEvent(id) if (RESONATE_1.0..RESONATE_1.0 + 15).contains(&id) => (id - RESONATE_1.0) as i32 + 1,
        _ => NO_VIBRATION_FREQUENCY,
    }
}

pub fn get_resonance_event_by_frequency(vibration_frequency: i32) -> GameEvent {
    GameEvent(RESONATE_1.0 + (vibration_frequency - 1) as u8)
}

pub fn get_redstone_strength_for_distance(distance: f32, listener_radius: i32) -> i32 {
    let power_scale = 15.0 / listener_radius as f64;
    (15 - mth::floor(power_scale * distance as f64)).max(1)
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Data {
    pub current_vibration: Option<VibrationInfo>,
    pub travel_time_in_ticks: i32,
    pub selection_strategy: VibrationSelector,
    pub reload_vibration_particle: bool,
}

impl Data {
    pub fn decrement_travel_time(&mut self) {
        self.travel_time_in_ticks = (self.travel_time_in_ticks - 1).max(0);
    }

    pub fn to_nbt(&self) -> Nbt {
        let mut fields = Vec::new();
        if let Some(current_vibration) = &self.current_vibration {
            fields.push(("event".to_string(), current_vibration.to_nbt()));
        }
        fields.push(("selector".into(), self.selection_strategy.to_nbt()));
        fields.push(("event_delay".into(), Nbt::Int(self.travel_time_in_ticks)));
        Nbt::Compound(fields.into())
    }

    pub fn from_nbt(nbt: &Nbt) -> Option<Data> {
        let selection_strategy = VibrationSelector::from_nbt(nbt.get("selector")?)?;
        let travel_time_in_ticks = match nbt.get("event_delay") {
            Some(delay) => delay.as_i64().filter(|&delay| delay >= 0)? as i32,
            None => 0,
        };
        let current_vibration = nbt.get("event").and_then(VibrationInfo::from_nbt);
        Some(Data { current_vibration, travel_time_in_ticks, selection_strategy, reload_vibration_particle: true })
    }

    pub fn decode(tag: &Nbt) -> DecodeResult<Data> {
        static CODEC: LazyLock<Pair> = LazyLock::new(|| {
            let vibration_info = || {
                record(vec![
                    req("game_event", fixed("minecraft:game_event")),
                    req("distance", float(codec_float_range(0.0, f32::MAX))),
                    req("pos", nbt(Codec::Func(decode_vec3))),
                    lenient_opt("source", nbt(Codec::Func(decode_uuid))),
                    lenient_opt("projectile_owner", nbt(Codec::Func(decode_uuid))),
                ])
            };
            let selector = record(vec![lenient_opt("event", vibration_info()), req("tick", (Codec::long(), Stream::Long))]);
            record(vec![lenient_opt("event", vibration_info()), req("selector", selector), always("event_delay", var_int(non_negative_int()), Nbt::Int(0))])
        });
        let DecodeResult { value, error } = CODEC.0.decode_partial(tag);
        DecodeResult { value: value.and_then(|nbt| Data::from_nbt(&nbt)), error }
    }
}

fn decode_vec3(tag: &Nbt) -> DecodeResult<Nbt> {
    extra_codecs::vec3(tag).map(|pos| Nbt::List(vec![Nbt::Double(pos.x), Nbt::Double(pos.y), Nbt::Double(pos.z)]))
}

fn decode_uuid(tag: &Nbt) -> DecodeResult<Nbt> {
    extra_codecs::uuid(tag).map(uuid_ints)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VibrationUser {
    SculkSensor { pos: BlockPos, id: u64 },
    CalibratedSculkSensor { pos: BlockPos, id: u64 },
    SculkShrieker { pos: BlockPos, id: u64 },
    Allay(EntityId),
    Warden(EntityId),
}

pub fn distance_between_in_blocks(origin: BlockPos, dest: BlockPos) -> f32 {
    origin.dist_sqr(dest).sqrt() as f32
}

fn is_occluded(level: &mut ServerLevel, origin: Vec3, dest: Vec3) -> bool {
    let from = Vec3::new(mth::floor(origin.x) as f64 + 0.5, mth::floor(origin.y) as f64 + 0.5, mth::floor(origin.z) as f64 + 0.5);
    let to = Vec3::new(mth::floor(dest.x) as f64 + 0.5, mth::floor(dest.y) as f64 + 0.5, mth::floor(dest.z) as f64 + 0.5);
    for direction in Direction::VALUES {
        let nudged_source = from.relative(direction, 1.0e-5f32 as f64);
        if !level.is_block_in_line(nudged_source, to, |state| state.is(&block_tags::OCCLUDES_VIBRATION_SIGNALS)) {
            return false;
        }
    }
    true
}

impl ServerLevel {
    pub fn vibration_data(&self, user: VibrationUser) -> Option<&Data> {
        match user {
            VibrationUser::SculkSensor { pos, id } | VibrationUser::CalibratedSculkSensor { pos, id } => self.sculk_sensor_vibration_data(pos, id),
            VibrationUser::SculkShrieker { pos, id } => self.sculk_shrieker_vibration_data(pos, id),
            VibrationUser::Allay(id) => self.allay_vibration_data(id),
            VibrationUser::Warden(id) => self.warden_vibration_data(id),
        }
    }

    pub fn vibration_data_mut(&mut self, user: VibrationUser) -> Option<&mut Data> {
        match user {
            VibrationUser::SculkSensor { pos, id } | VibrationUser::CalibratedSculkSensor { pos, id } => self.sculk_sensor_vibration_data_mut(pos, id),
            VibrationUser::SculkShrieker { pos, id } => self.sculk_shrieker_vibration_data_mut(pos, id),
            VibrationUser::Allay(id) => self.allay_vibration_data_mut(id),
            VibrationUser::Warden(id) => self.warden_vibration_data_mut(id),
        }
    }

    pub fn vibration_user_listener_radius(&self, user: VibrationUser) -> i32 {
        match user {
            VibrationUser::SculkSensor { .. } => crate::world::level::block::entity::sculk_sensor_block_entity::LISTENER_RANGE,
            VibrationUser::CalibratedSculkSensor { .. } => crate::world::level::block::entity::sculk_sensor_block_entity::CALIBRATED_LISTENER_RANGE,
            VibrationUser::SculkShrieker { .. } => crate::world::level::block::entity::sculk_shrieker_block_entity::LISTENER_RADIUS,
            VibrationUser::Allay(_) => crate::world::entity::animal::allay::allay::VIBRATION_EVENT_LISTENER_RANGE,
            VibrationUser::Warden(_) => crate::world::entity::monster::warden::warden::GAME_EVENT_LISTENER_RANGE,
        }
    }

    pub fn vibration_user_position_source(&self, user: VibrationUser) -> PositionSource {
        match user {
            VibrationUser::SculkSensor { pos, .. } | VibrationUser::CalibratedSculkSensor { pos, .. } | VibrationUser::SculkShrieker { pos, .. } => PositionSource::Block(pos),
            VibrationUser::Allay(id) => self.allay_position_source(id),
            VibrationUser::Warden(id) => self.warden_position_source(id),
        }
    }

    fn vibration_user_can_receive_vibration(&mut self, user: VibrationUser, pos: BlockPos, event: GameEvent, context: Context) -> bool {
        match user {
            VibrationUser::SculkSensor { pos: block_pos, id } => self.sculk_sensor_can_receive_vibration(block_pos, id, pos, event, context),
            VibrationUser::CalibratedSculkSensor { pos: block_pos, id } => self.calibrated_sculk_sensor_can_receive_vibration(block_pos, id, pos, event, context),
            VibrationUser::SculkShrieker { pos: block_pos, .. } => self.sculk_shrieker_can_receive_vibration(block_pos, context),
            VibrationUser::Allay(id) => self.allay_can_receive_vibration(id, pos),
            VibrationUser::Warden(id) => self.warden_can_receive_vibration(id, pos, context),
        }
    }

    fn vibration_user_on_receive_vibration(&mut self, user: VibrationUser, pos: BlockPos, event: GameEvent, source_entity: Option<EntityId>, projectile_owner: Option<EntityId>, receiving_distance: f32) {
        match user {
            VibrationUser::SculkSensor { pos: block_pos, id } | VibrationUser::CalibratedSculkSensor { pos: block_pos, id } => {
                self.sculk_sensor_on_receive_vibration(user, block_pos, id, pos, event, source_entity, projectile_owner, receiving_distance)
            }
            VibrationUser::SculkShrieker { pos: block_pos, .. } => self.sculk_shrieker_on_receive_vibration(block_pos, source_entity, projectile_owner),
            VibrationUser::Allay(id) => self.allay_on_receive_vibration(id, pos, event),
            VibrationUser::Warden(id) => self.warden_on_receive_vibration(id, pos, source_entity, projectile_owner),
        }
    }

    fn vibration_user_listenable_events(&self, user: VibrationUser) -> &'static Tag {
        match user {
            VibrationUser::SculkSensor { .. } | VibrationUser::CalibratedSculkSensor { .. } => &game_event_tags::VIBRATIONS,
            VibrationUser::SculkShrieker { .. } => &game_event_tags::SHRIEKER_CAN_LISTEN,
            VibrationUser::Allay(_) => &game_event_tags::ALLAY_CAN_LISTEN,
            VibrationUser::Warden(_) => &game_event_tags::WARDEN_CAN_LISTEN,
        }
    }

    fn vibration_user_can_trigger_avoid_vibration(&self, user: VibrationUser) -> bool {
        matches!(user, VibrationUser::SculkSensor { .. } | VibrationUser::CalibratedSculkSensor { .. } | VibrationUser::Warden(_))
    }

    fn vibration_user_requires_adjacent_chunks_to_be_ticking(&self, user: VibrationUser) -> bool {
        matches!(user, VibrationUser::SculkSensor { .. } | VibrationUser::CalibratedSculkSensor { .. } | VibrationUser::SculkShrieker { .. })
    }

    fn vibration_user_calculate_travel_time_in_ticks(&self, _user: VibrationUser, distance_to_destination: f32) -> i32 {
        mth::floor_f(distance_to_destination)
    }

    fn vibration_user_on_data_changed(&mut self, user: VibrationUser) {
        match user {
            VibrationUser::SculkSensor { pos, .. } | VibrationUser::CalibratedSculkSensor { pos, .. } | VibrationUser::SculkShrieker { pos, .. } => self.block_entity_set_changed(pos),
            VibrationUser::Allay(_) | VibrationUser::Warden(_) => {}
        }
    }

    fn vibration_user_is_valid_vibration(&mut self, user: VibrationUser, event: GameEvent, context: Context) -> bool {
        if !event.is(self.vibration_user_listenable_events(user)) {
            return false;
        }
        if let Some(source_entity) = context.source_entity.filter(|&source_entity| self.entity_ref(source_entity).is_some()) {
            if self.is_spectator(source_entity) {
                return false;
            }
            if self.is_stepping_carefully(source_entity) && event.is(&game_event_tags::IGNORE_VIBRATIONS_SNEAKING) {
                if self.vibration_user_can_trigger_avoid_vibration(user) && self.is_player(source_entity) {
                    criteria_triggers::AVOID_VIBRATION.trigger(self, source_entity);
                }
                return false;
            }
            if self.dampens_vibrations(source_entity) {
                return false;
            }
        }
        context.affected_state.is_none_or(|state: BlockState| !state.is(&block_tags::DAMPENS_VIBRATIONS))
    }

    pub fn vibration_listener_handle_game_event(&mut self, user: VibrationUser, event: GameEvent, context: Context, source_position: Vec3) -> bool {
        if self.vibration_data(user).is_none_or(|data| data.current_vibration.is_some()) {
            return false;
        }
        if !self.vibration_user_is_valid_vibration(user, event, context) {
            return false;
        }
        let Some(destination) = self.vibration_user_position_source(user).get_position(self) else { return false };
        if !self.vibration_user_can_receive_vibration(user, BlockPos::containing_vec(source_position), event, context) {
            return false;
        }
        if is_occluded(self, source_position, destination) {
            return false;
        }
        self.schedule_vibration(user, event, context, source_position, destination);
        true
    }

    pub fn vibration_listener_force_schedule_vibration(&mut self, user: VibrationUser, event: GameEvent, context: Context, origin: Vec3) {
        if let Some(position) = self.vibration_user_position_source(user).get_position(self) {
            self.schedule_vibration(user, event, context, origin, position);
        }
    }

    fn schedule_vibration(&mut self, user: VibrationUser, event: GameEvent, context: Context, origin: Vec3, dest: Vec3) {
        let info = VibrationInfo::of_entity(self, event, origin.distance_to(dest) as f32, origin, context.source_entity);
        let game_time = self.game_time;
        if let Some(data) = self.vibration_data_mut(user) {
            data.selection_strategy.add_candidate(info, game_time);
        }
    }

    pub fn vibration_ticker_tick(&mut self, user: VibrationUser) {
        if self.vibration_data(user).is_some_and(|data| data.current_vibration.is_none()) {
            self.try_select_and_schedule_vibration(user);
        }
        let Some(data) = self.vibration_data(user) else { return };
        let Some(current_vibration) = data.current_vibration else { return };
        let mut has_changed = data.travel_time_in_ticks > 0;
        self.try_reload_vibration_particle(user);
        let Some(data) = self.vibration_data_mut(user) else { return };
        data.decrement_travel_time();
        let travel_time_in_ticks = data.travel_time_in_ticks;
        if travel_time_in_ticks <= 0 {
            has_changed = self.receive_vibration(user, current_vibration);
        }
        if has_changed {
            self.vibration_user_on_data_changed(user);
        }
    }

    fn try_select_and_schedule_vibration(&mut self, user: VibrationUser) {
        let game_time = self.game_time;
        let Some(context) = self.vibration_data(user).and_then(|data| data.selection_strategy.chosen_candidate(game_time)) else { return };
        let travel_time_in_ticks = self.vibration_user_calculate_travel_time_in_ticks(user, context.distance);
        if let Some(data) = self.vibration_data_mut(user) {
            data.current_vibration = Some(context);
            data.travel_time_in_ticks = travel_time_in_ticks;
        }
        let origin = context.pos;
        let position_source = self.vibration_user_position_source(user);
        self.send_particles("minecraft:vibration", |w| {
            position_source.write(w);
            w.varint(travel_time_in_ticks);
        }, origin.x, origin.y, origin.z, 1, (0.0, 0.0, 0.0), 0.0);
        self.vibration_user_on_data_changed(user);
        if let Some(data) = self.vibration_data_mut(user) {
            data.selection_strategy.start_over();
        }
    }

    fn try_reload_vibration_particle(&mut self, user: VibrationUser) {
        let Some(data) = self.vibration_data(user) else { return };
        if !data.reload_vibration_particle {
            return;
        }
        let Some(current_vibration) = data.current_vibration else {
            if let Some(data) = self.vibration_data_mut(user) {
                data.reload_vibration_particle = false;
            }
            return;
        };
        let travel_time_in_ticks = data.travel_time_in_ticks;
        let origin = current_vibration.pos;
        let position_source = self.vibration_user_position_source(user);
        let destination = position_source.get_position(self).unwrap_or(origin);
        let initial_travel_time = self.vibration_user_calculate_travel_time_in_ticks(user, current_vibration.distance);
        let alpha = 1.0 - travel_time_in_ticks as f64 / initial_travel_time as f64;
        let new_initial_x = mth::lerp(alpha, origin.x, destination.x);
        let new_initial_y = mth::lerp(alpha, origin.y, destination.y);
        let new_initial_z = mth::lerp(alpha, origin.z, destination.z);
        let particle_was_sent = self.send_particles("minecraft:vibration", |w| {
            position_source.write(w);
            w.varint(travel_time_in_ticks);
        }, new_initial_x, new_initial_y, new_initial_z, 1, (0.0, 0.0, 0.0), 0.0) > 0;
        if particle_was_sent && let Some(data) = self.vibration_data_mut(user) {
            data.reload_vibration_particle = false;
        }
    }

    fn receive_vibration(&mut self, user: VibrationUser, current_vibration: VibrationInfo) -> bool {
        let origin = BlockPos::containing_vec(current_vibration.pos);
        let destination = self.vibration_user_position_source(user).get_position(self).map_or(origin, BlockPos::containing_vec);
        if self.vibration_user_requires_adjacent_chunks_to_be_ticking(user) && !self.are_adjacent_chunks_ticking(destination) {
            return false;
        }
        let source_entity = current_vibration.get_entity(self);
        let projectile_owner = current_vibration.get_projectile_owner(self);
        self.vibration_user_on_receive_vibration(user, origin, current_vibration.game_event, source_entity, projectile_owner, distance_between_in_blocks(origin, destination));
        if let Some(data) = self.vibration_data_mut(user) {
            data.current_vibration = None;
        }
        true
    }

    fn are_adjacent_chunks_ticking(&self, listener_pos: BlockPos) -> bool {
        let listener_chunk_pos = listener_pos.chunk();
        for x in listener_chunk_pos.x - 1..=listener_chunk_pos.x + 1 {
            for z in listener_chunk_pos.z - 1..=listener_chunk_pos.z + 1 {
                let chunk_pos = ChunkPos::new(x, z);
                if !self.in_block_ticking_range(chunk_pos) || self.chunks.loaded_chunk(chunk_pos).is_none() {
                    return false;
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(snbt: &str) -> Nbt {
        crate::nbt::parse_fully(snbt).expect("valid snbt")
    }

    #[test]
    fn data_without_an_event_decodes_with_defaults() {
        let decoded = Data::decode(&parse("{selector:{tick:7L}}"));
        assert!(decoded.error.is_none());
        let data = decoded.value.unwrap();
        assert!(data.current_vibration.is_none());
        assert_eq!(data.travel_time_in_ticks, 0);
    }

    #[test]
    fn a_broken_event_is_dropped_leniently_and_the_rest_of_the_data_survives() {
        let decoded = Data::decode(&parse("{event:{game_event:\"minecraft:nonexistent\",distance:1.0f,pos:[1.0d,2.0d,3.0d]},selector:{tick:7L},event_delay:4}"));
        assert!(decoded.error.is_none());
        let data = decoded.value.unwrap();
        assert!(data.current_vibration.is_none());
        assert_eq!(data.travel_time_in_ticks, 4);
    }

    #[test]
    fn data_without_a_selector_fails() {
        let decoded = Data::decode(&parse("{event_delay:4}"));
        assert!(decoded.error.is_some());
        assert!(decoded.value.is_none());
    }
}
