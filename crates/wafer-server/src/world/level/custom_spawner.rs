use crate::server::level::ServerLevel;
use crate::world::entity::npc::cat_spawner::CatSpawner;

pub enum CustomSpawner {
    Cat(CatSpawner),
    WanderingTrader(crate::world::entity::npc::wanderingtrader::wandering_trader_spawner::WanderingTraderSpawner),
}

impl CustomSpawner {
    pub fn tick(&mut self, level: &mut ServerLevel, spawn_enemies: bool) {
        match self {
            CustomSpawner::Cat(spawner) => spawner.tick(level, spawn_enemies),
            CustomSpawner::WanderingTrader(spawner) => spawner.tick(level, spawn_enemies),
        }
    }
}
