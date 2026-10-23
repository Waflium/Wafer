use wafer_core::{BlockPos, Vec3};
use wafer_data::BlockState;
use wafer_data::properties::HAS_RECORD;
use wafer_protocol::nbt::Nbt;

use super::BlockEntityKind;
use crate::server::level::ServerLevel;
use crate::world::entity::{ItemEntity, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::jukebox_song::JukeboxSong;
use crate::world::item::jukebox_song_player::JukeboxSongPlayer;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_CLIENTS;
use crate::world::level::storage::tag_value_input;

pub const SONG_ITEM_TAG_ID: &str = "RecordItem";
pub const TICKS_SINCE_SONG_STARTED_TAG_ID: &str = "ticks_since_song_started";

#[derive(Clone)]
pub struct JukeboxBlockEntity {
    pub item: ItemStack,
    pub jukebox_song_player: JukeboxSongPlayer,
}

impl Default for JukeboxBlockEntity {
    fn default() -> JukeboxBlockEntity {
        JukeboxBlockEntity { item: ItemStack::empty(), jukebox_song_player: JukeboxSongPlayer::default() }
    }
}

impl JukeboxBlockEntity {
    pub fn get_comparator_output(&self) -> i32 {
        JukeboxSong::from_stack(&self.item).map_or(0, |song| song.comparator_output)
    }

    fn read_song_item(input: &Nbt) -> ItemStack {
        tag_value_input::read(input, SONG_ITEM_TAG_ID, ItemStack::from_nbt_partial).unwrap_or_else(ItemStack::empty)
    }

    pub fn load_stops_song(&self, input: &Nbt) -> bool {
        !self.item.is_empty() && !ItemStack::is_same_item_same_components(&Self::read_song_item(input), &self.item)
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        let new_item = Self::read_song_item(input);
        if !self.item.is_empty() && !ItemStack::is_same_item_same_components(&new_item, &self.item) {
            self.jukebox_song_player.clear_song();
        }
        self.item = new_item;
        if let Some(ticks_since_song_started) = tag_value_input::get_i64(input, TICKS_SINCE_SONG_STARTED_TAG_ID)
            && let Some(song) = JukeboxSong::from_stack(&self.item)
        {
            self.jukebox_song_player.set_song_without_playing(song, ticks_since_song_started);
        }
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        if !self.item.is_empty() {
            output.push((SONG_ITEM_TAG_ID.into(), self.item.to_nbt()));
        }
        if self.jukebox_song_player.get_song().is_some() {
            output.push((TICKS_SINCE_SONG_STARTED_TAG_ID.into(), Nbt::Long(self.jukebox_song_player.get_ticks_since_song_started())));
        }
    }
}

impl ServerLevel {
    pub fn block_entity_load_with_components(&mut self, pos: BlockPos, input: &Nbt) {
        if let Some(super::BlockEntity { kind: BlockEntityKind::Jukebox(jukebox), .. }) = self.get_block_entity(pos)
            && jukebox.load_stops_song(input)
        {
            self.jukebox_song_player_stop(pos);
        }
        if let Some(entity) = self.get_block_entity(pos) {
            entity.load_with_components(input);
        }
        self.block_entity_loaded(pos);
    }

    pub fn jukebox(&mut self, pos: BlockPos) -> Option<&mut JukeboxBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::Jukebox(jukebox) => Some(jukebox),
            _ => None,
        }
    }

    fn jukebox_block_state(&self, pos: BlockPos) -> Option<BlockState> {
        self.chunks.block_entity_ref(pos).map(|entity| entity.block_state())
    }

    pub fn jukebox_on_song_changed(&mut self, pos: BlockPos) {
        let Some(state) = self.jukebox_block_state(pos) else { return };
        self.update_neighbors_at(pos, state.block());
        self.block_entity_set_changed(pos);
    }

    fn notify_item_changed_in_jukebox(&mut self, pos: BlockPos, was_inserted: bool) {
        let Some(state) = self.jukebox_block_state(pos) else { return };
        if self.block_state(pos) == state {
            self.set_block(pos, state.set(HAS_RECORD, was_inserted), UPDATE_CLIENTS);
            let block_state = self.jukebox_block_state(pos);
            self.game_event_at_with(crate::world::level::gameevent::game_event::BLOCK_CHANGE, pos, crate::world::level::gameevent::game_event::Context::of_state(block_state));
        }
    }

    pub fn jukebox_pop_out_the_item(&mut self, pos: BlockPos) {
        let Some(jukebox) = self.jukebox(pos) else { return };
        if jukebox.item.is_empty() {
            return;
        }
        let item_before_popping_out = self.jukebox_remove_the_item(pos);
        let item_pos = Vec3::at_lower_corner_with_offset(pos, 0.5, 1.01, 0.5).offset_random_xz(&mut self.random, 0.7);
        let mut entity = ItemEntity::new(item_pos, item_before_popping_out);
        entity.set_default_pickup_delay();
        self.add_fresh_entity(LevelEntity::Item(entity));
        self.jukebox_on_song_changed(pos);
    }

    pub fn jukebox_tick(&mut self, pos: BlockPos) {
        self.jukebox_song_player_tick(pos);
    }

    pub fn jukebox_get_the_item(&mut self, pos: BlockPos) -> ItemStack {
        self.jukebox(pos).map_or_else(ItemStack::empty, |jukebox| jukebox.item.clone())
    }

    pub fn jukebox_split_the_item(&mut self, pos: BlockPos) -> ItemStack {
        let Some(jukebox) = self.jukebox(pos) else { return ItemStack::empty() };
        let retrieved_item = std::mem::replace(&mut jukebox.item, ItemStack::empty());
        self.jukebox_set_the_item(pos, ItemStack::empty());
        retrieved_item
    }

    pub fn jukebox_remove_the_item(&mut self, pos: BlockPos) -> ItemStack {
        self.jukebox_split_the_item(pos)
    }

    pub fn jukebox_set_the_item(&mut self, pos: BlockPos, item_stack: ItemStack) {
        let Some(jukebox) = self.jukebox(pos) else { return };
        let item_was_inserted = !item_stack.is_empty();
        let maybe_song = JukeboxSong::from_stack(&item_stack);
        jukebox.item = item_stack;
        self.notify_item_changed_in_jukebox(pos, item_was_inserted);
        match maybe_song {
            Some(song) if item_was_inserted => self.jukebox_song_player_play(pos, song),
            _ => self.jukebox_song_player_stop(pos),
        }
    }

    pub fn jukebox_can_place_item(&mut self, pos: BlockPos, slot: usize, item_stack: &ItemStack) -> bool {
        item_stack.has(crate::world::item::component::JUKEBOX_PLAYABLE) && (slot != 0 || self.jukebox_get_the_item(pos).is_empty())
    }
}
