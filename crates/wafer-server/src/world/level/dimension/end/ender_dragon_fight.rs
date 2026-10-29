use std::io;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use wafer_core::random::{LegacyRandomSource, RandomSource};
use wafer_core::{AABB, BlockPos, ChunkPos, Direction, mth};
use wafer_data::{WORLD_VERSION, block_entity_types, blocks};
use wafer_protocol::nbt::Nbt;

use super::dragon_respawn_stage::DragonRespawnStage;
use crate::util::datafix::data_fix_types::DataFixTypes;
use crate::advancements::triggers::criteria_triggers;
use crate::network::chat::Component;
use crate::server::level::chunk_level;
use crate::server::level::events;
use crate::server::level::chunk_level::FullChunkStatus;
use crate::server::level::server_boss_event::{BossBarColor, BossBarOverlay, ServerBossEvent};
use crate::server::level::ticket::TicketType;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::boss::enderdragon::ender_dragon::EnderDragon;
use crate::world::entity::boss::enderdragon::phases::ender_dragon_phase::EnderDragonPhase;
use crate::world::entity::boss::enderdragon::phases::ender_dragon_phase_manager::switch_phase;
use crate::world::entity::boss::enderdragon::ender_dragon::ender_dragon_mut;
use crate::world::entity::entity_reference::EntityReference;
use crate::world::entity::{EntityId, LevelEntity, create_insecure_uuid};
use crate::world::level::BlockGetter;
use crate::world::level::block::state::pattern::block_pattern::{BlockPattern, BlockPatternBuilder, BlockPatternMatch, has_state};
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::feature::end_gateway_feature::EndGatewayFeature;
use crate::world::level::levelgen::feature::end_podium_feature::EndPodiumFeature;
use crate::world::level::levelgen::feature::end_spike_feature;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::storage::{read_saved_data_dat, write_dat};
use crate::world::level::world_gen_level::WorldGenLevel;

const MAX_TICKS_BEFORE_DRAGON_RESPAWN: i32 = 1200;
const TIME_BETWEEN_CRYSTAL_SCANS: i32 = 100;
pub const TIME_BETWEEN_PLAYER_SCANS: i32 = 20;
const ARENA_SIZE_CHUNKS: i32 = 8;
pub const ARENA_TICKET_LEVEL: i32 = 9;
const GATEWAY_COUNT: i32 = 20;
const GATEWAY_DISTANCE: f64 = 96.0;
pub const DRAGON_SPAWN_Y: i32 = 128;
const END_PODIUM_LOCATION: BlockPos = BlockPos::ZERO;

static EXIT_PORTAL_PATTERN: LazyLock<BlockPattern> = LazyLock::new(|| {
    BlockPatternBuilder::start()
        .aisle(&["       ", "       ", "       ", "   #   ", "       ", "       ", "       "])
        .aisle(&["       ", "       ", "       ", "   #   ", "       ", "       ", "       "])
        .aisle(&["       ", "       ", "       ", "   #   ", "       ", "       ", "       "])
        .aisle(&["  ###  ", " #   # ", "#     #", "#  #  #", "#     #", " #   # ", "  ###  "])
        .aisle(&["       ", "  ###  ", " ##### ", " ##### ", " ##### ", "  ###  ", "       "])
        .where_('#', has_state(|state| state.is_block(blocks::BEDROCK)))
        .build()
});

pub struct EnderDragonFight {
    dir: Option<PathBuf>,
    dirty: bool,
    dragon_event: Option<ServerBossEvent>,
    origin: BlockPos,
    gateways: Vec<i32>,
    ticks_since_dragon_seen: i32,
    alive_crystals: i32,
    ticks_since_crystals_scanned: i32,
    ticks_since_last_player_scan: i32,
    dragon_killed: bool,
    has_previously_killed_dragon: bool,
    pub skip_arena_loaded_check: bool,
    dragon_uuid: Option<u128>,
    needs_state_scanning: bool,
    exit_portal_location: Option<BlockPos>,
    respawn_stage: Option<DragonRespawnStage>,
    respawn_time: i32,
    respawn_crystals: Vec<u128>,
}

