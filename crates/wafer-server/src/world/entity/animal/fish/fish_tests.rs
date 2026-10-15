use wafer_core::random::LegacyRandomSource;
use wafer_core::{BlockPos, Vec3};
use wafer_data::{blocks, damage_types, items};
use wafer_protocol::nbt::Nbt;

use super::cod::Cod;
use super::pufferfish::Pufferfish;
use super::salmon::{self, Salmon};
use super::tropical_fish::{self, TropicalFish};
use crate::server::level::{ServerLevel, testing};
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::data::{DataValue, ids};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::mob::Mob;
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::{InteractionHand, InteractionResult};

const ORACLE: &str = include_str!("fish_oracle.txt");

fn pool_level() -> ServerLevel {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    for x in -6..=6 {
        for y in -61..=-50 {
            for z in -6..=6 {
                let edge = x == -6 || x == 6 || y == -61 || y == -50 || z == -6 || z == 6;
                let state = if edge { blocks::GLASS.default_state() } else { blocks::WATER.default_state() };
                level.set_block(BlockPos::new(x, y, z), state, UPDATE_ALL);
            }
        }
    }
    testing::player(&mut level, Vec3::new(0.5, -60.0, 22.5), 0.0, 0.0);
    for _ in 0..61 {
        level.tick();
    }
    level
}

struct Fish {
    create: fn() -> Mob,
    pos: Vec3,
    seed: i64,
    next_start_tick: i32,
    nbt: Vec<(String, Nbt)>,
}

fn fish(create: fn() -> Mob, pos: Vec3, seed: i64, next_start_tick: i32) -> Fish {
    Fish { create, pos, seed, next_start_tick, nbt: Vec::new() }
}

