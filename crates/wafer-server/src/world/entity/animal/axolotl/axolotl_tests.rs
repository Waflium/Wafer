use wafer_core::random::LegacyRandomSource;
use wafer_core::{BlockPos, Vec3};
use wafer_data::{blocks, damage_types, game_rule, items, mob_effects};
use wafer_protocol::nbt::Nbt;

use super::axolotl;
use crate::server::level::{ServerLevel, testing};
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::behavior::long_jump_to_random_pos;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::schedule::Activity;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::component::custom::parse_snbt;
use crate::world::item::{self as item, ItemStack};
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::{InteractionHand, InteractionResult};

pub fn duel_level() -> (ServerLevel, EntityId) {
    let mut level = testing::level();
    level.game_rules.set(game_rule::RANDOM_TICK_SPEED, 0);
    let player = testing::player(&mut level, Vec3::new(0.5, -54.0, 0.5), 0.0, 0.0);
    for _ in 0..20 {
        level.tick();
    }
    (level, player)
}

pub fn fill(level: &mut ServerLevel, from: (i32, i32, i32), to: (i32, i32, i32), block: wafer_data::Block) {
    for x in from.0..=to.0 {
        for y in from.1..=to.1 {
            for z in from.2..=to.2 {
                level.set_block(BlockPos::new(x, y, z), block.default_state(), UPDATE_ALL);
            }
        }
    }
}

pub fn pool(level: &mut ServerLevel) {
    fill(level, (-6, -61, -6), (6, -55, 6), blocks::GLASS);
    fill(level, (-5, -60, -5), (5, -56, 5), blocks::WATER);
}

pub fn summon(level: &mut ServerLevel, entity_type: &str, pos: Vec3, nbt: &str) -> EntityId {
    summon_with_parity(level, entity_type, pos, nbt, None)
}

pub fn summon_with_parity(level: &mut ServerLevel, entity_type: &str, pos: Vec3, nbt: &str, parity: Option<i32>) -> EntityId {
    let nbt = parse_snbt(&format!("{{id:\"{entity_type}\",Pos:[{}d,{}d,{}d],{nbt}}}", pos.x, pos.y, pos.z)).unwrap();
    let mut entity = loop {
        let entity = level.load_entity(&nbt).unwrap();
        if parity.is_none_or(|parity| entity.entity().id.rem_euclid(2) == parity) {
            break entity;
        }
    };
    let e = entity.entity_mut();
    let (y_rot, x_rot) = (e.y_rot, e.x_rot);
    e.snap_to(pos.x, pos.y, pos.z, y_rot, x_rot);
    let id = entity.entity().id;
    level.add_fresh_entity(entity);
    id
}

pub fn seed(level: &mut ServerLevel, mob: EntityId, seed: i64, sensor_delay: i64) {
    let mob = level.mob_mut(mob).unwrap();
    mob.entity.random = LegacyRandomSource::new(seed);
    mob.living.brain.seed_for_test(seed, sensor_delay);
}

pub fn seed_level(level: &mut ServerLevel, seed: i64) {
    level.random = LegacyRandomSource::new(seed);
    long_jump_to_random_pos::set_shuffle_seed(seed);
}

