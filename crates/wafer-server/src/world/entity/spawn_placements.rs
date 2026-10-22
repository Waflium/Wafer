use std::sync::LazyLock;

use wafer_core::{BlockPos, ChunkPos, Direction};
use wafer_core::random::RandomSource;
use wafer_data::{block_tags, blocks, fluid_tags};

use super::animal::{self, cow::mushroom_cow};
use super::entity_spawn_reason::EntitySpawnReason;
use super::entity_type::EntityType;
use super::entity_types as types;
use super::mob;
use super::monster::monster;
use super::spawn_placement_types::SpawnPlacementType;
use crate::world::attribute::environment_attributes;
use crate::world::difficulty::Difficulty;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::worldgen_random::WorldgenRandom;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::world_gen_level::WorldGenLevel;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpawnPredicate {
    AxolotlCheckAxolotlSpawnRules,
    WaterAnimalCheckSurfaceWaterAnimalSpawnRules,
    AgeableWaterCreatureCheckSurfaceAgeableWaterCreatureSpawnRules,
    DrownedCheckDrownedSpawnRules,
    GuardianCheckGuardianSpawnRules,
    TropicalFishCheckTropicalFishSpawnRules,
    ArmadilloCheckArmadilloSpawnRules,
    BatCheckBatSpawnRules,
    MonsterCheckAnyLightMonsterSpawnRules,
    MonsterCheckMonsterSpawnRules,
    MonsterCheckSurfaceMonstersSpawnRules,
    CamelCheckCamelSpawnRules,
    AnimalCheckAnimalSpawnRules,
    EndermiteCheckEndermiteSpawnRules,
    MobCheckMobSpawnRules,
    FrogCheckFrogSpawnRules,
    GhastCheckGhastSpawnRules,
    GlowSquidCheckGlowSquidSpawnRules,
    GoatCheckGoatSpawnRules,
    MagmaCubeCheckMagmaCubeSpawnRules,
    SulfurCubeCheckSulfurCubeSpawnRules,
    MushroomCowCheckMushroomSpawnRules,
    AbstractNautilusCheckNautilusSpawnRules,
    OcelotCheckOcelotSpawnRules,
    ParrotCheckParrotSpawnRules,
    HoglinCheckHoglinSpawnRules,
    PiglinCheckPiglinSpawnRules,
    PatrollingMonsterCheckPatrollingMonsterSpawnRules,
    PolarBearCheckPolarBearSpawnRules,
    RabbitCheckRabbitSpawnRules,
    SilverfishCheckSilverfishSpawnRules,
    SkeletonHorseCheckSkeletonHorseSpawnRules,
    SlimeCheckSlimeSpawnRules,
    StrayCheckStraySpawnRules,
    StriderCheckStriderSpawnRules,
    TurtleCheckTurtleSpawnRules,
    WolfCheckWolfSpawnRules,
    ZombifiedPiglinCheckZombifiedPiglinSpawnRules,
    FoxCheckFoxSpawnRules,
}

#[derive(Clone, Copy, Debug)]
struct Data {
    height_map: HeightmapType,
    placement: SpawnPlacementType,
    predicate: SpawnPredicate,
}

use HeightmapType::{MotionBlocking, MotionBlockingNoLeaves};
use SpawnPlacementType::{InLava, InWater, NoRestrictions, OnGround};
use SpawnPredicate as P;