pub fn get_podium_location(offset: BlockPos) -> BlockPos {
    END_PODIUM_LOCATION.offset(offset.x, offset.y, offset.z)
}

fn bool_field(data: &Nbt, name: &str, default: bool) -> Result<bool, String> {
    match data.get(name) {
        None => Ok(default),
        Some(tag) => tag.as_i64().map(|v| v as i8 != 0).ok_or_else(|| "Not a number".to_string()),
    }
}

fn int_array(tag: &Nbt) -> Option<Vec<i32>> {
    match tag {
        Nbt::IntArray(values) => Some(values.clone()),
        Nbt::List(values) => values.iter().map(|v| v.as_i64().map(|v| v as i32)).collect(),
        Nbt::ByteArray(values) => Some(values.iter().map(|&v| v as i32).collect()),
        Nbt::LongArray(values) => Some(values.iter().map(|&v| v as i32).collect()),
        _ => None,
    }
}

fn uuid_value(tag: &Nbt) -> Option<u128> {
    let values = int_array(tag).filter(|values| values.len() == 4)?;
    Some(values.iter().fold(0u128, |acc, &v| acc << 32 | v as u32 as u128))
}

fn uuid_tag(uuid: u128) -> Nbt {
    Nbt::IntArray(vec![(uuid >> 96) as i32, (uuid >> 64) as i32, (uuid >> 32) as i32, uuid as i32])
}

impl EnderDragonFight {
    fn file(dir: &Path) -> PathBuf {
        dir.join("minecraft").join("ender_dragon_fight.dat")
    }

    fn create_default(dir: Option<PathBuf>) -> EnderDragonFight {
        EnderDragonFight {
            dir,
            dirty: true,
            dragon_event: None,
            origin: BlockPos::ZERO,
            gateways: Vec::new(),
            ticks_since_dragon_seen: 0,
            alive_crystals: 0,
            ticks_since_crystals_scanned: 0,
            ticks_since_last_player_scan: 21,
            dragon_killed: false,
            has_previously_killed_dragon: false,
            skip_arena_loaded_check: false,
            dragon_uuid: None,
            needs_state_scanning: true,
            exit_portal_location: None,
            respawn_stage: None,
            respawn_time: 0,
            respawn_crystals: Vec::new(),
        }
    }

    fn decode(dir: Option<PathBuf>, data: &Nbt) -> Result<EnderDragonFight, String> {
        let mut fight = EnderDragonFight::create_default(dir);
        fight.dirty = false;
        fight.needs_state_scanning = bool_field(data, "needs_state_scanning", true)?;
        fight.dragon_killed = bool_field(data, "dragon_killed", false)?;
        fight.has_previously_killed_dragon = bool_field(data, "previously_killed", false)?;
        fight.respawn_stage = match data.get("respawn_stage") {
            None => None,
            Some(tag) => {
                let name = tag.as_str().ok_or("Not a string")?;
                Some(DragonRespawnStage::by_name(name).ok_or_else(|| format!("Unknown element name:{name}"))?)
            }
        };
        fight.respawn_time = match data.get("respawn_time") {
            None => 0,
            Some(tag) => tag.as_i64().ok_or("Not a number")? as i32,
        };
        fight.dragon_uuid = data.get("dragon_uuid").and_then(uuid_value);
        fight.exit_portal_location = data.get("exit_portal_location").and_then(int_array).filter(|v| v.len() == 3).map(|v| BlockPos::new(v[0], v[1], v[2]));
        fight.gateways = data.get("gateways").and_then(int_array).unwrap_or_default();
        fight.respawn_crystals = match data.get("respawn_crystals") {
            None => Vec::new(),
            Some(Nbt::List(crystals)) => crystals.iter().map(|tag| uuid_value(tag).ok_or("Invalid UUID")).collect::<Result<_, _>>()?,
            Some(_) => return Err("Not a list".into()),
        };
        Ok(fight)
    }

