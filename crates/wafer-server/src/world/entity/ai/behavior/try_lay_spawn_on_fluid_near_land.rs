use wafer_core::Direction;
use wafer_data::{Block, block_tags, fluid_tags};

use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::{EntityId, sound_event};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;

pub fn create(spawn_block: Block) -> OneShot {
    behavior_builder::create(|i| {
        i.absent(memory_module_type::ATTACK_TARGET);
        i.present(memory_module_type::WALK_TARGET);
        let pregnant = i.present(memory_module_type::IS_PREGNANT);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let e = level.entity(body);
            if e.is_in_water() || !e.on_ground {
                return false;
            }
            let below_pos = e.block_position().below();
            for direction in Direction::PLANE_HORIZONTAL {
                let relative_pos = below_pos.relative(direction);
                let relative_state = level.block_state(relative_pos);
                if relative_state.collision_voxel_shape().face_shape(Direction::Up).is_empty()
                    && (level.fluid_state(relative_pos).is(&fluid_tags::SUPPORTS_FROGSPAWN) || relative_state.is(&block_tags::SUPPORTS_FROGSPAWN))
                {
                    let spawn_pos = relative_pos.above();
                    if level.block_state(spawn_pos).is_air() {
                        let new_state = spawn_block.default_state();
                        level.set_block(spawn_pos, new_state, UPDATE_ALL);
                        level.play_sound_from_entity(None, body, sound_event("minecraft:entity.frog.lay_spawn"), SoundSource::Blocks, 1.0, 1.0);
                        pregnant.erase(level, body);
                        return true;
                    }
                }
            }
            true
        }
    })
}
