use wafer_core::Vec3;
use wafer_core::random::LegacyRandomSource;
use wafer_data::{damage_types, game_rule, items};
use wafer_protocol::nbt::Nbt;

use super::armadillo::{self, ArmadilloState};
use crate::server::level::{ServerLevel, testing};
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::mob::MobKind;
use crate::world::entity::schedule::Activity;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::component::custom::parse_snbt;
use crate::world::{InteractionHand, InteractionResult};

fn duel_level() -> (ServerLevel, EntityId) {
    let mut level = testing::level();
    level.game_rules.set(game_rule::RANDOM_TICK_SPEED, 0);
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    for _ in 0..20 {
        level.tick();
    }
    (level, player)
}

fn summon(level: &mut ServerLevel, entity_type: &str, pos: Vec3, nbt: &str) -> EntityId {
    let nbt = parse_snbt(&format!("{{id:\"{entity_type}\",Pos:[{}d,{}d,{}d],{nbt}}}", pos.x, pos.y, pos.z)).unwrap();
    let mut entity = level.load_entity(&nbt).unwrap();
    let e = entity.entity_mut();
    let (y_rot, x_rot) = (e.y_rot, e.x_rot);
    e.snap_to(pos.x, pos.y, pos.z, y_rot, x_rot);
    let id = entity.entity().id;
    level.add_fresh_entity(entity);
    id
}

fn seed(level: &mut ServerLevel, mob: EntityId, seed: i64, sensor_delay: i64) {
    let mob = level.mob_mut(mob).unwrap();
    mob.entity.random = LegacyRandomSource::new(seed);
    mob.living.brain.seed_for_test(seed, sensor_delay);
}

fn seed_level(level: &mut ServerLevel, seed: i64) {
    level.random = LegacyRandomSource::new(seed);
}

fn row(level: &ServerLevel, id: EntityId) -> String {
    let e = level.entity(id);
    let mob = level.mob(id).unwrap();
    let MobKind::Armadillo(armadillo) = &mob.kind else { unreachable!() };
    let brain = &mob.living.brain;
    let activity = brain.get_active_non_core_activity().map_or("-", Activity::get_name);
    let danger = if brain.has_memory_value(memory_module_type::DANGER_DETECTED_RECENTLY) {
        brain.get_time_until_expiry(memory_module_type::DANGER_DETECTED_RECENTLY).to_string()
    } else {
        "-".to_string()
    };
    format!(
        "{} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {} {activity} {} {} {} {danger} {} {:?} {:?}",
        e.tick_count,
        e.x(),
        e.y(),
        e.z(),
        e.y_rot,
        e.y_head_rot,
        e.delta_movement.x,
        e.delta_movement.y,
        e.delta_movement.z,
        e.on_ground,
        armadillo::get_state(e).get_serialized_name(),
        armadillo.in_state_ticks,
        armadillo.scute_time,
        level.is_panicking(id),
        mob.living.y_body_rot,
        mob.health,
    )
}

fn normalize(row: &str) -> Vec<String> {
    row.split(' ')
        .enumerate()
        .map(|(i, f)| match f.parse::<f64>() {
            Ok(v) if matches!(i, 4 | 5 | 16 | 17) => format!("{:?}", v as f32),
            Ok(v) if i > 0 => format!("{v:?}"),
            _ => f.to_lowercase(),
        })
        .collect()
}

fn oracle(label: &str) -> Vec<String> {
    let text = include_str!("armadillo_oracle.txt");
    text.lines().skip_while(|l| *l != format!("== {label}")).skip(1).take_while(|l| !l.starts_with("==")).map(str::to_string).collect()
}

fn rows(label: &str) -> Vec<String> {
    oracle(label).into_iter().filter(|l| l.starts_with(|c: char| c.is_ascii_digit())).collect()
}

fn assert_rows(label: &str, ids: &[EntityId], level: &mut ServerLevel, skip: usize, take: usize) {
    let expected = rows(label);
    assert!(expected.len() >= skip + take.min(1), "no oracle rows for {label}");
    for (tick, line) in expected.iter().enumerate().skip(skip).take(take) {
        level.tick();
        for (i, (&id, expected)) in ids.iter().zip(line.split(" | ")).enumerate() {
            let actual = if level.entity_ref(id).is_some() { row(level, id) } else { "gone".to_string() };
            assert_eq!(normalize(&actual), normalize(expected), "{label} tick {} entity {i}", tick + 1);
        }
    }
}

