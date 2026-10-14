use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use wafer_core::{BlockPos, Direction, Vec3, mth};
use wafer_data::packets::play::{clientbound, serverbound};
use wafer_protocol::Reader;

use crate::network::configuration::read_client_information;
use crate::network::connection::Packet;
use crate::server::level::ServerLevel;
use crate::network::chat::Component;
use crate::server::level::game_mode::{BlockBreakAction, GameType};
use crate::server::network::movement::Input;
use crate::util::Prediction;
use crate::world::entity::EntityId;
use crate::world::entity::player::player_model_part::PlayerModelPart;
use crate::network::hashed_stack::HashedStack;
use crate::world::container::ContainerRef;
use crate::world::inventory::abstract_container_menu::MenuKind;
use crate::world::inventory::recipe_book_menu::PostPlaceAction;
use crate::world::inventory::recipe_book_type::RecipeBookType;
use crate::world::item::component::swing_animation::SwingAnimation;
use crate::world::item::crafting::display::recipe_display_entry::RecipeDisplayId;
use crate::world::item::crafting::recipe_manager::RecipeManager;
use crate::world::item::{ItemStack, read_slot, validate_encodable};
use crate::world::level::BlockGetter;
use crate::world::level::block::BlockHitResult;
use crate::world::level::block::entity::sign_block_entity::{FilteredText, SignTextSlot};
use crate::world::{InteractionHand, InteractionResult, SwingSource};

const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(15);

mod player_action {
    pub const START_DESTROY_BLOCK: i32 = 0;
    pub const CHANGE_DESTROY_DIRECTION: i32 = 1;
    pub const ABORT_DESTROY_BLOCK: i32 = 2;
    pub const STOP_DESTROY_BLOCK: i32 = 3;
    pub const DROP_ALL_ITEMS: i32 = 4;
    pub const DROP_ITEM: i32 = 5;
    pub const RELEASE_USE_ITEM: i32 = 6;
    pub const SWAP_ITEM_WITH_OFFHAND: i32 = 7;
    pub const STAB: i32 = 8;
}

pub fn is_illegal_chat_char(c: char) -> bool {
    c == '\u{a7}' || c < ' ' || c == '\u{7f}'
}