    pub fn load(dir: Option<PathBuf>) -> EnderDragonFight {
        let Some(file) = dir.as_deref().map(EnderDragonFight::file) else { return EnderDragonFight::create_default(dir) };
        match read_saved_data_dat(&file, DataFixTypes::SavedDataEnderDragonFight) {
            Ok(Some(tag)) => match tag.get("data").ok_or_else(|| "Not a map: null".to_string()).and_then(|data| EnderDragonFight::decode(dir.clone(), data)) {
                Ok(fight) => fight,
                Err(error) => {
                    error!("Failed to parse saved data for 'SavedDataType[minecraft:ender_dragon_fight]': {error}");
                    EnderDragonFight::create_default(dir)
                }
            },
            Ok(None) => EnderDragonFight::create_default(dir),
            Err(_) => {
                error!("Error loading saved data: SavedDataType[minecraft:ender_dragon_fight]");
                EnderDragonFight::create_default(dir)
            }
        }
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(dir) = &self.dir else { return Ok(()) };
        if !self.dirty {
            return Ok(());
        }
        self.dirty = false;
        let mut data = vec![
            ("needs_state_scanning".to_string(), Nbt::Byte(self.needs_state_scanning as i8)),
            ("dragon_killed".to_string(), Nbt::Byte(self.dragon_killed as i8)),
            ("previously_killed".to_string(), Nbt::Byte(self.has_previously_killed_dragon as i8)),
        ];
        if let Some(stage) = self.respawn_stage {
            data.push(("respawn_stage".into(), Nbt::String(stage.get_serialized_name().into())));
        }
        data.push(("respawn_time".into(), Nbt::Int(self.respawn_time)));
        if let Some(uuid) = self.dragon_uuid {
            data.push(("dragon_uuid".into(), uuid_tag(uuid)));
        }
        if let Some(pos) = self.exit_portal_location {
            data.push(("exit_portal_location".into(), Nbt::IntArray(vec![pos.x, pos.y, pos.z])));
        }
        if !self.gateways.is_empty() {
            data.push(("gateways".into(), Nbt::List(self.gateways.iter().map(|&gateway| Nbt::Int(gateway)).collect())));
        }
        if !self.respawn_crystals.is_empty() {
            data.push(("respawn_crystals".into(), Nbt::List(self.respawn_crystals.iter().map(|&uuid| uuid_tag(uuid)).collect())));
        }
        write_dat(&EnderDragonFight::file(dir), &Nbt::Compound(vec![("data".into(), Nbt::Compound(data)), ("DataVersion".into(), Nbt::Int(WORLD_VERSION))]))
    }

    pub fn set_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn alive_crystals(&self) -> i32 {
        self.alive_crystals
    }

    pub fn has_previously_killed_dragon(&self) -> bool {
        self.has_previously_killed_dragon
    }

    pub fn dragon_uuid(&self) -> Option<u128> {
        self.dragon_uuid
    }
}

impl ServerLevel {
    fn fight(&mut self) -> &mut EnderDragonFight {
        self.dragon_fight.as_mut().expect("a dragon fight")
    }

    fn with_dragon_event(&mut self, f: impl FnOnce(&mut ServerBossEvent, &ServerLevel)) {
        let Some(mut dragon_event) = self.fight().dragon_event.take() else { return };
        f(&mut dragon_event, self);
        self.fight().dragon_event = Some(dragon_event);
    }

    fn dragon_event_players(&self) -> Vec<EntityId> {
        self.dragon_fight.as_ref().and_then(|fight| fight.dragon_event.as_ref()).map_or_else(Vec::new, |event| event.players.clone())
    }