fn sorted(nbt: Nbt) -> Nbt {
    match nbt {
        Nbt::Compound(mut entries) => {
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Nbt::Compound(entries.into_iter().map(|(k, v)| (k, sorted(v))).collect())
        }
        other => other,
    }
}

fn save(level: &ServerLevel, id: EntityId) -> Nbt {
    let mut out = Vec::new();
    level.entities.get(&id).unwrap().save_without_id(&mut out);
    Nbt::Compound(out)
}

fn assert_brain(level: &ServerLevel, id: EntityId, expected: &str) {
    assert_eq!(sorted(level.mob(id).unwrap().living.brain.pack().to_nbt()), sorted(parse_snbt(expected).unwrap()));
}

fn items(level: &ServerLevel, horizontal: bool) -> Vec<String> {
    let mut items: Vec<String> = level
        .entities
        .values()
        .filter_map(|e| match e {
            LevelEntity::Item(item) => {
                let pos = item.entity.position();
                let name = wafer_data::registry::ITEM[item.item.get_item().0 as usize];
                let (x, z) = if horizontal { (format!("{:?}", pos.x), format!("{:?}", pos.z)) } else { (String::new(), String::new()) };
                Some(format!("{} {name} {x} {:?} {z}", item.item.count, pos.y))
            }
            _ => None,
        })
        .collect();
    items.sort();
    items
}

fn oracle_items(label: &str, horizontal: bool) -> Vec<String> {
    let mut items: Vec<String> = oracle(label)
        .into_iter()
        .filter(|l| l.starts_with("item "))
        .map(|l| {
            let f: Vec<&str> = l.split(' ').collect();
            let coordinate = |s: &str| format!("{:?}", s.parse::<f64>().unwrap());
            let (x, z) = if horizontal { (coordinate(f[3]), coordinate(f[5])) } else { (String::new(), String::new()) };
            format!("{} {} {x} {} {z}", f[1], f[2], coordinate(f[4]))
        })
        .collect();
    items.sort();
    items
}

const IDLE: &str = "PersistenceRequired:1b,scute_time:20000";