impl ServerLevel {
    pub fn handle_game_packet(&mut self, id: EntityId, packet: &Packet) -> anyhow::Result<()> {
        let mut r = packet.reader();
        match packet.id {
            serverbound::KEEP_ALIVE => {
                let p = self.player_mut(id);
                if p.keep_alive_challenge.is_some() && p.keep_alive_challenge == Some(r.i64()?) {
                    let time = p.keep_alive_time.elapsed().as_millis() as i32;
                    p.latency = (p.latency * 3 + time) / 4;
                    p.keep_alive_challenge = None;
                } else {
                    p.disconnect("disconnect.timeout");
                }
            }
            serverbound::MOVE_PLAYER_POS | serverbound::MOVE_PLAYER_POS_ROT | serverbound::MOVE_PLAYER_ROT | serverbound::MOVE_PLAYER_STATUS_ONLY => {
                let has_pos = matches!(packet.id, serverbound::MOVE_PLAYER_POS | serverbound::MOVE_PLAYER_POS_ROT);
                let has_rot = matches!(packet.id, serverbound::MOVE_PLAYER_POS_ROT | serverbound::MOVE_PLAYER_ROT);
                let pos = if has_pos { Some(Vec3::new(r.f64()?, r.f64()?, r.f64()?)) } else { None };
                let rot = if has_rot { Some((r.f32()?, r.f32()?)) } else { None };
                let flags = r.u8()?;
                self.handle_move_player(id, pos, rot, flags & 1 != 0, flags & 2 != 0);
            }
            serverbound::ACCEPT_TELEPORTATION => {
                let teleport_id = r.varint()?;
                let pos = Vec3::new(r.f64()?, r.f64()?, r.f64()?);
                let (y_rot, x_rot) = (r.f32()?, r.f32()?);
                self.handle_accept_teleport(id, teleport_id, pos, y_rot, x_rot);
            }
            serverbound::PLAYER_INPUT => self.handle_player_input(id, Input::from_flags(r.u8()?)),
            serverbound::MOVE_VEHICLE => {
                let position = Vec3::new(r.f64()?, r.f64()?, r.f64()?);
                let (y_rot, x_rot) = (r.f32()?, r.f32()?);
                let on_ground = r.bool()?;
                self.handle_move_vehicle(id, position, y_rot, x_rot, on_ground);
            }
            serverbound::PADDLE_BOAT => {
                let (left, right) = (r.bool()?, r.bool()?);
                self.handle_paddle_boat(id, left, right);
            }
            serverbound::PLAYER_COMMAND => {
                let _entity = r.varint()?;
                let action = r.varint()?;
                if !(0..=6).contains(&action) {
                    anyhow::bail!("Unknown player command {action}");
                }
                let _data = r.varint()?;
                self.handle_player_command(id, action);
            }
            serverbound::PLAYER_LOADED => self.handle_player_loaded(id),
            serverbound::PLAYER_ABILITIES => {
                let flying = r.u8()? & 2 != 0;
                let abilities = &mut self.player_mut(id).abilities;
                abilities.flying = flying && abilities.mayfly;
            }
            serverbound::CLIENT_TICK_END => self.handle_client_tick_end(id),
            serverbound::PLAYER_ACTION => {
                let action = r.varint()?;
                if !(player_action::START_DESTROY_BLOCK..=player_action::STAB).contains(&action) {
                    anyhow::bail!("Unknown player action {action}");
                }
                let pos = r.position()?;
                let pos = BlockPos::new(pos.0, pos.1, pos.2);
                let direction = Direction::from_3d_data_value(r.u8()? as i32);
                let sequence = r.varint()?;
                self.handle_player_action(id, action, pos, direction, sequence);
            }
            serverbound::USE_ITEM_ON => {
                let hand = InteractionHand::from_id(r.varint()?);
                let pos = r.position()?;
                let block_pos = BlockPos::new(pos.0, pos.1, pos.2);
                let direction = Direction::from_3d_data_value(r.varint()?);
                let (cx, cy, cz) = (r.f32()?, r.f32()?, r.f32()?);
                let inside = r.bool()?;
                let world_border_hit = r.bool()?;
                let sequence = r.varint()?;
                let location = Vec3::new(block_pos.x as f64 + cx as f64, block_pos.y as f64 + cy as f64, block_pos.z as f64 + cz as f64);
                let hit = BlockHitResult { location, direction, block_pos, inside, world_border_hit };
                self.handle_use_item_on(id, hand, hit, sequence);
            }
            serverbound::USE_ITEM => {
                let hand = InteractionHand::from_id(r.varint()?);
                let sequence = r.varint()?;
                let (y_rot, x_rot) = (r.f32()?, r.f32()?);
                self.handle_use_item(id, hand, sequence, y_rot, x_rot);
            }
            serverbound::PUNCH => {
                self.player_mut(id).reset_last_action_time();
                let animation = self.player(id).item_in_hand(InteractionHand::MainHand).map_or(SwingAnimation::DEFAULT, |s| s.attack_animation());
                self.swing(id, InteractionHand::MainHand, animation, false);
                self.reset_attack_strength_ticker(id);
            }
            serverbound::ATTACK => {
                let entity_id = r.varint()?;
                self.handle_attack(id, entity_id);
            }
            serverbound::INTERACT => {
                let entity_id = r.varint()?;
                let hand = InteractionHand::from_id(r.varint()?);
                let (x, y, z) = r.lp_vec3()?;
                let using_secondary_action = r.bool()?;
                self.handle_interact(id, entity_id, hand, Vec3::new(x, y, z), using_secondary_action);
            }
            serverbound::SIGN_UPDATE => {
                let pos = r.position()?;
                let mut lines = Vec::with_capacity(4);
                for _ in 0..4 {
                    lines.push(r.string(384)?.to_string());
                }
                let slot = SignTextSlot::by_id(r.varint()?);
                self.handle_sign_update(id, BlockPos::new(pos.0, pos.1, pos.2), lines, slot);
            }
            serverbound::SET_BEACON => {
                let mut read_effect = || -> anyhow::Result<Option<wafer_data::MobEffect>> {
                    if !r.bool()? {
                        return Ok(None);
                    }
                    let id = r.varint()?;
                    usize::try_from(id).ok().filter(|&i| i < wafer_data::MOB_EFFECTS.len()).map(|i| Some(wafer_data::MobEffect(i as u8))).ok_or_else(|| anyhow::anyhow!("Unknown mob effect id {id}"))
                };
                let primary = read_effect()?;
                let secondary = read_effect()?;
                self.handle_set_beacon_packet(id, primary, secondary);
            }
            serverbound::CONTAINER_BUTTON_CLICK => {
                let container_id = r.varint()?;
                let button_id = r.varint()?;
                self.handle_container_button_click(id, container_id, button_id);
            }
            serverbound::CONTAINER_CLICK => self.handle_container_click(id, &mut r)?,
            serverbound::RENAME_ITEM => {
                let name = r.string(32767)?.to_string();
                self.handle_rename_item(id, &name);
            }
            serverbound::CONTAINER_SLOT_STATE_CHANGED => {
                let slot_id = r.varint()?;
                let container_id = r.varint()?;
                let new_state = r.bool()?;
                self.handle_container_slot_state_changed(id, slot_id, container_id, new_state);
            }
            serverbound::PLACE_RECIPE => {
                let container_id = r.varint()?;
                let recipe = RecipeDisplayId(r.varint()?);
                let use_max_items = r.bool()?;
                self.handle_place_recipe(id, container_id, recipe, use_max_items);
            }
            serverbound::RECIPE_BOOK_CHANGE_SETTINGS => {
                let book_type = r.varint()?;
                let book_type = RecipeBookType::by_id(book_type).ok_or_else(|| anyhow::anyhow!("Unknown recipe book type {book_type}"))?;
                let is_open = r.bool()?;
                let is_filtering = r.bool()?;
                self.handle_recipe_book_change_settings_packet(id, book_type, is_open, is_filtering);
            }
            serverbound::RECIPE_BOOK_SEEN_RECIPE => {
                let recipe = RecipeDisplayId(r.varint()?);
                self.handle_recipe_book_seen_recipe_packet(id, recipe);
            }
            serverbound::SET_CREATIVE_MODE_SLOT => {
                let slot = r.i16()?;
                let stack = read_slot(&mut r, true).map_err(anyhow::Error::msg)?;
                if let Some(stack) = &stack {
                    validate_encodable(stack).map_err(anyhow::Error::msg)?;
                }
                self.handle_set_creative_mode_slot(id, slot, stack);
            }
            serverbound::CONTAINER_CLOSE => {
                r.varint()?;
                self.do_close_container(id);
            }
            serverbound::PICK_ITEM_FROM_BLOCK => {
                let pos = r.position()?;
                let include_data = r.bool()?;
                self.handle_pick_item_from_block(id, BlockPos::new(pos.0, pos.1, pos.2), include_data);
            }
            serverbound::SET_CARRIED_ITEM => {
                let slot = r.i16()?;
                if (0..9).contains(&slot) {
                    if self.player(id).inventory.selected != slot as usize && self.used_item_hand(id) == InteractionHand::MainHand {
                        self.stop_using_item(id);
                    }
                    let p = self.player_mut(id);
                    p.inventory.selected = slot as usize;
                    p.reset_last_action_time();
                } else {
                    warn!("{} tried to set an invalid carried item", self.player(id).profile.name);
                }
            }
            serverbound::CHUNK_BATCH_RECEIVED => {
                let desired = r.f32()?;
                self.player_mut(id).chunk_sender.on_chunk_batch_received(desired);
            }
            serverbound::SEEN_ADVANCEMENTS => {
                let action = r.varint()?;
                if action == 0 {
                    let tab = crate::resources::identifier::read(r.string(32767)?).map_err(anyhow::Error::msg)?;
                    self.handle_seen_advancements(id, &tab);
                } else if action != 1 {
                    anyhow::bail!("Unknown seen advancements action {action}");
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_seen_advancements(&mut self, id: EntityId, tab: &str) {
        if let Some(advancement) = crate::server::server_advancement_manager::ServerAdvancementManager::get_bound().get(tab) {
            let p = self.player_mut(id);
            p.advancements.set_selected_tab(&p.connection, Some(advancement));
        }
    }

    fn handle_player_action(&mut self, id: EntityId, action: i32, pos: BlockPos, direction: Direction, sequence: i32) {
        use player_action::*;
        if !self.player(id).movement.has_client_loaded() {
            return;
        }
        self.player_mut(id).reset_last_action_time();
        match action {
            STAB => {}
            SWAP_ITEM_WITH_OFFHAND => {
                if !self.is_spectator(id) {
                    let p = self.player_mut(id);
                    let swap = p.item_in_hand(InteractionHand::OffHand);
                    let main = p.item_in_hand(InteractionHand::MainHand);
                    p.set_item_in_hand(InteractionHand::OffHand, main);
                    p.set_item_in_hand(InteractionHand::MainHand, swap);
                    self.stop_using_item(id);
                }
            }
            DROP_ITEM => {
                if !self.is_spectator(id) {
                    self.server_player_drop(id, false);
                }
            }
            DROP_ALL_ITEMS => {
                if !self.is_spectator(id) {
                    self.server_player_drop(id, true);
                }
            }
            RELEASE_USE_ITEM => self.release_using_item(id),
            START_DESTROY_BLOCK | CHANGE_DESTROY_DIRECTION | ABORT_DESTROY_BLOCK | STOP_DESTROY_BLOCK => {
                let action = match action {
                    START_DESTROY_BLOCK => BlockBreakAction::StartDestroyBlock,
                    CHANGE_DESTROY_DIRECTION => BlockBreakAction::ChangeDestroyDirection,
                    ABORT_DESTROY_BLOCK => BlockBreakAction::AbortDestroyBlock,
                    _ => BlockBreakAction::StopDestroyBlock,
                };
                let max_y = self.chunks.max_y();
                self.handle_block_break_action(id, pos, action, direction, max_y);
                self.ack_block_changes_up_to(id, sequence);
            }
            _ => {}
        }
    }

    fn ack_block_changes_up_to(&mut self, id: EntityId, sequence: i32) {
        let p = self.player_mut(id);
        p.ack_block_changes_up_to = p.ack_block_changes_up_to.max(sequence);
    }

    fn handle_use_item_on(&mut self, id: EntityId, hand: InteractionHand, hit: BlockHitResult, sequence: i32) {
        if !self.player(id).movement.has_client_loaded() {
            return;
        }
        self.ack_block_changes_up_to(id, sequence);
        let pos = hit.block_pos;
        if !self.is_within_block_interaction_range(id, pos, 1.0) {
            return;
        }
        let distance = hit.location.subtract(Vec3::at_center_of(pos));
        let limit = 1.0000001;
        if distance.x.abs() >= limit || distance.y.abs() >= limit || distance.z.abs() >= limit {
            let location = hit.location;
            warn!(
                "Rejecting UseItemOnPacket from {}: Location ({}, {}, {}) too far away from hit block BlockPos{{x={}, y={}, z={}}}.",
                self.player(id).profile.name,
                crate::nbt::java_double_to_string(location.x),
                crate::nbt::java_double_to_string(location.y),
                crate::nbt::java_double_to_string(location.z),
                pos.x,
                pos.y,
                pos.z
            );
            return;
        }
        let direction = hit.direction;
        self.player_mut(id).reset_last_action_time();
        let (min_y, max_y) = (self.chunks.min_y(), self.chunks.max_y());
        if pos.y > max_y {
            self.send_build_limit_message(id, true, max_y);
            return;
        } else if pos.y < min_y {
            self.send_build_limit_message(id, false, min_y);
            return;
        }
        let item_stack = self.player(id).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        let swing_animation = item_stack.interact_animation();
        if self.is_under_spawn_protection(pos, id) {
            self.send_spawn_protection_message(id, pos);
        } else if self.player(id).movement.awaiting_position_from_client.is_none() && self.may_interact(id, pos) {
            let result = self.use_item_on(id, hand, hit);
            if result.consumes_action() {
                let item_after = self.player(id).item_in_hand(hand).filter(|stack| stack.is(item_stack.get_item())).unwrap_or_else(ItemStack::empty);
                crate::advancements::triggers::criteria_triggers::ANY_BLOCK_USE.trigger(self, id, pos, &item_after);
            }
            let placement_attempt = was_block_placement_attempt(self.player(id), &item_stack);
            let swing = |level: &mut ServerLevel| {
                if let InteractionResult::Success { swing_source, .. } = result
                    && result.should_swing()
                {
                    level.swing_and_reset_attack_strength(id, hand, swing_animation, swing_source != SwingSource::Predicted);
                }
            };
            if direction == Direction::Up && !result.consumes_action() && pos.y >= max_y && placement_attempt {
                self.send_build_limit_message(id, true, max_y);
            } else {
                swing(self);
            }
            if !result.consumes_action() && placement_attempt {
                if direction == Direction::Up && pos.y >= max_y {
                    self.send_build_limit_message(id, true, max_y);
                } else if direction == Direction::Down && pos.y <= min_y {
                    self.send_build_limit_message(id, false, min_y);
                }
            } else {
                swing(self);
            }
        } else {
            self.send_build_limit_message(id, true, max_y);
        }
        for p in [pos, pos.relative(direction)] {
            let state = crate::world::level::BlockGetter::block_state(self, p);
            self.player(id).connection.send(clientbound::BLOCK_UPDATE, |w| {
                w.position(p.x, p.y, p.z);
                w.varint(state.id() as i32);
            });
        }
    }

    fn handle_attack(&mut self, id: EntityId, entity_id: EntityId) {
        if !self.player(id).movement.has_client_loaded() || self.is_spectator(id) {
            return;
        }
        let Some(target) = self.entity_ref(entity_id) else {
            self.player_mut(id).reset_last_action_time();
            return;
        };
        let (target_pos, target_bounds) = (target.block_position(), target.bounding_box());
        self.player_mut(id).reset_last_action_time();
        if !self.world_border.is_within_bounds(target_pos.x as f64, target_pos.z as f64) {
            return;
        }
        let main_hand_item = self.item_by_slot(id, wafer_data::EquipmentSlot::Mainhand).unwrap_or_else(ItemStack::empty);
        if !self.is_within_attack_range(id, &main_hand_item, &target_bounds, 3.0) || main_hand_item.has(crate::world::item::component::PIERCING_WEAPON) {
            return;
        }
        let invalid = match self.entities.get(&entity_id) {
            Some(crate::world::entity::LevelEntity::Item(_) | crate::world::entity::LevelEntity::ExperienceOrb(_)) => true,
            Some(crate::world::entity::LevelEntity::Projectile(projectile)) => projectile.abstract_arrow().is_some() && !self.is_attackable(entity_id),
            _ => entity_id == id,
        };
        if invalid {
            self.player_mut(id).disconnect("multiplayer.disconnect.invalid_entity_attacked");
            warn!("Player {} tried to attack an invalid entity", self.player(id).profile.name);
        } else if !self.cannot_attack_with_item(id, &main_hand_item, 5) {
            self.player_attack(id, entity_id);
        }
    }

    fn handle_interact(&mut self, id: EntityId, entity_id: EntityId, hand: InteractionHand, location: Vec3, using_secondary_action: bool) {
        if !self.player(id).movement.has_client_loaded() {
            return;
        }
        let target_pos = self.entity_ref(entity_id).map(|target| target.block_position());
        let p = self.player_mut(id);
        p.reset_last_action_time();
        p.entity.set_shift_key_down(using_secondary_action);
        let Some(target_pos) = target_pos else { return };
        if !self.world_border.is_within_bounds(target_pos.x as f64, target_pos.z as f64) || !self.is_within_entity_interaction_range(id, entity_id, 3.0) {
            return;
        }
        let used_item_stack = self.player(id).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        let result = self.interact_on(id, entity_id, hand, location);
        if let InteractionResult::Success { swing_source, was_item_interaction } = result {
            let awarded_for_stack = if was_item_interaction { used_item_stack.clone() } else { ItemStack::empty() };
            crate::advancements::triggers::criteria_triggers::PLAYER_INTERACTED_WITH_ENTITY.trigger(self, id, &awarded_for_stack, entity_id);
            if result.should_swing() {
                let swing_animation = used_item_stack.interact_animation();
                self.swing_and_reset_attack_strength(id, hand, swing_animation, swing_source != SwingSource::Predicted);
            }
        }
    }

    fn handle_use_item(&mut self, id: EntityId, hand: InteractionHand, sequence: i32, y_rot: f32, x_rot: f32) {
        if !self.player(id).movement.has_client_loaded() {
            return;
        }
        self.ack_block_changes_up_to(id, sequence);
        self.player_mut(id).reset_last_action_time();
        let Some(item_stack) = self.player(id).item_in_hand(hand).filter(|s| !s.is_empty()) else { return };
        let swing_animation = item_stack.interact_animation();
        let target_y_rot = mth::wrap_degrees(y_rot);
        let target_x_rot = mth::wrap_degrees(x_rot);
        let entity = &mut self.player_mut(id).entity;
        if target_x_rot != entity.x_rot || target_y_rot != entity.y_rot {
            entity.abs_snap_rotation_to(target_y_rot, target_x_rot);
        }
        let result = self.use_item(id, hand);
        if let InteractionResult::Success { swing_source, .. } = result
            && result.should_swing()
        {
            self.swing_and_reset_attack_strength(id, hand, swing_animation, swing_source != SwingSource::Predicted);
        }
    }

    fn handle_container_click(&mut self, id: EntityId, r: &mut Reader) -> anyhow::Result<()> {
        self.player_mut(id).reset_last_action_time();
        let window_id = r.varint()?;
        let state_id = r.varint()?;
        let slot = r.i16()?;
        let button = r.u8()?;
        let mode = r.varint()?;
        let mut claims = Vec::new();
        let count = r.varint()?;
        if count > 128 {
            anyhow::bail!("{count} elements exceeded max size of: 128");
        }
        for _ in 0..count.max(0) {
            let slot = r.i16()?;
            claims.retain(|(s, _): &(usize, HashedStack)| *s != slot as usize);
            claims.push((slot as usize, HashedStack::read(r)?));
        }
        let carried_claim = HashedStack::read(r)?;
        if self.container_menu_id(id) != window_id {
            return Ok(());
        }
        let claims = claims.into_iter().map(|(s, h)| (s as i16 as i32, h)).collect();
        self.handle_container_menu_click(id, state_id, slot as i32, button as i32, mode, claims, carried_claim)
            .map_err(|()| anyhow::anyhow!("Container click on slot {slot} failed"))
    }

    fn handle_set_creative_mode_slot(&mut self, id: EntityId, slot: i16, stack: Option<ItemStack>) {
        let p = self.player_mut(id);
        if !p.abilities.instabuild {
            return;
        }
        let drop = slot < 0;
        let valid_slot = (1..=45).contains(&slot);
        let valid_data = stack.as_ref().is_none_or(|s| s.is_empty() || s.count <= s.max_stack_size());
        if valid_slot && valid_data {
            let slot = slot as usize;
            let stack = stack.unwrap_or_else(ItemStack::empty);
            let Some(mut menu) = self.player_mut(id).inventory_menu.take() else { return };
            self.slot_set_by_player(&mut menu, id, slot, stack.clone());
            menu.set_remote_slot(slot, &stack);
            self.menu_broadcast_changes(&mut menu, id);
            self.restore_container_menu(id, menu);
        } else if drop && valid_data {
            let p = self.player_mut(id);
            if p.drop_spam_throttler.is_under_threshold() {
                p.drop_spam_throttler.increment();
                if let Some(stack) = stack.filter(|s| !s.is_empty()) {
                    self.player_drop(id, stack, true, Prediction::Predicted);
                }
            } else {
                warn!("Player {} was dropping items too fast in creative mode, ignoring.", p.profile.name);
            }
        }
    }


    pub fn tick_connection(&mut self, id: EntityId) -> bool {
        self.tick_connection_start(id);
        self.pending_portal_teleport = None;
        self.tick_connection_end(id)
    }

    pub fn tick_connection_start(&mut self, id: EntityId) {
        let p = self.player_mut(id);
        if p.ack_block_changes_up_to > -1 {
            let sequence = p.ack_block_changes_up_to;
            p.connection.send(clientbound::BLOCK_CHANGED_ACK, |w| w.varint(sequence));
            p.ack_block_changes_up_to = -1;
        }
        self.tick_player_start(id);
    }

    pub fn tick_connection_end(&mut self, id: EntityId) -> bool {
        if self.tick_player_end(id) {
            return false;
        }
        let p = self.player_mut(id);
        p.chat.tick();
        p.drop_spam_throttler.tick();
        if p.keep_alive_time.elapsed() >= KEEP_ALIVE_INTERVAL {
            if p.keep_alive_challenge.is_some() {
                p.disconnect("disconnect.timeout");
            } else {
                let challenge = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
                p.keep_alive_challenge = Some(challenge);
                p.keep_alive_time = Instant::now();
                p.connection.send(clientbound::KEEP_ALIVE, |w| w.i64(challenge));
            }
        }
        !self.player(id).disconnected
    }

    pub fn send_next_chunks(&mut self, id: EntityId) {
        self.process_chunk_events();
        let center = self.player(id).entity.chunk_position();
        let chunks = &self.chunks;
        let batch = self.players.get_mut(&id).expect("player in this level").chunk_sender.next_batch(center, |c| chunks.chunk_to_send(c));
        if batch.is_empty() {
            return;
        }
        let connection = self.player(id).connection.clone();
        connection.send(clientbound::CHUNK_BATCH_START, |_| {});
        for &pos in &batch {
            connection.send(clientbound::LEVEL_CHUNK_WITH_LIGHT, |w| self.chunks.write_chunk_with_light(pos, w));
        }
        connection.send(clientbound::CHUNK_BATCH_FINISHED, |w| w.varint(batch.len() as i32));
    }
}

#[cfg(test)]
mod tests {
    use wafer_core::Vec3;
    use wafer_data::items;
    use wafer_protocol::Writer;
    use wafer_protocol::nbt::Nbt;

    use super::*;
    use crate::server::level::testing;
    use crate::world::item::component as dc;

    fn creative_slot(level: &mut ServerLevel, player: EntityId, slot: i16, stack: &ItemStack) -> anyhow::Result<()> {
        let mut w = Writer::new();
        w.i16(slot);
        w.varint(stack.count);
        w.varint(stack.item.0 as i32);
        stack.components_patch().write(&mut w, true).unwrap();
        level.handle_game_packet(player, &Packet { id: serverbound::SET_CREATIVE_MODE_SLOT, data: w.into_inner() })
    }

    #[test]
    fn creative_players_set_slots_with_components() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        let mut sword = ItemStack::new(items::DIAMOND_SWORD, 1);
        sword.set(dc::DAMAGE, Some(Nbt::Int(7)));
        sword.set(dc::CUSTOM_NAME, Some(Nbt::String("Edge".into())));
        creative_slot(&mut level, player, 36, &sword).unwrap();
        assert_eq!(level.player(player).inventory.slots[36], None);
        level.player_mut(player).abilities.instabuild = true;
        creative_slot(&mut level, player, 36, &sword).unwrap();
        assert_eq!(level.player(player).inventory.slots[36], Some(sword.clone()));
        creative_slot(&mut level, player, 37, &sword.copy_with_count(2)).unwrap();
        assert_eq!(level.player(player).inventory.slots[37], None);
        assert!(creative_slot(&mut level, player, 37, &ItemStack::new(items::STONE, 100)).is_err());
    }

    fn player_action(level: &mut ServerLevel, player: EntityId, action: i32) {
        let mut w = Writer::new();
        w.varint(action);
        w.position(0, -60, 0);
        w.u8(1);
        w.varint(0);
        level.handle_game_packet(player, &Packet { id: serverbound::PLAYER_ACTION, data: w.into_inner() }).unwrap();
    }

    fn dropped(level: &ServerLevel) -> Vec<i32> {
        level.entities.values().filter_map(|e| if let crate::world::entity::LevelEntity::Item(item) = e { Some(item.item.count) } else { None }).collect()
    }

    #[test]
    fn player_actions_follow_vanilla_branches() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        level.handle_player_loaded(player);
        level.player_mut(player).inventory.slots[crate::world::item::HOTBAR_START] = Some(ItemStack::new(items::STONE, 5));
        player_action(&mut level, player, player_action::DROP_ITEM);
        assert_eq!(level.player(player).inventory.slots[crate::world::item::HOTBAR_START].as_ref().map(|s| s.count), Some(4));
        assert_eq!(dropped(&level), [1]);
        player_action(&mut level, player, player_action::DROP_ALL_ITEMS);
        assert_eq!(level.player(player).inventory.slots[crate::world::item::HOTBAR_START], None);
        assert_eq!(dropped(&level).iter().sum::<i32>(), 5);
        level.player_mut(player).inventory.slots[crate::world::item::HOTBAR_START] = Some(ItemStack::new(items::DIRT, 1));
        level.set_game_mode(player, GameType::Spectator);
        player_action(&mut level, player, player_action::SWAP_ITEM_WITH_OFFHAND);
        player_action(&mut level, player, player_action::DROP_ITEM);
        assert_eq!(level.player(player).inventory.slots[crate::world::item::HOTBAR_START].as_ref().map(|s| s.item), Some(items::DIRT));
        assert_eq!(level.player(player).inventory.slots[crate::world::item::OFFHAND], None);
        level.set_game_mode(player, GameType::Survival);
        player_action(&mut level, player, player_action::SWAP_ITEM_WITH_OFFHAND);
        assert_eq!(level.player(player).inventory.slots[crate::world::item::OFFHAND].as_ref().map(|s| s.item), Some(items::DIRT));
        let mut w = Writer::new();
        w.varint(9);
        assert!(level.handle_game_packet(player, &Packet { id: serverbound::PLAYER_ACTION, data: w.into_inner() }).is_err());
    }

    #[test]
    fn using_items_resets_attack_strength() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        level.handle_player_loaded(player);
        level.player_mut(player).inventory.slots[crate::world::item::HOTBAR_START] = Some(ItemStack::new(items::STONE, 2));
        level.player_mut(player).attack_strength_ticker = 10;
        let mut w = Writer::new();
        w.varint(0);
        w.position(2, -61, 0);
        w.varint(1);
        w.f32(0.5);
        w.f32(1.0);
        w.f32(0.5);
        w.bool(false);
        w.bool(false);
        w.varint(1);
        level.handle_game_packet(player, &Packet { id: serverbound::USE_ITEM_ON, data: w.into_inner() }).unwrap();
        assert_eq!(crate::world::level::BlockGetter::block_state(&mut level, BlockPos::new(2, -60, 0)), wafer_data::blocks::STONE.default_state());
        assert_eq!(level.player(player).attack_strength_ticker, 0);
        level.player_mut(player).inventory.slots[crate::world::item::HOTBAR_START] = Some(ItemStack::new(items::SNOWBALL, 2));
        level.player_mut(player).attack_strength_ticker = 10;
        level.tick_connection(player);
        let mut w = Writer::new();
        w.varint(0);
        w.varint(2);
        w.f32(30.0);
        w.f32(20.0);
        level.handle_game_packet(player, &Packet { id: serverbound::USE_ITEM, data: w.into_inner() }).unwrap();
        assert_eq!(level.player(player).attack_strength_ticker, 0);
        assert_eq!(level.player(player).entity.y_rot_o, 30.0);
    }

    #[test]
    fn saved_inventories_keep_components() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        let mut sword = ItemStack::new(items::DIAMOND_SWORD, 1);
        sword.set(dc::DAMAGE, Some(Nbt::Int(7)));
        sword.remove(dc::ATTRIBUTE_MODIFIERS);
        level.player_mut(player).inventory.slots[36] = Some(sword.clone());
        let saved = level.player(player).to_nbt("minecraft:overworld");
        let entry = &saved.get("Inventory").unwrap().as_list()[0];
        let expected = crate::nbt::parse_fully(
            "{Slot:0b,id:\"minecraft:diamond_sword\",count:1,components:{\"minecraft:damage\":7,\"!minecraft:attribute_modifiers\":{}}}",
        )
        .unwrap();
        assert_eq!(entry, &expected);
        let other = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        level.player_mut(other).load_nbt(&saved);
        assert_eq!(level.player(other).inventory.slots[36], Some(sword));
    }
}

