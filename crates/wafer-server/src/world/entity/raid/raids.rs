use std::io;
use std::path::{Path, PathBuf};

use wafer_core::fastutil::OpenHashMap;
use wafer_core::{BlockPos, Vec3};
use wafer_data::{WORLD_VERSION, game_rule};
use wafer_protocol::nbt::Nbt;

use super::raid::{self, Raid};
use crate::util::datafix::data_fix_types::DataFixTypes;
use crate::server::level::ServerLevel;
use crate::world::attribute::environment_attributes;
use crate::world::entity::EntityId;
use crate::world::entity::ai::village::poi::poi_manager::Occupancy;
use crate::world::level::storage::{read_saved_data_dat, write_dat};

pub struct Raids {
    dir: Option<PathBuf>,
    raid_map: OpenHashMap<i32, Raid>,
    detached: Vec<(i32, Raid)>,
    next_id: i32,
    tick: i32,
    dirty: bool,
}

impl Raids {
    fn file(dir: &Path) -> PathBuf {
        dir.join("minecraft").join("raids.dat")
    }

    fn empty(dir: Option<PathBuf>) -> Raids {
        Raids { dir, raid_map: OpenHashMap::new(), detached: Vec::new(), next_id: 1, tick: 0, dirty: true }
    }

    pub fn load(dir: Option<PathBuf>) -> Raids {
        let data = dir.as_deref().and_then(|dir| match read_saved_data_dat(&Raids::file(dir), DataFixTypes::SavedDataRaids) {
            Ok(tag) => tag.and_then(|t| t.get("data").cloned()),
            Err(_) => {
                error!("Error loading saved data: SavedDataType[minecraft:raids]");
                None
            }
        });
        let field = |name: &str| data.as_ref().and_then(|d| d.get(name)).and_then(Nbt::as_i64).map(|v| v as i32);
        let (Some(next_id), Some(tick)) = (field("next_id"), field("tick")) else { return Raids::empty(dir) };
        let mut raid_map = OpenHashMap::new();
        if let Some(Nbt::List(raids)) = data.as_ref().and_then(|d| d.get("raids")) {
            for entry in raids {
                let Some(id) = entry.get("id").and_then(Nbt::as_i64).map(|v| v as i32) else { continue };
                if let Some(raid) = Raid::load(entry) {
                    raid_map.insert(id, raid);
                }
            }
        }
        Raids { dir, raid_map, detached: Vec::new(), next_id, tick, dirty: false }
    }

    pub fn set_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn get(&self, raid_id: i32) -> Option<&Raid> {
        self.raid_map.get(raid_id)
    }

    pub fn contains(&self, raid_id: i32) -> bool {
        self.raid_map.contains_key(raid_id)
    }

    pub fn get_any(&self, raid_id: i32) -> Option<&Raid> {
        self.raid_map.get(raid_id).or_else(|| self.detached.iter().find(|(id, _)| *id == raid_id).map(|(_, raid)| raid))
    }

    pub fn get_any_mut(&mut self, raid_id: i32) -> Option<&mut Raid> {
        if self.raid_map.contains_key(raid_id) {
            return self.raid_map.get_mut(raid_id);
        }
        self.detached.iter_mut().find(|(id, _)| *id == raid_id).map(|(_, raid)| raid)
    }

    fn get_unique_id(&mut self) -> i32 {
        self.next_id += 1;
        self.next_id
    }

    pub fn get_nearby_raid(&self, pos: BlockPos, max_dist_sqr: i32) -> Option<&Raid> {
        self.get_nearby_raid_id(pos, max_dist_sqr).and_then(|id| self.raid_map.get(id))
    }

    pub fn get_nearby_raid_id(&self, pos: BlockPos, max_dist_sqr: i32) -> Option<i32> {
        let mut closest = None;
        let mut closest_distance_sqr = max_dist_sqr as f64;
        for (id, raid) in self.raid_map.iter() {
            let distance = raid.get_center().dist_sqr(pos);
            if raid.is_active() && distance < closest_distance_sqr {
                closest = Some(id);
                closest_distance_sqr = distance;
            }
        }
        closest
    }