const REGISTRATIONS: &[(&EntityType, SpawnPlacementType, HeightmapType, SpawnPredicate)] = &[
    (&types::AXOLOTL, InWater, MotionBlockingNoLeaves, P::AxolotlCheckAxolotlSpawnRules),
    (&types::COD, InWater, MotionBlockingNoLeaves, P::WaterAnimalCheckSurfaceWaterAnimalSpawnRules),
    (&types::DOLPHIN, InWater, MotionBlockingNoLeaves, P::AgeableWaterCreatureCheckSurfaceAgeableWaterCreatureSpawnRules),
    (&types::DROWNED, InWater, MotionBlockingNoLeaves, P::DrownedCheckDrownedSpawnRules),
    (&types::GUARDIAN, InWater, MotionBlockingNoLeaves, P::GuardianCheckGuardianSpawnRules),
    (&types::PUFFERFISH, InWater, MotionBlockingNoLeaves, P::WaterAnimalCheckSurfaceWaterAnimalSpawnRules),
    (&types::SALMON, InWater, MotionBlockingNoLeaves, P::WaterAnimalCheckSurfaceWaterAnimalSpawnRules),
    (&types::SQUID, InWater, MotionBlockingNoLeaves, P::AgeableWaterCreatureCheckSurfaceAgeableWaterCreatureSpawnRules),
    (&types::TROPICAL_FISH, InWater, MotionBlockingNoLeaves, P::TropicalFishCheckTropicalFishSpawnRules),
    (&types::ARMADILLO, OnGround, MotionBlockingNoLeaves, P::ArmadilloCheckArmadilloSpawnRules),
    (&types::BAT, OnGround, MotionBlockingNoLeaves, P::BatCheckBatSpawnRules),
    (&types::BLAZE, OnGround, MotionBlockingNoLeaves, P::MonsterCheckAnyLightMonsterSpawnRules),
    (&types::BOGGED, OnGround, MotionBlockingNoLeaves, P::MonsterCheckMonsterSpawnRules),
    (&types::BREEZE, OnGround, MotionBlockingNoLeaves, P::MonsterCheckAnyLightMonsterSpawnRules),
    (&types::CAMEL, OnGround, MotionBlockingNoLeaves, P::CamelCheckCamelSpawnRules),
    (&types::CAMEL_HUSK, OnGround, MotionBlockingNoLeaves, P::MonsterCheckSurfaceMonstersSpawnRules),
    (&types::CAVE_SPIDER, OnGround, MotionBlockingNoLeaves, P::MonsterCheckMonsterSpawnRules),
    (&types::CHICKEN, OnGround, MotionBlockingNoLeaves, P::AnimalCheckAnimalSpawnRules),
    (&types::COW, OnGround, MotionBlockingNoLeaves, P::AnimalCheckAnimalSpawnRules),
    (&types::CREEPER, OnGround, MotionBlockingNoLeaves, P::MonsterCheckMonsterSpawnRules),
    (&types::DONKEY, OnGround, MotionBlockingNoLeaves, P::AnimalCheckAnimalSpawnRules),
    (&types::ENDERMAN, OnGround, MotionBlockingNoLeaves, P::MonsterCheckMonsterSpawnRules),
    (&types::ENDERMITE, OnGround, MotionBlockingNoLeaves, P::EndermiteCheckEndermiteSpawnRules),
    (&types::ENDER_DRAGON, OnGround, MotionBlockingNoLeaves, P::MobCheckMobSpawnRules),
    (&types::FROG, OnGround, MotionBlockingNoLeaves, P::FrogCheckFrogSpawnRules),
    (&types::GHAST, OnGround, MotionBlockingNoLeaves, P::GhastCheckGhastSpawnRules),
    (&types::HAPPY_GHAST, OnGround, MotionBlockingNoLeaves, P::AnimalCheckAnimalSpawnRules),
    (&types::GIANT, OnGround, MotionBlockingNoLeaves, P::MonsterCheckMonsterSpawnRules),
    (&types::GLOW_SQUID, InWater, MotionBlockingNoLeaves, P::GlowSquidCheckGlowSquidSpawnRules),
    (&types::GOAT, OnGround, MotionBlockingNoLeaves, P::GoatCheckGoatSpawnRules),
    (&types::HORSE, OnGround, MotionBlockingNoLeaves, P::AnimalCheckAnimalSpawnRules),
    (&types::HUSK, OnGround, MotionBlockingNoLeaves, P::MonsterCheckSurfaceMonstersSpawnRules),
    (&types::IRON_GOLEM, OnGround, MotionBlockingNoLeaves, P::MobCheckMobSpawnRules),
    (&types::LLAMA, OnGround, MotionBlockingNoLeaves, P::AnimalCheckAnimalSpawnRules),
    (&types::MAGMA_CUBE, OnGround, MotionBlockingNoLeaves, P::MagmaCubeCheckMagmaCubeSpawnRules),
    (&types::SULFUR_CUBE, OnGround, MotionBlockingNoLeaves, P::SulfurCubeCheckSulfurCubeSpawnRules),
    (&types::MOOSHROOM, OnGround, MotionBlockingNoLeaves, P::MushroomCowCheckMushroomSpawnRules),
    (&types::MULE, OnGround, MotionBlockingNoLeaves, P::AnimalCheckAnimalSpawnRules),
    (&types::NAUTILUS, InWater, MotionBlockingNoLeaves, P::AbstractNautilusCheckNautilusSpawnRules),
    (&types::OCELOT, OnGround, MotionBlocking, P::OcelotCheckOcelotSpawnRules),
    (&types::PARROT, OnGround, MotionBlocking, P::ParrotCheckParrotSpawnRules),
    (&types::PIG, OnGround, MotionBlockingNoLeaves, P::AnimalCheckAnimalSpawnRules),
    (&types::HOGLIN, OnGround, MotionBlockingNoLeaves, P::HoglinCheckHoglinSpawnRules),
    (&types::PIGLIN, OnGround, MotionBlockingNoLeaves, P::PiglinCheckPiglinSpawnRules),
    (&types::PILLAGER, OnGround, MotionBlockingNoLeaves, P::PatrollingMonsterCheckPatrollingMonsterSpawnRules),
    (&types::POLAR_BEAR, OnGround, MotionBlockingNoLeaves, P::PolarBearCheckPolarBearSpawnRules),
    (&types::RABBIT, OnGround, MotionBlockingNoLeaves, P::RabbitCheckRabbitSpawnRules),
    (&types::SHEEP, OnGround, MotionBlockingNoLeaves, P::AnimalCheckAnimalSpawnRules),
    (&types::SILVERFISH, OnGround, MotionBlockingNoLeaves, P::SilverfishCheckSilverfishSpawnRules),
    (&types::SKELETON, OnGround, MotionBlockingNoLeaves, P::MonsterCheckMonsterSpawnRules),
    (&types::SKELETON_HORSE, OnGround, MotionBlockingNoLeaves, P::SkeletonHorseCheckSkeletonHorseSpawnRules),
    (&types::SLIME, OnGround, MotionBlockingNoLeaves, P::SlimeCheckSlimeSpawnRules),
    (&types::SNOW_GOLEM, OnGround, MotionBlockingNoLeaves, P::MobCheckMobSpawnRules),
    (&types::SPIDER, OnGround, MotionBlockingNoLeaves, P::MonsterCheckMonsterSpawnRules),
    (&types::STRAY, OnGround, MotionBlockingNoLeaves, P::StrayCheckStraySpawnRules),
    (&types::PARCHED, OnGround, MotionBlockingNoLeaves, P::MonsterCheckSurfaceMonstersSpawnRules),
    (&types::STRIDER, InLava, MotionBlockingNoLeaves, P::StriderCheckStriderSpawnRules),
    (&types::TURTLE, OnGround, MotionBlockingNoLeaves, P::TurtleCheckTurtleSpawnRules),
    (&types::VILLAGER, OnGround, MotionBlockingNoLeaves, P::MobCheckMobSpawnRules),
    (&types::WITCH, OnGround, MotionBlockingNoLeaves, P::MonsterCheckMonsterSpawnRules),
    (&types::WITHER, OnGround, MotionBlockingNoLeaves, P::MonsterCheckMonsterSpawnRules),
    (&types::WITHER_SKELETON, OnGround, MotionBlockingNoLeaves, P::MonsterCheckMonsterSpawnRules),
    (&types::WOLF, OnGround, MotionBlockingNoLeaves, P::WolfCheckWolfSpawnRules),
    (&types::ZOGLIN, OnGround, MotionBlockingNoLeaves, P::MonsterCheckAnyLightMonsterSpawnRules),
    (&types::CREAKING, OnGround, MotionBlockingNoLeaves, P::MonsterCheckMonsterSpawnRules),
    (&types::ZOMBIE, OnGround, MotionBlockingNoLeaves, P::MonsterCheckMonsterSpawnRules),
    (&types::ZOMBIE_HORSE, OnGround, MotionBlockingNoLeaves, P::MonsterCheckMonsterSpawnRules),
    (&types::ZOMBIFIED_PIGLIN, OnGround, MotionBlockingNoLeaves, P::ZombifiedPiglinCheckZombifiedPiglinSpawnRules),
    (&types::ZOMBIE_VILLAGER, OnGround, MotionBlockingNoLeaves, P::MonsterCheckMonsterSpawnRules),
    (&types::CAT, OnGround, MotionBlockingNoLeaves, P::AnimalCheckAnimalSpawnRules),
    (&types::ELDER_GUARDIAN, InWater, MotionBlockingNoLeaves, P::GuardianCheckGuardianSpawnRules),
    (&types::EVOKER, NoRestrictions, MotionBlockingNoLeaves, P::MonsterCheckMonsterSpawnRules),
    (&types::FOX, NoRestrictions, MotionBlockingNoLeaves, P::FoxCheckFoxSpawnRules),
    (&types::ILLUSIONER, NoRestrictions, MotionBlockingNoLeaves, P::MonsterCheckMonsterSpawnRules),
    (&types::PANDA, NoRestrictions, MotionBlockingNoLeaves, P::AnimalCheckAnimalSpawnRules),
    (&types::PHANTOM, NoRestrictions, MotionBlockingNoLeaves, P::MobCheckMobSpawnRules),
    (&types::RAVAGER, OnGround, MotionBlockingNoLeaves, P::MonsterCheckMonsterSpawnRules),
    (&types::SHULKER, NoRestrictions, MotionBlockingNoLeaves, P::MobCheckMobSpawnRules),
    (&types::TRADER_LLAMA, NoRestrictions, MotionBlockingNoLeaves, P::AnimalCheckAnimalSpawnRules),
    (&types::VEX, NoRestrictions, MotionBlockingNoLeaves, P::MonsterCheckMonsterSpawnRules),
    (&types::VINDICATOR, NoRestrictions, MotionBlockingNoLeaves, P::MonsterCheckMonsterSpawnRules),
    (&types::WANDERING_TRADER, OnGround, MotionBlockingNoLeaves, P::MobCheckMobSpawnRules),
    (&types::WARDEN, NoRestrictions, MotionBlockingNoLeaves, P::MonsterCheckMonsterSpawnRules),
];