fn was_block_placement_attempt(player: &crate::server::level::server_player::ServerPlayer, stack: &ItemStack) -> bool {
    if stack.is_empty() {
        return false;
    }
    let info = stack.item.info();
    (info.classes.contains(&"BlockItem")
        || info.classes.contains(&"BucketItem") && {
            let fields: serde_json::Value = serde_json::from_str(info.fields).unwrap_or_default();
            fields["content"].as_str().is_some_and(|c| c != "minecraft:empty")
        })
        && !player.cooldowns.is_on_cooldown(stack)
}

impl ServerLevel {
    fn handle_set_beacon_packet(&mut self, id: EntityId, primary: Option<wafer_data::MobEffect>, secondary: Option<wafer_data::MobEffect>) {
        if self.player(id).container_menu.as_ref().is_none_or(|menu| menu.kind != crate::world::inventory::abstract_container_menu::MenuKind::Beacon) {
            return;
        }
        let Some(mut menu) = self.player_mut(id).container_menu.take() else { return };
        if self.menu_still_valid(&menu, id) && !self.beacon_menu_update_effects(&mut menu, id, primary, secondary) {
            self.player_mut(id).disconnect("multiplayer.disconnect.generic");
            warn!("Player {} tried to set invalid beacon effects", self.entity_to_string(id));
        }
        if self.players.contains_key(&id) && self.player(id).container_menu.is_none() {
            self.player_mut(id).container_menu = Some(menu);
        }
    }

