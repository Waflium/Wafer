use wafer_core::random::LegacyRandomSource;
use wafer_core::{BlockPos, Vec3};
use wafer_data::{blocks, game_rule, items};
use wafer_protocol::Writer;
use wafer_protocol::nbt::Nbt;

use super::villager;
use crate::server::level::{ServerLevel, testing};
use crate::world::InteractionHand;
use crate::world::entity::ai::behavior::long_jump_to_random_pos;
use crate::world::entity::schedule::Activity;
use crate::world::entity::entity_type::EntityType;
use crate::world::entity::entity_types;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::inventory::abstract_container_menu::QUICK_MOVE;
use crate::world::item::ItemStack;
use crate::world::item::component::custom::parse_snbt;
use crate::world::level::update_flags::UPDATE_ALL;

fn duel_level() -> (ServerLevel, EntityId) {
    let mut level = testing::level();
    level.game_rules.set(game_rule::RANDOM_TICK_SPEED, 0);
    level.clock_ticks[0] = 6000;
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
    long_jump_to_random_pos::set_shuffle_seed(seed);
}

fn row(level: &ServerLevel, id: EntityId) -> String {
    let e = level.entity(id);
    let mut s = format!("{} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {}", e.tick_count, e.x(), e.y(), e.z(), e.y_rot, e.y_head_rot, e.delta_movement.x, e.delta_movement.y, e.delta_movement.z, e.on_ground);
    if level.is_villager(id) {
        let data = villager::get_villager_data(e);
        let mob = level.mob(id).unwrap();
        let activity = mob.living.brain.get_active_non_core_activity().map_or("-", Activity::get_name);
        s += &format!(" {activity} {} {} {}", data.profession.name(), data.level, level.get_villager_xp(id));
    }
    s += &format!(" {:?}", level.health(id));
    s
}

fn normalize(row: &str) -> Vec<String> {
    row.split(' ')
        .enumerate()
        .map(|(i, f)| match f.parse::<f64>() {
            Ok(v) if i == 4 || i == 5 => format!("{:?}", v as f32),
            Ok(v) if i > 0 => format!("{v:?}"),
            _ => f.to_lowercase(),
        })
        .collect()
}

fn oracle(label: &str) -> Vec<String> {
    let text = include_str!("villager_oracle.txt");
    text.lines().skip_while(|l| *l != format!("== {label}")).skip(1).take_while(|l| !l.starts_with("==") && *l != "done").map(str::to_string).collect()
}

fn oracle_value(label: &str, key: &str) -> String {
    oracle(label).into_iter().find_map(|line| line.strip_prefix(&format!("{key} ")).map(str::to_string)).unwrap_or_else(|| panic!("no {key} in {label}"))
}

fn is_row(line: &str) -> bool {
    line.split(' ').next().is_some_and(|f| f.parse::<i64>().is_ok())
}

fn assert_rows(label: &str, ids: &[EntityId], level: &mut ServerLevel, skip: usize, take: usize) {
    let expected: Vec<String> = oracle(label).into_iter().filter(|line| is_row(line)).collect();
    assert!(expected.len() >= skip + take.min(1), "no oracle rows for {label}");
    for (tick, line) in expected.iter().enumerate().skip(skip).take(take) {
        level.tick();
        let parts: Vec<&str> = line.split(" | ").collect();
        for (i, &id) in ids.iter().enumerate() {
            let actual = if level.entity_ref(id).is_some() { row(level, id) } else { "gone".to_string() };
            assert_eq!(normalize(&actual), normalize(parts[i]), "{label} tick {} entity {i}", tick + 1);
        }
    }
}

fn sorted(nbt: Nbt) -> Nbt {
    match nbt {
        Nbt::Compound(mut entries) => {
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Nbt::Compound(entries.into_iter().map(|(k, v)| (k, sorted(v))).collect())
        }
        Nbt::List(values) => Nbt::List(values.into_iter().map(sorted).collect()),
        other => other,
    }
}

fn without(nbt: Nbt, keys: &[&str]) -> Nbt {
    match nbt {
        Nbt::Compound(entries) => Nbt::Compound(entries.into_iter().filter(|(k, _)| !keys.contains(&k.as_str())).collect()),
        other => other,
    }
}