static DATA_BY_TYPE: LazyLock<Vec<Option<Data>>> = LazyLock::new(|| {
    let mut data = vec![None; types::BY_ID.len()];
    for &(entity_type, placement, height_map, predicate) in REGISTRATIONS {
        let previous = data[entity_type.id as usize].replace(Data { height_map, placement, predicate });
        assert!(previous.is_none(), "Duplicate registration for type {}", entity_type.get_key());
    }
    data
});

fn data(entity_type: &EntityType) -> Option<Data> {
    DATA_BY_TYPE[entity_type.id as usize]
}

pub fn get_placement_type(entity_type: &EntityType) -> SpawnPlacementType {
    data(entity_type).map_or(SpawnPlacementType::NoRestrictions, |data| data.placement)
}

pub fn is_spawn_position_ok(entity_type: &EntityType, level: &mut (impl ServerLevelAccessor + ?Sized), block_pos: BlockPos) -> bool {
    get_placement_type(entity_type).is_spawn_position_ok(level, block_pos, Some(entity_type))
}

pub fn get_heightmap_type(entity_type: Option<&EntityType>) -> HeightmapType {
    entity_type.and_then(data).map_or(HeightmapType::MotionBlockingNoLeaves, |data| data.height_map)
}

pub fn check_spawn_rules(entity_type: &EntityType, level: &mut (impl WorldGenLevel + ?Sized), spawn_reason: EntitySpawnReason, pos: BlockPos) -> bool {
    if !entity_type.is_allowed_in_peaceful() && level.get_difficulty() == Difficulty::Peaceful {
        return false;
    }
    match data(entity_type) {
        None => true,
        Some(data) => test(data.predicate, entity_type, level, spawn_reason, pos),
    }
}

