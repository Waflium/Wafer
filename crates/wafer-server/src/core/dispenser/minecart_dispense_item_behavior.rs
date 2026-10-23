use wafer_core::{Direction, Vec3};
use wafer_data::properties::RailShape;
use wafer_data::{BlockState, block_tags};

use super::block_source::BlockSource;
use super::dispense_item_behavior::DispenseItemBehavior;
use crate::server::level::ServerLevel;
use crate::world::item::{ItemStack, minecart_item};
use crate::world::level::BlockGetter;
use crate::world::level::block::base_rail_block;

pub fn execute(level: &mut ServerLevel, source: &BlockSource, mut dispensed: ItemStack) -> ItemStack {
    let direction = source.facing();
    let center = source.center();
    let spawn_x = center.x + direction.step_x() as f64 * 1.125;
    let spawn_y = center.y.floor() + direction.step_y() as f64;
    let spawn_z = center.z + direction.step_z() as f64 * 1.125;
    let front = source.pos.relative(direction);
    let block_front = level.block_state(front);
    let y_offset = if block_front.is(&block_tags::RAILS) {
        if base_rail_block::is_slope(get_rail_shape(block_front)) { 0.6 } else { 0.1 }
    } else {
        if !block_front.is_air() {
            return DispenseItemBehavior::Default.dispense(level, source, dispensed);
        }
        let block_below = level.block_state(front.below());
        if !block_below.is(&block_tags::RAILS) {
            return DispenseItemBehavior::Default.dispense(level, source, dispensed);
        }
        if direction != Direction::Down && base_rail_block::is_slope(get_rail_shape(block_below)) { -0.4 } else { -0.9 }
    };
    let spawn_pos = Vec3::new(spawn_x, spawn_y + y_offset, spawn_z);
    if let Some(minecart) = minecart_item::create_minecart(level, spawn_pos, dispensed.item.name(), &dispensed) {
        level.add_fresh_entity(minecart);
        dispensed.shrink(1);
    }
    dispensed
}

fn get_rail_shape(block_front: BlockState) -> RailShape {
    if block_front.block().is_instance_of("BaseRailBlock") { block_front.get(base_rail_block::shape_property(block_front.block())) } else { RailShape::NorthSouth }
}
