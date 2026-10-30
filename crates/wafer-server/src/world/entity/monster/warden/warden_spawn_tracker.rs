use wafer_core::{AABB, BlockPos, Vec3, mth};
use wafer_data::{registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::util::codec::{self, DecodeResult, Record};

pub const MAX_WARNING_LEVEL: i32 = 4;
const PLAYER_SEARCH_RADIUS: f64 = 16.0;
const WARNING_CHECK_DIAMETER: f64 = 48.0;
const DECREASE_WARNING_LEVEL_EVERY_INTERVAL: i32 = 12000;
const WARNING_LEVEL_INCREASE_COOLDOWN: i32 = 200;

pub fn try_warn(level: &mut ServerLevel, pos: BlockPos, trigger_player: EntityId) -> Option<i32> {
    if has_nearby_warden(level, pos) {
        return None;
    }
    let mut players = get_nearby_players(level, pos);
    if !players.contains(&trigger_player) {
        players.push(trigger_player);
    }
    if players.iter().any(|&player| level.player(player).warden_spawn_tracker.on_cooldown()) {
        return None;
    }
    let mut highest: Option<EntityId> = None;
    for &player in &players {
        if highest.is_none_or(|highest| level.player(player).warden_spawn_tracker.get_warning_level() > level.player(highest).warden_spawn_tracker.get_warning_level()) {
            highest = Some(player);
        }
    }
    let highest = highest?;
    let mut spawn_tracker = level.player(highest).warden_spawn_tracker;
    spawn_tracker.increase_warning_level();
    level.player_mut(highest).warden_spawn_tracker = spawn_tracker;
    for &player in &players {
        level.player_mut(player).warden_spawn_tracker.copy_data(&spawn_tracker);
    }
    Some(spawn_tracker.warning_level)
}

fn has_nearby_warden(level: &ServerLevel, pos: BlockPos) -> bool {
    let area_to_check = AABB::of_size(Vec3::at_center_of(pos), WARNING_CHECK_DIAMETER, WARNING_CHECK_DIAMETER, WARNING_CHECK_DIAMETER);
    let warden = registry_id(registry::ENTITY_TYPE, "minecraft:warden");
    level.entities.values().any(|entity| entity.entity().entity_type == warden && entity.entity().bounding_box().intersects(&area_to_check))
}

fn get_nearby_players(level: &ServerLevel, pos: BlockPos) -> Vec<EntityId> {
    let origin = Vec3::at_center_of(pos);
    level.players.keys().copied().filter(|&player| !level.is_spectator(player) && level.entity(player).position().closer_than(origin, PLAYER_SEARCH_RADIUS) && level.is_alive(player)).collect()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WardenSpawnTracker {
    ticks_since_last_warning: i32,
    warning_level: i32,
    cooldown_ticks: i32,
}

impl WardenSpawnTracker {
    pub fn new(ticks_since_last_warning: i32, warning_level: i32, cooldown_ticks: i32) -> WardenSpawnTracker {
        WardenSpawnTracker { ticks_since_last_warning, warning_level, cooldown_ticks }
    }

    pub fn tick(&mut self) {
        if self.ticks_since_last_warning >= DECREASE_WARNING_LEVEL_EVERY_INTERVAL {
            self.decrease_warning_level();
            self.ticks_since_last_warning = 0;
        } else {
            self.ticks_since_last_warning += 1;
        }
        if self.cooldown_ticks > 0 {
            self.cooldown_ticks -= 1;
        }
    }

    pub fn reset(&mut self) {
        self.ticks_since_last_warning = 0;
        self.warning_level = 0;
        self.cooldown_ticks = 0;
    }

    fn on_cooldown(&self) -> bool {
        self.cooldown_ticks > 0
    }

    fn increase_warning_level(&mut self) {
        if !self.on_cooldown() {
            self.ticks_since_last_warning = 0;
            self.cooldown_ticks = WARNING_LEVEL_INCREASE_COOLDOWN;
            self.set_warning_level(self.get_warning_level() + 1);
        }
    }

    fn copy_data(&mut self, copy_from: &WardenSpawnTracker) {
        self.warning_level = copy_from.warning_level;
        self.cooldown_ticks = copy_from.cooldown_ticks;
        self.ticks_since_last_warning = copy_from.ticks_since_last_warning;
    }

    fn decrease_warning_level(&mut self) {
        self.set_warning_level(self.get_warning_level() - 1);
    }

    pub fn set_warning_level(&mut self, warning_level: i32) {
        self.warning_level = mth::clamp_i(warning_level, 0, MAX_WARNING_LEVEL);
    }

    pub fn get_warning_level(&self) -> i32 {
        self.warning_level
    }

    pub fn to_nbt(&self) -> Nbt {
        Nbt::Compound(vec![
            ("ticks_since_last_warning".into(), Nbt::Int(self.ticks_since_last_warning)),
            ("warning_level".into(), Nbt::Int(self.warning_level)),
            ("cooldown_ticks".into(), Nbt::Int(self.cooldown_ticks)),
        ].into())
    }

    pub fn decode(input: &Nbt) -> DecodeResult<WardenSpawnTracker> {
        if let Err(error) = codec::map(input) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let mut field = |key: &str| match input.get(key) {
            Some(tag) => record.field(codec::int(tag).and_then(|value| if value < 0 { Err(format!("Value must be non-negative: {value}")) } else { Ok(value) })),
            None => Some(0),
        };
        let ticks_since_last_warning = field("ticks_since_last_warning");
        let warning_level = field("warning_level");
        let cooldown_ticks = field("cooldown_ticks");
        record.build(|| Some(WardenSpawnTracker::new(ticks_since_last_warning?, warning_level?, cooldown_ticks?)))
    }
}
