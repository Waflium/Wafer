use wafer_data::fluid_tags;

use super::block_source::BlockSource;
use super::dispense_item_behavior::DispenseItemBehavior;
use crate::server::level::ServerLevel;
use crate::world::entity::entity_save::apply_default_stack_config;
use crate::world::entity::entity_type::EntityType;
use crate::world::entity::LevelEntity;
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;

pub fn execute(level: &mut ServerLevel, source: &BlockSource, mut dispensed: ItemStack) -> ItemStack {
    let direction = source.facing();
    let center = source.center();
    let entity_type = dispensed.item.name();
    let Some(type_info) = EntityType::by_name(entity_type) else { return dispensed };
    let just_outside_dispenser = 0.5625 + type_info.get_width() as f64 / 2.0;
    let spawn_x = center.x + direction.step_x() as f64 * just_outside_dispenser;
    let spawn_y = center.y + (direction.step_y() as f32 * 1.125) as f64;
    let spawn_z = center.z + direction.step_z() as f64 * just_outside_dispenser;
    let front_pos = source.pos.relative(direction);
    let y_offset = if level.fluid_state(front_pos).is(&fluid_tags::WATER) {
        1.0
    } else {
        if !level.block_state(front_pos).is_air() || !level.fluid_state(front_pos.below()).is(&fluid_tags::WATER) {
            return DispenseItemBehavior::Default.dispense(level, source, dispensed);
        }
        0.0
    };
    if let Some(mut entity) = ServerLevel::create_entity(entity_type)
        && let LevelEntity::Boat(boat) = &mut entity
    {
        boat.set_initial_pos(spawn_x, spawn_y + y_offset, spawn_z);
        apply_default_stack_config(&mut entity, &dispensed);
        entity.entity_mut().set_y_rot(direction.to_y_rot());
        level.add_fresh_entity(entity);
        dispensed.shrink(1);
    }
    dispensed
}