    pub fn dragon_fight_init(&mut self, seed: i64, origin: BlockPos) {
        let id = create_insecure_uuid(&mut self.random);
        let mut dragon_event = ServerBossEvent::new(id, Component::translatable("entity.minecraft.ender_dragon"), BossBarColor::Pink, BossBarOverlay::Progress);
        dragon_event.play_boss_music = true;
        dragon_event.create_world_fog = true;
        let fight = self.fight();
        fight.origin = origin;
        fight.dragon_event = Some(dragon_event);
        if fight.gateways.is_empty() {
            let mut new_gateways: Vec<i32> = (0..GATEWAY_COUNT).collect();
            crate::util::shuffle(&mut new_gateways, &mut LegacyRandomSource::new(seed));
            fight.gateways.extend(new_gateways);
            fight.set_dirty();
        }
    }

    pub fn dragon_fight_tick(&mut self) {
        let visible = !self.fight().dragon_killed;
        if let Some(event) = &mut self.fight().dragon_event {
            event.set_visible(visible);
        }
        let fight = self.fight();
        fight.ticks_since_last_player_scan += 1;
        if fight.ticks_since_last_player_scan >= TIME_BETWEEN_PLAYER_SCANS {
            self.dragon_fight_update_players();
            self.fight().ticks_since_last_player_scan = 0;
        }
        if self.dragon_event_players().is_empty() {
            self.chunks.remove_ticket_with_radius(TicketType::Dragon, ChunkPos::new(0, 0), ARENA_TICKET_LEVEL);
            return;
        }
        self.chunks.add_ticket_with_radius(TicketType::Dragon, ChunkPos::new(0, 0), ARENA_TICKET_LEVEL);
        if !self.dragon_fight_is_arena_loaded() {
            return;
        }
        if self.fight().needs_state_scanning {
            self.dragon_fight_scan_state();
            let fight = self.fight();
            fight.needs_state_scanning = false;
            fight.set_dirty();
        }
        if let Some(stage) = self.fight().respawn_stage {
            let crystal_uuids = self.fight().respawn_crystals.clone();
            let respawn_crystals: Vec<EntityId> = crystal_uuids
                .into_iter()
                .filter_map(|uuid| EntityReference::of(uuid).get_entity(self))
                .filter(|&crystal| matches!(self.find_entity(crystal), Some(LevelEntity::EndCrystal(_))))
                .collect();
            if respawn_crystals.is_empty() {
                self.dragon_fight_abort_respawn_sequence();
                return;
            }
            let fight = self.fight();
            let time = fight.respawn_time;
            fight.respawn_time += 1;
            self.dragon_respawn_stage_tick(stage, &respawn_crystals, time);
            self.fight().set_dirty();
        }
        if !self.fight().dragon_killed {
            let fight = self.fight();
            let find = fight.dragon_uuid.is_none() || {
                fight.ticks_since_dragon_seen += 1;
                fight.ticks_since_dragon_seen >= MAX_TICKS_BEFORE_DRAGON_RESPAWN
            };
            if find {
                self.dragon_fight_find_or_create_dragon();
                self.fight().ticks_since_dragon_seen = 0;
            }
            let fight = self.fight();
            fight.ticks_since_crystals_scanned += 1;
            if fight.ticks_since_crystals_scanned >= TIME_BETWEEN_CRYSTAL_SCANS {
                self.dragon_fight_update_crystal_count();
                self.fight().ticks_since_crystals_scanned = 0;
            }
        }
    }