fn summon(level: &mut ServerLevel, fish: &Fish, parity: i32) -> EntityId {
    let mut mob = loop {
        let mob = (fish.create)();
        if mob.entity.id.rem_euclid(2) == parity {
            break mob;
        }
    };
    if !fish.nbt.is_empty() {
        let nbt = Nbt::Compound(fish.nbt.clone());
        mob.load(&nbt);
        for (key, id) in [("NoGravity", ids::NO_GRAVITY), ("Silent", ids::SILENT)] {
            if nbt.get(key).is_some() {
                mob.entity.data.set(id, DataValue::Boolean(true));
            }
        }
    }
    mob.persistence_required = true;
    mob.entity.snap_to(fish.pos.x, fish.pos.y, fish.pos.z, 0.0, 0.0);
    mob.entity.y_head_rot = 0.0;
    mob.living.y_body_rot = 0.0;
    mob.entity.random = LegacyRandomSource::new(fish.seed);
    if fish.next_start_tick >= 0 {
        for wrapped_goal in mob.ai.goal_selector.get_available_goals_mut() {
            if let Goal::FollowFlockLeader(goal) = wrapped_goal.get_goal_mut() {
                goal.next_start_tick = fish.next_start_tick;
            }
        }
    }
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

fn oracle(label: &str) -> (Vec<i32>, Vec<Vec<String>>) {
    let lines = ORACLE
        .lines()
        .skip_while(|line| *line != format!("== {label}"))
        .skip(1)
        .take_while(|line| !line.starts_with("==") && !line.starts_with("tropical") && !line.starts_with("salmon"));
    let mut parities = Vec::new();
    let mut rows = Vec::new();
    for line in lines {
        if let Some(rest) = line.strip_prefix("summoned ") {
            parities.push(rest.split(' ').nth(1).unwrap().trim_start_matches("parity=").parse().unwrap());
        } else if line != "done" {
            rows.push(line.split(' ').map(str::to_string).collect());
        }
    }
    (parities, rows)
}

fn row(level: &ServerLevel, id: EntityId) -> Vec<String> {
    let Some(LevelEntity::Mob(mob)) = level.entities.get(&id) else { return vec!["gone".into()] };
    let e = &mob.entity;
    let mut row = vec![
        e.tick_count.to_string(),
        java_double(e.x()),
        java_double(e.y()),
        java_double(e.z()),
        java_float(e.y_rot),
        java_double(e.delta_movement.y),
        e.air_supply().to_string(),
        java_float(mob.health),
    ];
    if matches!(mob.kind, crate::world::entity::mob::MobKind::Pufferfish(_)) {
        row.push(format!("puff={}", mob.get_puff_state()));
    }
    row
}

fn java_double(v: f64) -> String {
    v.to_string()
}

fn java_float(v: f32) -> String {
    v.to_string()
}

fn same_number(column: usize, a: &str, b: &str) -> bool {
    if column == 4 || column == 7 {
        return a.parse::<f32>().ok() == b.parse::<f32>().ok();
    }
    match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn assert_matches_oracle(label: &str, fishes: &[Fish], setup: impl FnOnce(&mut ServerLevel)) {
    let (parities, expected) = oracle(label);
    assert_eq!(parities.len(), fishes.len(), "{label} fish count");
    let mut level = pool_level();
    setup(&mut level);
    let ids: Vec<EntityId> = fishes.iter().zip(&parities).map(|(fish, &parity)| summon(&mut level, fish, parity)).collect();
    let mut rows = expected.iter().peekable();
    let mut tick = 0;
    while rows.peek().is_some() {
        if rows.peek().is_some_and(|row| row[0] == "hurt") {
            let hurt = rows.next().unwrap();
            let k: usize = hurt[1].parse().unwrap();
            level.hurt_server(ids[k], DamageSource::new(damage_types::MAGIC), hurt[2].parse().unwrap());
            continue;
        }
        level.tick();
        tick += 1;
        for (k, &id) in ids.iter().enumerate() {
            let expected_row = rows.next().unwrap();
            assert_eq!(expected_row[0], k.to_string());
            let actual = row(&level, id);
            let ok = expected_row.len() - 1 == actual.len() && expected_row[1..].iter().zip(&actual).enumerate().all(|(column, (a, b))| same_number(column, a, b));
            assert!(ok, "{label} fish {k} tick {tick}: expected {:?}, got {:?}", &expected_row[1..], actual);
        }
    }
}

#[test]
fn cods_swim_like_vanilla() {
    assert_matches_oracle("cod", &[fish(Cod::create, Vec3::new(0.5, -56.0, 0.5), 1, 5)], |_| {});
}

#[test]
fn large_salmon_swim_like_vanilla() {
    let mut salmon = fish(Salmon::create, Vec3::new(0.5, -56.0, 0.5), 2, 3);
    salmon.nbt = vec![("type".into(), Nbt::String("large".into()))];
    assert_matches_oracle("salmon", &[salmon], |_| {});
}

#[test]
fn tropical_fish_swim_like_vanilla() {
    let mut tropical_fish = fish(TropicalFish::create, Vec3::new(0.5, -56.0, 0.5), 3, 3);
    tropical_fish.nbt = vec![("Variant".into(), Nbt::Int(117506305))];
    assert_matches_oracle("tropical", &[tropical_fish], |_| {});
}

#[test]
fn cods_school_like_vanilla() {
    assert_matches_oracle(
        "school",
        &[
            fish(Cod::create, Vec3::new(-2.5, -56.0, 0.5), 4, 0),
            fish(Cod::create, Vec3::new(2.5, -56.0, 0.5), 5, 0),
            fish(Cod::create, Vec3::new(0.5, -56.0, 3.5), 6, 0),
        ],
        |_| {},
    );
}

fn pig() -> Mob {
    let LevelEntity::Mob(pig) = ServerLevel::create_entity("minecraft:pig").unwrap() else { unreachable!() };
    *pig
}

#[test]
fn pufferfish_puff_up_and_sting_a_pig_like_vanilla() {
    let mut pig = fish(pig, Vec3::new(0.5, -56.0, 1.0), 9, -1);
    pig.nbt = vec![("NoAI".into(), Nbt::Byte(1)), ("NoGravity".into(), Nbt::Byte(1)), ("Silent".into(), Nbt::Byte(1))];
    assert_matches_oracle("puffer", &[pig, fish(Pufferfish::create, Vec3::new(0.5, -56.0, 0.5), 7, -1)], |_| {});
}

#[test]
fn lonely_pufferfish_deflate_like_vanilla() {
    let mut pufferfish = fish(Pufferfish::create, Vec3::new(0.5, -56.0, 0.5), 11, -1);
    pufferfish.nbt = vec![("PuffState".into(), Nbt::Int(2))];
    assert_matches_oracle("lonely", &[pufferfish], |_| {});
}

#[test]
fn hurt_cods_panic_like_vanilla() {
    assert_matches_oracle("panic", &[fish(Cod::create, Vec3::new(0.5, -56.0, 0.5), 10, 5)], |_| {});
}

#[test]
fn salmon_sizes_from_finalize_spawn_match_vanilla() {
    let mut level = testing::level();
    for line in ORACLE.lines().filter(|line| line.starts_with("salmon ")) {
        let words: Vec<&str> = line.split(' ').collect();
        let seed: i64 = words[1].parse().unwrap();
        let mut mob = Salmon::create();
        mob.entity.random = LegacyRandomSource::new(seed);
        let difficulty = level.current_difficulty_at(mob.entity.block_position());
        let (entity, _) = level.finalize_spawn_detached(LevelEntity::Mob(Box::new(mob)), &difficulty, EntitySpawnReason::Natural, None);
        let LevelEntity::Mob(mob) = entity else { unreachable!() };
        assert_eq!(mob.get_salmon_variant().get_serialized_name(), words[2], "seed {seed}");
        assert_eq!(mob.entity.dimensions.width.to_string(), words[3], "seed {seed}");
        assert_eq!(mob.entity.dimensions.height.to_string(), words[4], "seed {seed}");
    }
}

#[test]
fn cods_flop_and_suffocate_on_land_like_vanilla() {
    assert_matches_oracle("flop", &[fish(Cod::create, Vec3::new(12.5, -60.0, 22.5), 8, 5)], |_| {});
}

#[test]
fn tropical_fish_variants_from_finalize_spawn_match_vanilla() {
    let mut level = testing::level();
    for line in ORACLE.lines().filter(|line| line.starts_with("tropical ")) {
        let mut words = line.split(' ').skip(1);
        let seed: i64 = words.next().unwrap().parse().unwrap();
        level.random = LegacyRandomSource::new(seed);
        let mut group_data: Option<SpawnGroupData> = None;
        for expected in words {
            let mut mob = TropicalFish::create();
            mob.entity.snap_to(0.5, -55.0, 0.5, 0.0, 0.0);
            let difficulty = level.current_difficulty_at(mob.entity.block_position());
            let (entity, next) = level.finalize_spawn_detached(LevelEntity::Mob(Box::new(mob)), &difficulty, EntitySpawnReason::Natural, group_data);
            group_data = next;
            let LevelEntity::Mob(mob) = entity else { unreachable!() };
            let mut out = Vec::new();
            mob.save(&mut out);
            let variant = out.iter().find(|(key, _)| key == "Variant").map(|(_, value)| value.clone());
            let (variant_id, max_group_size_reached) = expected.split_once('/').unwrap();
            assert_eq!(variant, Some(Nbt::Int(variant_id.parse().unwrap())), "seed {seed}");
            assert_eq!(mob.is_max_group_size_reached(0), max_group_size_reached == "1", "seed {seed}");
        }
    }
}

#[test]
fn puffed_pufferfish_sting_touching_players_like_vanilla() {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    level.game_rules.set(wafer_data::game_rule::NATURAL_HEALTH_REGENERATION, 0);
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    for _ in 0..100 {
        level.tick();
        level.tick_player(player);
    }
    let mut pufferfish = Pufferfish::create();
    pufferfish.load(&Nbt::Compound(vec![("NoAI".into(), Nbt::Byte(1)), ("PuffState".into(), Nbt::Int(2))]));
    pufferfish.entity.snap_to(0.5, -60.0, 0.5, 0.0, 0.0);
    level.add_fresh_entity(LevelEntity::Mob(Box::new(pufferfish)));
    for (tick, expected) in include_str!("sting_oracle.txt").lines().enumerate() {
        level.tick();
        level.tick_player(player);
        let living = level.living(player);
        let poison = living.effect(wafer_data::mob_effects::POISON).map_or(-1, |effect| effect.duration);
        let actual = format!("{:?} {} {}", level.player(player).health, living.damage_cooldown_time, poison);
        assert_eq!(actual, expected, "tick {}", tick + 1);
    }
}

#[test]
fn salmon_go_into_buckets_and_come_back_out() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 90.0);
    let mut salmon = Salmon::create();
    salmon.set_salmon_variant(salmon::Variant::Small);
    salmon.set_no_ai(true);
    salmon.set_health(2.0);
    salmon.entity.snap_to(0.5, -60.0, 2.0, 0.0, 0.0);
    let salmon_id = salmon.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(salmon)));
    level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::WATER_BUCKET, 1));
    assert_eq!(level.mob_interact(salmon_id, player, InteractionHand::MainHand), InteractionResult::SUCCESS);
    assert!(level.entity(salmon_id).is_removed());
    let bucket = level.player(player).inventory.slots[36].clone().unwrap();
    assert_eq!(bucket.item, items::SALMON_BUCKET);
    assert_eq!(bucket.get(dc::SALMON_SIZE), Some(&Nbt::String("small".into())));
    assert_eq!(bucket.get(dc::BUCKET_ENTITY_DATA), Some(&Nbt::Compound(vec![("NoAI".into(), Nbt::Byte(1)), ("Health".into(), Nbt::Float(2.0))])));
    level.tick();
    assert!(level.use_item(player, InteractionHand::MainHand).consumes_action());
    assert_eq!(level.player(player).inventory.slots[36], Some(ItemStack::new(items::BUCKET, 1)));
    assert_eq!(level.block_state(BlockPos::new(0, -60, 0)), blocks::WATER.default_state());
    let released = level.entities.values().find_map(|entity| match entity {
        LevelEntity::Mob(mob) if matches!(mob.kind, crate::world::entity::mob::MobKind::Salmon(_)) && !mob.entity.is_removed() => Some(mob),
        _ => None,
    });
    let released = released.expect("a released salmon");
    assert!(released.from_bucket());
    assert!(released.is_no_ai());
    assert_eq!(released.health, 2.0);
    assert_eq!(released.get_salmon_variant(), salmon::Variant::Small);
}