fn test(predicate: SpawnPredicate, entity_type: &EntityType, level: &mut (impl WorldGenLevel + ?Sized), spawn_reason: EntitySpawnReason, pos: BlockPos) -> bool {
    match predicate {
        P::AxolotlCheckAxolotlSpawnRules => level.block_state(pos.below()).is(&block_tags::AXOLOTLS_SPAWNABLE_ON),
        P::WaterAnimalCheckSurfaceWaterAnimalSpawnRules | P::AgeableWaterCreatureCheckSurfaceAgeableWaterCreatureSpawnRules => {
            water_animal_check_surface_water_animal_spawn_rules(level, pos)
        }
        P::DrownedCheckDrownedSpawnRules => drowned_check_drowned_spawn_rules(level, spawn_reason, pos),
        P::GuardianCheckGuardianSpawnRules => {
            (level.get_random().next_int_bounded(20) == 0 || !level.can_see_sky_from_below_water(pos))
                && level.get_difficulty() != Difficulty::Peaceful
                && (EntitySpawnReason::is_spawner(spawn_reason) || level.fluid_state(pos).is(&fluid_tags::WATER))
                && level.fluid_state(pos.below()).is(&fluid_tags::WATER)
        }
        P::TropicalFishCheckTropicalFishSpawnRules => super::animal::fish::tropical_fish::check_tropical_fish_spawn_rules(level, pos),
        P::ArmadilloCheckArmadilloSpawnRules => spawnable_on_and_bright(level, pos, &block_tags::ARMADILLO_SPAWNABLE_ON),
        P::BatCheckBatSpawnRules => bat_check_bat_spawn_rules(entity_type, level, spawn_reason, pos),
        P::MonsterCheckAnyLightMonsterSpawnRules => monster::check_any_light_monster_spawn_rules(entity_type, level, spawn_reason, pos),
        P::MonsterCheckMonsterSpawnRules => monster::check_monster_spawn_rules(entity_type, level, spawn_reason, pos),
        P::MonsterCheckSurfaceMonstersSpawnRules => monster::check_surface_monsters_spawn_rules(entity_type, level, spawn_reason, pos),
        P::CamelCheckCamelSpawnRules => spawnable_on_and_bright(level, pos, &block_tags::CAMELS_SPAWNABLE_ON),
        P::AnimalCheckAnimalSpawnRules => animal::check_animal_spawn_rules(entity_type, level, spawn_reason, pos),
        P::EndermiteCheckEndermiteSpawnRules | P::SilverfishCheckSilverfishSpawnRules => {
            if !monster::check_any_light_monster_spawn_rules(entity_type, level, spawn_reason, pos) {
                return false;
            }
            if EntitySpawnReason::is_spawner(spawn_reason) {
                return true;
            }
            level.get_nearest_player(pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5, 5.0, true).is_none()
        }
        P::MobCheckMobSpawnRules => mob::check_mob_spawn_rules(entity_type, level, spawn_reason, pos),
        P::FrogCheckFrogSpawnRules => spawnable_on_and_bright(level, pos, &block_tags::FROGS_SPAWNABLE_ON),
        P::GhastCheckGhastSpawnRules => {
            level.get_difficulty() != Difficulty::Peaceful && level.get_random().next_int_bounded(20) == 0 && mob::check_mob_spawn_rules(entity_type, level, spawn_reason, pos)
        }
        P::GlowSquidCheckGlowSquidSpawnRules => {
            pos.y <= level.get_sea_level() - 33 && level.raw_brightness(pos, 0) == 0 && level.block_state(pos).is_block(blocks::WATER)
        }
        P::GoatCheckGoatSpawnRules => spawnable_on_and_bright(level, pos, &block_tags::GOATS_SPAWNABLE_ON),
        P::MagmaCubeCheckMagmaCubeSpawnRules => level.get_difficulty() != Difficulty::Peaceful,
        P::SulfurCubeCheckSulfurCubeSpawnRules => true,
        P::MushroomCowCheckMushroomSpawnRules => mushroom_cow::check_mushroom_spawn_rules(entity_type, level, spawn_reason, pos),
        P::AbstractNautilusCheckNautilusSpawnRules => {
            let sea_level = level.get_sea_level();
            let min_spawn_level = sea_level - 25;
            pos.y >= min_spawn_level
                && pos.y <= sea_level - 5
                && level.fluid_state(pos.below()).is(&fluid_tags::WATER)
                && level.block_state(pos.above()).is_block(blocks::WATER)
        }
        P::OcelotCheckOcelotSpawnRules => level.get_random().next_int_bounded(3) != 0,
        P::ParrotCheckParrotSpawnRules => spawnable_on_and_bright(level, pos, &block_tags::PARROTS_SPAWNABLE_ON),
        P::HoglinCheckHoglinSpawnRules | P::PiglinCheckPiglinSpawnRules => !level.block_state(pos.below()).is_block(blocks::NETHER_WART_BLOCK),
        P::PatrollingMonsterCheckPatrollingMonsterSpawnRules => {
            super::monster::patrolling_monster::check_patrolling_monster_spawn_rules(entity_type, level, spawn_reason, pos)
        }
        P::PolarBearCheckPolarBearSpawnRules => {
            let biome = level.get_biome(pos);
            if !biome.is("minecraft:polar_bears_spawn_on_alternate_blocks") {
                animal::check_animal_spawn_rules(entity_type, level, spawn_reason, pos)
            } else {
                animal::is_bright_enough_to_spawn(level, pos) && level.block_state(pos.below()).is(&block_tags::POLAR_BEARS_SPAWNABLE_ON_ALTERNATE)
            }
        }
        P::RabbitCheckRabbitSpawnRules => super::animal::rabbit::rabbit::check_rabbit_spawn_rules(level, pos),
        P::SkeletonHorseCheckSkeletonHorseSpawnRules => {
            if !EntitySpawnReason::is_spawner(spawn_reason) {
                animal::check_animal_spawn_rules(entity_type, level, spawn_reason, pos)
            } else {
                EntitySpawnReason::ignores_light_requirements(spawn_reason) || animal::is_bright_enough_to_spawn(level, pos)
            }
        }
        P::SlimeCheckSlimeSpawnRules => slime_check_slime_spawn_rules(entity_type, level, spawn_reason, pos),
        P::StrayCheckStraySpawnRules => {
            let mut check_sky_pos = pos;
            loop {
                check_sky_pos = check_sky_pos.above();
                if !level.block_state(check_sky_pos).is_block(blocks::POWDER_SNOW) {
                    break;
                }
            }
            monster::check_monster_spawn_rules(entity_type, level, spawn_reason, pos) && (EntitySpawnReason::is_spawner(spawn_reason) || level.can_see_sky(check_sky_pos.below()))
        }
        P::StriderCheckStriderSpawnRules => {
            let mut check_pos = pos;
            loop {
                check_pos = check_pos.relative(Direction::Up);
                if !level.fluid_state(check_pos).is(&fluid_tags::LAVA) {
                    break;
                }
            }
            level.block_state(check_pos).is_air()
        }
        P::TurtleCheckTurtleSpawnRules => super::animal::turtle::turtle::check_turtle_spawn_rules(level, pos),
        P::WolfCheckWolfSpawnRules => spawnable_on_and_bright(level, pos, &block_tags::WOLVES_SPAWNABLE_ON),
        P::ZombifiedPiglinCheckZombifiedPiglinSpawnRules => {
            level.get_difficulty() != Difficulty::Peaceful && !level.block_state(pos.below()).is_block(blocks::NETHER_WART_BLOCK)
        }
        P::FoxCheckFoxSpawnRules => spawnable_on_and_bright(level, pos, &block_tags::FOXES_SPAWNABLE_ON),
    }
}

