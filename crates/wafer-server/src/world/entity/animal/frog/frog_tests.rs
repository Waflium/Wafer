use wafer_core::{BlockPos, Vec3};
use wafer_data::{blocks, items};
use wafer_protocol::nbt::Nbt;

use super::frog;
use crate::server::level::ServerLevel;
use crate::world::entity::animal::axolotl::axolotl_tests::{assert_rows, duel_level, oracle, pool, row, save, seed, seed_level, sorted, summon, summon_with_parity};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::variant::variant_utils;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::component::custom::parse_snbt;
use crate::world::level::BlockGetter;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::{InteractionHand, InteractionResult};

const ORACLE: &str = include_str!("frog_oracle.txt");

fn dropped_items(level: &ServerLevel) -> Vec<String> {
    level
        .entities
        .values()
        .filter_map(|e| match e {
            LevelEntity::Item(item) if !item.entity.is_removed() => Some(format!("item {} {}", item.item.count, wafer_data::registry::ITEM[item.item.get_item().0 as usize])),
            _ => None,
        })
        .collect()
}

fn expected_items(label: &str) -> Vec<String> {
    ORACLE
        .lines()
        .skip_while(|l| *l != format!("== {label}"))
        .skip(1)
        .take_while(|l| !l.starts_with("=="))
        .filter_map(|l| l.strip_prefix("item "))
        .map(|l| format!("item {}", l.trim_end_matches(" {}")))
        .collect()
}

