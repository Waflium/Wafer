use wafer_core::{BlockPos, ChunkPos, Vec3};
use wafer_protocol::nbt::Nbt;
use tokio::sync::oneshot;

use crate::server::Server;
use crate::server::level::chunk_holder::FutureId;
use crate::server::level::player_spawn_finder::Finished;
use crate::server::level::ticket::TicketType;
use crate::util::codec::{self, DecodeResult, Record};
use crate::util::extra_codecs;
use crate::world::level::storage::player_data_storage::PlayerDataStorage;
use crate::world::level::storage::tag_value_input;

const SPAWN_RADIUS: i32 = 3;
pub(crate) const PREPARE_SPAWN_TASK_LOGGER: &str = "net.minecraft.server.network.config.PrepareSpawnTask";

type SavedPosition = (Option<String>, Option<Vec3>, Option<(f32, f32)>);

fn decode_saved_position(tag: &Nbt) -> DecodeResult<SavedPosition> {
    if let Err(error) = codec::map(tag) {
        return DecodeResult::error(error);
    }
    let mut record = Record::default();
    let dimension = record.optional(tag, "Dimension", |dimension| DecodeResult::from(codec::identifier(dimension)));
    let position = record.optional(tag, "Pos", extra_codecs::vec3);
    let rotation = record.optional(tag, "Rotation", extra_codecs::vec2);
    record.build(|| Some((dimension, position, rotation)))
}

pub(crate) struct PrepareSpawnTask {
    pub uuid: u128,
    pub level: usize,
    pub position: Option<Vec3>,
    spawn_finder: Option<u64>,
    pub angle: (f32, f32),
    pub reply: Option<oneshot::Sender<Result<(), ()>>>,
    chunk_load_future: Option<FutureId>,
}

impl PrepareSpawnTask {
    pub const TYPE: &str = "prepare_spawn";

    fn spawn_chunk(&self) -> ChunkPos {
        let position = self.position.expect("spawn position");
        BlockPos::containing(position.x, position.y, position.z).chunk()
    }
}

impl Server {
    pub(crate) fn load_player_data(&self, name: &str, uuid: u128) -> Option<Nbt> {
        PlayerDataStorage::new(self.world_dir.as_ref()?).load(name, uuid)
    }

    fn begin_spawn_position(&mut self, saved: Option<&Nbt>) -> (usize, Option<Vec3>, Option<u64>, f32, f32) {
        let (saved_dimension, saved_position, saved_rotation) = saved.and_then(|nbt| tag_value_input::read_map(nbt, decode_saved_position)).unwrap_or_default();
        let spawn_level = saved_dimension.and_then(|dimension| self.get_level(&dimension)).unwrap_or_else(|| self.get_level(&self.spawn.dimension).unwrap_or(0));
        let spawn_finder = if saved_position.is_none() { Some(self.find_spawn(spawn_level, self.spawn.pos)) } else { None };
        let (y_rot, x_rot) = saved_rotation.unwrap_or((self.spawn.yaw, self.spawn.pitch));
        (spawn_level, saved_position, spawn_finder, y_rot, x_rot)
    }

    pub(crate) fn spawn_position(&mut self, saved: Option<&Nbt>) -> (usize, Vec3, f32, f32) {
        let (level, position, spawn_finder, y_rot, x_rot) = self.begin_spawn_position(saved);
        let position = match (position, spawn_finder) {
            (Some(position), _) => position,
            (None, Some(finder)) => self.join_spawn_finder(finder),
            (None, None) => unreachable!("a spawn position or a finder"),
        };
        (level, position, y_rot, x_rot)
    }

    pub(crate) fn start_prepare_spawn(&mut self, uuid: u128, name: &str, reply: oneshot::Sender<Result<(), ()>>) {
        let started = crate::util::catch_exception(|| {
            let _collector = tag_value_input::ScopedCollector::new_empty(PREPARE_SPAWN_TASK_LOGGER);
            let saved = self.load_player_data(name, uuid);
            self.begin_spawn_position(saved.as_ref())
        });
        match started {
            Ok((level, position, spawn_finder, y_rot, x_rot)) => {
                self.prepare_spawn_tasks.push(PrepareSpawnTask { uuid, level, position, spawn_finder, angle: (y_rot, x_rot), reply: Some(reply), chunk_load_future: None })
            }
            Err(exception) => {
                error!("Failed to start configuration task {}\n{exception}", PrepareSpawnTask::TYPE);
                let _ = reply.send(Err(()));
            }
        }
    }

    pub(crate) fn close_prepare_spawn(&mut self, uuid: u128) {
        let (closed, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.prepare_spawn_tasks).into_iter().partition(|task| task.uuid == uuid);
        self.prepare_spawn_tasks = kept;
        for task in closed {
            if let Some(finder) = task.spawn_finder.and_then(|finder| self.spawn_finders.get_mut(&finder)) {
                finder.cancel();
            }
        }
    }

    pub(crate) fn tick_prepare_spawns(&mut self) {
        let mut index = 0;
        while index < self.prepare_spawn_tasks.len() {
            if self.prepare_spawn_tasks[index].position.is_none() {
                let finder = self.prepare_spawn_tasks[index].spawn_finder.expect("a spawn finder");
                match self.spawn_finders.get(&finder).and_then(|finder| finder.finished()) {
                    None => index += 1,
                    Some(Finished::Spawn(position)) => {
                        self.spawn_finders.remove(&finder);
                        self.prepare_spawn_tasks[index].position = Some(position);
                    }
                    Some(Finished::Failed) => {
                        self.spawn_finders.remove(&finder);
                        let task = self.prepare_spawn_tasks.swap_remove(index);
                        if let Some(reply) = task.reply {
                            let _ = reply.send(Err(()));
                        }
                    }
                }
                continue;
            }
            let spawn_chunk = self.prepare_spawn_tasks[index].spawn_chunk();
            let task = &mut self.prepare_spawn_tasks[index];
            let chunks = &mut self.levels[task.level].chunks;
            if task.reply.is_none() {
                chunks.add_ticket_with_radius(TicketType::PlayerSpawn, spawn_chunk, SPAWN_RADIUS);
                index += 1;
                continue;
            }
            let chunk_load_future = match task.chunk_load_future {
                Some(future) => future,
                None => {
                    let future = chunks.add_ticket_and_load_with_radius(TicketType::PlayerSpawn, spawn_chunk, SPAWN_RADIUS);
                    task.chunk_load_future = Some(future);
                    future
                }
            };
            let loaded = chunks.is_future_done(chunk_load_future);
            let task = &mut self.prepare_spawn_tasks[index];
            if loaded && let Some(reply) = task.reply.take() {
                if reply.send(Ok(())).is_err() {
                    self.prepare_spawn_tasks.swap_remove(index);
                    continue;
                }
            }
            index += 1;
        }
    }
}
