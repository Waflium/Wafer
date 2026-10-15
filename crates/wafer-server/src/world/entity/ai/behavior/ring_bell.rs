use wafer_core::random::RandomSource;
use wafer_data::blocks;

use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::level::BlockGetter;
use crate::world::level::block::bell_block::BellBlock;

const BELL_RING_CHANCE: f32 = 0.95;
pub const RING_BELL_FROM_DISTANCE: f64 = 3.0;

pub fn create() -> OneShot {
    behavior_builder::create(|i| {
        let meeting_point = i.present(memory_module_type::MEETING_POINT);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            if level.random.next_float() <= BELL_RING_CHANCE {
                return false;
            }
            let pos = meeting_point.get(level, body).pos;
            if pos.closer_than(level.entity(body).block_position(), RING_BELL_FROM_DISTANCE) {
                let state = level.block_state(pos);
                if state.is_block(blocks::BELL) {
                    BellBlock.attempt_to_ring(Some(body), level, pos, None);
                }
            }
            true
        }
    })
}