    fn dragon_fight_scan_state(&mut self) {
        info!("Scanning for legacy world dragon fight...");
        let active_portal_exists = self.dragon_fight_has_active_exit_portal();
        if active_portal_exists {
            info!("Found that the dragon has been killed in this world already.");
            self.fight().has_previously_killed_dragon = true;
        } else {
            info!("Found that the dragon has not yet been killed in this world.");
            self.fight().has_previously_killed_dragon = false;
            if self.dragon_fight_find_exit_portal().is_none() {
                self.dragon_fight_spawn_exit_portal(false);
            }
        }
        let entities = self.get_dragons();
        match entities.first() {
            None => self.fight().dragon_killed = true,
            Some(&dragon) => {
                let uuid = self.entity(dragon).uuid;
                self.fight().dragon_uuid = Some(uuid);
                info!("Found that there's a dragon still alive ({})", self.entity_to_string(dragon));
                self.fight().dragon_killed = false;
                if !active_portal_exists {
                    info!("But we didn't have a portal, let's remove it.");
                    self.discard(dragon);
                    self.fight().dragon_uuid = None;
                }
            }
        }
        let fight = self.fight();
        if !fight.has_previously_killed_dragon && fight.dragon_killed {
            fight.dragon_killed = false;
        }
        fight.set_dirty();
    }

    pub fn get_dragons(&self) -> Vec<EntityId> {
        self.all_entities().filter(|&id| self.is_ender_dragon(id) && self.is_alive(id)).collect()
    }

    fn dragon_fight_find_or_create_dragon(&mut self) {
        let entities = self.get_dragons();
        match entities.first() {
            None => {
                self.dragon_fight_create_new_dragon();
            }
            Some(&dragon) => {
                let uuid = self.entity(dragon).uuid;
                let fight = self.fight();
                fight.dragon_uuid = Some(uuid);
                fight.set_dirty();
            }
        }
    }

    pub fn dragon_fight_set_respawn_stage(&mut self, stage: DragonRespawnStage) {
        let fight = self.fight();
        if fight.respawn_stage.is_none() {
            panic!("Dragon respawn isn't in progress, can't skip ahead in the animation.");
        }
        fight.respawn_time = 0;
        if stage == DragonRespawnStage::End {
            fight.respawn_stage = None;
            fight.dragon_killed = false;
            if let Some(dragon) = self.dragon_fight_create_new_dragon() {
                for player in self.dragon_event_players() {
                    criteria_triggers::SUMMONED_ENTITY.trigger(self, player, dragon);
                }
            }
        } else {
            fight.respawn_stage = Some(stage);
        }
        self.fight().set_dirty();
    }

    fn dragon_fight_block_entities_in(&mut self, chunk_x: i32, chunk_z: i32) -> Vec<BlockPos> {
        let chunk = self.get_chunk(ChunkPos::new(chunk_x, chunk_z));
        chunk
            .block_entities
            .map
            .keys_in_order()
            .into_iter()
            .filter(|pos| chunk.block_entities.map.get(pos).is_some_and(|be| matches!(be.ty(), block_entity_types::END_PORTAL | block_entity_types::END_GATEWAY)))
            .collect()
    }

    fn dragon_fight_has_active_exit_portal(&mut self) -> bool {
        for x in -ARENA_SIZE_CHUNKS..=ARENA_SIZE_CHUNKS {
            for z in -ARENA_SIZE_CHUNKS..=ARENA_SIZE_CHUNKS {
                if !self.dragon_fight_block_entities_in(x, z).is_empty() {
                    return true;
                }
            }
        }
        false
    }

    fn dragon_fight_find_exit_portal(&mut self) -> Option<BlockPatternMatch> {
        let chunk_origin = self.fight().origin.chunk();
        for x in -ARENA_SIZE_CHUNKS + chunk_origin.x..=ARENA_SIZE_CHUNKS + chunk_origin.x {
            for z in -ARENA_SIZE_CHUNKS + chunk_origin.z..=ARENA_SIZE_CHUNKS + chunk_origin.z {
                for pos in self.dragon_fight_block_entities_in(x, z) {
                    if let Some(found) = EXIT_PORTAL_PATTERN.find(self, pos) {
                        let pos_in_world = found.get_block(3, 3, 3);
                        let fight = self.fight();
                        if fight.exit_portal_location.is_none() {
                            fight.exit_portal_location = Some(pos_in_world);
                            fight.set_dirty();
                        }
                        return Some(found);
                    }
                }
            }
        }
        let end_podium_location = get_podium_location(self.fight().origin);
        let max_y = self.get_heightmap_pos(HeightmapType::MotionBlocking, end_podium_location).y;
        let mut y = max_y;
        while y >= self.get_min_y() {
            if let Some(found) = EXIT_PORTAL_PATTERN.find(self, BlockPos::new(end_podium_location.x, y, end_podium_location.z)) {
                let pos_in_world = found.get_block(3, 3, 3);
                let fight = self.fight();
                if fight.exit_portal_location.is_none() {
                    fight.exit_portal_location = Some(pos_in_world);
                    fight.set_dirty();
                }
                return Some(found);
            }
            y -= 1;
        }
        None
    }

