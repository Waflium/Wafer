use crate::server::level::ServerLevel;
use crate::world::entity::npc::cat_spawner::CatSpawner;

pub enum CustomSpawner {
    Patrol(crate::world::level::levelgen::patrol_spawner::PatrolSpawner),
    Cat(CatSpawner),
    VillageSiege(crate::world::entity::ai::village::village_siege::VillageSiege),
    WanderingTrader(crate::world::entity::npc::wanderingtrader::wandering_trader_spawner::WanderingTraderSpawner),
}

impl CustomSpawner {
    pub fn tick(&mut self, level: &mut ServerLevel, spawn_enemies: bool) {
        match self {
            CustomSpawner::Patrol(spawner) => spawner.tick(level, spawn_enemies),
            CustomSpawner::Cat(spawner) => spawner.tick(level, spawn_enemies),
            CustomSpawner::VillageSiege(spawner) => spawner.tick(level, spawn_enemies),
            CustomSpawner::WanderingTrader(spawner) => spawner.tick(level, spawn_enemies),
        }
    }
}
