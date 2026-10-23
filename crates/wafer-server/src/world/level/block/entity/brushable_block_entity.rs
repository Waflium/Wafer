use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Direction, Vec3};
use wafer_data::BlockState;
use wafer_data::properties::DUSTED;
use wafer_protocol::nbt::Nbt;

use super::{BlockEntityKind, int_value, long_value};
use crate::server::level::ServerLevel;
use crate::server::level::events::PARTICLES_AND_SOUND_BRUSH_BLOCK_COMPLETE;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::item::ItemEntity;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::block::brushable_block;
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::level::storage::loot::loot_table::LootTable;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::level::storage::tag_value_input;

const BRUSH_COOLDOWN_TICKS: i64 = 10;
const BRUSH_RESET_TICKS: i64 = 40;
const REQUIRED_BRUSHES_TO_BREAK: i32 = 10;
const ITEM_WIDTH: f64 = 0.25;
const ITEM_HEIGHT: f32 = 0.25;

#[derive(Clone)]
pub struct BrushableBlockEntity {
    brush_count: i32,
    brush_count_resets_at_tick: i64,
    cool_down_ends_at_tick: i64,
    pub item: ItemStack,
    pub hit_direction: Option<Direction>,
    pub loot_table: Option<String>,
    pub loot_table_seed: i64,
}

impl Default for BrushableBlockEntity {
    fn default() -> BrushableBlockEntity {
        BrushableBlockEntity { brush_count: 0, brush_count_resets_at_tick: 0, cool_down_ends_at_tick: 0, item: ItemStack::empty(), hit_direction: None, loot_table: None, loot_table_seed: 0 }
    }
}

impl BrushableBlockEntity {
    fn try_load_loot_table(&mut self, input: &Nbt) -> bool {
        self.loot_table = tag_value_input::read_or_error(input, "LootTable", crate::util::codec::identifier);
        self.loot_table_seed = tag_value_input::get_numeric(input, "LootTableSeed").and_then(long_value).unwrap_or(0);
        self.loot_table.is_some()
    }

    fn try_save_loot_table(&self, output: &mut Vec<(String, Nbt)>) -> bool {
        let Some(loot_table) = &self.loot_table else { return false };
        output.push(("LootTable".into(), Nbt::String(loot_table.clone())));
        if self.loot_table_seed != 0 {
            output.push(("LootTableSeed".into(), Nbt::Long(self.loot_table_seed)));
        }
        true
    }

    pub fn get_update_tag(&self) -> Nbt {
        let mut tag = Vec::new();
        if let Some(hit_direction) = self.hit_direction {
            tag.push(("hit_direction".into(), Nbt::Byte(hit_direction.ordinal() as i8)));
        }
        if !self.item.is_empty() {
            tag.push(("item".into(), self.item.to_nbt()));
        }
        Nbt::Compound(tag)
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.item = if !self.try_load_loot_table(input) {
            tag_value_input::read(input, "item", ItemStack::from_nbt_partial).unwrap_or_else(ItemStack::empty)
        } else {
            ItemStack::empty()
        };
        self.hit_direction = tag_value_input::get_numeric(input, "hit_direction").and_then(int_value).map(|value| Direction::from_3d_data_value(value as i8 as i32));
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        if !self.try_save_loot_table(output) && !self.item.is_empty() {
            output.push(("item".into(), self.item.to_nbt()));
        }
    }

    pub fn set_loot_table(&mut self, loot_table: &str, seed: i64) {
        self.loot_table = Some(loot_table.to_owned());
        self.loot_table_seed = seed;
    }

    fn get_completion_state(&self) -> i32 {
        if self.brush_count == 0 {
            0
        } else if self.brush_count < 3 {
            1
        } else if self.brush_count < 6 {
            2
        } else {
            3
        }
    }
}

impl ServerLevel {
    fn brushable_mut(&mut self, pos: BlockPos) -> Option<&mut BrushableBlockEntity> {
        match self.get_block_entity(pos).map(|entity| &mut entity.kind) {
            Some(BlockEntityKind::Brushable(brushable)) => Some(brushable),
            _ => None,
        }
    }

    fn brushable_block_state(&mut self, pos: BlockPos) -> BlockState {
        match self.get_block_entity(pos).map(|entity| entity.block_state()) {
            Some(state) => state,
            None => self.block_state(pos),
        }
    }