    fn dragon_fight_is_arena_loaded(&self) -> bool {
        let fight = self.dragon_fight.as_ref().expect("a dragon fight");
        if fight.skip_arena_loaded_check {
            return true;
        }
        let chunk_origin = fight.origin.chunk();
        for x in -ARENA_SIZE_CHUNKS + chunk_origin.x..=ARENA_SIZE_CHUNKS + chunk_origin.x {
            let z = ARENA_SIZE_CHUNKS + chunk_origin.z;
            let pos = ChunkPos::new(x, z);
            if self.chunks.loaded_chunk(pos).is_none() {
                return false;
            }
            let status = self.chunks.ticket_level(pos).map_or(FullChunkStatus::Inaccessible, chunk_level::full_status);
            if !status.is_or_after(FullChunkStatus::BlockTicking) {
                return false;
            }
        }
        for x in chunk_origin.x - 1..=chunk_origin.x + 1 {
            for z in chunk_origin.z - 1..=chunk_origin.z + 1 {
                if !self.entity_manager.are_entities_loaded(ChunkPos::new(x, z).pack()) {
                    return false;
                }
            }
        }
        true
    }

    fn dragon_fight_update_players(&mut self) {
        let fight = self.dragon_fight.as_ref().expect("a dragon fight");
        let (cx, cy, cz) = (fight.origin.x as f64, (DRAGON_SPAWN_Y + fight.origin.y) as f64, fight.origin.z as f64);
        let new_players: Vec<EntityId> = self
            .players
            .keys()
            .copied()
            .filter(|&player| self.is_alive(player) && self.entity(player).distance_to_sqr_xyz(cx, cy, cz) <= 192.0 * 192.0)
            .collect();
        self.with_dragon_event(|event, level| {
            for &player in &new_players {
                event.add_player(std::slice::from_ref(level), player);
            }
            let to_remove: Vec<EntityId> = event.players.iter().copied().filter(|player| !new_players.contains(player)).collect();
            for player in to_remove {
                event.remove_player(player);
            }
        });
    }

    pub fn dragon_fight_update_crystal_count(&mut self) {
        self.fight().ticks_since_crystals_scanned = 0;
        let mut alive_crystals = 0;
        for spike in end_spike_feature::get_spikes_for_level(self.seed) {
            alive_crystals += self.dragon_fight_end_crystals_in(&spike.get_top_bounding_box()).len() as i32;
        }
        self.fight().alive_crystals = alive_crystals;
    }

    fn dragon_fight_end_crystals_in(&self, area: &AABB) -> Vec<EntityId> {
        self.get_entities_matching(None, area, |level, id| !level.is_spectator(id) && matches!(level.find_entity(id), Some(LevelEntity::EndCrystal(_))))
    }

