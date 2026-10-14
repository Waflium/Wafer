use crate::server::level::ServerLevel;

pub enum CustomSpawner {}

impl CustomSpawner {
    pub fn tick(&mut self, _level: &mut ServerLevel, _spawn_enemies: bool) {
        match *self {}
    }
}
