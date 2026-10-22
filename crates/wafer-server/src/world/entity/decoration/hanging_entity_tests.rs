use wafer_core::{BlockPos, Direction, Vec3};
use wafer_data::{blocks, items};

use crate::server::level::testing;
use crate::world::entity::LevelEntity;
use crate::world::entity::decoration::item_frame::ItemFrame;
use crate::world::entity::decoration::painting::Painting;
use crate::world::item::ItemStack;
use crate::world::level::update_flags::UPDATE_ALL;

#[test]
fn hanging_entities_sit_on_walls_and_feed_comparators() {
    let mut level = testing::level();
    for y in -61..=-56 {
        for z in 0..=10 {
            level.set_block(BlockPos::new(3, y, z), blocks::STONE.default_state(), UPDATE_ALL);
        }
    }
    let frame = ItemFrame::new(false, BlockPos::new(2, -60, 4), Direction::West);
    assert_eq!(frame.entity.position(), Vec3::new(2.96875, -59.5, 4.5));
    let id = frame.entity.id;
    level.add_fresh_entity(LevelEntity::ItemFrame(frame));
    assert!(level.item_frame_survives(id));
    assert_eq!(level.item_frame_get_analog_output(id), 0);
    level.item_frame_set_item(id, ItemStack::new(items::DIAMOND, 3), true);
    assert_eq!(level.item_frame_get_analog_output(id), 1);

    let painting = Painting::create(&mut level, BlockPos::new(2, -58, 8), Direction::West).expect("a painting fits");
    let bounding_box = painting.entity.bounding_box();
    assert_eq!(bounding_box.max_x - bounding_box.min_x, 0.0625);
    let painting_id = painting.entity.id;
    level.add_fresh_entity(LevelEntity::Painting(painting));
    assert!(level.hanging_entity_survives(painting_id));
    level.set_block(BlockPos::new(3, -58, 8), blocks::AIR.default_state(), UPDATE_ALL);
    assert!(!level.hanging_entity_survives(painting_id));
}
