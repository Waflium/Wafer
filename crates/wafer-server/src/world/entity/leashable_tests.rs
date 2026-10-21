use wafer_core::{BlockPos, Vec3};
use wafer_data::blocks;
use wafer_protocol::nbt::Nbt;

use crate::server::level::testing;
use crate::world::entity::animal::cow::cow::Cow;
use crate::world::entity::decoration::leash_fence_knot_entity::{self, LeashFenceKnotEntity};
use crate::world::entity::LevelEntity;
use crate::world::level::update_flags::UPDATE_ALL;

#[test]
fn leashes_save_their_holder_and_move_to_fence_knots() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let mut cow = Cow::create();
    cow.entity.snap_to(2.5, -60.0, 0.5, 0.0, 0.0);
    let cow = {
        let id = cow.entity.id;
        level.add_fresh_entity(LevelEntity::Mob(Box::new(cow)));
        id
    };
    level.set_leashed_to(cow, player, true);
    assert!(level.is_leashed(cow));
    let mut out = Vec::new();
    level.mob(cow).unwrap().save(&mut out);
    let leash = out.iter().find(|(k, _)| k == "leash").map(|(_, v)| v.clone()).unwrap();
    assert!(matches!(leash, Nbt::Compound(ref fields) if fields.iter().any(|(k, _)| k == "UUID")));
    level.set_block(BlockPos::new(0, -60, 3), blocks::OAK_FENCE.default_state(), UPDATE_ALL);
    let result = leash_fence_knot_entity::bind_player_mobs(&mut level, player, BlockPos::new(0, -60, 3));
    assert!(result.consumes_action());
    let knot = LeashFenceKnotEntity::get_knot(&level, BlockPos::new(0, -60, 3)).unwrap();
    assert_eq!(level.get_leash_holder(cow), Some(knot));
    let mut out = Vec::new();
    level.mob(cow).unwrap().save(&mut out);
    assert!(out.iter().any(|(k, v)| k == "leash" && *v == Nbt::IntArray(vec![0, -60, 3])));
    level.drop_leash(cow);
    assert!(!level.is_leashed(cow));
    assert!(level.entity_ref(knot).is_none_or(|e| e.is_removed()));
}
