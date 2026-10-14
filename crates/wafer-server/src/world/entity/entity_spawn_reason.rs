#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntitySpawnReason {
    Natural,
    ChunkGeneration,
    Spawner,
    Structure,
    Breeding,
    MobSummoned,
    Jockey,
    Event,
    Conversion,
    Reinforcement,
    Triggered,
    Bucket,
    SpawnItemUse,
    Command,
    Dispenser,
    Patrol,
    TrialSpawner,
    Load,
    DimensionTravel,
}

impl EntitySpawnReason {
    pub fn is_spawner(reason: EntitySpawnReason) -> bool {
        reason == EntitySpawnReason::Spawner || reason == EntitySpawnReason::TrialSpawner
    }

    pub fn ignores_light_requirements(reason: EntitySpawnReason) -> bool {
        reason == EntitySpawnReason::TrialSpawner
    }
}
