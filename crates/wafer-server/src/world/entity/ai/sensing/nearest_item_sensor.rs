use crate::server::level::ServerLevel;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::entity::ai::memory::memory_module_type::{self, Erased, MemoryModuleType};

const XZ_RANGE: f64 = 32.0;
const Y_RANGE: f64 = 16.0;
pub const MAX_DISTANCE_TO_WANTED_ITEM: f64 = 32.0;

pub fn requires() -> Vec<MemoryModuleType<Erased>> {
    vec![memory_module_type::NEAREST_VISIBLE_WANTED_ITEM.erase()]
}

pub fn do_tick(level: &mut ServerLevel, body: EntityId) {
    let area = level.entity(body).bounding_box().inflate(XZ_RANGE, Y_RANGE, XZ_RANGE);
    let mut items = level.items_in(&area, None);
    let position = level.entity(body).position();
    items.sort_by(|&a, &b| position.distance_to_sqr(level.entity(a).position()).total_cmp(&position.distance_to_sqr(level.entity(b).position())));
    let mut nearest_visible_loved_item = None;
    for item in items {
        let Some(LevelEntity::Item(item_entity)) = level.entities.get(&item) else { continue };
        let stack = item_entity.item.clone();
        if !level.wants_to_pick_up(body, &stack) {
            continue;
        }
        if level.entity(item).position().distance_to_sqr(position) >= MAX_DISTANCE_TO_WANTED_ITEM * MAX_DISTANCE_TO_WANTED_ITEM {
            continue;
        }
        if level.has_line_of_sight(body, item) {
            nearest_visible_loved_item = Some(item);
            break;
        }
    }
    level.get_brain_mut(body).set_memory_optional(memory_module_type::NEAREST_VISIBLE_WANTED_ITEM, nearest_visible_loved_item);
}