fn saved(level: &ServerLevel, id: EntityId) -> Nbt {
    let mut out = Vec::new();
    level.save_without_id(id, &mut out);
    Nbt::Compound(out)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn offers_packet(level: &mut ServerLevel, id: EntityId, container_id: i32) -> String {
    let offers = level.get_offers(id).clone();
    let mut w = Writer::new();
    w.varint(container_id);
    offers.write(&mut w);
    w.varint(villager::get_villager_data(level.entity(id)).level);
    w.varint(level.get_villager_xp(id));
    w.bool(level.show_progress_bar(id));
    w.bool(level.can_restock(id));
    hex(w.as_slice())
}

fn item_key(stack: &ItemStack) -> String {
    let name = stack.get_item().name();
    if name.contains(':') { name.to_string() } else { format!("minecraft:{name}") }
}

fn menu(level: &mut ServerLevel, player: EntityId) -> String {
    let menu = level.take_container_menu(player).unwrap();
    let mut s = "menu".to_string();
    for i in 0..menu.slots.len() {
        let stack = level.slot_get(&menu, player, i);
        if !stack.is_empty() {
            s += &format!(" {i}={}x{}", stack.count, item_key(&stack));
        }
    }
    level.restore_container_menu(player, menu);
    s
}

fn click(level: &mut ServerLevel, player: EntityId, slot: i32, button: i32, input: i32) {
    let mut menu = level.take_container_menu(player).unwrap();
    level.menu_clicked(&mut menu, player, slot, button, input).unwrap();
    level.menu_broadcast_changes(&mut menu, player);
    level.restore_container_menu(player, menu);
}

fn orbs(level: &ServerLevel) -> Vec<String> {
    let mut orbs: Vec<(EntityId, String)> = level
        .entities
        .iter()
        .filter_map(|(&id, entity)| match entity {
            LevelEntity::ExperienceOrb(orb) => Some((id, format!("orb {} {:?} {:?} {:?}", orb.value(), orb.entity.x(), orb.entity.y(), orb.entity.z()))),
            _ => None,
        })
        .collect();
    orbs.sort_by_key(|(id, _)| *id);
    orbs.into_iter().map(|(_, s)| s).collect()
}

fn uuid_tag(level: &ServerLevel, id: EntityId) -> Nbt {
    let Nbt::Compound(entries) = saved(level, id) else { unreachable!() };
    entries.into_iter().find(|(k, _)| k == "UUID").unwrap().1
}

fn replaced(nbt: Nbt, from: &Nbt, to: &Nbt) -> Nbt {
    match nbt {
        nbt if nbt == *from => to.clone(),
        Nbt::Compound(entries) => Nbt::Compound(entries.into_iter().map(|(k, v)| (k, replaced(v, from, to))).collect()),
        Nbt::List(values) => Nbt::List(values.into_iter().map(|v| replaced(v, from, to)).collect()),
        other => other,
    }
}

fn assert_saved(level: &ServerLevel, villager: EntityId, expected: &str, probe: &[(Nbt, Nbt)]) {
    let mut expected = parse_snbt(expected).unwrap();
    for (from, to) in probe {
        expected = replaced(expected, from, to);
    }
    let ignored = ["UUID"];
    assert_eq!(sorted(without(saved(level, villager), &ignored)), sorted(without(expected, &ignored)));
}

fn oracle_values(label: &str, key: &str) -> Vec<String> {
    oracle(label).into_iter().filter_map(|line| line.strip_prefix(&format!("{key} ")).map(str::to_string)).collect()
}

fn claim_job() -> (ServerLevel, EntityId, EntityId) {
    let (mut level, player) = duel_level();
    level.game_time = 1000;
    level.set_block(BlockPos::new(12, -60, 0), blocks::LECTERN.default_state(), UPDATE_ALL);
    let villager = summon(&mut level, "minecraft:villager", Vec3::new(8.5, -60.0, 0.5), r#"Tags:["v"],Rotation:[0f,0f],PersistenceRequired:1b"#);
    seed(&mut level, villager, 42, 1);
    seed_level(&mut level, 7);
    assert_rows("job", &[villager], &mut level, 0, 33);
    seed(&mut level, villager, 43, 1);
    assert_rows("job", &[villager], &mut level, 33, usize::MAX);
    (level, player, villager)
}

#[test]
fn villagers_claim_a_job_site_like_vanilla() {
    let (mut level, _player, villager) = claim_job();
    assert_saved(&level, villager, &oracle_value("job", "nbt"), &[]);
    let offers = level.get_offers(villager).to_nbt();
    assert_eq!(sorted(offers), sorted(parse_snbt(&oracle_value("job", "offers")).unwrap()));
    assert_eq!(offers_packet(&mut level, villager, 1), oracle_value("job", "bytes"));
}

#[test]
fn trading_levels_villagers_up_like_vanilla() {
    let (mut level, player, villager) = claim_job();
    trade(&mut level, player, villager);
}

#[test]
fn villagers_restock_and_turn_into_witches_like_vanilla() {
    let (mut level, player, villager) = claim_job();
    trade(&mut level, player, villager);
    let restocks = oracle_values("restock", "restock");
    for expected in &restocks {
        let should = level.should_restock(villager);
        if should {
            level.restock(villager);
        }
        assert_eq!(should.to_string(), *expected);
    }
    assert_eq!(sorted(level.get_offers(villager).to_nbt()), sorted(parse_snbt(&oracle_value("restock", "offers")).unwrap()));
    assert_eq!(offers_packet(&mut level, villager, 2), oracle_value("restock", "bytes"));
    assert_rows("restock", &[villager], &mut level, 0, usize::MAX);
    let probe = parse_snbt(PROBE_UUID).unwrap();
    assert_saved(&level, villager, &oracle_value("restock", "nbt"), &[(probe, uuid_tag(&level, player))]);
    let saved_villager = level.save_entity(villager).unwrap();
    let loaded = level.load_entity(&saved_villager).unwrap();
    let mut out = Vec::new();
    loaded.save_without_id(&mut out);
    let reloaded = Nbt::Compound(out);
    assert_eq!(reloaded.get("VillagerDataFinalized"), Some(&Nbt::Byte(1)));
    assert_eq!(sorted(without(reloaded, &["VillagerDataFinalized"])), sorted(without(saved(&level, villager), &["VillagerDataFinalized"])));
    let pos = level.entity(villager).position();
    summon(&mut level, "minecraft:lightning_bolt", pos, "");
    for _ in 0..3 {
        level.tick();
    }
    let count = |level: &ServerLevel, entity_type: &EntityType| level.entities.values().filter(|e| !e.entity().is_removed() && e.entity().entity_type == entity_type.id).count();
    assert_eq!(format!("minecraft:villager {}", count(&level, &entity_types::VILLAGER)), oracle_values("witch", "count")[0]);
    assert_eq!(format!("minecraft:witch {}", count(&level, &entity_types::WITCH)), oracle_values("witch", "count")[1]);
    let witch = *level.entities.iter().find(|(_, e)| e.entity().entity_type == entity_types::WITCH.id).unwrap().0;
    let random = ["UUID", "Motion", "Pos", "Rotation", "Fire", "attributes", "equipment", "drop_chances", "active_effects", "Brain"];
    let expected = without(parse_snbt(&oracle_value("witch", "nbt")).unwrap(), &random);
    assert_eq!(sorted(without(saved(&level, witch), &random)), sorted(expected));
}

const PROBE_UUID: &str = "[I;-1350628206,1717452137,-1191405020,-716073933]";

fn trade(level: &mut ServerLevel, player: EntityId, villager: EntityId) {
    level.teleport_to(player, 12.5, -60.0, 3.5);
    level.player_mut(player).inventory.add(ItemStack::new(items::EMERALD, 128));
    let location = level.entity(villager).position();
    assert!(level.interact_on(player, villager, InteractionHand::MainHand, location).consumes_action());
    assert_eq!(level.container_menu_id(player), 1);
    assert_rows("trade", &[villager], level, 0, 5);
    let menus = oracle_values("trade", "menu");
    let menus: Vec<String> = menus.iter().map(|m| format!("menu {}", m.rsplit_once(" carried=").unwrap().0)).collect();
    level.handle_select_trade(player, 1);
    assert_eq!(menu(level, player), menus[0]);
    click(level, player, 2, 0, QUICK_MOVE);
    assert_eq!(menu(level, player), menus[1]);
    assert_eq!(orbs(level).iter().map(|o| o.split(' ').nth(1).unwrap().to_string()).collect::<Vec<_>>(), oracle_values("trade", "orb").iter().map(|o| o.split(' ').next().unwrap().to_string()).collect::<Vec<_>>());
    assert_eq!(orbs(level).iter().map(|o| o.split(' ').skip(2).map(|f| format!("{:?}", f.parse::<f64>().unwrap())).collect::<Vec<_>>()).collect::<Vec<_>>(), oracle_values("trade", "orb").iter().map(|o| o.split(' ').skip(1).map(|f| format!("{:?}", f.parse::<f64>().unwrap())).collect::<Vec<_>>()).collect::<Vec<_>>());
    level.handle_select_trade(player, 1);
    assert_eq!(menu(level, player), menus[2]);
    click(level, player, 2, 0, QUICK_MOVE);
    assert_eq!(menu(level, player), menus[3]);
    let all_offers = oracle_values("trade", "offers");
    let all_bytes = oracle_values("trade", "bytes");
    assert_eq!(sorted(level.get_offers(villager).to_nbt()), sorted(parse_snbt(&all_offers[0]).unwrap()));
    assert_eq!(offers_packet(level, villager, 1), all_bytes[0]);
    assert_rows("trade", &[villager], level, 5, 5);
    level.close_container(player);
    assert_rows("trade", &[villager], level, 10, usize::MAX);
    let probe = parse_snbt(PROBE_UUID).unwrap();
    assert_saved(level, villager, &oracle_value("trade", "nbt"), &[(probe, uuid_tag(level, player))]);
    assert_eq!(sorted(level.get_offers(villager).to_nbt()), sorted(parse_snbt(&all_offers[1]).unwrap()));
    assert_eq!(offers_packet(level, villager, 2), all_bytes[1]);
}