    fn handle_rename_item(&mut self, id: EntityId, name: &str) {
        if self.player(id).container_menu.as_ref().is_none_or(|menu| menu.kind != MenuKind::Anvil) {
            return;
        }
        let Some(mut menu) = self.player_mut(id).container_menu.take() else { return };
        if self.menu_still_valid(&menu, id) {
            self.anvil_menu_set_item_name(&mut menu, id, name);
        }
        self.restore_container_menu(id, menu);
    }

    fn handle_container_slot_state_changed(&mut self, id: EntityId, slot_id: i32, container_id: i32, new_state: bool) {
        if self.is_spectator(id) || container_id != self.container_menu_id(id) {
            return;
        }
        let crafter_block_entity = self
            .player(id)
            .container_menu
            .as_ref()
            .filter(|menu| menu.kind == MenuKind::Crafter)
            .and_then(|menu| menu.container)
            .and_then(ContainerRef::single_block);
        if let Some(pos) = crafter_block_entity {
            self.crafter_set_slot_state(pos, slot_id, new_state);
        }
    }

    fn handle_recipe_book_seen_recipe_packet(&mut self, id: EntityId, recipe: RecipeDisplayId) {
        if let Some(entry) = RecipeManager::get().get_recipe_from_display(recipe) {
            self.player_mut(id).recipe_book.remove_highlight(entry.parent);
        }
    }

