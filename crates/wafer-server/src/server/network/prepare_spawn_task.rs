use wafer_core::{BlockPos, ChunkPos, Vec3};
use wafer_protocol::nbt::Nbt;
use tokio::sync::oneshot;

use crate::server::Server;
use crate::server::level::game_mode::GameType;
use crate::server::level::ticket::TicketType;
use crate::util::codec::{self, DecodeResult, Record};
use crate::util::extra_codecs;
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
    pub position: Vec3,
    pub angle: (f32, f32),
    pub reply: Option<oneshot::Sender<()>>,
    chunk_load_requested: bool,
}

impl PrepareSpawnTask {
    fn spawn_chunk(&self) -> ChunkPos {
        BlockPos::containing(self.position.x, self.position.y, self.position.z).chunk()
    }
}

impl Server {
    pub(crate) fn load_player_data(&self, name: &str, uuid: u128) -> Option<Nbt> {
        let dir = self.world_dir.as_ref()?;
        match crate::world::level::storage::read_dat(&dir.player(uuid)) {
            Ok(nbt) => nbt.map(|tag| {
                let version = crate::nbt::get_data_version(&tag, -1);
                crate::util::datafix::data_fix_types::DataFixTypes::Player.update_tag_to_current_version(tag, version)
            }),
            Err(_) => {
                warn!("Failed to load player data for {name}");
                None
            }
        }
    }

    pub(crate) fn spawn_position(&mut self, saved: Option<&Nbt>) -> (usize, Vec3, f32, f32) {
        let (saved_dimension, saved_position, saved_rotation) = saved.and_then(|nbt| tag_value_input::read_map(nbt, decode_saved_position)).unwrap_or_default();
        let spawn_level = saved_dimension.and_then(|dimension| self.get_level(&dimension)).unwrap_or_else(|| self.get_level(&self.spawn.dimension).unwrap_or(0));
        let adventure = self.default_game_type.get() == GameType::Adventure;
        let spawn_pos = self.spawn.pos;
        let position = saved_position.unwrap_or_else(|| self.levels[spawn_level].find_spawn(spawn_pos, adventure));
        let (y_rot, x_rot) = saved_rotation.unwrap_or((self.spawn.yaw, self.spawn.pitch));
        (spawn_level, position, y_rot, x_rot)
    }

    pub(crate) fn start_prepare_spawn(&mut self, uuid: u128, name: &str, reply: oneshot::Sender<()>) {
        let _collector = tag_value_input::ScopedCollector::new_empty(PREPARE_SPAWN_TASK_LOGGER);
        let saved = self.load_player_data(name, uuid);
        let (level, position, y_rot, x_rot) = self.spawn_position(saved.as_ref());
        self.prepare_spawn_tasks.push(PrepareSpawnTask { uuid, level, position, angle: (y_rot, x_rot), reply: Some(reply), chunk_load_requested: false });
    }

    pub(crate) fn tick_prepare_spawns(&mut self) {
        let mut index = 0;
        while index < self.prepare_spawn_tasks.len() {
            let spawn_chunk = self.prepare_spawn_tasks[index].spawn_chunk();
            let task = &mut self.prepare_spawn_tasks[index];
            let chunks = &mut self.levels[task.level].chunks;
            if task.reply.is_none() {
                chunks.add_ticket_with_radius(TicketType::PlayerSpawn, spawn_chunk, SPAWN_RADIUS);
                index += 1;
                continue;
            }
            if !task.chunk_load_requested {
                task.chunk_load_requested = true;
                chunks.add_ticket_with_radius(TicketType::PlayerSpawn, spawn_chunk, SPAWN_RADIUS);
            }
            let loaded = (-SPAWN_RADIUS..=SPAWN_RADIUS)
                .all(|dx| (-SPAWN_RADIUS..=SPAWN_RADIUS).all(|dz| chunks.has_chunk(ChunkPos::new(spawn_chunk.x + dx, spawn_chunk.z + dz))));
            let task = &mut self.prepare_spawn_tasks[index];
            if loaded && let Some(reply) = task.reply.take() {
                if reply.send(()).is_err() {
                    self.prepare_spawn_tasks.swap_remove(index);
                    continue;
                }
            }
            index += 1;
        }
    }
}