    pub fn dragon_fight_set_dragon_killed(&mut self, dragon: EntityId) {
        if Some(self.entity(dragon).uuid) != self.fight().dragon_uuid {
            return;
        }
        if let Some(event) = &mut self.fight().dragon_event {
            event.set_progress(0.0);
            event.set_visible(false);
        }
        self.dragon_fight_spawn_exit_portal(true);
        self.dragon_fight_spawn_new_gateway();
        if !self.fight().has_previously_killed_dragon {
            let podium = get_podium_location(self.fight().origin);
            let pos = self.get_heightmap_pos(HeightmapType::MotionBlocking, podium);
            WorldGenLevel::set_block_and_update(self, pos, blocks::DRAGON_EGG.default_state());
        }
        let fight = self.fight();
        fight.has_previously_killed_dragon = true;
        fight.dragon_killed = true;
        fight.set_dirty();
    }

    fn dragon_fight_spawn_new_gateway(&mut self) {
        let Some(gateway) = self.fight().gateways.pop() else { return };
        let x = mth::floor(GATEWAY_DISTANCE * (2.0 * (-std::f64::consts::PI + (std::f64::consts::PI / 20.0) * gateway as f64)).cos());
        let z = mth::floor(GATEWAY_DISTANCE * (2.0 * (-std::f64::consts::PI + (std::f64::consts::PI / 20.0) * gateway as f64)).sin());
        self.dragon_fight_spawn_new_gateway_at(BlockPos::new(x, 75, z));
        self.fight().set_dirty();
    }

    fn dragon_fight_spawn_new_gateway_at(&mut self, pos: BlockPos) {
        self.level_event(None, events::ANIMATION_END_GATEWAY_SPAWN, pos, 0);
        EndGatewayFeature::delayed_exit_search().place(self, pos);
    }

    fn dragon_fight_spawn_exit_portal(&mut self, activated: bool) {
        if self.fight().exit_portal_location.is_none() {
            let podium = get_podium_location(self.fight().origin);
            let mut exit_portal_location = self.get_heightmap_pos(HeightmapType::MotionBlockingNoLeaves, podium).below();
            while self.block_state(exit_portal_location).is_block(blocks::BEDROCK) && exit_portal_location.y > 63 {
                exit_portal_location = exit_portal_location.below();
            }
            let y = (self.get_min_y() + 1).max(exit_portal_location.y);
            let fight = self.fight();
            fight.exit_portal_location = Some(BlockPos::new(exit_portal_location.x, y, exit_portal_location.z));
            fight.set_dirty();
        }
        let exit_portal_location = self.fight().exit_portal_location.expect("an exit portal location");
        if EndPodiumFeature::new(activated).place(self, exit_portal_location) {
            let chunk_radius = wafer_core::mth::positive_ceil_div(4, 16);
            self.chunks.wait_for_light_before_sending(exit_portal_location.chunk(), chunk_radius);
        }
    }

    fn dragon_fight_create_new_dragon(&mut self) -> Option<EntityId> {
        let origin = self.fight().origin;
        self.load_chunk(BlockPos::new(origin.x, DRAGON_SPAWN_Y + origin.y, origin.z).chunk());
        let mut dragon = EnderDragon::create();
        let state = ender_dragon_mut(&mut dragon);
        state.dragon_fight = true;
        state.fight_origin = origin;
        switch_phase(&mut dragon, EnderDragonPhase::HoldingPattern);
        let y_rot = self.random.next_float() * 360.0;
        dragon.entity.snap_to(origin.x as f64, (DRAGON_SPAWN_Y + origin.y) as f64, origin.z as f64, y_rot, 0.0);
        let (id, uuid) = (dragon.entity.id, dragon.entity.uuid);
        self.add_fresh_entity(LevelEntity::Mob(Box::new(dragon)));
        let fight = self.fight();
        fight.dragon_uuid = Some(uuid);
        fight.set_dirty();
        Some(id)
    }

    pub fn dragon_fight_update_dragon(&mut self, dragon: EntityId) {
        if Some(self.entity(dragon).uuid) != self.fight().dragon_uuid {
            return;
        }
        let progress = self.health(dragon) / self.max_health(dragon);
        let name = self.entity(dragon).has_custom_name().then(|| self.display_name(dragon));
        if let Some(event) = &mut self.fight().dragon_event {
            event.set_progress(progress);
            if let Some(name) = name {
                event.set_name(name);
            }
        }
        self.fight().ticks_since_dragon_seen = 0;
    }

