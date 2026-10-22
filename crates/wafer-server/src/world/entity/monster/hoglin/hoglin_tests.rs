use wafer_core::Vec3;

use super::hoglin::Hoglin;
use crate::server::level::testing;
use crate::world::entity::LevelEntity;
use crate::world::entity::mob::MobKind;

#[test]
fn hoglins_turn_into_zoglins_in_the_overworld() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let mut mob = Hoglin::create();
    mob.entity.snap_to(2.5, -60.0, 4.5, 0.0, 0.0);
    mob.persistence_required = true;
    if let MobKind::Hoglin(hoglin) = &mut mob.kind {
        hoglin.time_in_overworld = 300;
    }
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    for _ in 0..3 {
        level.tick();
    }
    assert!(level.entity_ref(id).is_none_or(|e| e.is_removed()), "the hoglin is still there");
    let zoglin = level.entities.iter().find(|(_, e)| matches!(e, LevelEntity::Mob(m) if matches!(m.kind, MobKind::Zoglin(_)))).map(|(&id, _)| id);
    assert!(zoglin.is_some(), "no zoglin");
}
