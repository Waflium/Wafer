use wafer_core::Vec3;
use wafer_core::random::LegacyRandomSource;
use wafer_data::{blocks, game_rule, items};
use wafer_protocol::nbt::Nbt;

use super::goat;
use crate::server::level::{ServerLevel, testing};
use crate::world::InteractionHand;
use crate::world::entity::EntityId;
use crate::world::entity::ai::behavior::long_jump_to_random_pos;
use crate::world::entity::schedule::Activity;
use crate::world::item::ItemStack;
use crate::world::item::component::custom::parse_snbt;

const COOLED_DOWN: &str = r#"Brain:{memories:{"minecraft:ram_cooldown_ticks":{value:6000},"minecraft:long_jump_cooling_down":{value:6000}}}"#;

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

fn seed(level: &mut ServerLevel, goat: EntityId, seed: i64, sensor_delay: i64) {
    let mob = level.mob_mut(goat).unwrap();
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
    if let Some(mob) = level.mob(id)
        && let crate::world::entity::mob::MobKind::Goat(_) = mob.kind
    {
        let activity = mob.living.brain.get_active_non_core_activity().map_or("-", Activity::get_name);
        s += &format!(" {activity} {} {} {:?}", goat::has_left_horn(e), goat::has_right_horn(e), e.pose());
    }
    s
}

fn java_row(line: &str) -> Vec<String> {
    line.split(" | ")
        .map(|part| {
            let fields: Vec<&str> = part.split(' ').collect();
            let keep = if fields.len() > 10 { fields.len() - 1 } else { fields.len() };
            fields[..keep].join(" ")
        })
        .collect()
}

fn normalize(row: &str) -> Vec<String> {
    row.split(' ')
        .enumerate()
        .map(|(i, f)| match f.parse::<f64>() {
            Ok(v) if i == 4 || i == 5 => format!("{:?}", v as f32),
            Ok(v) if i > 0 => format!("{v:?}"),
            _ => f.to_lowercase().replace('_', ""),
        })
        .collect()
}

fn oracle(label: &str) -> Vec<String> {
    let text = include_str!("goat_oracle.txt");
    text.lines().skip_while(|l| *l != format!("== {label}")).skip(1).take_while(|l| !l.starts_with("==")).map(str::to_string).collect()
}

fn assert_rows(label: &str, ids: &[EntityId], level: &mut ServerLevel) {
    assert_rows_from(label, ids, level, 0, usize::MAX);
}

fn assert_rows_from(label: &str, ids: &[EntityId], level: &mut ServerLevel, skip: usize, take: usize) {
    let expected = oracle(label);
    assert!(!expected.is_empty(), "no oracle rows for {label}");
    for (tick, line) in expected.iter().enumerate().skip(skip).take(take) {
        level.tick();
        let expected = java_row(line);
        for (i, &id) in ids.iter().enumerate() {
            let actual = if level.entity_ref(id).is_some() { row(level, id) } else { "gone".to_string() };
            assert_eq!(normalize(&actual), normalize(&expected[i]), "{label} tick {} entity {i}", tick + 1);
        }
    }
}

fn assert_rows_close(label: &str, id: EntityId, level: &mut ServerLevel, skip: usize, tolerance: f64) {
    for (tick, line) in oracle(label).iter().enumerate().skip(skip) {
        level.tick();
        let expected = normalize(&java_row(line)[0]);
        let actual = normalize(&row(level, id));
        for (i, (a, e)) in actual.iter().zip(&expected).enumerate() {
            match (a.parse::<f64>(), e.parse::<f64>()) {
                (Ok(a), Ok(e)) if i > 0 => assert!((a - e).abs() <= tolerance, "{label} tick {} field {i}: {a} vs {e}", tick + 1),
                _ => assert_eq!(a, e, "{label} tick {} field {i}", tick + 1),
            }
        }
    }
}