    pub fn brushable_brush(&mut self, pos: BlockPos, game_time: i64, user: EntityId, direction: Direction, brush: &ItemStack) -> bool {
        let Some(brushable) = self.brushable_mut(pos) else { return false };
        if brushable.hit_direction.is_none() {
            brushable.hit_direction = Some(direction);
        }
        brushable.brush_count_resets_at_tick = game_time + BRUSH_RESET_TICKS;
        if game_time < brushable.cool_down_ends_at_tick {
            return false;
        }
        brushable.cool_down_ends_at_tick = game_time + BRUSH_COOLDOWN_TICKS;
        self.brushable_unpack_loot_table(pos, user, brush);
        let Some(brushable) = self.brushable_mut(pos) else { return false };
        let previous_completion_state = brushable.get_completion_state();
        brushable.brush_count += 1;
        if brushable.brush_count >= REQUIRED_BRUSHES_TO_BREAK {
            self.brushable_brushing_completed(pos, user, brush);
            return true;
        }
        let completion_state = brushable.get_completion_state();
        let block = self.brushable_block_state(pos).block();
        self.schedule_tick(pos, block, brushable_block::TICK_DELAY);
        if previous_completion_state != completion_state {
            let previous_state = self.brushable_block_state(pos);
            self.set_block(pos, previous_state.set(DUSTED, completion_state), UPDATE_ALL);
        }
        false
    }

    fn brushable_unpack_loot_table(&mut self, pos: BlockPos, user: EntityId, brush: &ItemStack) {
        let Some(brushable) = self.brushable_mut(pos) else { return };
        let Some(loot_table_key) = brushable.loot_table.clone() else { return };
        let loot_table_seed = brushable.loot_table_seed;
        let loot_table = LootTable::get(&loot_table_key);
        if self.is_player(user) {
            crate::advancements::triggers::criteria_triggers::GENERATE_LOOT.trigger(self, user, &loot_table_key);
        }
        let luck = self.attribute_value(user, Attribute::Luck) as f32;
        let mut params = LootParams::new(self);
        params.origin = Some(Vec3::at_center_of(pos));
        params.luck = luck;
        params.this_entity = Some(user);
        params.tool = Some(brush);
        let loot = loot_table.get_random_items_seeded(params, loot_table_seed);
        if loot.len() > 1 {
            warn!("Expected max 1 loot from loot table {}, but got {}", loot_table_key, loot.len());
        }
        let item = loot.into_iter().next().unwrap_or_else(ItemStack::empty);
        let Some(brushable) = self.brushable_mut(pos) else { return };
        brushable.item = item;
        brushable.loot_table = None;
        self.block_entity_set_changed(pos);
    }

    fn brushable_brushing_completed(&mut self, pos: BlockPos, user: EntityId, brush: &ItemStack) {
        self.brushable_drop_content(pos, user, brush);
        let block_state = self.brushable_block_state(pos);
        self.level_event(None, PARTICLES_AND_SOUND_BRUSH_BLOCK_COMPLETE, pos, block_state.id() as i32);
        let turns_into = brushable_block::of(block_state.block()).map_or(wafer_data::blocks::AIR, |block| block.get_turns_into());
        self.set_block(pos, turns_into.default_state(), UPDATE_ALL);
    }

    fn brushable_drop_content(&mut self, pos: BlockPos, user: EntityId, brush: &ItemStack) {
        self.brushable_unpack_loot_table(pos, user, brush);
        let Some(brushable) = self.brushable_mut(pos) else { return };
        if brushable.item.is_empty() {
            return;
        }
        let size = ITEM_WIDTH;
        let center_range = 1.0 - size;
        let half_size = size / 2.0;
        let drop_direction = brushable.hit_direction.unwrap_or(Direction::Up);
        let drop_pos = pos.relative(drop_direction);
        let xo = drop_pos.x as f64 + 0.5 * center_range + half_size;
        let yo = drop_pos.y as f64 + 0.5 + (ITEM_HEIGHT / 2.0) as f64;
        let zo = drop_pos.z as f64 + 0.5 * center_range + half_size;
        let count = self.random.next_int_bounded(21) + 10;
        let brushable = self.brushable_mut(pos).expect("a brushable block entity");
        let dropped = brushable.item.split(count);
        brushable.item = ItemStack::empty();
        let mut entity = ItemEntity::new(Vec3::new(xo, yo, zo), dropped);
        entity.entity.set_delta_movement(Vec3::ZERO);
        self.add_fresh_entity(LevelEntity::Item(entity));
    }

    pub fn brushable_check_reset(&mut self, pos: BlockPos) {
        let game_time = self.game_time;
        let Some(brushable) = self.brushable_mut(pos) else { return };
        if brushable.brush_count != 0 && game_time >= brushable.brush_count_resets_at_tick {
            let previous_completion_state = brushable.get_completion_state();
            brushable.brush_count = (brushable.brush_count - 2).max(0);
            let completion_state = brushable.get_completion_state();
            brushable.brush_count_resets_at_tick = game_time + 4;
            if previous_completion_state != completion_state {
                let state = self.brushable_block_state(pos);
                self.set_block(pos, state.set(DUSTED, completion_state), UPDATE_ALL);
            }
        }
        let Some(brushable) = self.brushable_mut(pos) else { return };
        if brushable.brush_count == 0 {
            brushable.hit_direction = None;
            brushable.brush_count_resets_at_tick = 0;
            brushable.cool_down_ends_at_tick = 0;
        } else {
            let block = self.brushable_block_state(pos).block();
            self.schedule_tick(pos, block, brushable_block::TICK_DELAY);
        }
    }
}