#[test]
fn frogs_long_jump_like_vanilla() {
    let (mut level, _player) = duel_level();
    pool(&mut level);
    let frog = summon(&mut level, "minecraft:frog", Vec3::new(10.5, -60.0, 10.5), r#"Tags:["f"],Rotation:[0f,0f],PersistenceRequired:1b,Brain:{memories:{"minecraft:long_jump_cooling_down":{value:5}}}"#);
    seed(&mut level, frog, 42, 1);
    seed_level(&mut level, 7);
    assert_rows(ORACLE, "jump", &[frog], &mut level, 0, usize::MAX);
    assert_eq!(save(&level, frog).get("Brain"), Some(&parse_snbt(r#"{memories:{"minecraft:long_jump_cooling_down":{value:45}}}"#).unwrap()));
}

#[test]
fn warm_frogs_eat_magma_cubes_into_pearlescent_froglights() {
    let (mut level, _player) = duel_level();
    pool(&mut level);
    let magma_cube = summon(&mut level, "minecraft:magma_cube", Vec3::new(23.5, -60.0, 0.5), r#"Tags:["m"],Size:0,NoAI:1b,PersistenceRequired:1b"#);
    let frog = summon(
        &mut level,
        "minecraft:frog",
        Vec3::new(20.5, -60.0, 0.5),
        r#"Tags:["f2"],Rotation:[0f,0f],PersistenceRequired:1b,variant:"minecraft:warm",Brain:{memories:{"minecraft:long_jump_cooling_down":{value:6000}}}"#,
    );
    seed(&mut level, frog, 42, 1);
    seed_level(&mut level, 7);
    assert_rows(ORACLE, "tongue", &[frog, magma_cube], &mut level, 0, usize::MAX);
    assert_eq!(dropped_items(&level), expected_items("tongue"));
    assert_eq!(save(&level, frog).get("Brain"), Some(&parse_snbt(r#"{memories:{"minecraft:long_jump_cooling_down":{value:5890}}}"#).unwrap()));
}

#[test]
fn cold_frogs_eat_small_slimes_like_vanilla() {
    let (mut level, _player) = duel_level();
    pool(&mut level);
    let slime = summon(&mut level, "minecraft:slime", Vec3::new(23.5, -60.0, 0.5), r#"Tags:["s"],Size:0,NoAI:1b,PersistenceRequired:1b"#);
    let frog = summon(
        &mut level,
        "minecraft:frog",
        Vec3::new(20.5, -60.0, 0.5),
        r#"Tags:["f3"],Rotation:[0f,0f],PersistenceRequired:1b,variant:"minecraft:cold",Brain:{memories:{"minecraft:long_jump_cooling_down":{value:6000}}}"#,
    );
    seed(&mut level, frog, 43, 1);
    seed_level(&mut level, 8);
    assert_rows(ORACLE, "slime", &[frog, slime], &mut level, 0, usize::MAX);
    assert_eq!(dropped_items(&level), expected_items("slime"));
}

#[test]
fn frogs_breed_and_lay_frogspawn_like_vanilla() {
    let (mut level, _player) = duel_level();
    pool(&mut level);
    for x in 8..=15 {
        for z in 17..=19 {
            level.set_block(BlockPos::new(x, -61, z), blocks::WATER.default_state(), UPDATE_ALL);
        }
    }
    let cooled_down = r#"Rotation:[0f,0f],PersistenceRequired:1b,InLove:600,Brain:{memories:{"minecraft:long_jump_cooling_down":{value:6000}}}"#;
    let a = summon(&mut level, "minecraft:frog", Vec3::new(10.5, -60.0, 14.5), &format!(r#"Tags:["a"],{cooled_down}"#));
    let b = summon(&mut level, "minecraft:frog", Vec3::new(13.5, -60.0, 14.5), &format!(r#"Tags:["b"],{cooled_down}"#));
    seed(&mut level, a, 42, 1);
    seed(&mut level, b, 43, 1);
    seed_level(&mut level, 7);
    assert_rows(ORACLE, "breed", &[a, b], &mut level, 0, usize::MAX);
    let expected: Vec<BlockPos> = ORACLE
        .lines()
        .filter_map(|l| l.strip_prefix("frogspawn "))
        .map(|l| {
            let v: Vec<i32> = l.split(' ').map(|n| n.parse().unwrap()).collect();
            BlockPos::new(v[0], v[1], v[2])
        })
        .collect();
    let mut found = Vec::new();
    for x in -10..=30 {
        for y in -62..=-58 {
            for z in -10..=30 {
                let pos = BlockPos::new(x, y, z);
                if level.block_state(pos).is_block(blocks::FROGSPAWN) {
                    found.push(pos);
                }
            }
        }
    }
    assert_eq!(found, expected);
    for frog in [a, b] {
        assert_eq!(save(&level, frog).get("Brain"), Some(&parse_snbt(r#"{memories:{"minecraft:long_jump_cooling_down":{value:5600}}}"#).unwrap()));
    }
}

#[test]
fn amphibians_save_and_load_their_fields() {
    let (mut level, _player) = duel_level();
    let cases = [
        ("minecraft:axolotl", r#"Variant:4,FromBucket:1b,Age:-50,PlayingDead:1b,Brain:{memories:{"minecraft:play_dead_ticks":{value:7},"minecraft:has_hunting_cooldown":{value:1b,ttl:99L}}}"#),
        ("minecraft:frog", r#"variant:"minecraft:cold",Brain:{memories:{"minecraft:long_jump_cooling_down":{value:33},"minecraft:is_pregnant":{value:{}}}}"#),
        ("minecraft:tadpole", "Age:777,AgeLocked:1b"),
    ];
    for (entity_type, nbt) in cases {
        let id = summon(&mut level, entity_type, Vec3::new(30.5, -60.0, 30.5), &format!("{nbt},NoAI:1b"));
        let saved = save(&level, id);
        let Nbt::Compound(mut fields) = saved.clone() else { unreachable!() };
        fields.push(("id".into(), Nbt::String(entity_type.into())));
        let loaded = level.load_entity(&Nbt::Compound(fields)).unwrap();
        let mut out = Vec::new();
        loaded.save_without_id(&mut out);
        assert_eq!(sorted(Nbt::Compound(out.into())), sorted(saved.clone()), "{entity_type}");
        match entity_type {
            "minecraft:axolotl" => {
                assert_eq!(saved.get("Variant"), Some(&Nbt::Int(4)));
                assert_eq!(saved.get("FromBucket"), Some(&Nbt::Byte(1)));
                assert_eq!(saved.get("Air"), Some(&Nbt::Short(6000)));
                assert_eq!(saved.get("PlayingDead"), None);
            }
            "minecraft:frog" => {
                assert_eq!(saved.get("variant"), Some(&Nbt::String("minecraft:cold".into())));
                assert_eq!(saved.get("Age"), None);
                assert_eq!(sorted(saved.get("Brain").unwrap().clone()), sorted(parse_snbt(r#"{memories:{"minecraft:long_jump_cooling_down":{value:33},"minecraft:is_pregnant":{value:{}}}}"#).unwrap()));
            }
            _ => {
                assert_eq!(saved.get("Age"), Some(&Nbt::Int(777)));
                assert_eq!(saved.get("AgeLocked"), Some(&Nbt::Byte(1)));
                assert_eq!(saved.get("FromBucket"), Some(&Nbt::Byte(1)));
            }
        }
    }
}

#[test]
fn frogs_only_eat_the_smallest_cubes() {
    let (mut level, _player) = duel_level();
    let small = summon(&mut level, "minecraft:slime", Vec3::new(23.5, -60.0, 0.5), "Size:0,NoAI:1b");
    let big = summon(&mut level, "minecraft:magma_cube", Vec3::new(26.5, -60.0, 0.5), "Size:1,NoAI:1b");
    let cow = summon(&mut level, "minecraft:cow", Vec3::new(29.5, -60.0, 0.5), "NoAI:1b");
    assert!(frog::can_eat(&level, small));
    assert!(!frog::can_eat(&level, big));
    assert!(!frog::can_eat(&level, cow));
}

#[test]
fn tadpoles_grow_into_frogs_like_vanilla() {
    let (mut level, _player) = duel_level();
    pool(&mut level);
    let parity = ORACLE.lines().find_map(|l| l.strip_prefix("parity t ")).map(|p| p.parse().unwrap());
    let tadpole = summon_with_parity(&mut level, "minecraft:tadpole", Vec3::new(0.5, -58.0, 0.5), r#"Tags:["t"],Rotation:[0f,0f],Age:23985"#, parity);
    seed(&mut level, tadpole, 42, 1);
    seed_level(&mut level, 7);
    let mut tracked = tadpole;
    let mut converted = false;
    for (tick, line) in oracle(ORACLE, "tadpole").iter().enumerate() {
        level.tick();
        if converted {
            continue;
        }
        if level.entity_ref(tracked).is_none_or(|e| e.is_removed()) {
            tracked = frogs(&level)[0];
            converted = true;
        }
        let (mut actual, mut expected) = (normalize(&row(&level, tracked)), normalize(line));
        if converted {
            actual.remove(5);
            expected.remove(5);
        }
        assert_eq!(actual, expected, "tadpole tick {}", tick + 1);
    }
    assert!(converted);
    let nbt = save(&level, tracked);
    assert_eq!(nbt.get("variant"), Some(&Nbt::String("minecraft:temperate".into())));
    assert_eq!(nbt.get("PersistenceRequired"), Some(&Nbt::Byte(1)));
    assert_eq!(level.get_brain(tracked).get_memory(crate::world::entity::ai::memory::memory_module_type::LONG_JUMP_COOLDOWN_TICKS), Some(&107));
    let attributes = parse_snbt(r#"[{base:16.0d,id:"minecraft:follow_range",modifiers:[{amount:-0.06634658987245282d,id:"minecraft:random_spawn_bonus",operation:"add_multiplied_base"}]},{base:1.0d,id:"minecraft:movement_speed"}]"#).unwrap();
    assert_eq!(sorted_list(nbt.get("attributes").unwrap().clone()), sorted_list(attributes));
}

fn sorted_list(nbt: Nbt) -> Vec<String> {
    let mut entries: Vec<String> = nbt.as_list().iter().map(|e| format!("{:?}", sorted(e.clone()))).collect();
    entries.sort();
    entries
}

fn normalize(row: &str) -> Vec<String> {
    let fields: Vec<&str> = row.split(' ').collect();
    let last = fields.len() - 1;
    fields
        .iter()
        .enumerate()
        .map(|(i, f)| match f.parse::<f64>() {
            Ok(v) if (4..=6).contains(&i) || i == last => format!("{:?}", v as f32),
            Ok(v) => format!("{v:?}"),
            _ => f.to_lowercase().replace('_', ""),
        })
        .collect()
}

fn frogs(level: &ServerLevel) -> Vec<EntityId> {
    level.entities.iter().filter(|(_, e)| wafer_data::registry::ENTITY_TYPE[e.entity().entity_type as usize] == "minecraft:frog" && !e.entity().is_removed()).map(|(&id, _)| id).collect()
}

#[test]
fn feeding_tadpoles_slime_balls_speeds_up_growth() {
    let (mut level, player) = duel_level();
    pool(&mut level);
    let tadpole = summon(&mut level, "minecraft:tadpole", Vec3::new(0.5, -58.0, 0.5), "Age:0,NoAI:1b");
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::SLIME_BALL, 2)));
    let location = level.entity(tadpole).position();
    assert_eq!(level.interact_on(player, tadpole, InteractionHand::MainHand, location), InteractionResult::SUCCESS);
    assert_eq!(save(&level, tadpole).get("Age"), Some(&Nbt::Int(2400)));
    assert_eq!(level.player(player).item_in_hand(InteractionHand::MainHand).map(|s| s.count), Some(1));
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::GOLDEN_DANDELION, 1)));
    assert_eq!(level.interact_on(player, tadpole, InteractionHand::MainHand, location), InteractionResult::SUCCESS);
    let nbt = save(&level, tadpole);
    assert_eq!(nbt.get("AgeLocked"), Some(&Nbt::Byte(1)));
    assert_eq!(nbt.get("Age"), Some(&Nbt::Int(0)));
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::SLIME_BALL, 2)));
    assert_eq!(level.interact_on(player, tadpole, InteractionHand::MainHand, location), InteractionResult::Pass);
}

#[test]
fn frogspawn_hatches_vanillas_tadpoles() {
    let (mut level, _player) = duel_level();
    for x in 20..=22 {
        for z in 20..=22 {
            level.set_block(BlockPos::new(x, -61, z), blocks::WATER.default_state(), UPDATE_ALL);
        }
    }
    let pos = BlockPos::new(21, -60, 21);
    level.set_block(pos, blocks::FROGSPAWN.default_state(), UPDATE_ALL);
    level.random = wafer_core::random::LegacyRandomSource::new(99);
    let state = level.block_state(pos);
    crate::world::level::block::behaviour(state.block()).tick(state, &mut level, pos);
    let rows: Vec<String> = level
        .entities
        .values()
        .filter(|e| wafer_data::registry::ENTITY_TYPE[e.entity().entity_type as usize] == "minecraft:tadpole")
        .map(|e| {
            let e = e.entity();
            format!("row {:?} {:?} {:?} {:?} {:?}", e.x(), e.y(), e.z(), e.y_rot, e.x_rot)
        })
        .collect();
    let expected: Vec<String> = ORACLE.lines().filter(|l| l.starts_with("row ")).map(str::to_string).collect();
    assert_eq!(rows, expected);
    for id in level.entities.keys().copied().collect::<Vec<_>>() {
        if wafer_data::registry::ENTITY_TYPE[level.entity(id).entity_type as usize] == "minecraft:tadpole" {
            let nbt = save(&level, id);
            assert_eq!(nbt.get("PersistenceRequired"), Some(&Nbt::Byte(1)));
            assert_eq!(nbt.get("FromBucket"), Some(&Nbt::Byte(1)));
            assert_eq!(nbt.get("attributes"), Some(&Nbt::List(Vec::new())));
        }
    }
    assert!(level.block_state(pos).is_air());
}

#[test]
fn frogs_pick_their_variant_by_biome() {
    use crate::server::commands::fill_biome_command;
    let (mut level, _player) = duel_level();
    let biome = |name: &str| crate::world::level::chunk::Biome::by_name(name).unwrap();
    fill_biome_command::fill(&mut level, BlockPos::new(32, -64, 32), BlockPos::new(47, 0, 47), biome("minecraft:desert"), |_| true, |_| {}).unwrap();
    fill_biome_command::fill(&mut level, BlockPos::new(48, -64, 32), BlockPos::new(63, 0, 47), biome("minecraft:snowy_plains"), |_| true, |_| {}).unwrap();
    let expected: Vec<&str> = ORACLE.lines().filter_map(|l| l.strip_prefix("frogvariant ")).collect();
    for (i, (seed, pos)) in [(21, Vec3::new(10.5, -60.0, 10.5)), (22, Vec3::new(40.5, -60.0, 40.5)), (23, Vec3::new(56.5, -60.0, 40.5))].into_iter().enumerate() {
        level.random = wafer_core::random::LegacyRandomSource::new(seed);
        let mut mob = frog::Frog::create();
        mob.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
        let difficulty = level.get_current_difficulty_at(BlockPos::containing_vec(pos));
        let (entity, _) = level.finalize_spawn_detached(LevelEntity::Mob(Box::new(mob)), &difficulty, EntitySpawnReason::Command, None);
        let LevelEntity::Mob(mob) = &entity else { unreachable!() };
        let mut out = Vec::new();
        entity.save_without_id(&mut out);
        let nbt = Nbt::Compound(out.into());
        let variant = variant_utils::variant_name(frog::VARIANT_REGISTRY, frog::get_variant(&mob.entity));
        let parts: Vec<&str> = expected[i].splitn(4, ' ').collect();
        assert_eq!(format!("\"{variant}\""), parts[0]);
        assert_eq!(nbt.get("Brain"), Some(&parse_snbt(parts[1]).unwrap()));
        assert_eq!(nbt.get("attributes"), Some(&parse_snbt(parts[2]).unwrap()));
    }
}
