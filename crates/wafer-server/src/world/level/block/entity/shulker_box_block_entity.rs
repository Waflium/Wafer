use wafer_core::BlockPos;
use wafer_core::random::RandomSource;
use wafer_data::{BlockState, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::base_container_block_entity::{BaseContainerBlockEntity, save_all_items};
use super::BlockEntityKind;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::update_flags::UPDATE_ALL;

pub const EVENT_SET_OPEN_COUNT: i32 = 1;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AnimationStatus {
    Closed,
    Opening,
    Opened,
    Closing,
}

#[derive(Clone)]
pub struct ShulkerBoxBlockEntity {
    pub base: BaseContainerBlockEntity,
    pub open_count: i32,
    pub animation_status: AnimationStatus,
    pub progress: f32,
    pub progress_old: f32,
}

impl Default for ShulkerBoxBlockEntity {
    fn default() -> ShulkerBoxBlockEntity {
        ShulkerBoxBlockEntity { base: BaseContainerBlockEntity::with_size(27), open_count: 0, animation_status: AnimationStatus::Closed, progress: 0.0, progress_old: 0.0 }
    }
}

impl ShulkerBoxBlockEntity {
    pub const CONTAINER_SIZE: usize = 27;

    pub fn trigger_event(&mut self, b0: i32, b1: i32) -> bool {
        if b0 != EVENT_SET_OPEN_COUNT {
            return false;
        }
        self.open_count = b1;
        if b1 == 0 {
            self.animation_status = AnimationStatus::Closing;
        }
        if b1 == 1 {
            self.animation_status = AnimationStatus::Opening;
        }
        true
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.base.load_additional(input);
        self.base.items = vec![ItemStack::empty(); Self::CONTAINER_SIZE];
        if !self.base.try_load_loot_table(input) {
            self.base.load_all_items(input, Self::CONTAINER_SIZE);
        }
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        self.base.save_additional(output);
        if !self.base.try_save_loot_table(output) {
            save_all_items(output, &self.base.items, false);
        }
    }

    pub fn is_closed(&self) -> bool {
        self.animation_status == AnimationStatus::Closed
    }
}

impl ServerLevel {
    fn shulker_box(&mut self, pos: BlockPos) -> Option<&mut ShulkerBoxBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::ShulkerBox(shulker) => Some(shulker),
            _ => None,
        }
    }

    pub fn shulker_box_tick(&mut self, pos: BlockPos, state: BlockState) {
        let Some(shulker) = self.shulker_box(pos) else { return };
        shulker.progress_old = shulker.progress;
        let opening = shulker.animation_status == AnimationStatus::Opening;
        let mut updates = 0;
        match shulker.animation_status {
            AnimationStatus::Closed => shulker.progress = 0.0,
            AnimationStatus::Opening => {
                shulker.progress += 0.1;
                if shulker.progress_old == 0.0 {
                    updates += 1;
                }
                if shulker.progress >= 1.0 {
                    shulker.animation_status = AnimationStatus::Opened;
                    shulker.progress = 1.0;
                    updates += 1;
                }
            }
            AnimationStatus::Opened => shulker.progress = 1.0,
            AnimationStatus::Closing => {
                shulker.progress -= 0.1;
                if shulker.progress_old == 1.0 {
                    updates += 1;
                }
                if shulker.progress <= 0.0 {
                    shulker.animation_status = AnimationStatus::Closed;
                    shulker.progress = 0.0;
                    updates += 1;
                }
            }
        }
        let (progress_old, progress) = (shulker.progress_old, shulker.progress);
        for _ in 0..updates {
            self.update_neighbour_shapes(state, pos, UPDATE_ALL, crate::world::level::update_flags::UPDATE_LIMIT);
            self.update_neighbors_at(pos, state.block());
        }
        if opening {
            self.shulker_box_move_collided_entities(pos, state, progress_old, progress);
        }
    }

    fn shulker_box_move_collided_entities(&mut self, pos: BlockPos, state: BlockState, progress_old: f32, progress: f32) {
        if !state.block().is_instance_of("ShulkerBoxBlock") {
            return;
        }
        let direction = state.get(wafer_data::properties::FACING);
        let aabb = crate::world::entity::monster::shulker::get_progress_delta_aabb(1.0, direction, progress_old, progress, wafer_core::Vec3::at_bottom_center_of(pos));
        let entities = self.get_entities(None, &aabb);
        for entity in entities {
            if self.get_piston_push_reaction(entity) != wafer_data::PushReaction::IgnoreEntity {
                let delta = wafer_core::Vec3::new(
                    (aabb.x_size() + 0.01) * direction.step_x() as f64,
                    (aabb.y_size() + 0.01) * direction.step_y() as f64,
                    (aabb.z_size() + 0.01) * direction.step_z() as f64,
                );
                self.move_entity(entity, crate::world::entity::MoverType::ShulkerBox, delta);
            }
        }
    }

    pub fn shulker_box_start_open(&mut self, pos: BlockPos, user: EntityId) {
        let Some(shulker) = self.shulker_box(pos) else { return };
        if shulker.open_count < 0 {
            shulker.open_count = 0;
        }
        shulker.open_count += 1;
        let open_count = shulker.open_count;
        let block = self.block_state(pos).block();
        self.block_event(pos, block, EVENT_SET_OPEN_COUNT, open_count);
        if open_count == 1 {
            self.game_event_at(Some(user), crate::world::level::gameevent::game_event::CONTAINER_OPEN, pos);
            let pitch = self.random.next_float() * 0.1 + 0.9;
            self.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:block.shulker_box.open"), SoundSource::Blocks, 0.5, pitch);
        }
    }

    pub fn shulker_box_stop_open(&mut self, pos: BlockPos, user: EntityId) {
        let Some(shulker) = self.shulker_box(pos) else { return };
        shulker.open_count -= 1;
        let open_count = shulker.open_count;
        let block = self.block_state(pos).block();
        self.block_event(pos, block, EVENT_SET_OPEN_COUNT, open_count);
        if open_count <= 0 {
            self.game_event_at(Some(user), crate::world::level::gameevent::game_event::CONTAINER_CLOSE, pos);
            let pitch = self.random.next_float() * 0.1 + 0.9;
            self.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:block.shulker_box.close"), SoundSource::Blocks, 0.5, pitch);
        }
    }
}

use crate::world::level::BlockGetter as _;