fn spawnable_on_and_bright(level: &mut (impl ServerLevelAccessor + ?Sized), pos: BlockPos, spawnable_on: &wafer_data::Tag) -> bool {
    level.block_state(pos.below()).is(spawnable_on) && animal::is_bright_enough_to_spawn(level, pos)
}

fn water_animal_check_surface_water_animal_spawn_rules(level: &mut (impl ServerLevelAccessor + ?Sized), pos: BlockPos) -> bool {
    let sea_level = level.get_sea_level();
    let min_spawn_level = sea_level - 13;
    pos.y >= min_spawn_level && pos.y <= sea_level && level.fluid_state(pos.below()).is(&fluid_tags::WATER) && level.block_state(pos.above()).is_block(blocks::WATER)
}

fn drowned_check_drowned_spawn_rules(level: &mut (impl ServerLevelAccessor + ?Sized), spawn_reason: EntitySpawnReason, pos: BlockPos) -> bool {
    if !level.fluid_state(pos.below()).is(&fluid_tags::WATER) && !EntitySpawnReason::is_spawner(spawn_reason) {
        return false;
    }
    let biome = level.get_biome(pos);
    let can_monster_spawn = level.get_difficulty() != Difficulty::Peaceful
        && (EntitySpawnReason::ignores_light_requirements(spawn_reason) || monster::is_dark_enough_to_spawn(level, pos))
        && (EntitySpawnReason::is_spawner(spawn_reason) || level.fluid_state(pos).is(&fluid_tags::WATER));
    if !can_monster_spawn || !EntitySpawnReason::is_spawner(spawn_reason) && spawn_reason != EntitySpawnReason::Reinforcement {
        if biome.is("minecraft:more_frequent_drowned_spawns") {
            level.get_random().next_int_bounded(15) == 0 && can_monster_spawn
        } else {
            level.get_random().next_int_bounded(40) == 0 && pos.y < level.get_sea_level() - 5 && can_monster_spawn
        }
    } else {
        true
    }
}

