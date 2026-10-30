use wafer_core::{BlockPos, Vec3};
use wafer_data::{EquipmentSlot, blocks, game_rule, items};
use wafer_protocol::nbt::Nbt;

use super::spawn_data::SpawnData;
use crate::server::level::{ServerLevel, testing};
use crate::world::entity::LevelEntity;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::update_flags::UPDATE_ALL;

const SPAWNER: BlockPos = BlockPos::new(0, -60, 0);

fn snbt(input: &str) -> Nbt {
    crate::nbt::parse_fully(input).unwrap_or_else(|_| panic!("valid snbt: {input}"))
}

fn level_with_spawner(spawner: &str) -> ServerLevel {
    let mut level = testing::level();
    testing::player(&mut level, Vec3::new(0.5, -60.0, 8.5), 0.0, 0.0);
    level.set_block(SPAWNER, blocks::SPAWNER.default_state(), UPDATE_ALL);
    let BlockEntityKind::Spawner(entity) = &mut level.get_block_entity(SPAWNER).expect("the spawner").kind else { panic!("not a spawner") };
    entity.load_additional(&snbt(spawner));
    level
}

fn spawner_tag(level: &mut ServerLevel) -> Nbt {
    let BlockEntityKind::Spawner(spawner) = &level.get_block_entity(SPAWNER).expect("the spawner").kind else { panic!("not a spawner") };
    let mut output = Vec::new();
    spawner.save_additional(&mut output);
    Nbt::Compound(output.into())
}

fn delay(level: &mut ServerLevel) -> Nbt {
    spawner_tag(level).get("Delay").cloned().expect("a delay")
}

fn pigs(level: &ServerLevel) -> Vec<&LevelEntity> {
    level.entities.values().filter(|e| e.entity().entity_type == crate::world::entity::entity_types::PIG.id).collect()
}

const FIXED_PIG: &str = "{SpawnData:{entity:{id:\"minecraft:pig\",NoAI:1b,Pos:[2.5d,-60.0d,0.5d]},custom_spawn_rules:{}},MinSpawnDelay:10,MaxSpawnDelay:10}";

#[test]
fn spawners_wait_for_their_delay_then_spawn() {
    let mut level = level_with_spawner(FIXED_PIG);
    for _ in 0..20 {
        level.tick();
    }
    assert!(pigs(&level).is_empty());
    assert_eq!(delay(&mut level), Nbt::Short(0));
    level.tick();
    let spawned = pigs(&level);
    assert_eq!(spawned.len(), 1);
    let pig = spawned[0].entity();
    assert_eq!(pig.position(), Vec3::new(2.5, -60.0, 0.5));
    assert_eq!(pig.x_rot, 0.0);
    assert_eq!(delay(&mut level), Nbt::Short(10));
}

#[test]
fn spawners_keep_trying_when_nothing_fits() {
    let mut level = level_with_spawner(FIXED_PIG);
    for _ in 0..21 {
        level.tick();
    }
    for _ in 0..11 {
        level.tick();
    }
    assert_eq!(pigs(&level).len(), 1);
    assert_eq!(delay(&mut level), Nbt::Short(0));
}

#[test]
fn spawners_stop_at_max_nearby_entities() {
    let mut level = level_with_spawner(
        "{SpawnData:{entity:{id:\"minecraft:pig\",NoAI:1b},custom_spawn_rules:{}},MinSpawnDelay:1,MaxSpawnDelay:1,SpawnCount:4,MaxNearbyEntities:2,Delay:0s}",
    );
    for _ in 0..200 {
        level.tick();
    }
    assert_eq!(pigs(&level).len(), 2);
}

#[test]
fn spawners_need_a_player_and_the_game_rule() {
    let mut level = level_with_spawner("{SpawnData:{entity:{id:\"minecraft:pig\",NoAI:1b},custom_spawn_rules:{}},RequiredPlayerRange:4}");
    for _ in 0..40 {
        level.tick();
    }
    assert!(pigs(&level).is_empty());
    assert_eq!(delay(&mut level), Nbt::Short(20));
    let mut level = level_with_spawner(FIXED_PIG);
    level.game_rules.set(game_rule::SPAWNER_BLOCKS_WORK, 0);
    for _ in 0..40 {
        level.tick();
    }
    assert!(pigs(&level).is_empty());
    assert_eq!(delay(&mut level), Nbt::Short(20));
}

#[test]
fn spawners_spawn_passengers_and_equipment() {
    let mut level = level_with_spawner(
        "{SpawnData:{entity:{id:\"minecraft:pig\",NoAI:1b,Pos:[2.5d,-60.0d,0.5d],Passengers:[{id:\"minecraft:zombie\",NoAI:1b}]},custom_spawn_rules:{},equipment:{loot_table:\"minecraft:equipment/trial_chamber_melee\",slot_drop_chances:0.0f}}}",
    );
    for _ in 0..21 {
        level.tick();
    }
    let spawned = pigs(&level);
    assert_eq!(spawned.len(), 1);
    let pig = spawned[0].entity();
    assert_eq!(pig.passengers.len(), 1);
    let LevelEntity::Mob(zombie) = &level.entities[&pig.passengers[0]] else { panic!("not a mob") };
    assert_eq!(zombie.entity.vehicle, Some(pig.id));
    let LevelEntity::Mob(pig) = spawned[0] else { panic!("not a mob") };
    let sword = pig.equipment[EquipmentSlot::Mainhand as usize].as_ref().expect("a weapon");
    assert!(sword.item == items::IRON_SWORD || sword.item == items::DIAMOND_SWORD, "{:?}", sword.item);
    assert_eq!(pig.drop_chances[EquipmentSlot::Mainhand as usize], 0.0);
}

#[test]
fn spawn_data_saves_like_vanilla() {
    let data = SpawnData::decode(&snbt(
        "{entity:{id:\"pig\"},custom_spawn_rules:{block_light_limit:[0,7],sky_light_limit:{min_inclusive:5,max_inclusive:5}},equipment:{loot_table:\"a:b\",slot_drop_chances:0.5f}}",
    ))
    .expect("valid spawn data");
    assert_eq!(
        crate::nbt::to_string(&data.encode()),
        "{custom_spawn_rules:{block_light_limit:[0,7],sky_light_limit:5},entity:{id:\"minecraft:pig\"},equipment:{loot_table:\"a:b\",slot_drop_chances:0.5f}}"
    );
    let data = SpawnData::decode(&snbt("{entity:{id:\"Not An Id\"},custom_spawn_rules:{block_light_limit:\"bad\"},equipment:{loot_table:\"a:b\",slot_drop_chances:{head:0.5f}}}")).expect("valid spawn data");
    assert_eq!(crate::nbt::to_string(&data.encode()), "{custom_spawn_rules:{},entity:{},equipment:{loot_table:\"a:b\",slot_drop_chances:{head:0.5f}}}");
    assert!(SpawnData::decode(&snbt("{entity:{},custom_spawn_rules:{sky_light_limit:[0,16]}}")).is_err());
    assert!(SpawnData::decode(&snbt("{entity:1}")).is_err());
}
