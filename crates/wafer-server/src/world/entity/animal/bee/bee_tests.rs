use wafer_core::{BlockPos, Vec3};
use wafer_data::blocks;
use wafer_data::properties::LEVEL_HONEY;
use wafer_protocol::nbt::Nbt;

use super::bee::{self, Bee};
use crate::server::level::{ServerLevel, testing};
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::level::BlockGetter;
use crate::world::level::block::beehive_block;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::block::entity::beehive_block_entity::{BeeReleaseStatus, Occupant};
use crate::world::level::update_flags::UPDATE_ALL;

const HIVE: BlockPos = BlockPos::new(0, -60, 0);

fn add_bee(level: &mut ServerLevel, pos: Vec3, data: Vec<(String, Nbt)>) -> EntityId {
    let mut mob = Bee::create();
    mob.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    mob.persistence_required = true;
    Bee::read_additional_save_data(&mut mob, &Nbt::Compound(data));
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

fn occupant_count(level: &mut ServerLevel) -> i32 {
    match &level.get_block_entity(HIVE).expect("a beehive").kind {
        BlockEntityKind::Beehive(beehive) => beehive.get_occupant_count(),
        _ => unreachable!("a beehive"),
    }
}

fn bees(level: &ServerLevel) -> Vec<EntityId> {
    level.entities.values().filter_map(|e| match e {
        LevelEntity::Mob(mob) if bee::is_bee(mob) && !mob.entity.is_removed() => Some(mob.entity.id),
        _ => None,
    }).collect()
}

#[test]
fn bees_with_nectar_enter_their_hive_and_make_honey() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(20.5, -60.0, 20.5), 0.0, 0.0);
    level.set_block(HIVE, blocks::BEEHIVE.default_state(), UPDATE_ALL);
    let data = vec![("HasNectar".to_string(), Nbt::Byte(1)), ("hive_pos".to_string(), Nbt::IntArray(vec![HIVE.x, HIVE.y, HIVE.z]))];
    let id = add_bee(&mut level, Vec3::new(0.5, -60.0, -0.6), data);
    for _ in 0..200 {
        level.tick();
        if level.entity_ref(id).is_none_or(|e| e.is_removed()) {
            break;
        }
    }
    assert!(level.entity_ref(id).is_none_or(|e| e.is_removed()), "the bee never entered its hive");
    assert_eq!(occupant_count(&mut level), 1);
    for _ in 0..3000 {
        level.tick();
        if occupant_count(&mut level) == 0 {
            break;
        }
    }
    assert_eq!(occupant_count(&mut level), 0, "the bee never left its hive");
    assert!(level.block_state(HIVE).get(LEVEL_HONEY) >= 1);
    let released = bees(&level);
    assert_eq!(released.len(), 1);
    assert!(!bee::has_nectar(level.entity(released[0])));
}

#[test]
fn emptying_a_hive_angers_the_bees_at_the_player() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, -2.5), 0.0, 0.0);
    level.set_block(HIVE, blocks::BEEHIVE.default_state().set(LEVEL_HONEY, 5), UPDATE_ALL);
    if let BlockEntityKind::Beehive(beehive) = &mut level.get_block_entity(HIVE).expect("a beehive").kind {
        beehive.store_bee(Occupant::create(0));
        beehive.store_bee(Occupant::create(0));
    }
    let state = level.block_state(HIVE);
    beehive_block::release_bees_and_reset_honey_level(&mut level, state, HIVE, Some(player), BeeReleaseStatus::Emergency);
    assert_eq!(level.block_state(HIVE).get(LEVEL_HONEY), 0);
    assert_eq!(occupant_count(&mut level), 0);
    let released = bees(&level);
    assert_eq!(released.len(), 2);
    for bee in released {
        assert_eq!(level.get_target(bee), Some(player));
        assert!(level.entity(bee).is_no_gravity());
    }
}

#[test]
fn angry_bees_sting_once_and_lose_their_anger() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let id = add_bee(&mut level, Vec3::new(2.5, -60.0, 0.5), Vec::new());
    level.set_target(id, Some(player));
    level.set_time_to_remain_angry(id, 600);
    for _ in 0..400 {
        level.tick();
        if bee::has_stung(level.entity(id)) {
            break;
        }
    }
    assert!(bee::has_stung(level.entity(id)), "the bee never stung");
    assert_eq!(level.get_stinger_count(player), 1);
    assert!(!level.is_angry(id));
}

#[test]
fn bees_save_vanilla_fields() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let id = add_bee(&mut level, Vec3::new(4.5, -60.0, 4.5), vec![("flower_pos".to_string(), Nbt::IntArray(vec![1, 2, 3]))]);
    let mut out = Vec::new();
    level.save_without_id(id, &mut out);
    let keys: Vec<&str> = out.iter().map(|(key, _)| key.as_str()).collect();
    for key in ["flower_pos", "HasNectar", "HasStung", "TicksSincePollination", "CannotEnterHiveTicks", "CropsGrownSincePollination", "anger_end_time", "Age"] {
        assert!(keys.contains(&key), "missing {key}");
    }
    assert!(!keys.contains(&"hive_pos"));
}

#[test]
fn bees_fly_around() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(20.5, -60.0, 20.5), 0.0, 0.0);
    let id = add_bee(&mut level, Vec3::new(0.5, -58.0, 0.5), Vec::new());
    let start = level.entity(id).position();
    for _ in 0..400 {
        level.tick();
    }
    assert!(level.entity(id).position().distance_to(start) > 1.0, "the bee never moved");
}
