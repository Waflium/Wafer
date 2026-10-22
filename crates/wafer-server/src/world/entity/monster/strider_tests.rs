use wafer_core::{BlockPos, Vec3};
use wafer_data::blocks;

use super::strider::{self, Strider};
use crate::server::level::{ServerLevel, testing};
use crate::world::entity::attributes::Attribute;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::level::update_flags::UPDATE_ALL;

fn add_strider(level: &mut ServerLevel, pos: Vec3) -> EntityId {
    let mut mob = Strider::create();
    mob.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    mob.persistence_required = true;
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

fn fill_lava(level: &mut ServerLevel) {
    for x in 10..=20 {
        for z in 10..=20 {
            for y in -63..=-61 {
                level.set_block(BlockPos::new(x, y, z), blocks::LAVA.default_state(), UPDATE_ALL);
            }
        }
    }
}

#[test]
fn striders_stand_on_lava() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    fill_lava(&mut level);
    let id = add_strider(&mut level, Vec3::new(15.5, -59.0, 15.5));
    let mut landed = false;
    for _ in 0..40 {
        level.tick();
        let e = level.entity(id);
        landed |= e.on_ground && e.y() == -60.5;
        assert!(e.y() >= -60.5, "the strider sank to {}", e.y());
    }
    assert!(landed, "the strider never stood on the lava");
    assert!(!strider::is_suffocating(level.entity(id)));
}

#[test]
fn striders_suffocate_away_from_lava() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let id = add_strider(&mut level, Vec3::new(4.5, -60.0, 4.5));
    level.tick();
    assert!(strider::is_suffocating(level.entity(id)));
    assert_eq!(level.attribute_value(id, Attribute::MovementSpeed), 0.175f32 as f64 * (1.0 + -0.34f32 as f64));
}
