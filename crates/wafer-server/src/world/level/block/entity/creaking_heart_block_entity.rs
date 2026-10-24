use std::collections::{HashSet, VecDeque};

use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Direction, Vec3};
use wafer_data::properties::{CREAKING_HEART_STATE, CreakingHeartState, WATERLOGGED};
use wafer_data::{BlockState, block_tags, blocks, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::BlockEntityKind;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::{EntityId, entity_types};
use crate::world::level::BlockGetter;
use crate::world::level::block::creaking_heart_block;
use crate::world::level::block::multiface_block;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::util::spawn_util::{self, Strategy};

const PLAYER_DETECTION_RANGE: f64 = 32.0;
const DISTANCE_CREAKING_TOO_FAR: f64 = 34.0;
const SPAWN_RANGE_XZ: i32 = 16;
const SPAWN_RANGE_Y: i32 = 8;
const ATTEMPTS_PER_SPAWN: i32 = 5;
const UPDATE_TICKS: i32 = 20;
const UPDATE_TICKS_VARIANCE: i32 = 5;
const HURT_CALL_TOTAL_TICKS: i32 = 100;
const HURT_CALL_INTERVAL: i32 = 10;
const HURT_CALL_PARTICLE_TICKS: i32 = 50;
const MAX_DEPTH: i32 = 2;
const MAX_COUNT: i32 = 64;
const TICKS_GRACE_PERIOD: i64 = 30;
const CREAKING_ORANGE: i32 = 16545810;
const CREAKING_GRAY: i32 = 6250335;

fn sound(name: &str) -> i32 {
    registry_id(registry::SOUND_EVENT, name)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CreakingInfo {
    Creaking { id: EntityId, uuid: u128 },
    Uuid(u128),
}

#[derive(Clone, Default)]
pub struct CreakingHeartBlockEntity {
    creaking_info: Option<CreakingInfo>,
    ticks_existed: i64,
    ticker: i32,
    emitter: i32,
    emitter_target: Option<Vec3>,
    output_signal: i32,
}

#[derive(PartialEq, Eq)]
enum TraversalNodeStatus {
    Accept,
    Stop,
}

impl CreakingHeartBlockEntity {
    pub fn load_additional(&mut self, input: &Nbt) {
        match input.get("creaking").and_then(crate::world::entity::entity_save::uuid_from_nbt) {
            Some(uuid) => {
                self.creaking_info = Some(CreakingInfo::Uuid(uuid));
                self.ticks_existed = 0;
            }
            None => self.creaking_info = None,
        }
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        let uuid = match self.creaking_info {
            Some(CreakingInfo::Creaking { uuid, .. } | CreakingInfo::Uuid(uuid)) => Some(uuid),
            None => None,
        };
        if let Some(uuid) = uuid {
            output.push(("creaking".into(), crate::world::level::storage::uuid_ints(uuid)));
        }
    }

    pub fn get_analog_output_signal(&self) -> i32 {
        self.output_signal
    }
}

impl ServerLevel {
    fn creaking_heart(&mut self, pos: BlockPos) -> Option<&mut CreakingHeartBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::CreakingHeart(heart) => Some(heart),
            _ => None,
        }
    }

    pub fn creaking_heart_server_tick(&mut self, pos: BlockPos, state: BlockState) {
        let Some(entity) = self.creaking_heart(pos) else { return };
        entity.ticks_existed += 1;
        let computed_output_signal = self.creaking_heart_compute_analog_output_signal(pos);
        let entity = self.creaking_heart(pos).expect("a creaking heart");
        if entity.output_signal != computed_output_signal {
            entity.output_signal = computed_output_signal;
            self.update_neighbour_for_output_signal(pos, blocks::CREAKING_HEART);
        }
        let entity = self.creaking_heart(pos).expect("a creaking heart");
        if entity.emitter > 0 {
            if entity.emitter > HURT_CALL_PARTICLE_TICKS {
                self.creaking_heart_emit_particles(pos, 1, true);
                self.creaking_heart_emit_particles(pos, 1, false);
            }
            let entity = self.creaking_heart(pos).expect("a creaking heart");
            if entity.emitter % HURT_CALL_INTERVAL == 0 && entity.emitter_target.is_some() {
                if let Some(creaking) = self.creaking_heart_get_creaking_protector(pos) {
                    let center = self.entity(creaking).bounding_box().center();
                    self.creaking_heart(pos).expect("a creaking heart").emitter_target = Some(center);
                }
                let entity = self.creaking_heart(pos).expect("a creaking heart");
                let emitter_target = entity.emitter_target.expect("an emitter target");
                let emitter = entity.emitter;
                let heart_position = Vec3::at_center_of(pos);
                let progress = 0.2 + 0.8 * (HURT_CALL_TOTAL_TICKS - emitter) as f32 / 100.0;
                let sound_location = heart_position.subtract(emitter_target).scale(progress as f64).add_vec(emitter_target);
                let sound_pos = BlockPos::containing_vec(sound_location);
                let volume = emitter as f32 / 2.0 / 100.0 + 0.5;
                self.play_sound(None, sound_pos, sound("minecraft:block.creaking_heart.hurt"), SoundSource::Blocks, volume, 1.0);
            }
            self.creaking_heart(pos).expect("a creaking heart").emitter -= 1;
        }
        let entity = self.creaking_heart(pos).expect("a creaking heart");
        let ticker = entity.ticker;
        entity.ticker -= 1;
        if ticker < 0 {
            let next = self.random.next_int_bounded(UPDATE_TICKS_VARIANCE) + UPDATE_TICKS;
            self.creaking_heart(pos).expect("a creaking heart").ticker = next;
            let updated_state = self.creaking_heart_update_creaking_state(state, pos);
            if updated_state != state {
                self.set_block(pos, updated_state, UPDATE_ALL);
                if updated_state.get(CREAKING_HEART_STATE) == CreakingHeartState::Uprooted {
                    return;
                }
            }
            let Some(entity) = self.creaking_heart(pos) else { return };
            if entity.creaking_info.is_none() {
                if updated_state.get(CREAKING_HEART_STATE) == CreakingHeartState::Awake
                    && self.is_spawning_monsters()
                    && self.difficulty != Difficulty::Peaceful
                    && crate::world::level::server_level_accessor::ServerLevelAccessor::get_nearest_player(self, pos.x as f64, pos.y as f64, pos.z as f64, PLAYER_DETECTION_RANGE, false).is_some()
                    && let Some(creaking) = self.creaking_heart_spawn_protector(pos)
                {
                    self.creaking_heart_set_creaking_info(pos, creaking);
                    self.make_sound(creaking, crate::world::entity::sound_event("minecraft:entity.creaking.spawn"));
                    self.play_sound(None, pos, sound("minecraft:block.creaking_heart.spawn"), SoundSource::Blocks, 1.0, 1.0);
                }
            } else if let Some(creaking) = self.creaking_heart_get_creaking_protector(pos) {
                let creaking_active = self.environment_attributes().get_value_at(crate::world::attribute::environment_attributes::CREAKING_ACTIVE, pos);
                let persistence_required = self.mob(creaking).is_some_and(|mob| mob.persistence_required);
                if !creaking_active && !persistence_required || self.creaking_heart_distance_to_creaking(pos) > DISTANCE_CREAKING_TOO_FAR || self.creaking_player_is_stuck_in_you(creaking) {
                    self.creaking_heart_remove_protector(pos, None);
                }
            }
        }
    }

    fn creaking_heart_update_creaking_state(&mut self, state: BlockState, pos: BlockPos) -> BlockState {
        let has_creaking = self.creaking_heart(pos).is_some_and(|entity| entity.creaking_info.is_some());
        if !creaking_heart_block::has_required_logs(state, self, pos) && !has_creaking {
            return state.set(CREAKING_HEART_STATE, CreakingHeartState::Uprooted);
        }
        let creaking_active = self.environment_attributes().get_value_at(crate::world::attribute::environment_attributes::CREAKING_ACTIVE, pos);
        state.set(CREAKING_HEART_STATE, if creaking_active { CreakingHeartState::Awake } else { CreakingHeartState::Dormant })
    }

    fn creaking_heart_distance_to_creaking(&mut self, pos: BlockPos) -> f64 {
        match self.creaking_heart_get_creaking_protector(pos) {
            Some(creaking) => {
                let bottom_center = Vec3::at_bottom_center_of(pos);
                self.entity(creaking).distance_to_sqr_xyz(bottom_center.x, bottom_center.y, bottom_center.z).sqrt()
            }
            None => 0.0,
        }
    }

    fn creaking_heart_clear_creaking_info(&mut self, pos: BlockPos) {
        if let Some(entity) = self.creaking_heart(pos) {
            entity.creaking_info = None;
        }
        self.block_entity_set_changed(pos);
    }

    pub fn creaking_heart_set_creaking_info(&mut self, pos: BlockPos, creaking: EntityId) {
        let uuid = self.entity(creaking).uuid;
        if let Some(entity) = self.creaking_heart(pos) {
            entity.creaking_info = Some(CreakingInfo::Creaking { id: creaking, uuid });
        }
        self.block_entity_set_changed(pos);
    }

    fn creaking_heart_set_creaking_uuid(&mut self, pos: BlockPos, uuid: u128) {
        if let Some(entity) = self.creaking_heart(pos) {
            entity.creaking_info = Some(CreakingInfo::Uuid(uuid));
            entity.ticks_existed = 0;
        }
        self.block_entity_set_changed(pos);
    }

    pub fn creaking_heart_get_creaking_protector(&mut self, pos: BlockPos) -> Option<EntityId> {
        let info = self.creaking_heart(pos)?.creaking_info?;
        let uuid = match info {
            CreakingInfo::Creaking { id, uuid } => {
                if self.entity_ref(id).is_some_and(|e| !e.is_removed()) {
                    return Some(id);
                }
                self.creaking_heart_set_creaking_uuid(pos, uuid);
                uuid
            }
            CreakingInfo::Uuid(uuid) => uuid,
        };
        match self.entity_manager.visible_entity_storage.get(uuid).filter(|&entity| self.is_creaking(entity)) {
            Some(resolved_creaking) => {
                self.creaking_heart_set_creaking_info(pos, resolved_creaking);
                Some(resolved_creaking)
            }
            None => {
                if self.creaking_heart(pos).is_some_and(|entity| entity.ticks_existed >= TICKS_GRACE_PERIOD) {
                    self.creaking_heart_clear_creaking_info(pos);
                }
                None
            }
        }
    }

    fn creaking_heart_spawn_protector(&mut self, pos: BlockPos) -> Option<EntityId> {
        let spawned_creaking = spawn_util::try_spawn_mob(self, &entity_types::CREAKING, EntitySpawnReason::Spawner, pos, ATTEMPTS_PER_SPAWN, SPAWN_RANGE_XZ, SPAWN_RANGE_Y, Strategy::OnTopOfColliderNoLeaves, true)?;
        self.entity_game_event(spawned_creaking, crate::world::level::gameevent::game_event::ENTITY_PLACE);
        self.broadcast_entity_event(spawned_creaking, 60);
        self.creaking_set_transient(spawned_creaking, pos);
        Some(spawned_creaking)
    }

    pub fn creaking_heart_creaking_hurt(&mut self, pos: BlockPos) {
        let Some(creaking) = self.creaking_heart_get_creaking_protector(pos) else { return };
        if self.creaking_heart(pos).is_some_and(|entity| entity.emitter <= 0) {
            self.creaking_heart_emit_particles(pos, 20, false);
            if self.block_state(pos).get(CREAKING_HEART_STATE) == CreakingHeartState::Awake {
                let number_of_clumps = self.random.next_int_between_inclusive(2, 3);
                for _ in 0..number_of_clumps {
                    if let Some(block_pos) = self.creaking_heart_spread_resin(pos) {
                        self.play_sound(None, block_pos, sound("minecraft:block.resin.place"), SoundSource::Blocks, 1.0, 1.0);
                        let heart_state = self.chunks.block_entity_ref(pos).map(|entity| entity.block_state());
                        self.game_event_at_with(crate::world::level::gameevent::game_event::BLOCK_PLACE, block_pos, crate::world::level::gameevent::game_event::Context::of_state(heart_state));
                    }
                }
            }
            let center = self.entity(creaking).bounding_box().center();
            let entity = self.creaking_heart(pos).expect("a creaking heart");
            entity.emitter = HURT_CALL_TOTAL_TICKS;
            entity.emitter_target = Some(center);
        }
    }

    fn creaking_heart_spread_resin(&mut self, world_position: BlockPos) -> Option<BlockPos> {
        let mut placed_resin = None;
        let mut nodes: VecDeque<(BlockPos, i32)> = VecDeque::from([(world_position, 0)]);
        let mut visited: HashSet<i64> = HashSet::new();
        let mut count = 0;
        while let Some((current_pos, depth)) = nodes.pop_front() {
            if !visited.insert(current_pos.as_long()) {
                continue;
            }
            let next = self.creaking_heart_spread_resin_node(current_pos, &mut placed_resin);
            if next == TraversalNodeStatus::Stop {
                break;
            }
            count += 1;
            if count >= MAX_COUNT {
                return placed_resin;
            }
            if depth < MAX_DEPTH {
                let mut directions = Direction::VALUES;
                crate::util::shuffle(&mut directions, &mut self.random);
                for dir in directions {
                    let neighbour_pos = current_pos.relative(dir);
                    if self.block_state(neighbour_pos).is(&block_tags::PALE_OAK_LOGS) {
                        nodes.push_back((neighbour_pos, depth + 1));
                    }
                }
            }
        }
        placed_resin
    }

    fn creaking_heart_spread_resin_node(&mut self, pos: BlockPos, placed_resin: &mut Option<BlockPos>) -> TraversalNodeStatus {
        if !self.block_state(pos).is(&block_tags::PALE_OAK_LOGS) {
            return TraversalNodeStatus::Accept;
        }
        let mut directions = Direction::VALUES;
        crate::util::shuffle(&mut directions, &mut self.random);
        for dir in directions {
            let neighbour_pos = pos.relative(dir);
            let mut neighbour_state = self.block_state(neighbour_pos);
            let opposite = dir.opposite();
            if neighbour_state.is_air() {
                neighbour_state = blocks::RESIN_CLUMP.default_state();
            } else if neighbour_state.is_block(blocks::WATER) && neighbour_state.fluid_state().is_source() {
                neighbour_state = blocks::RESIN_CLUMP.default_state().set(WATERLOGGED, true);
            }
            if neighbour_state.is_block(blocks::RESIN_CLUMP) && !multiface_block::has_face(neighbour_state, opposite) {
                self.set_block(neighbour_pos, neighbour_state.set(multiface_block::get_face_property(opposite), true), UPDATE_ALL);
                *placed_resin = Some(neighbour_pos);
                return TraversalNodeStatus::Stop;
            }
        }
        TraversalNodeStatus::Accept
    }

    fn creaking_heart_emit_particles(&mut self, pos: BlockPos, count: i32, towards_creaking: bool) {
        let Some(creaking) = self.creaking_heart_get_creaking_protector(pos) else { return };
        let color = if towards_creaking { CREAKING_ORANGE } else { CREAKING_GRAY };
        for _ in 0..count {
            let box_ = self.entity(creaking).bounding_box();
            let random = &mut self.random;
            let mut source = Vec3::new(box_.min_x, box_.min_y, box_.min_z).add(random.next_double() * box_.x_size(), random.next_double() * box_.y_size(), random.next_double() * box_.z_size());
            let mut destination = Vec3::new(pos.x as f64, pos.y as f64, pos.z as f64).add(random.next_double(), random.next_double(), random.next_double());
            if towards_creaking {
                std::mem::swap(&mut source, &mut destination);
            }
            let duration = random.next_int_bounded(40) + 10;
            self.send_particles_forced(
                "minecraft:trail",
                |w| {
                    w.f64(destination.x);
                    w.f64(destination.y);
                    w.f64(destination.z);
                    w.i32(color);
                    w.varint(duration);
                },
                source.x,
                source.y,
                source.z,
                1,
                (0.0, 0.0, 0.0),
                0.0,
            );
        }
    }

    pub fn creaking_heart_pre_remove_side_effects(&mut self, pos: BlockPos) {
        self.creaking_heart_remove_protector(pos, None);
    }

    pub fn creaking_heart_remove_protector(&mut self, pos: BlockPos, damage_source: Option<DamageSource>) {
        let Some(creaking) = self.creaking_heart_get_creaking_protector(pos) else { return };
        match damage_source {
            None => self.creaking_tear_down(creaking),
            Some(damage_source) => {
                self.creaking_death_effects(creaking, damage_source);
                self.creaking_set_tearing_down(creaking);
                self.set_health(creaking, 0.0);
            }
        }
        self.creaking_heart_clear_creaking_info(pos);
    }

    pub fn creaking_heart_is_protector(&mut self, pos: BlockPos, creaking: EntityId) -> bool {
        self.creaking_heart_get_creaking_protector(pos) == Some(creaking)
    }

    fn creaking_heart_compute_analog_output_signal(&mut self, pos: BlockPos) -> i32 {
        if self.creaking_heart(pos).is_some_and(|entity| entity.creaking_info.is_some()) && self.creaking_heart_get_creaking_protector(pos).is_some() {
            let distance = self.creaking_heart_distance_to_creaking(pos);
            let scaled_distance = distance.clamp(0.0, 32.0) / 32.0;
            15 - (scaled_distance * 15.0).floor() as i32
        } else {
            0
        }
    }
}