    fn handle_recipe_book_change_settings_packet(&mut self, id: EntityId, book_type: RecipeBookType, is_open: bool, is_filtering: bool) {
        self.player_mut(id).recipe_book.base.set_book_setting(book_type, is_open, is_filtering);
    }

    fn handle_place_recipe(&mut self, id: EntityId, container_id: i32, recipe: RecipeDisplayId, use_max_items: bool) {
        self.player_mut(id).reset_last_action_time();
        if self.is_spectator(id) || self.container_menu_id(id) != container_id {
            return;
        }
        let Some(mut menu) = self.take_container_menu(id) else { return };
        if self.menu_still_valid(&menu, id)
            && let Some(display_info) = RecipeManager::get().get_recipe_from_display(recipe)
            && let Some(recipe) = RecipeManager::get().by_key(display_info.parent)
            && self.player(id).recipe_book.contains(recipe.id)
            && menu.is_recipe_book_menu()
            && !recipe.value.placement_info().is_impossible_to_place()
        {
            let allow_dropping_items_to_clear = self.player(id).game_mode.game_type.is_creative();
            match self.recipe_book_menu_handle_placement(&mut menu, id, use_max_items, allow_dropping_items_to_clear, recipe) {
                Some(PostPlaceAction::PlaceGhostRecipe) => self.player(id).connection.send(clientbound::PLACE_GHOST_RECIPE, |w| {
                    w.varint(menu.container_id);
                    display_info.display.display.write(w);
                }),
                Some(PostPlaceAction::Nothing) => {}
                None => error!(
                    "Failed to handle packet ServerboundPlaceRecipePacket[containerId={container_id}, recipe=RecipeDisplayId[index={}], useMaxItems={use_max_items}], suppressing error",
                    display_info.display.id.0
                ),
            }
        }
        self.restore_container_menu(id, menu);
    }