fn bat_check_bat_spawn_rules(entity_type: &EntityType, level: &mut (impl ServerLevelAccessor + ?Sized), spawn_reason: EntitySpawnReason, pos: BlockPos) -> bool {
    if pos.y >= level.get_heightmap_pos(HeightmapType::WorldSurface, pos).y {
        return false;
    }
    if level.get_random().next_boolean() {
        return false;
    }
    let brightness = level.get_max_local_raw_brightness(pos);
    if brightness > level.get_random().next_int_bounded(4) {
        return false;
    }
    if !level.block_state(pos.below()).is(&block_tags::BATS_SPAWNABLE_ON) { false } else { mob::check_mob_spawn_rules(entity_type, level, spawn_reason, pos) }
}

fn slime_check_slime_spawn_rules(entity_type: &EntityType, level: &mut (impl WorldGenLevel + ?Sized), spawn_reason: EntitySpawnReason, pos: BlockPos) -> bool {
    if level.get_difficulty() != Difficulty::Peaceful {
        if EntitySpawnReason::is_spawner(spawn_reason) {
            return mob::check_mob_spawn_rules(entity_type, level, spawn_reason, pos);
        }
        if level.get_biome(pos).is("minecraft:allows_surface_slime_spawns") && pos.y > 50 && pos.y < 70 {
            let surface_slime_spawn_chance = level.get_environment_attribute(environment_attributes::SURFACE_SLIME_SPAWN_CHANCE, pos);
            if level.get_random().next_float() < surface_slime_spawn_chance {
                let brightness = level.get_max_local_raw_brightness(pos);
                if brightness <= level.get_random().next_int_bounded(8) {
                    return mob::check_mob_spawn_rules(entity_type, level, spawn_reason, pos);
                }
            }
        }
        let chunk_pos = ChunkPos::new(pos.x >> 4, pos.z >> 4);
        let slime_chunk = WorldgenRandom::seed_slime_chunk(chunk_pos.x, chunk_pos.z, level.get_seed(), 987234911).next_int_bounded(10) == 0;
        if level.get_random().next_int_bounded(10) == 0 && slime_chunk && pos.y < 40 {
            return mob::check_mob_spawn_rules(entity_type, level, spawn_reason, pos);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{SpawnPlacementType, get_heightmap_type, get_placement_type};
    use crate::world::entity::entity_type::EntityType;
    use crate::world::entity::entity_types;
    use crate::world::level::chunk::HeightmapType;

    #[test]
    fn placements_and_heightmaps_match_vanilla() {
        let lines: Vec<&str> = include_str!("spawn_placements_oracle.txt").lines().collect();
        assert_eq!(lines.len(), entity_types::BY_ID.len());
        for (id, line) in lines.iter().enumerate() {
            let mut parts = line.split(' ');
            let (name, placement, heightmap) = (parts.next().unwrap(), parts.next().unwrap(), parts.next().unwrap());
            let entity_type = EntityType::by_id(id as i32);
            assert_eq!(entity_type.id, id as i32);
            assert_eq!(entity_type.get_key(), name);
            let expected_placement = match placement {
                "OnGround" => SpawnPlacementType::OnGround,
                "InWater" => SpawnPlacementType::InWater,
                "InLava" => SpawnPlacementType::InLava,
                _ => SpawnPlacementType::NoRestrictions,
            };
            let expected_heightmap = if heightmap == "MOTION_BLOCKING" { HeightmapType::MotionBlocking } else { HeightmapType::MotionBlockingNoLeaves };
            assert_eq!(get_placement_type(entity_type), expected_placement, "{name}");
            assert_eq!(get_heightmap_type(Some(entity_type)), expected_heightmap, "{name}");
        }
    }
}