#[test]
fn fish_have_vanillas_attributes_and_sizes() {
    let cod = Cod::create();
    assert_eq!(cod.health, 3.0);
    assert_eq!(cod.living.attributes.value(Attribute::MaxHealth), 3.0);
    assert_eq!(cod.living.attributes.value(Attribute::MovementSpeed), 0.7);
    let pufferfish = Pufferfish::create();
    assert_eq!((pufferfish.entity.dimensions.width, pufferfish.entity.dimensions.height), (0.35, 0.35));
    let mut salmon = Salmon::create();
    salmon.set_salmon_variant(salmon::Variant::Large);
    assert_eq!((salmon.entity.dimensions.width, salmon.entity.dimensions.height), (0.7 * 1.5, 0.4 * 1.5));
    assert_eq!(tropical_fish::Variant::from_packed_id(117506305).get_packed_id(), 117506305);
}

#[test]
fn fish_save_and_load_their_data() {
    let mut tropical = TropicalFish::create();
    tropical.set_packed_variant(tropical_fish::COMMON_VARIANTS[7].get_packed_id());
    tropical.set_from_bucket(true);
    let mut out = Vec::new();
    tropical.save(&mut out);
    assert!(out.contains(&("FromBucket".into(), Nbt::Byte(1))));
    let mut loaded = TropicalFish::create();
    loaded.load(&Nbt::Compound(out));
    assert_eq!(loaded.get_packed_variant(), tropical_fish::COMMON_VARIANTS[7].get_packed_id());
    assert!(loaded.from_bucket());
    let mut pufferfish = Pufferfish::create();
    pufferfish.load(&Nbt::Compound(vec![("PuffState".into(), Nbt::Int(7))]));
    assert_eq!(pufferfish.get_puff_state(), 2);
    assert_eq!((pufferfish.entity.dimensions.width, pufferfish.entity.dimensions.height), (0.7, 0.7));
}