    fn handle_sign_update(&mut self, id: EntityId, pos: BlockPos, lines: Vec<String>, slot: SignTextSlot) {
        let lines: Vec<FilteredText> = lines.iter().map(|line| FilteredText::pass_through(crate::util::string_util::strip_color(line))).collect();
        self.update_sign_text(id, pos, slot, &lines);
    }

    fn update_sign_text(&mut self, id: EntityId, pos: BlockPos, slot: SignTextSlot, lines: &[FilteredText]) {
        self.player_mut(id).reset_last_action_time();
        if self.is_loaded(pos) {
            if self.sign(pos).is_none() {
                return;
            }
            self.sign_update_sign_text(pos, id, slot, lines);
        }
    }

    fn handle_pick_item_from_block(&mut self, id: EntityId, pos: BlockPos, include_data: bool) {
        if !self.is_within_block_interaction_range(id, pos, 1.0) || !self.is_loaded(pos) {
            return;
        }
        let state = self.block_state(pos);
        let include_data = self.player(id).abilities.instabuild && include_data;
        let mut stack = crate::world::level::block::of(state).get_clone_item_stack(self, pos, state, include_data);
        if stack.is_empty() {
            return;
        }
        if include_data {
            self.add_block_data_to_item(state, pos, &mut stack);
        }
        self.try_pick_item(id, stack);
    }

    fn add_block_data_to_item(&self, state: wafer_data::BlockState, pos: BlockPos, stack: &mut ItemStack) {
        if !state.has_block_entity() {
            return;
        }
        let Some(block_entity) = self.chunks.block_entity_ref(pos) else { return };
        let wafer_protocol::nbt::Nbt::Compound(mut output) = block_entity.save_custom_only() else { return };
        block_entity.remove_components_from_tag(&mut output);
        output.retain(|(key, _)| key != "id");
        if output.is_empty() {
            stack.set(crate::world::item::component::BLOCK_ENTITY_DATA, None);
        } else {
            output.push(("id".into(), wafer_protocol::nbt::Nbt::String(block_entity.ty().name().into())));
            stack.set(crate::world::item::component::BLOCK_ENTITY_DATA, Some(wafer_protocol::nbt::Nbt::Compound(output)));
        }
        for (component, value) in block_entity.collect_components() {
            stack.set(component, value);
        }
    }

    pub(crate) fn handle_pick_item_from_entity(&mut self, id: EntityId, target: EntityId) -> bool {
        let exists = self.players.contains_key(&target) || self.entities.contains_key(&target);
        if !exists || !self.is_within_entity_interaction_range(id, target, 3.0) {
            return false;
        }
        if let Some(stack) = self.pick_result(target) {
            self.try_pick_item(id, stack);
        }
        true
    }

    fn pick_result(&self, entity: EntityId) -> Option<ItemStack> {
        match self.entities.get(&entity) {
            Some(crate::world::entity::LevelEntity::Minecart(minecart)) => Some(ItemStack::new(minecart.get_pick_result(), 1)),
            Some(crate::world::entity::LevelEntity::Boat(boat)) => Some(ItemStack::new(boat.get_pick_result(), 1)),
            _ => None,
        }
    }

    fn try_pick_item(&mut self, id: EntityId, stack: ItemStack) {
        let p = self.player_mut(id);
        match p.inventory.find_slot_matching_item(&stack) {
            Some(slot) if crate::world::item::Inventory::is_hotbar_slot(slot) => p.inventory.selected = slot,
            Some(slot) => {
                p.inventory.pick_slot(slot);
            }
            None if p.abilities.instabuild => {
                p.inventory.add_and_pick_item(stack);
            }
            None => {}
        }
        let selected = p.inventory.selected as i32;
        p.connection.send(clientbound::SET_HELD_SLOT, |w| w.varint(selected));
        self.broadcast_inventory_menu_changes(id);
    }
}

impl crate::server::Server {
    pub(in crate::server) fn handle_pick_item_from_entity(&mut self, id: EntityId, packet: &Packet) -> anyhow::Result<()> {
        let mut r = packet.reader();
        let target = r.varint()?;
        let include_data = r.bool()?;
        let level = self.entity_level_mut(id);
        if level.handle_pick_item_from_entity(id, target) && include_data && level.can_use_game_master_blocks(id) && level.is_player(target) {
            let source = crate::commands::source::CommandSourceStack::player(self, id);
            crate::server::commands::fetch_profile_command::print_for_avatar(self, &source, target);
        }
        Ok(())
    }

    pub(in crate::server) fn handle_client_information(&mut self, id: EntityId, packet: &Packet) -> anyhow::Result<()> {
        let info = read_client_information(&packet.data)?;
        let player = self.player_mut(id);
        let was_hat_shown = player.is_model_part_shown(PlayerModelPart::Hat);
        player.update_options(info);
        let show_hat = player.is_model_part_shown(PlayerModelPart::Hat);
        if show_hat != was_hat_shown {
            let uuid = player.profile.uuid;
            let packet = crate::network::connection::encode(clientbound::PLAYER_INFO_UPDATE, |w| {
                w.u8(1 << 7);
                w.varint(1);
                w.uuid(uuid);
                w.bool(show_hat);
            });
            for other in self.players() {
                other.connection.send_raw(packet.clone());
            }
        }
        Ok(())
    }

    pub(in crate::server) fn handle_set_game_rule(&mut self, id: EntityId, packet: &Packet) -> anyhow::Result<()> {
        let mut r = packet.reader();
        let count = r.varint()?;
        if count < 0 {
            anyhow::bail!("Negative game rule count {count}");
        }
        let mut entries = Vec::new();
        for _ in 0..count {
            let game_rule_key = crate::resources::identifier::parse(r.string(32767)?).map_err(anyhow::Error::msg)?;
            let value = r.string(32767)?.to_owned();
            entries.push((game_rule_key, value));
        }
        let player = self.player(id);
        if self.profile_permission_level(player.profile.uuid) < 2 {
            warn!("Player {} tried to set game rule values without required permissions", player.profile.name);
            return Ok(());
        }
        for (game_rule_key, value) in entries {
            match wafer_data::GameRule::by_name(&game_rule_key).filter(|_| game_rule_key.starts_with("minecraft:")) {
                Some(rule) => self.set_game_rule_value(rule, &value),
                None => warn!("Received request to set unknown game rule: ResourceKey[minecraft:game_rule / {game_rule_key}]"),
            }
        }
        Ok(())
    }

