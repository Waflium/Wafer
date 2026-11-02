use wafer_core::BlockPos;
use wafer_data::BlockState;
use wafer_data::packets::play::clientbound;
use wafer_protocol::nbt::Nbt;

use super::ServerLevel;
use crate::network::connection::encode;
use crate::network::protocol::game::clientbound_block_entity_data_packet;
use crate::util::profiling::profiler;
use crate::world::container::ContainerRef;
use crate::world::entity::attributes::Attribute;
use crate::world::level::block::entity::{BlockEntity, BlockEntityKind, BlockEntityTicker};
use crate::world::level::chunk::block_entities::EntityCreationType;
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::level::storage::loot::loot_table::LootTable;

const BOUND_TICKING_BLOCK_ENTITY_FOR_THROWABLE: &str = "net.minecraft.world.level.chunk.LevelChunk$BoundTickingBlockEntity.tick(LevelChunk.java:783)";
const BOUND_TICKING_BLOCK_ENTITY_ADD_CATEGORY: &str = "net.minecraft.world.level.chunk.LevelChunk$BoundTickingBlockEntity.tick(LevelChunk.java:784)";
const BOUND_TICKING_BLOCK_ENTITY_REPORTED: &str = "net.minecraft.world.level.chunk.LevelChunk$BoundTickingBlockEntity.tick(LevelChunk.java:787)";

impl ServerLevel {
    pub fn get_block_entity(&mut self, pos: BlockPos) -> Option<&mut BlockEntity> {
        if !self.is_in_valid_bounds(pos) {
            return None;
        }
        self.load_chunk(pos.chunk());
        self.chunks.block_entity(pos, EntityCreationType::Immediate)
    }

    pub fn set_block_entity(&mut self, entity: BlockEntity) {
        if self.is_in_valid_bounds(entity.pos()) {
            self.load_chunk(entity.pos().chunk());
            self.chunks.add_and_register_block_entity(entity);
        }
    }

    pub fn remove_block_entity(&mut self, pos: BlockPos) -> Option<BlockEntity> {
        if !self.is_in_valid_bounds(pos) {
            return None;
        }
        self.load_chunk(pos.chunk());
        let removed = self.chunks.remove_block_entity(pos);
        if let Some(entity) = &removed {
            self.block_entity_set_removed(pos, entity);
        }
        removed
    }

    pub fn block_entity_changed(&mut self, pos: BlockPos) {
        if self.chunks.loaded_chunk(pos.chunk()).is_some() {
            self.chunks.count_get_chunk(pos.chunk());
            if let Some(chunk) = self.chunks.chunk_if_present(pos.chunk()) {
                chunk.mark_unsaved();
            }
        }
    }

    pub fn block_entity_set_changed(&mut self, pos: BlockPos) {
        let Some(block_entity) = self.chunks.block_entity_ref(pos) else { return };
        if matches!(block_entity.kind, BlockEntityKind::Beehive(_)) {
            self.beehive_set_changed(pos);
        }
        let Some(state) = self.chunks.block_entity_ref(pos).map(BlockEntity::block_state) else { return };
        self.block_entity_set_changed_static(pos, state);
    }

    pub fn block_entity_set_changed_static(&mut self, pos: BlockPos, state: BlockState) {
        self.block_entity_changed(pos);
        if !state.is_air() {
            self.update_neighbour_for_output_signal(pos, state.block());
        }
    }

    pub fn block_entity_set_removed(&mut self, pos: BlockPos, entity: &BlockEntity) {
        match entity.kind {
            BlockEntityKind::Jukebox(_) => {
                self.game_event_at_with(crate::world::level::gameevent::game_event::JUKEBOX_STOP_PLAY, pos, crate::world::level::gameevent::game_event::Context::of_state(Some(entity.block_state())));
                self.level_event(None, crate::server::level::events::SOUND_STOP_JUKEBOX_SONG, pos, 0);
            }
            BlockEntityKind::Beacon(_) => self.beacon_play_sound(pos, "minecraft:block.beacon.deactivate"),
            _ => {}
        }
    }

