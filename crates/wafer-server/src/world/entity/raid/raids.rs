use std::io;
use std::path::{Path, PathBuf};

use wafer_core::BlockPos;
use wafer_data::WORLD_VERSION;
use wafer_protocol::nbt::Nbt;

use super::raid::Raid;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::storage::{read_dat, write_dat};

pub struct Raids {
    dir: Option<PathBuf>,
    raids: Option<Nbt>,
    raid_map: Vec<(i32, Raid)>,
    next_id: i32,
    tick: i32,
    dirty: bool,
}

impl Raids {
    fn file(dir: &Path) -> PathBuf {
        dir.join("minecraft").join("raids.dat")
    }

    pub fn load(dir: Option<PathBuf>) -> Raids {
        let data = dir.as_deref().and_then(|dir| match read_dat(&Raids::file(dir)) {
            Ok(tag) => tag.and_then(|t| t.get("data").cloned()),
            Err(_) => {
                error!("Error loading saved data: SavedDataType[minecraft:raids]");
                None
            }
        });
        let field = |name: &str| data.as_ref().and_then(|d| d.get(name)).and_then(Nbt::as_i64).map(|v| v as i32);
        match (field("next_id"), field("tick")) {
            (Some(next_id), Some(tick)) => {
                let raids = data.as_ref().and_then(|d| d.get("raids")).cloned();
                Raids { dir, raids, raid_map: Vec::new(), next_id, tick, dirty: false }
            }
            _ => Raids { dir, raids: None, raid_map: Vec::new(), next_id: 1, tick: 0, dirty: true },
        }
    }

    pub fn get(&self, raid_id: i32) -> Option<&Raid> {
        self.raid_map.iter().find(|(id, _)| *id == raid_id).map(|(_, raid)| raid)
    }

    pub fn get_id(&self, raid: &Raid) -> Option<i32> {
        self.raid_map.iter().find(|(_, other)| std::ptr::eq(other, raid)).map(|(id, _)| *id)
    }

    pub fn get_nearby_raid(&self, pos: BlockPos, max_dist_sqr: i32) -> Option<&Raid> {
        let mut closest = None;
        let mut closest_distance_sqr = max_dist_sqr as f64;
        for (_, raid) in &self.raid_map {
            let distance = raid.get_center().dist_sqr(pos);
            if raid.is_active() && distance < closest_distance_sqr {
                closest = Some(raid);
                closest_distance_sqr = distance;
            }
        }
        closest
    }

    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        if self.tick % 200 == 0 {
            self.dirty = true;
        }
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(dir) = &self.dir else { return Ok(()) };
        if !self.dirty {
            return Ok(());
        }
        self.dirty = false;
        let mut data = Vec::new();
        if let Some(raids) = &self.raids
            && !matches!(raids, Nbt::List(list) if list.is_empty())
        {
            data.push(("raids".into(), raids.clone()));
        }
        data.push(("next_id".into(), Nbt::Int(self.next_id)));
        data.push(("tick".into(), Nbt::Int(self.tick)));
        write_dat(&Raids::file(dir), &Nbt::Compound(vec![("data".into(), Nbt::Compound(data)), ("DataVersion".into(), Nbt::Int(WORLD_VERSION))]))
    }
}

pub fn can_join_raid(level: &ServerLevel, raider: EntityId) -> bool {
    level.is_alive(raider) && level.mob(raider).and_then(|mob| mob.raider()).is_some_and(|raider| raider.can_join_raid()) && level.living(raider).no_action_time <= 2400
}

impl ServerLevel {
    pub fn get_raid_at(&self, pos: BlockPos) -> Option<&Raid> {
        self.raids.get_nearby_raid(pos, 9216)
    }
}