#[test]
fn armadillos_roll_up_near_undead_and_unroll_like_vanilla() {
    let (mut level, _player) = duel_level();
    let husk = summon(&mut level, "minecraft:husk", Vec3::new(4.5, -60.0, 8.5), r#"Tags:["h"],Rotation:[0f,0f],PersistenceRequired:1b,NoAI:1b"#);
    let a = summon(&mut level, "minecraft:armadillo", Vec3::new(0.5, -60.0, 8.5), &format!(r#"Tags:["a"],Rotation:[0f,0f],{IDLE}"#));
    seed(&mut level, a, 42, 1);
    seed_level(&mut level, 7);
    assert_rows("scare", &[a], &mut level, 0, usize::MAX);
    assert_brain(&level, a, r#"{memories:{"minecraft:danger_detected_recently":{ttl:76L,value:1b},"minecraft:gaze_cooldown_ticks":{value:139}}}"#);
    assert_eq!(save(&level, a).get("state"), Some(&Nbt::String("scared".into())));
    level.teleport_to(husk, 40.5, -60.0, 40.5);
    assert_rows("unroll", &[a], &mut level, 0, usize::MAX);
    assert_eq!(save(&level, a).get("state"), Some(&Nbt::String("idle".into())));
}

#[test]
fn armadillos_curl_up_when_hurt_like_vanilla() {
    let (mut level, player) = duel_level();
    let b = summon(&mut level, "minecraft:armadillo", Vec3::new(0.5, -60.0, 4.5), &format!(r#"Tags:["b"],Rotation:[0f,0f],{IDLE}"#));
    seed(&mut level, b, 42, 1);
    seed_level(&mut level, 7);
    let mut hurt = |level: &mut ServerLevel, source: DamageSource, damage: f32| {
        assert!(level.hurt_server(b, source, damage));
        level.mob(b).unwrap().health
    };
    assert_rows("hurt", &[b], &mut level, 0, 5);
    assert_eq!(hurt(&mut level, DamageSource::entity(damage_types::PLAYER_ATTACK, player), 2.0), 10.0);
    assert_rows("hurt", &[b], &mut level, 5, 40);
    assert_eq!(hurt(&mut level, DamageSource::entity(damage_types::PLAYER_ATTACK, player), 3.0), 9.0);
    assert_rows("hurt", &[b], &mut level, 45, 10);
    assert_eq!(hurt(&mut level, DamageSource::new(damage_types::CACTUS), 1.0), 9.0);
    assert_rows("hurt", &[b], &mut level, 55, usize::MAX);
    assert_brain(&level, b, r#"{memories:{"minecraft:gaze_cooldown_ticks":{value:24}}}"#);
}

#[test]
fn tempted_armadillos_walk_vanillas_path() {
    let (mut level, player) = duel_level();
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::SPIDER_EYE, 1)));
    let t = summon(&mut level, "minecraft:armadillo", Vec3::new(8.5, -60.0, 0.5), &format!(r#"Tags:["t"],Rotation:[0f,0f],{IDLE}"#));
    seed(&mut level, t, 42, 1);
    seed_level(&mut level, 7);
    assert_rows("tempt", &[t], &mut level, 0, usize::MAX);
}

#[test]
fn armadillos_breed_like_vanilla() {
    let (mut level, _player) = duel_level();
    let p = summon(&mut level, "minecraft:armadillo", Vec3::new(0.5, -60.0, 8.5), &format!(r#"Tags:["p"],Rotation:[0f,0f],InLove:600,{IDLE}"#));
    let q = summon(&mut level, "minecraft:armadillo", Vec3::new(4.5, -60.0, 8.5), &format!(r#"Tags:["q"],Rotation:[0f,0f],InLove:600,{IDLE}"#));
    seed(&mut level, p, 42, 1);
    seed(&mut level, q, 43, 1);
    seed_level(&mut level, 7);
    assert_rows("breed", &[p, q], &mut level, 0, 77);
    for _ in 77..140 {
        level.tick();
    }
    let babies: Vec<EntityId> = level.entities.keys().copied().filter(|&id| level.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::Armadillo(_)) && mob.is_baby())).collect();
    assert_eq!(babies.len(), 1);
    let baby = babies[0];
    let nbt = save(&level, baby);
    assert_eq!(nbt.get("Age"), Some(&Nbt::Int(-23934)));
    assert_eq!(nbt.get("state"), Some(&Nbt::String("idle".into())));
    assert_eq!(level.entity(baby).dimensions, armadillo::get_default_dimensions(true));
}

#[test]
fn armadillos_shed_scutes_like_vanilla() {
    let (mut level, _player) = duel_level();
    let s = summon(&mut level, "minecraft:armadillo", Vec3::new(0.5, -60.0, 12.5), r#"Tags:["s"],Rotation:[0f,0f],PersistenceRequired:1b,scute_time:3"#);
    seed(&mut level, s, 42, 1);
    seed_level(&mut level, 7);
    assert_rows("scute", &[s], &mut level, 0, usize::MAX);
    assert_eq!(items(&level, false), oracle_items("scute", false));
    assert_eq!(save(&level, s).get("scute_time"), Some(&Nbt::Int(7731)));
}

#[test]
fn armadillos_are_brushed_and_fed_like_vanilla() {
    let (mut level, player) = duel_level();
    let r = summon(&mut level, "minecraft:armadillo", Vec3::new(2.5, -60.0, 0.5), r#"Tags:["r"],NoAI:1b"#);
    let k = summon(&mut level, "minecraft:armadillo", Vec3::new(2.5, -60.0, 4.5), r#"Tags:["k"],Age:-100,NoAI:1b"#);
    let c = summon(&mut level, "minecraft:armadillo", Vec3::new(2.5, -60.0, 6.5), r#"Tags:["c"],state:"scared",NoAI:1b"#);
    seed_level(&mut level, 9);
    let mut interact = |level: &mut ServerLevel, target: EntityId, item: wafer_data::Item, count: i32, damage: i32| {
        let mut stack = ItemStack::new(item, count);
        stack.set_damage_value(damage);
        level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(stack));
        let location = level.entity(target).position();
        let result = level.interact_on(player, target, InteractionHand::MainHand, location);
        let held = level.player(player).item_in_hand(InteractionHand::MainHand).filter(|s| !s.is_empty()).map(|s| (s.get_item(), s.count, s.damage_value()));
        (result, held, level.is_in_love(target))
    };
    assert_eq!(interact(&mut level, r, items::BRUSH, 1, 0), (InteractionResult::SUCCESS, Some((items::BRUSH, 1, 16)), false));
    assert_eq!(interact(&mut level, r, items::BRUSH, 1, 50), (InteractionResult::SUCCESS, None, false));
    assert_eq!(interact(&mut level, k, items::BRUSH, 1, 0), (InteractionResult::Pass, Some((items::BRUSH, 1, 0)), false));
    assert_eq!(interact(&mut level, c, items::SPIDER_EYE, 3, 0), (InteractionResult::Pass, Some((items::SPIDER_EYE, 3, 0)), false));
    assert_eq!(interact(&mut level, c, items::BRUSH, 1, 0), (InteractionResult::SUCCESS, Some((items::BRUSH, 1, 16)), false));
    assert_eq!(interact(&mut level, r, items::SPIDER_EYE, 3, 0), (InteractionResult::SUCCESS_SERVER, Some((items::SPIDER_EYE, 2, 0)), true));
    assert_eq!(interact(&mut level, k, items::SPIDER_EYE, 3, 0), (InteractionResult::SUCCESS, Some((items::SPIDER_EYE, 2, 0)), false));
    assert_eq!(items(&level, true), oracle_items("brush", true));
    assert_eq!(armadillo::get_state(level.entity(c)), ArmadilloState::Scared);
    assert_eq!(level.mob(k).unwrap().ageable().unwrap().age, -100);
}

#[test]
fn armadillos_save_and_load_like_vanilla() {
    let (mut level, _player) = duel_level();
    let c = summon(&mut level, "minecraft:armadillo", Vec3::new(2.5, -60.0, 6.5), r#"Tags:["c"],UUID:[I;1,2,3,4],state:"unrolling",scute_time:1234,NoAI:1b"#);
    let nbt = save(&level, c);
    assert_eq!(nbt.get("state"), Some(&Nbt::String("unrolling".into())));
    assert_eq!(nbt.get("scute_time"), Some(&Nbt::Int(1234)));
    assert_eq!(nbt.get("Brain"), Some(&parse_snbt("{memories:{}}").unwrap()));
    assert_eq!(nbt.get("attributes"), Some(&Nbt::List(Vec::new())));
    let Nbt::Compound(mut fields) = nbt.clone() else { unreachable!() };
    fields.push(("id".into(), Nbt::String("minecraft:armadillo".into())));
    let loaded = level.load_entity(&Nbt::Compound(fields)).unwrap();
    let mut out = Vec::new();
    loaded.save_without_id(&mut out);
    assert_eq!(sorted(Nbt::Compound(out)), sorted(nbt));
    let bad = summon(&mut level, "minecraft:armadillo", Vec3::new(4.5, -60.0, 6.5), r#"state:"sleeping",NoAI:1b"#);
    assert_eq!(armadillo::get_state(level.entity(bad)), ArmadilloState::Idle);
}

#[test]
fn dispensers_brush_adult_armadillos() {
    use crate::core::dispenser::block_source::BlockSource;
    use crate::core::dispenser::dispense_item_behavior::get_dispense_method;
    use wafer_data::properties::FACING;
    let (mut level, _player) = duel_level();
    let source = BlockSource { pos: wafer_core::BlockPos::new(2, -60, 0), state: wafer_data::blocks::DISPENSER.default_state().set(FACING, wafer_core::Direction::South) };
    let brush = ItemStack::new(items::BRUSH, 1);
    let dispense = |level: &mut ServerLevel, stack: ItemStack| get_dispense_method(&stack).dispense(level, &source, stack);
    let nothing = dispense(&mut level, brush.clone());
    assert_eq!(nothing.damage_value(), 0);
    let baby = summon(&mut level, "minecraft:armadillo", Vec3::new(2.5, -60.0, 1.5), r#"Age:-100,NoAI:1b"#);
    assert_eq!(dispense(&mut level, brush.clone()).damage_value(), 0);
    let adult = summon(&mut level, "minecraft:armadillo", Vec3::new(2.5, -60.0, 1.5), r#"NoAI:1b"#);
    assert_eq!(dispense(&mut level, brush).damage_value(), 16);
    assert_eq!(items(&level, true), vec![format!("1 minecraft:armadillo_scute {:?} {:?} {:?}", 2.5, -60.0, 1.5)]);
    assert!(level.mob(baby).is_some() && level.mob(adult).is_some());
}

#[test]
fn armadillos_are_scared_by_sprinting_players_undead_and_attackers() {
    use crate::server::level::game_mode::GameType;
    let (mut level, player) = duel_level();
    let a = summon(&mut level, "minecraft:armadillo", Vec3::new(0.5, -60.0, 6.5), r#"NoAI:1b"#);
    let cow = summon(&mut level, "minecraft:cow", Vec3::new(3.5, -60.0, 6.5), r#"NoAI:1b"#);
    let far_husk = summon(&mut level, "minecraft:husk", Vec3::new(0.5, -57.0, 6.5), r#"NoAI:1b"#);
    let husk = summon(&mut level, "minecraft:husk", Vec3::new(7.5, -60.0, 6.5), r#"NoAI:1b"#);
    assert!(!armadillo::is_scared_by(&level, a, player));
    assert!(!armadillo::is_scared_by(&level, a, cow));
    assert!(!armadillo::is_scared_by(&level, a, far_husk));
    assert!(armadillo::is_scared_by(&level, a, husk));
    level.set_sprinting(player, true);
    assert!(armadillo::is_scared_by(&level, a, player));
    level.set_game_mode(player, GameType::Spectator);
    assert!(!armadillo::is_scared_by(&level, a, player));
    level.set_game_mode(player, GameType::Survival);
    level.set_sprinting(player, false);
    assert!(level.hurt_server(a, DamageSource::entity(damage_types::MOB_ATTACK, cow), 1.0));
    assert!(armadillo::is_scared_by(&level, a, cow));
    assert_eq!(armadillo::get_state(level.entity(a)), ArmadilloState::Idle);
}

#[test]
fn rolled_up_armadillos_take_reduced_damage() {
    let (mut level, player) = duel_level();
    let c = summon(&mut level, "minecraft:armadillo", Vec3::new(2.5, -60.0, 6.5), r#"state:"scared",NoAI:1b"#);
    assert!(level.hurt_server(c, DamageSource::entity(damage_types::PLAYER_ATTACK, player), 5.0));
    assert_eq!(level.mob(c).unwrap().health, 10.0);
    let i = summon(&mut level, "minecraft:armadillo", Vec3::new(6.5, -60.0, 6.5), r#"NoAI:1b"#);
    assert!(level.hurt_server(i, DamageSource::entity(damage_types::PLAYER_ATTACK, player), 5.0));
    assert_eq!(level.mob(i).unwrap().health, 7.0);
}

#[test]
fn removed_entities_still_count_by_their_last_bounding_box() {
    let (mut level, _) = duel_level();
    let armadillo = summon(&mut level, "minecraft:armadillo", Vec3::new(4.5, -60.0, 4.5), "");
    let zombie = summon(&mut level, "minecraft:zombie", Vec3::new(6.5, -60.0, 4.5), "");
    let golem = summon(&mut level, "minecraft:iron_golem", Vec3::new(2.5, -60.0, 4.5), "");
    level.get_brain_mut(armadillo).set_memory(memory_module_type::NEAREST_LIVING_ENTITIES, vec![zombie, golem]);
    level.discard(zombie);
    level.discard(golem);
    level.tick();
    assert!(level.find_entity(zombie).is_none() && level.find_entity(golem).is_none());
    assert!(armadillo::is_scared_by(&level, armadillo, zombie));
    let villager = summon(&mut level, "minecraft:villager", Vec3::new(3.5, -60.0, 6.5), "");
    level.get_brain_mut(villager).set_memory(memory_module_type::NEAREST_LIVING_ENTITIES, vec![golem]);
    crate::world::entity::ai::sensing::golem_sensor::check_for_nearby_golem(&mut level, villager);
    assert_eq!(level.get_brain(villager).get_memory(memory_module_type::GOLEM_DETECTED_RECENTLY), Some(&true));
}