    pub fn pre_remove_side_effects(&mut self, pos: BlockPos) {
        let Some(entity) = self.get_block_entity(pos) else { return };
        let (is_container, is_furnace) = match &entity.kind {
            BlockEntityKind::Chest(_) | BlockEntityKind::Barrel(_) | BlockEntityKind::Hopper(_) | BlockEntityKind::Dispenser(_) | BlockEntityKind::BrewingStand(_) => (true, false),
            BlockEntityKind::Furnace(_) => (true, true),
            BlockEntityKind::Jukebox(_) => return self.jukebox_pop_out_the_item(pos),
            BlockEntityKind::Campfire(_) => return self.campfire_pre_remove_side_effects(pos),
            BlockEntityKind::CreakingHeart(_) => return self.creaking_heart_pre_remove_side_effects(pos),
            BlockEntityKind::SculkShrieker(_) => {
                let state = entity.block_state();
                return self.sculk_shrieker_pre_remove_side_effects(pos, state);
            }
            BlockEntityKind::PistonMoving(_) => return self.piston_moving_final_tick(pos),
            BlockEntityKind::ChiseledBookShelf(_) => {
                return crate::world::containers::drop_contents(self, pos, ContainerRef::ChiseledBookShelf(pos));
            }
            BlockEntityKind::Shelf(_) => return crate::world::containers::drop_contents(self, pos, ContainerRef::Shelf(pos)),
            BlockEntityKind::DecoratedPot(_) => return crate::world::containers::drop_contents(self, pos, ContainerRef::DecoratedPot(pos)),
            BlockEntityKind::Lectern(_) => {
                let state = entity.block_state();
                return self.lectern_pre_remove_side_effects(pos, state);
            }
            _ => (false, false),
        };
        if is_container {
            let container = ContainerRef::block(self, pos);
            crate::world::containers::drop_contents(self, pos, container);
        }
        if is_furnace {
            self.get_recipes_to_award_and_pop_experience(pos, wafer_core::Vec3::at_center_of(pos));
        }
    }

    pub fn unpack_loot_table(&mut self, pos: BlockPos, player: Option<crate::world::entity::EntityId>) {
        self.load_chunk(pos.chunk());
        let Some(entity) = self.chunks.block_entity(pos, EntityCreationType::Check) else { return };
        if !entity.is_randomizable() {
            return;
        }
        let (loot_table_key, loot_table_seed, container) = match &mut entity.kind {
            BlockEntityKind::DecoratedPot(pot) => {
                let Some(loot_table_key) = pot.loot_table.take() else { return };
                (loot_table_key, pot.loot_table_seed, ContainerRef::DecoratedPot(pos))
            }
            _ => {
                let Some(base) = entity.container_base_mut() else { return };
                let Some(loot_table_key) = base.loot_table.take() else { return };
                (loot_table_key, base.loot_table_seed, ContainerRef::block(self, pos))
            }
        };
        let loot_table = LootTable::get(&loot_table_key);
        if let Some(player) = player.filter(|&player| self.is_player(player)) {
            crate::server::main_thread_frames::within(&["net.minecraft.world.RandomizableContainer.unpackLootTable(RandomizableContainer.java:79)"], || {
                crate::advancements::triggers::criteria_triggers::GENERATE_LOOT.trigger(self, player, &loot_table_key)
            });
        }
        let luck = player.map(|player| self.attribute_value(player, Attribute::Luck) as f32);
        let mut params = LootParams::new(self);
        params.origin = Some(wafer_core::Vec3::at_center_of(pos));
        if let Some(luck) = luck {
            params.luck = luck;
            params.this_entity = player;
        }
        loot_table.fill(container, params, loot_table_seed);
    }

    pub fn block_entity_update_packet(&mut self, pos: BlockPos) -> Option<Vec<u8>> {
        let entity = self.get_block_entity(pos)?;
        if !entity.has_update_packet() {
            return None;
        }
        let tag = entity.get_update_tag();
        let ty = entity.ty();
        Some(encode(clientbound::BLOCK_ENTITY_DATA, |p| clientbound_block_entity_data_packet::write(p, pos, ty.0 as i32, &tag)))
    }