    pub fn get_id_at(&self, pos: BlockPos) -> Option<i32> {
        self.get_nearby_raid_id(pos, raid::VALID_RAID_RADIUS_SQR)
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(dir) = &self.dir else { return Ok(()) };
        if !self.dirty {
            return Ok(());
        }
        self.dirty = false;
        let mut data = Vec::new();
        let raids: Vec<Nbt> = self
            .raid_map
            .iter()
            .map(|(id, raid)| {
                let mut fields = vec![("id".to_string(), Nbt::Int(id))];
                raid.save(&mut fields);
                Nbt::Compound(fields)
            })
            .collect();
        if !raids.is_empty() {
            data.push(("raids".into(), Nbt::List(raids)));
        }
        data.push(("next_id".into(), Nbt::Int(self.next_id)));
        data.push(("tick".into(), Nbt::Int(self.tick)));
        write_dat(&Raids::file(dir), &Nbt::Compound(vec![("data".into(), Nbt::Compound(data)), ("DataVersion".into(), Nbt::Int(WORLD_VERSION))]))
    }
}

pub fn can_join_raid(level: &ServerLevel, raider: EntityId) -> bool {
    level.is_alive(raider) && level.mob(raider).and_then(|mob| mob.raider()).is_some_and(|raider| raider.can_join_raid()) && level.living(raider).no_action_time <= raid::MAX_NO_ACTION_TIME
}

impl ServerLevel {
    pub fn get_raid_at(&self, pos: BlockPos) -> Option<&Raid> {
        self.raids.get_nearby_raid(pos, raid::VALID_RAID_RADIUS_SQR)
    }

    pub fn raids_tick(&mut self) {
        self.raids.tick = self.raids.tick.wrapping_add(1);
        let ids: Vec<i32> = self.raids.raid_map.keys().collect();
        for id in ids {
            if !self.game_rules.get_bool(game_rule::RAIDS) {
                self.raid_stop(id);
            }
            if self.raids.get(id).is_some_and(Raid::is_stopped) {
                let raid = self.raids.raid_map.remove(id).expect("a stopped raid");
                for raider in raid.get_all_raiders() {
                    if self.mob(raider).and_then(crate::world::entity::mob::Mob::raider).is_some_and(|r| r.get_current_raid_id() == Some(id)) {
                        self.raider_mut(raider).detach_raid();
                    }
                }
                self.raids.detached.push((id, raid));
                self.raids.set_dirty();
            } else {
                self.raid_tick(id);
            }
        }
        if self.raids.tick % 200 == 0 {
            self.raids.set_dirty();
        }
    }

    pub fn create_or_extend_raid(&mut self, player: EntityId, raid_position: BlockPos) -> Option<i32> {
        if self.is_spectator(player) {
            return None;
        }
        if !self.game_rules.get_bool(game_rule::RAIDS) {
            return None;
        }
        if !self.environment_attributes().get_value_at(environment_attributes::CAN_START_RAID, raid_position) {
            return None;
        }
        let posses = self.chunks.poi_manager.get_in_range(&|poi_type| poi_type.is_village(), raid_position, 64, Occupancy::IsOccupied);
        let mut count = 0;
        let mut pos_totals = Vec3::ZERO;
        for p in &posses {
            let pos = p.pos;
            pos_totals = pos_totals.add(pos.x as f64, pos.y as f64, pos.z as f64);
            count += 1;
        }
        let raid_center_pos = if count > 0 { BlockPos::containing_vec(pos_totals.scale(1.0 / count as f64)) } else { raid_position };
        let raid_id = match self.raids.get_id_at(raid_center_pos) {
            Some(id) => id,
            None => {
                let raid = Raid::new(raid_center_pos, self.difficulty);
                let id = self.raids.get_unique_id();
                self.raids.raid_map.insert(id, raid);
                id
            }
        };
        let raid = self.raids.get(raid_id).expect("a raid");
        if !raid.is_started() || raid.get_raid_omen_level() < raid.get_max_raid_omen_level() {
            self.raid_absorb_raid_omen(raid_id, player);
        }
        self.raids.set_dirty();
        Some(raid_id)
    }
}
