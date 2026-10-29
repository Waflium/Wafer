use wafer_core::Vec3;

use crate::server::level::{ServerLevel, testing};
use crate::world::entity::EntityId;

fn add(level: &mut ServerLevel, pos: Vec3, nbt: wafer_protocol::nbt::Nbt) -> EntityId {
    let mut entity = ServerLevel::create_entity("minecraft:happy_ghast").expect("a happy ghast");
    entity.load(&nbt).expect("loads");
    entity.entity_mut().set_pos(pos);
    let id = entity.entity().id;
    level.add_fresh_entity(entity);
    id
}

#[test]
fn ghastlings_are_small_and_use_their_brain() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 8.5), 180.0, 0.0);
    let id = add(&mut level, Vec3::new(0.5, -55.0, 0.5), wafer_protocol::nbt::Nbt::Compound(vec![("Age".into(), wafer_protocol::nbt::Nbt::Int(-24000))]));
    assert!(level.is_baby(id));
    assert_eq!(level.entity(id).dimensions.width, 4.0 * 0.2375);
    assert!(level.mob(id).unwrap().ai.goal_selector.get_available_goals().is_empty());
    for _ in 0..40 {
        level.tick();
    }
    assert!(level.entity_ref(id).is_some());
}

#[test]
fn adult_happy_ghasts_float_and_save_their_still_timeout() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 8.5), 180.0, 0.0);
    let id = add(&mut level, Vec3::new(0.5, -50.0, 0.5), wafer_protocol::nbt::Nbt::Compound(vec![("still_timeout".into(), wafer_protocol::nbt::Nbt::Int(5))]));
    assert!(!level.is_baby(id));
    assert_eq!(level.mob(id).unwrap().ai.goal_selector.get_available_goals().len(), 3);
    assert!(level.happy_ghast_is_on_still_timeout(id));
    for _ in 0..80 {
        level.tick();
    }
    assert!(!level.happy_ghast_is_on_still_timeout(id));
    let mut out = Vec::new();
    level.save_without_id(id, &mut out);
    assert!(out.iter().any(|(k, v)| k == "still_timeout" && *v == wafer_protocol::nbt::Nbt::Int(0)));
}

#[test]
fn riding_a_happy_ghast_plays_the_goggle_sounds() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 8.5), 180.0, 0.0);
    let id = add(&mut level, Vec3::new(0.5, -55.0, 0.5), wafer_protocol::nbt::Nbt::Compound(Vec::new()));
    assert!(level.start_riding(player, id, true, true));
    assert!(level.is_vehicle(id));
    level.tick();
    level.stop_riding(player);
    assert!(!level.is_vehicle(id));
}
