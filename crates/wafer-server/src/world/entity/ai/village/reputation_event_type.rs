#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReputationEventType {
    ZombieVillagerCured,
    GolemKilled,
    VillagerHurt,
    VillagerKilled,
    Trade,
}