    pub fn tick_block_entities(&mut self) {
        let tick_block_entities = self.tick_rate_manager.runs_normally();
        let tickers = &mut self.chunks.block_entity_tickers;
        tickers.ticking = true;
        let pending = std::mem::take(&mut tickers.pending);
        tickers.list.extend(pending);
        let mut list = std::mem::take(&mut tickers.list);
        let mut kept = 0;
        for i in 0..list.len() {
            let (id, pos) = list[i];
            let Some((ticker, logged_invalid_block_state)) = self.chunks.bound_ticker(id, pos) else { continue };
            list[kept] = list[i];
            kept += 1;
            if tick_block_entities && self.in_block_ticking_range(pos.chunk()) {
                self.tick_bound_block_entity(pos, ticker, logged_invalid_block_state);
            }
        }
        list.truncate(kept);
        let tickers = &mut self.chunks.block_entity_tickers;
        list.append(&mut tickers.list);
        tickers.list = list;
        tickers.ticking = false;
    }

    fn tick_bound_block_entity(&mut self, pos: BlockPos, ticker: BlockEntityTicker, logged_invalid_block_state: bool) {
        if !self.world_border.is_within_bounds_pos(pos) || !self.chunks.is_ticking(pos.chunk()) || !self.entity_manager.are_entities_loaded(pos.chunk().pack()) {
            return;
        }
        let Some(entity) = self.chunks.block_entity_ref(pos) else { return };
        let ty = entity.ty();
        let cached_block = entity.block_state();
        let _zone = profiler::zone_with(|| ty.name().to_string());
        let state = self.chunks.loaded_chunk(pos.chunk()).map(|c| c.block_state(pos)).unwrap_or_else(|| wafer_data::blocks::AIR.default_state());
        if !ty.is_valid(state) {
            if !logged_invalid_block_state {
                self.chunks.set_logged_invalid_block_state(pos, true);
                warn!("Block entity {} @ {pos} state {state:?} invalid for ticking:", ty.name());
            }
            return;
        }
        if logged_invalid_block_state {
            self.chunks.set_logged_invalid_block_state(pos, false);
        }
        let _frames = crate::server::main_thread_frames::enter_static(&[
            "net.minecraft.world.level.chunk.LevelChunk$BoundTickingBlockEntity.tick(LevelChunk.java:773)",
            "net.minecraft.world.level.chunk.LevelChunk$RebindableTickingBlockEntityWrapper.tick(LevelChunk.java:827)",
            "net.minecraft.world.level.Level.tickBlockEntities(Level.java:475)",
            "net.minecraft.server.level.ServerLevel.tick(ServerLevel.java:461)",
            crate::server::command_frames::TICK_CHILDREN_LEVELS,
        ]);
        if let Err(panic) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.tick_block_entity_ticker(pos, state, ticker))) {
            let below = crate::server::main_thread_frames::current().split_off(1);
            let mut report = match panic.downcast::<crate::crash_report::ReportedException>() {
                Ok(reported) => reported.0,
                Err(panic) => {
                    let creator: Vec<&'static str> = std::iter::once(BOUND_TICKING_BLOCK_ENTITY_FOR_THROWABLE).chain(below.iter().copied()).collect();
                    let _creator = crate::util::java_stack::enter_all(&creator);
                    crate::crash_report::CrashReport::for_throwable(&crate::crash_report::throwable_of(&panic), "Ticking block entity")
                }
            };
            let category_frames = std::iter::once(BOUND_TICKING_BLOCK_ENTITY_ADD_CATEGORY).chain(below.iter().copied()).map(str::to_string).collect();
            let category = report.add_category_with_stack_trace("Block entity being ticked", category_frames);
            category.set_detail("Name", crate::world::level::block::entity::name_for_reporting(ty));
            category.set_detail("Cached block", format!("{cached_block:?}"));
            category.set_detail("Actual block", format!("{:?}", self.chunks.loaded_block_state(pos)));
            category.set_detail("Block location", crate::crash_report::CrashReportCategory::format_location(self.chunks.min_y(), self.chunks.max_y(), pos.x, pos.y, pos.z));
            report.throw_reported(std::iter::once(BOUND_TICKING_BLOCK_ENTITY_REPORTED).chain(below).collect());
        }
    }

    fn tick_block_entity_ticker(&mut self, pos: BlockPos, state: BlockState, ticker: BlockEntityTicker) {
        match ticker {
            BlockEntityTicker::Furnace => self.furnace_server_tick(pos, state),
            BlockEntityTicker::Hopper => self.hopper_push_items_tick(pos, state),
            BlockEntityTicker::ShulkerBox => self.shulker_box_tick(pos, state),
            BlockEntityTicker::DaylightDetector => self.daylight_detector_tick_entity(pos, state),
            BlockEntityTicker::Jukebox => self.jukebox_tick(pos),
            BlockEntityTicker::Conduit => self.conduit_server_tick(pos, state),
            BlockEntityTicker::CreakingHeart => self.creaking_heart_server_tick(pos, state),
            BlockEntityTicker::SculkSensor => {
                if let Some(user) = self.sculk_sensor_vibration_user(pos) {
                    self.vibration_ticker_tick(user);
                }
            }
            BlockEntityTicker::SculkCatalyst => self.sculk_catalyst_server_tick(pos),
            BlockEntityTicker::SculkShrieker => {
                if let Some(user) = self.sculk_shrieker_vibration_user(pos) {
                    self.vibration_ticker_tick(user);
                }
            }
            BlockEntityTicker::Beacon => self.beacon_tick(pos),
            BlockEntityTicker::BrewingStand => self.brewing_stand_server_tick(pos, state),
            BlockEntityTicker::CampfireCook => self.campfire_cook_tick(pos, state),
            BlockEntityTicker::CampfireCooldown => self.campfire_cooldown_tick(pos, state),
            BlockEntityTicker::Bell => self.bell_server_tick(pos),
            BlockEntityTicker::Vault => self.vault_server_tick(pos, state),
            BlockEntityTicker::TrialSpawner => self.trial_spawner_server_tick(pos, state),
            BlockEntityTicker::PotentSulfurWet => self.potent_sulfur_server_nausea_effect_tick(pos),
            BlockEntityTicker::PotentSulfurDormant => {
                self.potent_sulfur_server_waiting_countdown_tick(pos, state);
                self.potent_sulfur_server_nausea_effect_tick(pos);
            }
            BlockEntityTicker::PotentSulfurErupting => {
                self.potent_sulfur_launch_entity_tick(pos);
                self.potent_sulfur_server_waiting_countdown_tick(pos, state);
            }
            BlockEntityTicker::PotentSulfurContinuous => self.potent_sulfur_launch_entity_tick(pos),
            BlockEntityTicker::Crafter => self.crafter_server_tick(pos, state),
            BlockEntityTicker::Sign => self.sign_tick(pos),
            BlockEntityTicker::EndGateway => self.end_gateway_portal_tick(pos, state),
            BlockEntityTicker::Spawner => crate::world::level::block::entity::spawner_block_entity::SpawnerBlockEntity::server_tick(self, pos),
            BlockEntityTicker::Piston => self.piston_moving_tick(pos),
            BlockEntityTicker::Beehive => self.beehive_server_tick(pos, state),
        }
    }

    pub fn loaded_block_entity_data(&self, pos: BlockPos) -> Option<Nbt> {
        if !self.is_loaded(pos) {
            return None;
        }
        Some(self.chunks.block_entity_ref(pos)?.save_with_full_metadata())
    }

    pub fn block_entity_data(&mut self, pos: BlockPos) -> Option<Nbt> {
        Some(self.get_block_entity(pos)?.save_with_full_metadata())
    }
}