    pub fn dragon_fight_on_crystal_destroyed(&mut self, crystal: EntityId, source: &DamageSource) {
        let uuid = self.entity(crystal).uuid;
        let fight = self.fight();
        if fight.respawn_stage.is_some() && fight.respawn_crystals.contains(&uuid) {
            self.dragon_fight_abort_respawn_sequence();
            return;
        }
        self.dragon_fight_update_crystal_count();
        if let Some(dragon) = self.fight().dragon_uuid.and_then(|uuid| self.entity_manager.visible_entity_storage.get(uuid))
            && self.is_ender_dragon(dragon)
        {
            let pos = self.entity(crystal).block_position();
            self.ender_dragon_on_crystal_destroyed(dragon, crystal, pos, source);
        }
    }

    fn dragon_fight_abort_respawn_sequence(&mut self) {
        let fight = self.fight();
        fight.respawn_stage = None;
        fight.respawn_time = 0;
        self.dragon_fight_reset_spike_crystals();
        self.dragon_fight_spawn_exit_portal(true);
        self.fight().set_dirty();
    }

    pub fn dragon_fight_try_respawn(&mut self) {
        let fight = self.fight();
        if !fight.dragon_killed || fight.respawn_stage.is_some() {
            return;
        }
        let location = match fight.exit_portal_location {
            Some(location) => location,
            None => {
                if self.dragon_fight_find_exit_portal().is_none() {
                    self.dragon_fight_spawn_exit_portal(true);
                }
                self.fight().exit_portal_location.expect("an exit portal location")
            }
        };
        let mut crystals = Vec::new();
        let center = location.above();
        for direction in Direction::PLANE_HORIZONTAL {
            let found = self.dragon_fight_end_crystals_in(&AABB::of_block(center.relative_n(direction, 3)));
            if found.is_empty() {
                return;
            }
            crystals.extend(found);
        }
        self.dragon_fight_respawn_dragon(crystals);
    }

    fn dragon_fight_respawn_dragon(&mut self, crystals: Vec<EntityId>) {
        let fight = self.fight();
        if !fight.dragon_killed || fight.respawn_stage.is_some() {
            return;
        }
        while let Some(portal) = self.dragon_fight_find_exit_portal() {
            for x in 0..EXIT_PORTAL_PATTERN.get_width() as i32 {
                for y in 0..EXIT_PORTAL_PATTERN.get_height() as i32 {
                    for z in 0..EXIT_PORTAL_PATTERN.get_depth() as i32 {
                        let pos = portal.get_block(x, y, z);
                        let state = self.block_state(pos);
                        if state.is_block(blocks::BEDROCK) || state.is_block(blocks::END_PORTAL) {
                            WorldGenLevel::set_block_and_update(self, pos, blocks::END_STONE.default_state());
                        }
                    }
                }
            }
        }
        let fight = self.fight();
        fight.respawn_stage = Some(DragonRespawnStage::Start);
        fight.respawn_time = 0;
        self.dragon_fight_spawn_exit_portal(false);
        let respawn_crystals = crystals.iter().map(|&crystal| self.entity(crystal).uuid).collect();
        let fight = self.fight();
        fight.respawn_crystals = respawn_crystals;
        fight.set_dirty();
    }

    pub fn dragon_fight_reset_spike_crystals(&mut self) {
        for spike in end_spike_feature::get_spikes_for_level(self.seed) {
            for crystal in self.dragon_fight_end_crystals_in(&spike.get_top_bounding_box()) {
                if let Some(LevelEntity::EndCrystal(end_crystal)) = self.find_entity_mut(crystal) {
                    end_crystal.entity.invulnerable = false;
                    end_crystal.set_beam_target(None);
                }
            }
        }
    }
}
