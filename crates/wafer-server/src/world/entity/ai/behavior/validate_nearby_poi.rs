use wafer_core::{AABB, BlockPos};
use wafer_data::block_tags;
use wafer_data::properties::OCCUPIED;

use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::core::global_pos::GlobalPos;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::MemoryModuleType;
use crate::world::entity::ai::village::poi::poi_types::PoiType;
use crate::world::level::BlockGetter;

const MAX_DISTANCE: f64 = 16.0;

pub fn create(poi_type: impl Fn(PoiType) -> bool + 'static, memory_type: MemoryModuleType<GlobalPos>) -> OneShot {
    behavior_builder::create(|i| {
        let memory = i.present(memory_type);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let global_pos = memory.get(level, body);
            let pos = global_pos.pos;
            if level.dimension == global_pos.dimension && pos.closer_to_center_than(level.entity(body).position(), MAX_DISTANCE) {
                let ty = level.chunks.poi_manager.get_type(pos);
                if !ty.is_some_and(&poi_type) {
                    memory.erase(level, body);
                } else if bed_is_occupied(level, pos, body) {
                    memory.erase(level, body);
                    if !bed_is_occupied_by_villager(level, pos) {
                        level.chunks.poi_manager.release(pos);
                    }
                }
                true
            } else {
                false
            }
        }
    })
}

fn bed_is_occupied(level: &mut ServerLevel, poi_pos: BlockPos, body: EntityId) -> bool {
    let block_state = level.block_state(poi_pos);
    block_state.block().is(&block_tags::VILLAGERS_CAN_SLEEP_ON_BED) && block_state.get(OCCUPIED) && !level.is_sleeping(body)
}

fn bed_is_occupied_by_villager(level: &ServerLevel, poi_pos: BlockPos) -> bool {
    let area = AABB::of_block(poi_pos);
    !level.get_living_entities(&area, |level, entity| level.is_villager(entity) && level.is_sleeping(entity)).is_empty()
}