    fn set_game_rule_value(&mut self, rule: wafer_data::GameRule, value: &str) {
        let units: Vec<u16> = value.encode_utf16().collect();
        let mut reader = crate::commands::brigadier::StringReader::new(&units);
        let parsed = match rule.info().kind {
            wafer_data::GameRuleType::Bool => reader.read_boolean().ok().map(i32::from),
            wafer_data::GameRuleType::Int => reader.read_int().ok().filter(|v| (rule.info().min..=rule.info().max).contains(v)),
        };
        let Some(parsed_value) = parsed else { return };
        if reader.can_read() {
            return;
        }
        if !self.game_rules.has(rule) {
            warn!("Tried to set invalid game rule 'minecraft:{}' to value '{}'", rule.info().name, crate::world::level::gamerules::GameRules::serialize(rule, parsed_value));
        } else {
            self.set_game_rule(rule, parsed_value);
        }
        self.broadcast_game_rule_change_to_operators(rule, parsed_value);
    }

    fn broadcast_game_rule_change_to_operators(&mut self, rule: wafer_data::GameRule, value: i32) {
        let serialized = crate::world::level::gamerules::GameRules::serialize(rule, value);
        let message = Component::translatable_with("commands.gamerule.set", vec![rule.info().name.into(), serialized.as_str().into()]);
        let ops: Vec<EntityId> = self.player_list.iter().copied().filter(|&op| self.is_op(self.player(op).profile.uuid)).collect();
        for op in ops {
            self.send_player_system_message(op, &message, false);
        }
    }

    pub(in crate::server) fn send_game_rule_values(&mut self, id: EntityId) {
        let player = self.player(id);
        if self.profile_permission_level(player.profile.uuid) < 2 {
            warn!("Player {} tried to request game rule values without required permissions", player.profile.name);
            return;
        }
        let game_rules = &self.game_rules;
        let values: Vec<(wafer_data::GameRule, String)> = game_rules.available().map(|rule| (rule, crate::world::level::gamerules::GameRules::serialize(rule, game_rules.get(rule)))).collect();
        player.connection.send(clientbound::GAME_RULE_VALUES, |w| {
            w.varint(values.len() as i32);
            for (rule, value) in &values {
                w.string(&format!("minecraft:{}", rule.info().name));
                w.string(value);
            }
        });
    }

    pub(in crate::server) fn handle_change_game_mode(&mut self, id: EntityId, packet: &Packet) -> anyhow::Result<()> {
        let mode = GameType::by_id(packet.reader().varint()?);
        let player = self.player(id);
        if self.profile_permission_level(player.profile.uuid) < 2 {
            let name = Component::translatable(&format!("selectWorld.gameMode.{}", mode.name())).get_string();
            warn!("Player {} tried to change game mode to {name} without required permissions", player.profile.name);
        } else {
            crate::server::commands::game_mode_command::set_game_mode(self, id, mode);
        }
        Ok(())
    }

    pub(in crate::server) fn handle_resource_pack_response(&mut self, id: EntityId, packet: &Packet) -> anyhow::Result<()> {
        const DECLINED: i32 = 1;
        let mut r = packet.reader();
        let (pack, action) = (r.uuid()?, r.varint()?);
        if !(0..8).contains(&action) {
            anyhow::bail!("unknown resource pack action {action}");
        }
        if action == DECLINED && self.config.server_resource_pack.as_ref().is_some_and(|info| info.is_required) {
            let player = self.player_mut(id);
            info!("Disconnecting {} due to resource pack {} rejection", player.profile.name, crate::world::level::storage::format_uuid(pack));
            player.disconnect("multiplayer.requiredTexturePrompt.disconnect");
        }
        Ok(())
    }
}

#[cfg(test)]
mod pick_item_tests {
    use wafer_core::{BlockPos, Vec3};
    use wafer_data::properties::{LEVEL, PISTON_TYPE, PistonType};
    use wafer_data::{Item, blocks, items};

    use crate::server::level::game_mode::GameType;
    use crate::server::level::{ServerLevel, testing};
    use crate::world::entity::EntityId;
    use crate::world::item::{HOTBAR_START, ItemStack, MAIN_START};
    use crate::world::level::BlockGetter;
    use crate::world::level::update_flags::UPDATE_ALL;

    const TARGET: BlockPos = BlockPos::new(0, -59, 2);

    fn setup(game_type: GameType) -> (ServerLevel, EntityId) {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        level.set_game_mode(player, game_type);
        (level, player)
    }

    fn held(level: &ServerLevel, player: EntityId) -> Option<Item> {
        level.player(player).inventory.held().filter(|s| !s.is_empty()).map(|s| s.item)
    }

    fn clone_item(level: &mut ServerLevel, state: wafer_data::BlockState) -> Item {
        level.set_block(TARGET, state, UPDATE_ALL);
        let state = level.block_state(TARGET);
        crate::world::level::block::of(state).get_clone_item_stack(level, TARGET, state, false).item
    }

    #[test]
    fn survival_moves_a_stored_stack_to_the_hotbar() {
        let (mut level, player) = setup(GameType::Survival);
        level.set_block(TARGET, blocks::STONE.default_state(), UPDATE_ALL);
        level.player_mut(player).inventory.slots[MAIN_START + 11] = Some(ItemStack::new(items::STONE, 5));
        level.handle_pick_item_from_block(player, TARGET, false);
        assert_eq!(held(&level, player), Some(items::STONE));
        assert!(level.player(player).inventory.slots[MAIN_START + 11].is_none());
    }

    #[test]
    fn survival_without_the_item_changes_nothing() {
        let (mut level, player) = setup(GameType::Survival);
        level.set_block(TARGET, blocks::STONE.default_state(), UPDATE_ALL);
        level.handle_pick_item_from_block(player, TARGET, false);
        assert_eq!(held(&level, player), None);
    }

    #[test]
    fn creative_adds_the_item_and_keeps_the_old_one() {
        let (mut level, player) = setup(GameType::Creative);
        level.set_block(TARGET, blocks::STONE.default_state(), UPDATE_ALL);
        for slot in 0..9 {
            level.player_mut(player).inventory.slots[HOTBAR_START + slot] = Some(ItemStack::new(items::DIRT, 1));
        }
        level.handle_pick_item_from_block(player, TARGET, false);
        assert_eq!(held(&level, player), Some(items::STONE));
        assert_eq!(level.player(player).inventory.slots[MAIN_START].as_ref().map(|s| s.item), Some(items::DIRT));
    }

    #[test]
    fn creative_with_data_copies_the_chest_contents() {
        let (mut level, player) = setup(GameType::Creative);
        level.set_block(TARGET, blocks::CHEST.default_state(), UPDATE_ALL);
        level.get_block_entity(TARGET).unwrap().container_base_mut().unwrap().items[0] = ItemStack::new(items::DIAMOND, 3);
        level.handle_pick_item_from_block(player, TARGET, true);
        let held = level.player(player).inventory.held().cloned().unwrap();
        assert_eq!(held.item, items::CHEST);
        assert!(held.get(crate::world::item::component::CONTAINER).is_some());
        assert!(held.get(crate::world::item::component::BLOCK_ENTITY_DATA).is_none());
        let (mut level, player) = setup(GameType::Survival);
        level.set_block(TARGET, blocks::CHEST.default_state(), UPDATE_ALL);
        level.player_mut(player).inventory.slots[HOTBAR_START + 4] = Some(ItemStack::new(items::CHEST, 1));
        level.handle_pick_item_from_block(player, TARGET, true);
        assert_eq!(level.player(player).inventory.selected, 4);
    }