pub fn row(level: &ServerLevel, id: EntityId) -> String {
    let e = level.entity(id);
    let mut s = format!(
        "{} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {}",
        e.tick_count, e.x(), e.y(), e.z(), e.y_rot, e.y_head_rot, e.x_rot, e.delta_movement.x, e.delta_movement.y, e.delta_movement.z, e.on_ground
    );
    let mob = level.mob(id).unwrap();
    if matches!(mob.kind, crate::world::entity::mob::MobKind::Axolotl(_) | crate::world::entity::mob::MobKind::Frog(_) | crate::world::entity::mob::MobKind::Tadpole(_)) {
        s += &format!(" {}", mob.living.brain.get_active_non_core_activity().map_or("-", Activity::get_name));
    }
    s += &format!(" {:?} {}", e.pose(), e.air_supply());
    if let crate::world::entity::mob::MobKind::Axolotl(_) = mob.kind {
        s += &format!(" dead={}", axolotl::is_playing_dead(e));
    }
    s += &format!(" {:?}", mob.health);
    s
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

pub fn oracle(text: &str, label: &str) -> Vec<String> {
    text.lines().skip_while(|l| *l != format!("== {label}")).skip(1).take_while(|l| !l.starts_with("==")).filter(|l| l.as_bytes().first().is_some_and(u8::is_ascii_digit) || l.starts_with("gone")).map(str::to_string).collect()
}

pub fn assert_rows(text: &str, label: &str, ids: &[EntityId], level: &mut ServerLevel, skip: usize, take: usize) {
    let expected = oracle(text, label);
    assert!(!expected.is_empty(), "no oracle rows for {label}");
    for (tick, line) in expected.iter().enumerate().skip(skip).take(take) {
        level.tick();
        for (i, (&id, expected)) in ids.iter().zip(line.split(" | ")).enumerate() {
            let actual = if level.entity_ref(id).is_some_and(|e| !e.is_removed()) { row(level, id) } else { "gone".to_string() };
            assert_eq!(normalize(&actual), normalize(expected), "{label} tick {} entity {i}", tick + 1);
        }
    }
}

pub fn save(level: &ServerLevel, id: EntityId) -> Nbt {
    let mut out = Vec::new();
    level.entities.get(&id).unwrap().save_without_id(&mut out);
    Nbt::Compound(out)
}

const ORACLE: &str = include_str!("axolotl_oracle.txt");

#[test]
fn axolotls_hunt_cod_like_vanilla() {
    let (mut level, _player) = duel_level();
    pool(&mut level);
    let cod = summon(&mut level, "minecraft:cod", Vec3::new(3.5, -58.0, 0.5), r#"Tags:["c"],NoAI:1b,PersistenceRequired:1b"#);
    let axolotl = summon(&mut level, "minecraft:axolotl", Vec3::new(0.5, -58.0, 0.5), r#"Tags:["a"],Rotation:[0f,0f],PersistenceRequired:1b,Variant:2"#);
    seed(&mut level, axolotl, 42, 1);
    seed_level(&mut level, 7);
    assert_rows(ORACLE, "hunt", &[axolotl, cod], &mut level, 0, usize::MAX);
    let brain = level.get_brain(axolotl);
    assert_eq!(brain.get_time_until_expiry(memory_module_type::HAS_HUNTING_COOLDOWN), 2318);
    assert_eq!(save(&level, axolotl).get("Brain"), Some(&parse_snbt(r#"{memories:{"minecraft:has_hunting_cooldown":{ttl:2318L,value:1b}}}"#).unwrap()));
}

fn play_dead(seed_value: i64, label: &str) {
    let (mut level, _player) = duel_level();
    pool(&mut level);
    let pig = summon(&mut level, "minecraft:pig", Vec3::new(8.5, -60.0, 0.5), r#"Tags:["z"],NoAI:1b,PersistenceRequired:1b"#);
    let axolotl = summon(&mut level, "minecraft:axolotl", Vec3::new(0.5, -58.0, 0.5), r#"Tags:["a2"],Rotation:[0f,0f],PersistenceRequired:1b"#);
    seed(&mut level, axolotl, seed_value, 1);
    seed_level(&mut level, 9);
    assert_rows(ORACLE, label, &[axolotl], &mut level, 0, 5);
    level.hurt_server(axolotl, DamageSource::entity(damage_types::MOB_ATTACK, pig), 1.0);
    assert_rows(ORACLE, label, &[axolotl], &mut level, 5, usize::MAX);
}

#[test]
fn axolotls_play_dead_like_vanilla() {
    play_dead(5, "playdead");
    play_dead(3, "playdead3");
}

#[test]
fn axolotls_regenerate_players_who_finish_their_prey() {
    let (mut level, player) = duel_level();
    pool(&mut level);
    let cod = summon(&mut level, "minecraft:cod", Vec3::new(3.5, -58.0, 0.5), r#"Tags:["c3"],NoAI:1b,PersistenceRequired:1b"#);
    let axolotl = summon(&mut level, "minecraft:axolotl", Vec3::new(0.5, -58.0, 0.5), r#"Tags:["a3"],Rotation:[0f,0f],PersistenceRequired:1b"#);
    seed(&mut level, axolotl, 42, 1);
    seed_level(&mut level, 7);
    assert_rows(ORACLE, "regen", &[axolotl, cod], &mut level, 0, 30);
    level.hurt_server(cod, DamageSource::entity(damage_types::PLAYER_ATTACK, player), 10.0);
    for tick in 30..40 {
        assert_rows(ORACLE, "regen", &[axolotl], &mut level, tick, 1);
        level.tick_player(player);
    }
    let regeneration = level.living(player).effect(mob_effects::REGENERATION).map(|effect| effect.duration);
    assert_eq!(regeneration, Some(90));
}

#[test]
fn axolotls_and_tadpoles_go_into_buckets_like_vanilla() {
    use crate::world::item::component as dc;
    let (mut level, player) = duel_level();
    pool(&mut level);
    let axolotl = summon(
        &mut level,
        "minecraft:axolotl",
        Vec3::new(0.5, -58.0, 0.5),
        r#"Tags:["a4"],PersistenceRequired:1b,Variant:3,Age:-100,Brain:{memories:{"minecraft:has_hunting_cooldown":{value:1b,ttl:300L}}}"#,
    );
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::WATER_BUCKET, 1)));
    let location = level.entity(axolotl).position();
    assert_eq!(level.interact_on(player, axolotl, InteractionHand::MainHand, location), InteractionResult::SUCCESS);
    let bucket = level.player(player).item_in_hand(InteractionHand::MainHand).unwrap();
    assert_eq!(bucket.get_item(), items::AXOLOTL_BUCKET);
    assert_eq!(bucket.get(dc::AXOLOTL_VARIANT), Some(&Nbt::String("cyan".into())));
    let expected = parse_snbt("{Age:-100,AgeLocked:0b,Health:14.0f,HuntingCooldown:300L,PersistenceRequired:1b}").unwrap();
    assert_eq!(sorted(bucket.get(dc::BUCKET_ENTITY_DATA).unwrap().clone()), sorted(expected));
    let released = release(&mut level, &bucket, "minecraft:axolotl");
    let nbt = save(&level, released);
    for (key, value) in [("Age", "-100"), ("FromBucket", "1b"), ("Variant", "3"), ("PersistenceRequired", "1b"), ("Health", "14.0f"), ("Air", "6000s"), ("attributes", "[]")] {
        assert_eq!(nbt.get(key), Some(&parse_snbt(value).unwrap()), "{key}");
    }
    assert_eq!(nbt.get("Brain"), Some(&parse_snbt(r#"{memories:{"minecraft:has_hunting_cooldown":{ttl:300L,value:1b}}}"#).unwrap()));

    let tadpole = summon(&mut level, "minecraft:tadpole", Vec3::new(0.5, -58.0, 0.5), r#"Tags:["t4"],Age:1234,Health:5f"#);
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::WATER_BUCKET, 1)));
    let location = level.entity(tadpole).position();
    assert_eq!(level.interact_on(player, tadpole, InteractionHand::MainHand, location), InteractionResult::SUCCESS);
    let bucket = level.player(player).item_in_hand(InteractionHand::MainHand).unwrap();
    assert_eq!(bucket.get_item(), items::TADPOLE_BUCKET);
    let expected = parse_snbt("{Age:1234,AgeLocked:0b,Health:5.0f}").unwrap();
    assert_eq!(sorted(bucket.get(dc::BUCKET_ENTITY_DATA).unwrap().clone()), sorted(expected));
    let released = release(&mut level, &bucket, "minecraft:tadpole");
    let nbt = save(&level, released);
    for (key, value) in [("Age", "1234"), ("FromBucket", "1b"), ("PersistenceRequired", "0b"), ("Health", "5.0f"), ("Air", "300s"), ("AgeLocked", "0b")] {
        assert_eq!(nbt.get(key), Some(&parse_snbt(value).unwrap()), "{key}");
    }
}

#[test]
fn grown_tadpoles_turn_into_frogs_when_loaded() {
    use crate::world::item::component as dc;
    let (mut level, _player) = duel_level();
    pool(&mut level);
    let nbt = parse_snbt(r#"{id:"minecraft:tadpole",Pos:[0.5d,-58.0d,0.5d],Age:24000,CustomName:"t"}"#).unwrap();
    let tadpole = level.load_entity(&nbt).unwrap();
    assert!(tadpole.entity().is_removed());
    let frog = level.entities.values().find(|e| wafer_data::registry::ENTITY_TYPE[e.entity().entity_type as usize] == "minecraft:frog").unwrap();
    assert_eq!(frog.entity().position(), Vec3::new(0.5, -58.0 - 0.09999999403953552, 0.5));
    assert!(!level.add_fresh_entity(tadpole));

    let mut bucket = ItemStack::new(items::TADPOLE_BUCKET, 1);
    bucket.set(dc::BUCKET_ENTITY_DATA, Some(parse_snbt("{Age:24000}").unwrap()));
    let frog = release(&mut level, &bucket, "minecraft:frog");
    assert_eq!(save(&level, frog).get("PersistenceRequired"), Some(&Nbt::Byte(1)));
    assert!(!level.entities.values().any(|e| wafer_data::registry::ENTITY_TYPE[e.entity().entity_type as usize] == "minecraft:tadpole"));
}

fn release(level: &mut ServerLevel, bucket: &ItemStack, entity_type: &str) -> EntityId {
    let before: Vec<EntityId> = level.entities.keys().copied().collect();
    let mob_bucket = item::mob_bucket_item::MobBucketItem::for_item(bucket.get_item(), &serde_json::json!({"emptySound": "minecraft:item.bucket.empty_fish"})).unwrap();
    mob_bucket.check_extra_content(level, None, bucket, BlockPos::new(0, -58, 2));
    level
        .entities
        .iter()
        .find(|(id, e)| !before.contains(id) && wafer_data::registry::ENTITY_TYPE[e.entity().entity_type as usize] == entity_type)
        .map(|(&id, _)| id)
        .unwrap()
}

pub fn sorted(nbt: Nbt) -> Nbt {
    match nbt {
        Nbt::Compound(mut entries) => {
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Nbt::Compound(entries.into_iter().map(|(k, v)| (k, sorted(v))).collect())
        }
        Nbt::List(entries) => Nbt::List(entries.into_iter().map(sorted).collect()),
        other => other,
    }
}

#[test]
fn summoned_axolotls_pick_vanillas_variants() {
    let (mut level, _player) = duel_level();
    pool(&mut level);
    let mut variants = Vec::new();
    for seed in [11, 12, 13, 14, 15, 16, 17, 18] {
        seed_level(&mut level, seed);
        let mut mob = axolotl::Axolotl::create();
        mob.entity.snap_to(0.5, -58.0, 0.5, 0.0, 0.0);
        let difficulty = crate::world::level::server_level_accessor::ServerLevelAccessor::get_current_difficulty_at(&mut level, BlockPos::new(0, -58, 0));
        let (entity, _) = level.finalize_spawn_detached(LevelEntity::Mob(Box::new(mob)), &difficulty, crate::world::entity::entity_spawn_reason::EntitySpawnReason::Command, None);
        variants.push(axolotl::get_variant(entity.entity()).get_id());
    }
    let expected: Vec<i32> = ORACLE.lines().filter_map(|l| l.strip_prefix("variant ")).map(|v| v.parse().unwrap()).collect();
    assert_eq!(variants, expected);
}

#[test]
fn bred_axolotls_are_sometimes_blue() {
    let (mut level, _player) = duel_level();
    pool(&mut level);
    let a = summon(&mut level, "minecraft:axolotl", Vec3::new(0.5, -58.0, 0.5), "Variant:1,NoAI:1b");
    let b = summon(&mut level, "minecraft:axolotl", Vec3::new(1.5, -58.0, 0.5), "Variant:2,NoAI:1b");
    let mut blue = 0;
    for seed in 0..12000 {
        level.entity_mut(a).random = LegacyRandomSource::new(seed);
        let baby = level.axolotl_get_breed_offspring(a, b).unwrap();
        let variant = axolotl::get_variant(&baby.entity);
        assert!(baby.persistence_required);
        match variant {
            axolotl::Variant::Blue => blue += 1,
            axolotl::Variant::Wild | axolotl::Variant::Gold => {}
            other => panic!("{other:?} from wild and gold parents"),
        }
    }
    assert!((5..=15).contains(&blue), "{blue} blue babies");
}

#[test]
fn axolotls_spawn_in_water_above_clay() {
    use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
    use crate::world::entity::{entity_types, spawn_placements};
    let (mut level, _player) = duel_level();
    fill(&mut level, (20, -61, 20), (20, -61, 20), blocks::CLAY);
    fill(&mut level, (20, -60, 20), (20, -59, 20), blocks::WATER);
    fill(&mut level, (24, -60, 20), (24, -59, 20), blocks::WATER);
    assert!(spawn_placements::check_spawn_rules(&entity_types::AXOLOTL, &mut level, EntitySpawnReason::Natural, BlockPos::new(20, -60, 20)));
    assert!(!spawn_placements::check_spawn_rules(&entity_types::AXOLOTL, &mut level, EntitySpawnReason::Natural, BlockPos::new(24, -60, 20)));
}

#[test]
fn axolotls_dry_out_on_land() {
    let (mut level, _player) = duel_level();
    let axolotl = summon(&mut level, "minecraft:axolotl", Vec3::new(10.5, -60.0, 10.5), "NoAI:0b,Air:3");
    assert_eq!(level.entity(axolotl).air_supply(), 3);
    for _ in 0..22 {
        level.tick();
    }
    assert_eq!(level.entity(axolotl).air_supply(), -19);
    assert_eq!(level.mob(axolotl).unwrap().health, 14.0);
    level.tick();
    assert_eq!(level.entity(axolotl).air_supply(), 0);
    assert!(level.mob(axolotl).unwrap().health < 14.0);
    level.rehydrate(axolotl);
    assert_eq!(level.entity(axolotl).air_supply(), 1800);
    let drowned = level.get_last_damage_source(axolotl).map(|source| source.ty);
    assert_eq!(drowned, Some(damage_types::DRY_OUT));
}