fn assert_cooldowns(level: &ServerLevel, goat: EntityId, long_jump: i32, ram: i32) {
    use crate::world::entity::ai::memory::memory_module_type;
    let brain = level.get_brain(goat);
    assert_eq!(brain.get_memory(memory_module_type::LONG_JUMP_COOLDOWN_TICKS), Some(&long_jump));
    assert_eq!(brain.get_memory(memory_module_type::RAM_COOLDOWN_TICKS), Some(&ram));
    let saved = level.mob(goat).unwrap().living.brain.pack().to_nbt();
    let expected = parse_snbt(&format!(r#"{{memories:{{"minecraft:long_jump_cooling_down":{{value:{long_jump}}},"minecraft:ram_cooldown_ticks":{{value:{ram}}}}}}}"#)).unwrap();
    assert_eq!(sorted(saved), sorted(expected));
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

fn fill(level: &mut ServerLevel, from: (i32, i32, i32), to: (i32, i32, i32), block: wafer_data::Block) {
    use crate::world::level::update_flags::UPDATE_ALL;
    for x in from.0..=to.0 {
        for y in from.1..=to.1 {
            for z in from.2..=to.2 {
                level.set_block(wafer_core::BlockPos::new(x, y, z), block.default_state(), UPDATE_ALL);
            }
        }
    }
}

#[test]
fn tempted_goats_walk_vanillas_path() {
    let (mut level, player) = duel_level();
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::WHEAT, 1)));
    let goat = summon(&mut level, "minecraft:goat", Vec3::new(8.5, -60.0, 0.5), &format!(r#"Tags:["g"],Rotation:[0f,0f],PersistenceRequired:1b,{COOLED_DOWN}"#));
    seed(&mut level, goat, 42, 1);
    seed_level(&mut level, 7);
    assert_rows("tempt", &[goat], &mut level);
    assert_cooldowns(&level, goat, 5840, 5840);
}

const RAM_READY_SOON: &str = r#"Brain:{memories:{"minecraft:long_jump_cooling_down":{value:6000},"minecraft:ram_cooldown_ticks":{value:5}}}"#;

#[test]
fn goats_ram_like_vanilla() {
    let (mut level, _player) = duel_level();
    let cow = summon(&mut level, "minecraft:cow", Vec3::new(0.5, -60.0, 26.5), r#"Tags:["c"],Rotation:[0f,0f],PersistenceRequired:1b,NoAI:1b"#);
    let goat = summon(&mut level, "minecraft:goat", Vec3::new(0.5, -60.0, 20.5), &format!(r#"Tags:["g"],Rotation:[0f,0f],PersistenceRequired:1b,{RAM_READY_SOON}"#));
    seed(&mut level, goat, 42, 1);
    seed_level(&mut level, 7);
    assert_rows("ram", &[goat, cow], &mut level);
    assert_eq!(level.mob(cow).unwrap().health, 8.0);
    assert_cooldowns(&level, goat, 5880, 2180);
}

#[test]
fn goats_charge_into_blocks_like_vanilla() {
    use crate::world::level::update_flags::UPDATE_ALL;
    let (mut level, _player) = duel_level();
    level.set_block(wafer_core::BlockPos::new(0, -60, 27), wafer_data::blocks::STONE.default_state(), UPDATE_ALL);
    let cow = summon(&mut level, "minecraft:cow", Vec3::new(0.5, -60.0, 26.5), r#"Tags:["c2"],Rotation:[0f,0f],PersistenceRequired:1b,NoAI:1b"#);
    let goat = summon(&mut level, "minecraft:goat", Vec3::new(0.5, -60.0, 20.5), &format!(r#"Tags:["g2"],Rotation:[0f,0f],PersistenceRequired:1b,{RAM_READY_SOON}"#));
    seed(&mut level, goat, 42, 1);
    seed_level(&mut level, 7);
    assert_rows_from("horn", &[goat, cow], &mut level, 0, 36);
    level.teleport_to(cow, 0.5, -60.0, 60.5);
    assert_rows_from("horn", &[goat], &mut level, 36, usize::MAX);
    assert_cooldowns(&level, goat, 5924, 2181);
}

const LONG_JUMP_READY_SOON: &str = r#"Brain:{memories:{"minecraft:ram_cooldown_ticks":{value:6000},"minecraft:long_jump_cooling_down":{value:5}}}"#;

#[test]
fn goats_long_jump_off_pillars_like_vanilla() {
    let (mut level, _player) = duel_level();
    fill(&mut level, (8, -60, 8), (8, -56, 8), blocks::STONE);
    let goat = summon(&mut level, "minecraft:goat", Vec3::new(8.5, -55.0, 8.5), &format!(r#"Tags:["g"],Rotation:[0f,0f],PersistenceRequired:1b,{LONG_JUMP_READY_SOON}"#));
    seed(&mut level, goat, 42, 1);
    seed_level(&mut level, 7);
    assert_rows_from("jump", &[goat], &mut level, 0, 46);
    assert_rows_close("jump", goat, &mut level, 46, 1.0e-12);
    assert_cooldowns(&level, goat, 817, 5840);
}

#[test]
fn goats_breed_like_vanilla() {
    use crate::world::entity::attributes::Attribute;
    use crate::world::entity::mob::MobKind;
    let (mut level, _player) = duel_level();
    let a = summon(&mut level, "minecraft:goat", Vec3::new(0.5, -60.0, 8.5), &format!(r#"Tags:["a"],Rotation:[0f,0f],PersistenceRequired:1b,InLove:600,{COOLED_DOWN}"#));
    let b = summon(&mut level, "minecraft:goat", Vec3::new(4.5, -60.0, 8.5), &format!(r#"Tags:["b"],Rotation:[0f,0f],PersistenceRequired:1b,InLove:600,{COOLED_DOWN}"#));
    seed(&mut level, a, 42, 1);
    seed(&mut level, b, 43, 1);
    seed_level(&mut level, 7);
    assert_rows_from("breed", &[a, b], &mut level, 0, 77);
    let babies: Vec<EntityId> = level.entities.keys().copied().filter(|&id| level.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::Goat(_)) && mob.is_baby())).collect();
    assert_eq!(babies.len(), 1);
    let baby = babies[0];
    let ticks_until_vanillas_dump = 140 - 77;
    assert_cooldowns(&level, baby, 1119 + ticks_until_vanillas_dump, 5590 + ticks_until_vanillas_dump);
    let mob = level.mob(baby).unwrap();
    assert_eq!(mob.ageable().unwrap().age, -23934 - ticks_until_vanillas_dump);
    assert!(!goat::is_screaming_goat(&mob.entity));
    assert_eq!(mob.living.attributes.base_value(Attribute::AttackDamage), 1.0);
}

fn instrument(stack: &ItemStack) -> String {
    format!("{} {:?}", wafer_data::registry::ITEM[stack.get_item().0 as usize], stack.get(crate::world::item::component::INSTRUMENT))
}

#[test]
fn horns_drop_like_vanilla() {
    let (mut level, _player) = duel_level();
    let r1 = summon(&mut level, "minecraft:goat", Vec3::new(2.5, -60.0, 0.5), r#"Tags:["r1"],UUID:[I;1,2,3,4],NoAI:1b"#);
    let r2 = summon(&mut level, "minecraft:goat", Vec3::new(2.5, -60.0, 2.5), r#"Tags:["r2"],UUID:[I;-17,99,123456789,-5],NoAI:1b"#);
    let s1 = summon(&mut level, "minecraft:goat", Vec3::new(2.5, -60.0, 4.5), r#"Tags:["s1"],UUID:[I;1,2,3,5],IsScreamingGoat:1b,NoAI:1b"#);
    let s2 = summon(&mut level, "minecraft:goat", Vec3::new(2.5, -60.0, 6.5), r#"Tags:["s2"],UUID:[I;77,-88,99,-1010],IsScreamingGoat:1b,NoAI:1b"#);
    let l = summon(&mut level, "minecraft:goat", Vec3::new(2.5, -60.0, 8.5), r#"Tags:["l"],UUID:[I;5,6,7,8],HasRightHorn:0b,NoAI:1b"#);
    let n = summon(&mut level, "minecraft:goat", Vec3::new(2.5, -60.0, 10.5), r#"Tags:["n"],UUID:[I;5,6,7,9],HasRightHorn:0b,HasLeftHorn:0b,NoAI:1b"#);
    let k = summon(&mut level, "minecraft:goat", Vec3::new(2.5, -60.0, 12.5), r#"Tags:["k"],Age:-100,NoAI:1b"#);
    let seek = r#"minecraft:goat_horn Some(String("minecraft:seek_goat_horn"))"#;
    let yearn = r#"minecraft:goat_horn Some(String("minecraft:yearn_goat_horn"))"#;
    assert_eq!(instrument(&level.create_horn(r1)), seek);
    assert_eq!(instrument(&level.create_horn(r2)), seek);
    assert_eq!(instrument(&level.create_horn(s1)), yearn);
    assert_eq!(instrument(&level.create_horn(s2)), yearn);
    let mut drop = |level: &mut ServerLevel, goat: EntityId, seed: i64| {
        level.entity_mut(goat).random = LegacyRandomSource::new(seed);
        let dropped = level.goat_drop_horn(goat);
        let e = level.entity(goat);
        (dropped, goat::has_left_horn(e), goat::has_right_horn(e))
    };
    assert_eq!(drop(&mut level, r1, 11), (true, false, true));
    assert_eq!(drop(&mut level, r2, 12), (true, false, true));
    assert_eq!(drop(&mut level, r2, 13), (true, false, false));
    assert_eq!(drop(&mut level, r2, 14), (false, false, false));
    assert_eq!(drop(&mut level, l, 15), (true, false, false));
    assert_eq!(drop(&mut level, n, 16), (false, false, false));
    assert_eq!(drop(&mut level, k, 17), (false, true, true));
    let items: Vec<(String, Vec3, Vec3)> = level
        .entities
        .values()
        .filter_map(|e| match e {
            crate::world::entity::LevelEntity::Item(item) => Some((instrument(&item.item), item.entity.position(), item.entity.delta_movement)),
            _ => None,
        })
        .collect();
    let expected = [
        (Vec3::new(2.5, -60.0, 0.5), Vec3::new(0.08586658537387848, 0.47093886137008667, -0.11439710110425949)),
        (Vec3::new(2.5, -60.0, 2.5), Vec3::new(-0.176068976521492, 0.4100276529788971, -0.11220794171094894)),
        (Vec3::new(2.5, -60.0, 2.5), Vec3::new(0.09192128479480743, 0.4315439462661743, -0.02215459942817688)),
        (Vec3::new(2.5, -60.0, 8.5), Vec3::new(0.09199295938014984, 0.6163181066513062, -0.15779009461402893)),
    ];
    assert_eq!(items.len(), expected.len());
    for ((name, pos, delta), (expected_pos, expected_delta)) in items.iter().zip(expected) {
        assert_eq!(name, seek);
        assert_eq!(*pos, expected_pos);
        assert_eq!(*delta, expected_delta);
    }
}

#[test]
fn goats_are_milked_and_fed_like_vanilla() {
    use crate::world::InteractionResult;
    let (mut level, player) = duel_level();
    let r1 = summon(&mut level, "minecraft:goat", Vec3::new(2.5, -60.0, 0.5), r#"Tags:["r1"],NoAI:1b"#);
    let s1 = summon(&mut level, "minecraft:goat", Vec3::new(2.5, -60.0, 4.5), r#"Tags:["s1"],IsScreamingGoat:1b,NoAI:1b"#);
    let k = summon(&mut level, "minecraft:goat", Vec3::new(2.5, -60.0, 12.5), r#"Tags:["k"],Age:-100,NoAI:1b"#);
    let mut interact = |level: &mut ServerLevel, goat: EntityId, item: wafer_data::Item, count: i32| {
        level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(item, count)));
        let location = level.entity(goat).position();
        let result = level.interact_on(player, goat, InteractionHand::MainHand, location);
        let held = level.player(player).item_in_hand(InteractionHand::MainHand).map(|s| (s.get_item(), s.count));
        let milk = level.player(player).inventory.slots.iter().flatten().filter(|s| s.is(items::MILK_BUCKET)).map(|s| s.count).sum::<i32>();
        (result, held, milk)
    };
    assert_eq!(interact(&mut level, r1, items::BUCKET, 2), (InteractionResult::SUCCESS, Some((items::BUCKET, 1)), 1));
    assert_eq!(interact(&mut level, r1, items::BUCKET, 1), (InteractionResult::SUCCESS, Some((items::MILK_BUCKET, 1)), 2));
    assert_eq!(interact(&mut level, k, items::BUCKET, 1), (InteractionResult::Pass, Some((items::BUCKET, 1)), 1));
    assert_eq!(interact(&mut level, s1, items::WHEAT, 3), (InteractionResult::SUCCESS_SERVER, Some((items::WHEAT, 2)), 1));
    assert!(level.is_in_love(s1));
    assert_eq!(interact(&mut level, k, items::WHEAT, 3), (InteractionResult::SUCCESS, Some((items::WHEAT, 2)), 1));
    assert!(!level.is_in_love(k));
    assert_eq!(level.mob(k).unwrap().ageable().unwrap().age, -100);
}

#[test]
fn summoned_goats_save_like_vanilla() {
    let (mut level, _player) = duel_level();
    let r2 = summon(&mut level, "minecraft:goat", Vec3::new(2.5, -60.0, 2.5), r#"Tags:["r2"],UUID:[I;-17,99,123456789,-5],NoAI:1b,HasLeftHorn:0b,HasRightHorn:0b"#);
    let k = summon(&mut level, "minecraft:goat", Vec3::new(2.5, -60.0, 12.5), r#"Tags:["k"],Age:-100,NoAI:1b"#);
    let save = |level: &ServerLevel, id: EntityId| {
        let mut out = Vec::new();
        level.entities.get(&id).unwrap().save_without_id(&mut out);
        Nbt::Compound(out.into())
    };
    let r2_nbt = save(&level, r2);
    assert_eq!(r2_nbt.get("Brain"), Some(&parse_snbt("{memories:{}}").unwrap()));
    assert_eq!(r2_nbt.get("HasLeftHorn"), Some(&Nbt::Byte(0)));
    assert_eq!(r2_nbt.get("HasRightHorn"), Some(&Nbt::Byte(0)));
    assert_eq!(r2_nbt.get("IsScreamingGoat"), Some(&Nbt::Byte(0)));
    assert_eq!(r2_nbt.get("attributes"), Some(&Nbt::List(Vec::new())));
    let k_nbt = save(&level, k);
    assert_eq!(k_nbt.get("attributes"), Some(&parse_snbt(r#"[{base:1.0d,id:"minecraft:attack_damage"}]"#).unwrap()));
    let Nbt::Compound(mut fields) = r2_nbt.clone() else { unreachable!() };
    fields.push(("id".into(), Nbt::String("minecraft:goat".into())));
    let loaded = level.load_entity(&Nbt::Compound(fields)).unwrap();
    let mut out = Vec::new();
    loaded.save_without_id(&mut out);
    assert_eq!(sorted(Nbt::Compound(out.into())), sorted(r2_nbt));
}

#[test]
fn goats_take_ten_less_fall_damage() {
    let (mut level, _player) = duel_level();
    let goat = summon(&mut level, "minecraft:goat", Vec3::new(2.5, -60.0, 2.5), r#"NoAI:1b"#);
    let cow = summon(&mut level, "minecraft:cow", Vec3::new(4.5, -60.0, 2.5), r#"NoAI:1b"#);
    assert_eq!(level.calculate_fall_damage(cow, 20.0, 1.0), 17);
    assert_eq!(level.calculate_fall_damage(goat, 20.0, 1.0), 7);
    assert!(level.calculate_fall_damage(goat, 10.0, 1.0) < 0);
}

#[test]
fn goats_give_up_long_jumps_out_of_pits_like_vanilla() {
    let (mut level, _player) = duel_level();
    fill(&mut level, (8, -62, 8), (8, -61, 8), blocks::AIR);
    let goat = summon(&mut level, "minecraft:goat", Vec3::new(8.5, -62.0, 8.5), &format!(r#"Tags:["g"],Rotation:[0f,0f],PersistenceRequired:1b,{LONG_JUMP_READY_SOON}"#));
    seed(&mut level, goat, 42, 1);
    seed_level(&mut level, 7);
    assert_rows("pit", &[goat], &mut level);
    assert_cooldowns(&level, goat, 167, 5840);
}