    #[test]
    fn out_of_reach_is_ignored() {
        let (mut level, player) = setup(GameType::Creative);
        let far = BlockPos::new(0, -59, 9);
        level.set_block(far, blocks::STONE.default_state(), UPDATE_ALL);
        level.handle_pick_item_from_block(player, far, false);
        assert_eq!(held(&level, player), None);
    }

    #[test]
    fn clone_items_follow_vanilla_blocks() {
        let (mut level, _) = setup(GameType::Creative);
        assert_eq!(clone_item(&mut level, blocks::WHEAT.default_state()), items::WHEAT_SEEDS);
        assert_eq!(clone_item(&mut level, blocks::CARROTS.default_state()), items::CARROT);
        assert_eq!(clone_item(&mut level, blocks::CANDLE_CAKE.default_state()), items::CAKE);
        assert_eq!(clone_item(&mut level, blocks::POTTED_POPPY.default_state()), items::POPPY);
        assert_eq!(clone_item(&mut level, blocks::FLOWER_POT.default_state()), items::FLOWER_POT);
        assert_eq!(clone_item(&mut level, blocks::PISTON_HEAD.default_state().set(PISTON_TYPE, PistonType::Sticky)), items::STICKY_PISTON);
        assert_eq!(clone_item(&mut level, blocks::BAMBOO_SAPLING.default_state()), items::BAMBOO);
        assert_eq!(clone_item(&mut level, blocks::ATTACHED_PUMPKIN_STEM.default_state()), items::PUMPKIN_SEEDS);
        assert_eq!(clone_item(&mut level, blocks::NETHER_PORTAL.default_state()), items::AIR);
        let light = blocks::LIGHT.default_state().set(LEVEL, 7);
        level.set_block(TARGET, light, UPDATE_ALL);
        let stack = crate::world::level::block::of(light).get_clone_item_stack(&level, TARGET, light, false);
        assert_eq!(stack.get(crate::world::item::component::BLOCK_STATE), Some(&crate::world::item::component::values::block_state_properties_nbt(&[("level", "7")])));
    }
}

#[cfg(test)]
mod server_tests {
    use tokio::sync::mpsc::UnboundedReceiver;
    use wafer_core::Vec3;
    use wafer_data::packets::play::{clientbound, serverbound};
    use wafer_protocol::{Reader, Writer};

    use crate::commands::perform_prefixed_command;
    use crate::commands::source::CommandSourceStack;
    use crate::network::connection::{Outbound, Packet};
    use crate::network::login::{Profile, offline_uuid};
    use crate::server::Server;
    use crate::server::level::testing;
    use crate::world::entity::EntityId;

    fn server() -> Server {
        Server::new(crate::ServerConfig { level_type: crate::LevelType::Flat, ..crate::ServerConfig::default() }).unwrap()
    }

    fn run(server: &mut Server, command: &str) {
        let source = CommandSourceStack::server(server);
        perform_prefixed_command(server, &source, command);
    }

    fn join(server: &mut Server, name: &str) -> (EntityId, UnboundedReceiver<Vec<u8>>) {
        let profile = Profile { uuid: offline_uuid(name), name: name.into(), properties: Vec::new() };
        let id = testing::server_player_with_profile(server, profile, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        let (out, packets) = Outbound::capture();
        server.levels[0].player_mut(id).connection = out;
        server.levels[0].handle_player_loaded(id);
        server.name_to_id_cache.add(crate::server::players::NameAndId { id: offline_uuid(name), name: name.into() });
        (id, packets)
    }

    fn packets(rx: &mut UnboundedReceiver<Vec<u8>>) -> Vec<(i32, Vec<u8>)> {
        let mut out = Vec::new();
        while let Ok(packet) = rx.try_recv() {
            let mut r = Reader::new(&packet);
            let id = r.varint().unwrap();
            out.push((id, packet[packet.len() - r.remaining()..].to_vec()));
        }
        out
    }

    fn packet(id: i32, write: impl FnOnce(&mut Writer)) -> Packet {
        let mut w = Writer::new();
        write(&mut w);
        Packet { id, data: w.into_inner() }
    }

    #[test]
    fn attacking_an_arrow_disconnects() {
        let mut server = server();
        let (id, _) = join(&mut server, "Alice");
        run(&mut server, "summon arrow 1.5 -60 0.5 {NoGravity:1b}");
        let level = &mut server.levels[0];
        let arrow = *level.entities.keys().next().unwrap();
        level.handle_game_packet(id, &packet(serverbound::ATTACK, |w| w.varint(arrow))).unwrap();
        assert!(level.player(id).disconnected);
    }

    #[test]
    fn hat_changes_are_broadcast() {
        let mut server = server();
        let (alice, _) = join(&mut server, "Alice");
        let (_, mut bob_packets) = join(&mut server, "Bob");
        let information = |skin_parts: u8| {
            packet(serverbound::CLIENT_INFORMATION, |w| {
                w.string("en_us");
                w.u8(10);
                w.varint(0);
                w.bool(true);
                w.u8(skin_parts);
                w.varint(1);
                w.bool(false);
                w.bool(true);
                w.varint(0);
            })
        };
        server.handle_client_information(alice, &information(1)).unwrap();
        assert!(packets(&mut bob_packets).is_empty());
        server.handle_client_information(alice, &information(0x41)).unwrap();
        let mut expected = Writer::new();
        expected.u8(0x80);
        expected.varint(1);
        expected.uuid(offline_uuid("Alice"));
        expected.bool(true);
        assert_eq!(packets(&mut bob_packets), [(clientbound::PLAYER_INFO_UPDATE, expected.into_inner())]);
    }

    #[test]
    fn game_rules_need_game_master_permission() {
        let mut server = server();
        let (alice, mut alice_packets) = join(&mut server, "Alice");
        let set_keep_inventory = packet(serverbound::SET_GAME_RULE, |w| {
            w.varint(2);
            w.string("minecraft:keep_inventory");
            w.string("true");
            w.string("minecraft:no_such_rule");
            w.string("1");
        });
        server.handle_set_game_rule(alice, &set_keep_inventory).unwrap();
        assert!(!server.game_rules.get_bool(wafer_data::game_rule::KEEP_INVENTORY));
        server.send_game_rule_values(alice);
        assert!(packets(&mut alice_packets).is_empty());
        run(&mut server, "op Alice");
        packets(&mut alice_packets);
        server.handle_set_game_rule(alice, &set_keep_inventory).unwrap();
        assert!(server.game_rules.get_bool(wafer_data::game_rule::KEEP_INVENTORY));
        assert_eq!(packets(&mut alice_packets).iter().filter(|p| p.0 == clientbound::SYSTEM_CHAT).count(), 1);
        let bad_value = packet(serverbound::SET_GAME_RULE, |w| {
            w.varint(1);
            w.string("minecraft:keep_inventory");
            w.string("false ");
        });
        server.handle_set_game_rule(alice, &bad_value).unwrap();
        assert!(server.game_rules.get_bool(wafer_data::game_rule::KEEP_INVENTORY));
        server.send_game_rule_values(alice);
        let sent = packets(&mut alice_packets);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].0, clientbound::GAME_RULE_VALUES);
    }
}
