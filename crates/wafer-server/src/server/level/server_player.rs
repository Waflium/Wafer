use std::borrow::Cow;
use std::time::Instant;

use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::packets::play::clientbound;
use wafer_data::{WORLD_VERSION, items, registry, registry_id};
use wafer_protocol::Writer;
use wafer_protocol::nbt::{Nbt, TagMap};

use super::chunk_map::ChunkTrackingView;

const LOAD_AND_SPAWN_PARENT_VEHICLE_SITE: [&str; 5] = [
    crate::world::entity::entity_save::LOAD_ENTITY_RECURSIVE_REASON,
    "net.minecraft.server.level.ServerPlayer.loadAndSpawnParentVehicle(ServerPlayer.java:520)",
    "net.minecraft.server.network.config.PrepareSpawnTask$Ready.lambda$spawn$1(PrepareSpawnTask.java:202)",
    "java.base/java.util.Optional.ifPresent(Optional.java:178)",
    "net.minecraft.server.network.config.PrepareSpawnTask$Ready.spawn(PrepareSpawnTask.java:200)",
];
use crate::advancements::triggers::criteria_triggers;
use super::game_mode::ServerPlayerGameMode;
use crate::network::chat::remote_chat_session::RemoteChatSession;
use crate::network::chat::{Arg, ChatFormatting, Component};
use crate::network::configuration::ClientInfo;
use crate::network::connection::Outbound;
use crate::network::protocol::game::{clientbound_player_combat_kill_packet, clientbound_system_chat_packet};
use crate::network::login::Profile;
use crate::util::codec::{self, DecodeResult};
use crate::core::global_pos::GlobalPos;
use crate::stats::server_recipe_book::ServerRecipeBook;
use crate::util::extra_codecs;
use crate::util::problem_reporter::PathElement;
use crate::util::tick_throttler::TickThrottler;
use crate::world::level::block::entity::base_container_block_entity::decode_item_stack_with_slot;
use crate::world::level::block::entity::int_or;
use crate::world::level::storage::tag_value_input;
use super::game_mode::GameType;
use crate::server::network::chat::ChatState;
use crate::server::network::chunk_sender::PlayerChunkSender;
use crate::server::network::movement::{Input, PlayerMovement};
use crate::world::effect::MobEffectInstance;
use crate::world::entity::attributes::{Attribute, AttributeModifier, Operation};
use crate::world::entity::data::{DataValue, ids, player_defaults};
use super::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::living_entity::{LivingEntity, remove_mob_effect, update_mob_effect};
use crate::world::entity::{Entity, EntityDimensions, EntityId, entity_event, flags};
use crate::world::scores::criteria::ObjectiveCriteria;
use crate::world::scores::score_holder::ScoreHolder;
use crate::world::scores::team::Visibility;
use wafer_data::{EquipmentSlot, MobEffect, game_rule};
use crate::world::inventory::abstract_container_menu::AbstractContainerMenu;
use crate::world::item::component as dc;
use crate::world::item::{Inventory, ItemStack, OFFHAND, inventory_slot, map_item, protocol_slot};
use crate::world::level::saveddata::maps::map_id::MapId;
use crate::world::entity::monster::warden::warden_spawn_tracker::WardenSpawnTracker;
use crate::world::entity::player::abilities::Abilities;
use crate::world::entity::player::player_model_part::PlayerModelPart;
use crate::world::level::portal::teleport_transition::{PostTeleportTransition, TeleportTransition};
use crate::world::level::storage::RespawnData;
use crate::world::InteractionHand;
use crate::stats::server_stats_counter::ServerStatsCounter;
use crate::stats::stat::Stat;
use crate::stats::stats;
use crate::stats::stats_counter::StatsCounter;
use crate::world::food::constants::{EXHAUSTION_SPRINT, EXHAUSTION_SWIM};

pub const DIMENSIONS: EntityDimensions = crate::world::entity::avatar::STANDING_DIMENSIONS;
const SERVER_PLAYER_LOGGER: &str = "net.minecraft.server.level.ServerPlayer";
const RESPAWN_ENTITY_ON_SHOULDER_FRAME: &str = "net.minecraft.server.level.ServerPlayer.respawnEntityOnShoulder(ServerPlayer.java:911)";
pub(crate) const DO_TICK_REPORT_FRAME: &str = "net.minecraft.server.level.ServerPlayer.doTick(ServerPlayer.java:818)";
const REMOVE_ENTITIES_ON_SHOULDER_LEFT_FRAME: &str = "net.minecraft.server.level.ServerPlayer.removeEntitiesOnShoulder(ServerPlayer.java:900)";
const REMOVE_ENTITIES_ON_SHOULDER_RIGHT_FRAME: &str = "net.minecraft.server.level.ServerPlayer.removeEntitiesOnShoulder(ServerPlayer.java:902)";
const HANDLE_SHOULDER_ENTITIES_FRAMES: [&str; 5] = [
    "net.minecraft.server.level.ServerPlayer.handleShoulderEntities(ServerPlayer.java:859)",
    "net.minecraft.world.entity.player.Player.aiStep(Player.java:519)",
    "net.minecraft.world.entity.LivingEntity.tick(LivingEntity.java:2886)",
    "net.minecraft.world.entity.player.Player.tick(Player.java:276)",
    "net.minecraft.server.level.ServerPlayer.doTick(ServerPlayer.java:740)",
];
const GAME_EVENT_WIN_GAME: u8 = 4;

const CREATIVE_BLOCK_INTERACTION_RANGE_MODIFIER: AttributeModifier =
    AttributeModifier { id: Cow::Borrowed("minecraft:creative_mode_block_range"), amount: 0.5, operation: Operation::AddValue };
const CREATIVE_ENTITY_INTERACTION_RANGE_MODIFIER: AttributeModifier =
    AttributeModifier { id: Cow::Borrowed("minecraft:creative_mode_entity_range"), amount: 2.0, operation: Operation::AddValue };
const WAYPOINT_TRANSMIT_RANGE_CROUCH_MODIFIER: AttributeModifier =
    AttributeModifier { id: Cow::Borrowed("minecraft:waypoint_transmit_range_crouch"), amount: -1.0, operation: Operation::AddMultipliedTotal };

#[derive(Clone, PartialEq, Debug)]
pub struct RespawnConfig {
    pub respawn_data: RespawnData,
    pub forced: bool,
}

pub struct LastRecorded {
    pub health_and_absorption: f32,
    pub food_level: i32,
    pub air_level: i32,
    pub armor: i32,
    pub level: i32,
    pub experience: i32,
}

impl Default for LastRecorded {
    fn default() -> LastRecorded {
        LastRecorded { health_and_absorption: f32::from_bits(1), food_level: i32::MIN, air_level: i32::MIN, armor: i32::MIN, level: i32::MIN, experience: i32::MIN }
    }
}

pub struct ServerPlayer {
    pub entity: Entity,
    pub profile: Profile,
    pub connection: Outbound,
    pub client: ClientInfo,
    pub requested_view_distance: i32,
    pub chunk_tracking_view: Option<ChunkTrackingView>,
    pub chunk_sender: PlayerChunkSender,
    pub last_section_pos: (i32, i32, i32),
    pub game_mode: ServerPlayerGameMode,
    pub abilities: Abilities,
    pub respawn_config: Option<RespawnConfig>,
    pub loaded_game_types: (Option<GameType>, Option<GameType>),
    pub is_changing_dimension: bool,
    pub seen_credits: bool,
    pub won_game: bool,
    pub spawn_extra_particles_on_fall: bool,
    pub raid_omen_position: Option<BlockPos>,
    pub warden_spawn_tracker: WardenSpawnTracker,
    pub entered_nether_position: Option<Vec3>,
    pub current_explosion_impact_pos: Option<Vec3>,
    pub current_explosion_cause: Option<EntityId>,
    pub levitation_start_pos: Option<Vec3>,
    pub levitation_start_time: i32,
    pub sleep_counter: i32,
    pub health: f32,
    pub food_data: crate::world::food::FoodData,
    pub experience: crate::world::entity::player::experience::Experience,
    pub last_sent_health: f32,
    pub last_sent_food: i32,
    pub last_food_saturation_zero: bool,
    pub last_sent_exp: i32,
    pub attack_strength_ticker: i32,
    pub item_swap_ticker: i32,
    pub last_item_in_main_hand: ItemStack,
    pub last_recorded: LastRecorded,
    pub last_death_location: Option<(String, BlockPos)>,
    pub inventory: Inventory,
    pub ender_chest_inventory: crate::world::inventory::player_ender_chest_container::PlayerEnderChestContainer,
    pub inventory_menu: Option<AbstractContainerMenu>,
    pub container_menu: Option<AbstractContainerMenu>,
    pub recipe_book: crate::stats::server_recipe_book::ServerRecipeBook,
    pub stats: ServerStatsCounter,
    pub advancements: crate::server::player_advancements::PlayerAdvancements,
    pub carried: Option<ItemStack>,
    pub last_window_id: i32,
    pub living: LivingEntity,
    pub movement: PlayerMovement,
    pub last_client_input: Input,
    pub known_movement: Vec3,
    pub last_action_time: Instant,
    pub starting_to_fall_position: Option<Vec3>,
    pub entered_lava_on_vehicle_position: Option<Vec3>,
    pub ack_block_changes_up_to: i32,
    pub drop_spam_throttler: TickThrottler,
    pub keep_alive_time: Instant,
    pub keep_alive_challenge: Option<i64>,
    pub address: Option<std::net::SocketAddr>,
    pub latency: i32,
    pub received_packets: i32,
    pub average_received_packets: f32,
    pub connection_tick_count: i32,
    pub chat: ChatState,
    pub chat_session: Option<RemoteChatSession>,
    pub disconnected: bool,
    pub disconnect_reason: Option<String>,
    pub disconnect_site: Option<crate::server::players::player_list::DisconnectSite>,
    pub command_suggestions: crate::server::network::server_command_suggestions_provider::ServerCommandSuggestionsProvider,
    pub post_effects: Vec<String>,
    pub post_effects_dirty: bool,
    pub camera: Option<EntityId>,
    pub ender_pearls: Vec<EntityId>,
    pub player_list_order: u64,
    pub fishing: Option<EntityId>,
    pub shoulder_entity_left: Nbt,
    pub shoulder_entity_right: Nbt,
    pub time_entity_sat_on_shoulder: i64,
    pub cooldowns: crate::world::item::item_cooldowns::ItemCooldowns,
}

fn decode_respawn_config(tag: &Nbt) -> DecodeResult<RespawnConfig> {
    if let Err(error) = codec::map(tag) {
        return DecodeResult::error(error);
    }
    let mut record = codec::Record::default();
    let respawn_data = record.absorb(RespawnData::decode_map(tag));
    let forced = record.with_default(tag, "forced", false, codec::bool_value);
    record.build(|| Some(RespawnConfig { respawn_data: respawn_data?, forced }))
}

fn read_player_mode(nbt: &Nbt, key: &str) -> Option<GameType> {
    tag_value_input::read(nbt, key, |tag| DecodeResult::from(codec::int(tag))).map(GameType::by_id)
}

pub fn is_empty_compound(tag: &Nbt) -> bool {
    matches!(tag, Nbt::Compound(fields) if fields.is_empty())
}

fn extract_parrot_variant(tag: &Nbt) -> Option<u32> {
    if is_empty_compound(tag) || tag.get("id").and_then(Nbt::as_str) != Some("minecraft:parrot") {
        return None;
    }
    let variant = match tag.get("Variant")? {
        Nbt::Float(variant) => *variant as i32,
        Nbt::Double(variant) => *variant as i32,
        other => match other.as_i64() {
            Some(variant) => variant as i32,
            None => {
                error!("Failed to read field (Variant={}): Not a number", crate::nbt::to_string(other));
                return None;
            }
        },
    };
    Some(crate::world::entity::animal::parrot::parrot::Variant::by_id(variant) as u32)
}

impl ServerPlayer {
    pub fn set_shoulder_entity_left(&mut self, tag: Nbt) {
        let variant = extract_parrot_variant(&tag);
        self.shoulder_entity_left = tag;
        self.entity.data.set(ids::PLAYER_SHOULDER_PARROT_LEFT, DataValue::OptionalUnsignedInt(variant));
    }

    pub fn set_shoulder_entity_right(&mut self, tag: Nbt) {
        let variant = extract_parrot_variant(&tag);
        self.shoulder_entity_right = tag;
        self.entity.data.set(ids::PLAYER_SHOULDER_PARROT_RIGHT, DataValue::OptionalUnsignedInt(variant));
    }
}

impl ServerPlayer {
    pub fn send_overlay_message(&self, message: &Component) {
        self.send_system_message(message, true);
    }

    pub fn send_system_message(&self, message: &Component, overlay: bool) {
        if !self.accepts_system_messages(overlay) {
            return;
        }
        let fallback = || {
            self.accepts_system_messages(false).then(|| {
                let contents = Component::literal(message.get_string_with_limit(256)).with_color(ChatFormatting::Yellow);
                let message = Component::translatable_with("multiplayer.message_not_delivered", vec![Arg::Component(contents)]).with_color(ChatFormatting::Red);
                crate::network::connection::encode(clientbound::SYSTEM_CHAT, |w| clientbound_system_chat_packet::write(w, &message, false))
            })
        };
        self.connection.send_with_fallback(
            clientbound::SYSTEM_CHAT,
            |w| clientbound_system_chat_packet::write(w, message, overlay),
            self.accepts_system_messages(false).then_some(clientbound::SYSTEM_CHAT),
            fallback,
        );
    }

    fn accepts_system_messages(&self, overlay: bool) -> bool {
        if self.client.chat_visibility == crate::network::configuration::ChatVisibility::Hidden { overlay } else { true }
    }

    pub fn update_options(&mut self, info: ClientInfo) {
        self.requested_view_distance = info.view_distance as i32;
        self.entity.data.set(ids::PLAYER_MODE_CUSTOMISATION, DataValue::Byte(info.skin_parts as i8));
        self.entity.data.set(ids::PLAYER_MAIN_HAND, DataValue::HumanoidArm(info.main_hand));
        self.client = info;
    }

    pub fn new(profile: Profile, client: ClientInfo, connection: Outbound, spawn: BlockPos) -> ServerPlayer {
        let profile_id = profile.uuid;
        let mut entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:player"), DIMENSIONS, player_defaults());
        entity.uuid = profile.uuid;
        entity.set_pos(Vec3::new(spawn.x as f64 + 0.5, spawn.y as f64, spawn.z as f64 + 0.5));
        entity.data.set(ids::HEALTH, DataValue::Float(20.0));
        let last_section_pos = (mth::floor(entity.x()) >> 4, mth::floor(entity.y()) >> 4, mth::floor(entity.z()) >> 4);
        let mut player = ServerPlayer {
            entity,
            profile,
            connection,
            requested_view_distance: 0,
            client: ClientInfo::default(),
            chunk_tracking_view: None,
            chunk_sender: PlayerChunkSender::default(),
            last_section_pos,
            game_mode: ServerPlayerGameMode::default(),
            abilities: Abilities::default(),
            respawn_config: None,
            is_changing_dimension: false,
            seen_credits: false,
            won_game: false,
            spawn_extra_particles_on_fall: false,
            raid_omen_position: None,
            warden_spawn_tracker: WardenSpawnTracker::default(),
            loaded_game_types: (None, None),
            entered_nether_position: None,
            current_explosion_impact_pos: None,
            current_explosion_cause: None,
            levitation_start_pos: None,
            levitation_start_time: 0,
            sleep_counter: 0,
            health: 20.0,
            food_data: Default::default(),
            experience: Default::default(),
            last_sent_health: -1.0e8,
            last_sent_food: -99999999,
            last_food_saturation_zero: true,
            last_sent_exp: -99999999,
            attack_strength_ticker: 0,
            item_swap_ticker: 0,
            last_item_in_main_hand: ItemStack::empty(),
            last_recorded: LastRecorded::default(),
            last_death_location: None,
            inventory: Inventory::new(),
            ender_chest_inventory: Default::default(),
            inventory_menu: Some(AbstractContainerMenu::inventory()),
            container_menu: None,
            recipe_book: Default::default(),
            stats: ServerStatsCounter::default(),
            advancements: Default::default(),
            carried: None,
            drop_spam_throttler: TickThrottler::new(20, 1480),
            last_window_id: 0,
            living: LivingEntity::player(),
            movement: PlayerMovement::default(),
            last_client_input: Input::default(),
            known_movement: Vec3::ZERO,
            last_action_time: Instant::now(),
            starting_to_fall_position: None,
            entered_lava_on_vehicle_position: None,
            ack_block_changes_up_to: -1,
            keep_alive_time: Instant::now(),
            keep_alive_challenge: None,
            address: None,
            latency: 0,
            received_packets: 0,
            average_received_packets: 0.0,
            connection_tick_count: 0,
            chat: ChatState::new(profile_id, &crate::ServerConfig::default()),
            chat_session: None,
            disconnected: false,
            disconnect_reason: None,
            disconnect_site: None,
            command_suggestions: Default::default(),
            post_effects: Vec::new(),
            post_effects_dirty: true,
            camera: None,
            ender_pearls: Vec::new(),
            player_list_order: 0,
            fishing: None,
            shoulder_entity_left: Nbt::Compound(Vec::new().into()),
            shoulder_entity_right: Nbt::Compound(Vec::new().into()),
            time_entity_sat_on_shoulder: 0,
            cooldowns: Default::default(),
        };
        player.update_options(client);
        player
    }

    pub fn ip_address(&self) -> String {
        match self.address {
            Some(address) => address.ip().to_canonical().to_string(),
            None => "<unknown>".to_string(),
        }
    }

    pub fn id(&self) -> EntityId {
        self.entity.id
    }

    pub fn is_secondary_use_active(&self) -> bool {
        self.entity.is_shift_key_down()
    }

    pub fn abilities_flying(&self) -> bool {
        self.abilities.flying
    }

    pub fn abilities_may_fly(&self) -> bool {
        self.abilities.mayfly
    }

    pub fn set_game_mode_for_player(&mut self, game_type: GameType, previous: Option<GameType>) {
        self.game_mode.previous_game_type = previous;
        self.game_mode.game_type = game_type;
        game_type.update_player_abilities(&mut self.abilities);
    }

    pub fn item_in_hand(&self, hand: InteractionHand) -> Option<ItemStack> {
        match hand {
            InteractionHand::MainHand => self.inventory.held().cloned(),
            InteractionHand::OffHand => self.inventory.slots[OFFHAND].clone(),
        }
    }

    pub fn hand_slot(&self, hand: InteractionHand) -> usize {
        match hand {
            InteractionHand::MainHand => self.inventory.selected_slot(),
            InteractionHand::OffHand => OFFHAND,
        }
    }

    pub fn set_item_in_hand(&mut self, hand: InteractionHand, stack: Option<ItemStack>) {
        let slot = self.hand_slot(hand);
        self.inventory.slots[slot] = stack.filter(|s| s.count > 0 && s.item != items::AIR);
    }

    pub fn update_item_in_hand(&mut self, hand: InteractionHand, item_stack: &ItemStack) {
        let changed = match &self.inventory.slots[self.hand_slot(hand)] {
            Some(current) => !ItemStack::matches(current, item_stack),
            None => !item_stack.is_empty(),
        };
        if changed {
            self.set_item_in_hand(hand, Some(item_stack.clone()));
        }
    }

    pub fn to_nbt(&self, dimension: &str) -> Nbt {
        let s = |v: &str| Nbt::String(v.into());
        let mut nbt = Vec::new();
        self.entity.save_without_id(&mut nbt);
        nbt.push(("Health".into(), Nbt::Float(self.health)));
        nbt.push(("HurtTime".into(), Nbt::Short(self.living.hurt_time as i16)));
        nbt.push(("DeathTime".into(), Nbt::Short(self.living.death_time as i16)));
        let absorption = match self.entity.data.get(ids::PLAYER_ABSORPTION) {
            DataValue::Float(v) => *v,
            _ => 0.0,
        };
        nbt.push(("AbsorptionAmount".into(), Nbt::Float(absorption)));
        nbt.push(("current_impulse_context_reset_grace_time".into(), Nbt::Int(self.living.current_impulse_context_reset_grace_time)));
        if let Some(pos) = self.living.current_impulse_impact_pos {
            nbt.push(("current_explosion_impact_pos".into(), Nbt::List(vec![Nbt::Double(pos.x), Nbt::Double(pos.y), Nbt::Double(pos.z)])));
        }
        nbt.push(("attributes".into(), self.living.attributes.to_nbt()));
        if !self.living.active_effects.is_empty() {
            nbt.push(("active_effects".into(), Nbt::List(self.living.active_effects.iter().map(MobEffectInstance::to_nbt).collect())));
        }
        nbt.push(("FallFlying".into(), Nbt::Byte(self.entity.is_fall_flying() as i8)));
        if let DataValue::OptionalBlockPos(Some(pos)) = self.entity.data.get(ids::SLEEPING_POS) {
            nbt.push(("sleeping_pos".into(), Nbt::IntArray(vec![pos.x, pos.y, pos.z])));
        }
        nbt.push(("Brain".into(), self.living.brain.pack().to_nbt()));
        if let Some(player) = self.living.last_hurt_by_player {
            nbt.push(("last_hurt_by_player".into(), crate::world::level::storage::uuid_ints(player)));
            nbt.push(("last_hurt_by_player_memory_time".into(), Nbt::Int(self.living.last_hurt_by_player_memory_time)));
        }
        if let Some(mob) = self.living.last_hurt_by_mob {
            nbt.push(("last_hurt_by_mob".into(), crate::world::level::storage::uuid_ints(mob)));
            nbt.push(("ticks_since_last_hurt_by_mob".into(), Nbt::Int(self.entity.tick_count - self.living.last_hurt_by_mob_timestamp)));
        }
        let stack_at = |slot: usize| self.inventory.slots[slot].as_ref().filter(|s| !s.is_empty());
        let equipment: Vec<(String, Nbt)> = [("offhand", OFFHAND), ("feet", 8), ("legs", 7), ("chest", 6), ("head", 5)]
            .into_iter()
            .filter_map(|(name, slot)| Some((name.to_string(), stack_at(slot)?.to_nbt())))
            .collect();
        if !equipment.is_empty() || stack_at(self.inventory.selected_slot()).is_some() {
            nbt.push(("equipment".into(), Nbt::Compound(equipment.into())));
        }
        if self.living.locator_bar_icon.has_data() {
            nbt.push(("locator_bar_icon".into(), self.living.locator_bar_icon.to_nbt()));
        }
        nbt.push(("DataVersion".into(), Nbt::Int(WORLD_VERSION)));
        let inventory = (0..36)
            .filter_map(|index| {
                let stack = self.inventory.slots[inventory_slot(index)].as_ref().filter(|s| !s.is_empty())?;
                let Nbt::Compound(mut fields) = stack.to_nbt() else { return None };
                fields.insert(0, ("Slot".into(), Nbt::Byte(index as i8)));
                Some(Nbt::Compound(fields))
            })
            .collect();
        nbt.push(("Inventory".into(), Nbt::List(inventory)));
        nbt.push(("SelectedItemSlot".into(), Nbt::Int(self.inventory.selected as i32)));
        self.experience.save(&mut nbt);
        nbt.push(("SleepTimer".into(), Nbt::Short(self.sleep_counter as i16)));
        nbt.push(("Score".into(), Nbt::Int(self.score())));
        self.food_data.save(&mut nbt);
        nbt.push(("abilities".into(), self.abilities.to_nbt()));
        nbt.push(("EnderItems".into(), self.ender_chest_inventory.store_as_slots()));
        if let Some((dimension, pos)) = &self.last_death_location {
            nbt.push((
                "LastDeathLocation".into(),
                Nbt::Compound(vec![("dimension".into(), s(dimension)), ("pos".into(), Nbt::IntArray(vec![pos.x, pos.y, pos.z]))].into()),
            ));
        }
        nbt.push(("warden_spawn_tracker".into(), self.warden_spawn_tracker.to_nbt()));
        nbt.push(("playerGameType".into(), Nbt::Int(self.game_mode.game_type.id())));
        if let Some(previous) = self.game_mode.previous_game_type {
            nbt.push(("previousPlayerGameType".into(), Nbt::Int(previous.id())));
        }
        nbt.push(("seenCredits".into(), Nbt::Byte(self.seen_credits as i8)));
        if let Some(pos) = self.entered_nether_position {
            nbt.push(("entered_nether_pos".into(), Nbt::List(vec![Nbt::Double(pos.x), Nbt::Double(pos.y), Nbt::Double(pos.z)])));
        }
        if let Some(pos) = self.current_explosion_impact_pos {
            nbt.push(("last_explosion_impact_pos".into(), Nbt::List(vec![Nbt::Double(pos.x), Nbt::Double(pos.y), Nbt::Double(pos.z)])));
        }
        nbt.push((crate::stats::server_recipe_book::RECIPE_BOOK_TAG.into(), self.recipe_book.pack()));
        nbt.push(("Dimension".into(), s(dimension)));
        if let Some(respawn) = &self.respawn_config {
            let mut fields = respawn.respawn_data.to_nbt_fields();
            if respawn.forced {
                fields.push(("forced".into(), Nbt::Byte(1)));
            }
            nbt.push(("respawn".into(), Nbt::Compound(fields.into())));
        }
        nbt.push(("spawn_extra_particles_on_fall".into(), Nbt::Byte(self.spawn_extra_particles_on_fall as i8)));
        if let Some(pos) = self.raid_omen_position {
            nbt.push(("raid_omen_position".into(), Nbt::IntArray(vec![pos.x, pos.y, pos.z])));
        }
        nbt.push(("post_effects".into(), Nbt::List(self.post_effects.iter().map(|p| s(p)).collect())));
        if !is_empty_compound(&self.shoulder_entity_left) {
            nbt.push(("ShoulderEntityLeft".into(), self.shoulder_entity_left.clone()));
        }
        if !is_empty_compound(&self.shoulder_entity_right) {
            nbt.push(("ShoulderEntityRight".into(), self.shoulder_entity_right.clone()));
        }
        Nbt::Compound(nbt.into())
    }

    pub fn saved_sleeping_pos(nbt: &Nbt) -> Option<BlockPos> {
        extra_codecs::block_pos(nbt.get("sleeping_pos")?).value
    }

    pub fn load_nbt(&mut self, nbt: &Nbt) -> Result<(), crate::util::java_throwable::JavaThrowable> {
        self.entity.load(nbt)?;
        let mut health = 0.0;
        let mut absorption = 0.0;
        let mut equipment: [Option<ItemStack>; 8] = Default::default();
        let sleeping = crate::world::entity::living_entity::read_additional_save_data(&mut self.entity, &mut self.living, &mut health, &mut absorption, &mut equipment, nbt);
        self.health = health;
        self.entity.data.set(ids::PLAYER_ABSORPTION, DataValue::Float(absorption));
        for (slot, index) in [(EquipmentSlot::Offhand, OFFHAND), (EquipmentSlot::Feet, 8), (EquipmentSlot::Legs, 7), (EquipmentSlot::Chest, 6), (EquipmentSlot::Head, 5)] {
            if let Some(stack) = equipment[slot as usize].take() {
                self.inventory.slots[index] = Some(stack);
            }
        }
        for (slot, stack) in tag_value_input::list_or_empty(nbt, "Inventory", decode_item_stack_with_slot) {
            if let Some(slot) = protocol_slot(slot as u8 as i8) {
                self.inventory.slots[slot] = Some(stack);
            }
        }
        self.inventory.selected = (int_or(nbt, "SelectedItemSlot", 0).max(0) as usize).min(8);
        self.sleep_counter = tag_value_input::get_i64(nbt, "SleepTimer").map_or(0, |v| v as i16 as i32);
        self.experience.load(nbt, &mut self.entity.random);
        self.set_score(int_or(nbt, "Score", 0));
        self.food_data = crate::world::food::FoodData::load(nbt);
        if let Some(abilities) = tag_value_input::read(nbt, "abilities", Abilities::decode) {
            self.abilities = abilities;
        }
        let walking_speed = self.abilities.walking_speed as f64;
        self.living.attributes.instance(Attribute::MovementSpeed).expect("players have movement speed").set_base_value(walking_speed);
        self.ender_chest_inventory.from_decoded_slots(tag_value_input::list_or_empty(nbt, "EnderItems", decode_item_stack_with_slot));
        self.last_death_location = tag_value_input::read(nbt, "LastDeathLocation", |tag| DecodeResult::from(GlobalPos::decode(tag))).map(|pos| (pos.dimension, pos.pos));
        self.warden_spawn_tracker = tag_value_input::read(nbt, "warden_spawn_tracker", WardenSpawnTracker::decode).unwrap_or_default();
        self.entered_nether_position = tag_value_input::read(nbt, "entered_nether_pos", extra_codecs::vec3);
        self.current_explosion_impact_pos = tag_value_input::read(nbt, "last_explosion_impact_pos", extra_codecs::vec3);
        self.seen_credits = tag_value_input::get_boolean(nbt, "seenCredits").unwrap_or(false);
        if let Some(packed) = tag_value_input::read(nbt, crate::stats::server_recipe_book::RECIPE_BOOK_TAG, ServerRecipeBook::decode_packed) {
            self.recipe_book.load_untrusted(packed);
        }
        if sleeping {
            self.entity.data.set(ids::SLEEPING_POS, DataValue::OptionalBlockPos(None));
            self.sleep_counter = 0;
        }
        self.respawn_config = tag_value_input::read(nbt, "respawn", decode_respawn_config);
        self.spawn_extra_particles_on_fall = tag_value_input::get_boolean(nbt, "spawn_extra_particles_on_fall").unwrap_or(false);
        self.raid_omen_position = tag_value_input::read(nbt, "raid_omen_position", extra_codecs::block_pos);
        self.loaded_game_types = (read_player_mode(nbt, "playerGameType"), read_player_mode(nbt, "previousPlayerGameType"));
        if let Some(post_effects) = tag_value_input::read(nbt, "post_effects", |tag| codec::list_partial(tag, |effect| DecodeResult::from(codec::identifier(effect)))) {
            self.post_effects.extend(post_effects);
        }
        let compound = |key: &str| {
            tag_value_input::read_or_error(nbt, key, |tag| match tag {
                Nbt::Compound(_) => Ok(tag.copy()),
                other => Err(format!("Not a compound tag: {}", crate::nbt::to_string(other))),
            })
            .unwrap_or(Nbt::Compound(Vec::new().into()))
        };
        self.set_shoulder_entity_left(compound("ShoulderEntityLeft"));
        self.set_shoulder_entity_right(compound("ShoulderEntityRight"));
        let p = self.entity.position();
        self.last_section_pos = (mth::floor(p.x) >> 4, mth::floor(p.y) >> 4, mth::floor(p.z) >> 4);
        Ok(())
    }

    pub fn write_chat_session(&self, p: &mut Writer) {
        let session = self.get_chat_session();
        p.bool(session.is_some());
        if let Some(session) = session {
            session.write_data(p);
        }
    }

    pub fn write_info(&self, p: &mut Writer) {
        p.uuid(self.profile.uuid);
        p.string_max(&self.profile.name, 16);
        self.profile.write_properties(p);
        self.write_chat_session(p);
        p.varint(self.game_mode.game_type.id());
        p.bool(true);
        p.varint(self.latency);
        p.bool(false);
        p.varint(0);
        p.bool(self.is_model_part_shown(PlayerModelPart::Hat));
    }

    pub fn is_model_part_shown(&self, part: PlayerModelPart) -> bool {
        let customisation = match self.entity.data.get(ids::PLAYER_MODE_CUSTOMISATION) {
            DataValue::Byte(b) => *b as i32,
            _ => 0,
        };
        customisation & part.get_mask() == part.get_mask()
    }

    pub fn update_player_attributes(&mut self) {
        let creative = self.game_mode.game_type == GameType::Creative;
        let crouching = self.entity.is_crouching();
        let attributes = &mut self.living.attributes;
        for (attribute, modifier, applies) in [
            (Attribute::BlockInteractionRange, CREATIVE_BLOCK_INTERACTION_RANGE_MODIFIER, creative),
            (Attribute::EntityInteractionRange, CREATIVE_ENTITY_INTERACTION_RANGE_MODIFIER, creative),
            (Attribute::WaypointTransmitRange, WAYPOINT_TRANSMIT_RANGE_CROUCH_MODIFIER, crouching),
        ] {
            if let Some(instance) = attributes.instance(attribute) {
                if applies {
                    instance.add_or_update_transient_modifier(modifier);
                } else {
                    instance.remove_modifier(&modifier.id);
                }
            }
        }
    }

    pub fn send_post_effects(&mut self) {
        self.post_effects_dirty = false;
        let post_effects = &self.post_effects;
        self.connection.send(clientbound::POST_EFFECTS, |w| {
            w.varint(post_effects.len() as i32);
            for post_effect in post_effects {
                w.string(post_effect);
            }
        });
    }

    pub fn add_post_effect(&mut self, post_effect: &str) -> bool {
        if self.post_effects.iter().any(|p| p == post_effect) {
            return false;
        }
        self.post_effects.push(post_effect.to_string());
        self.post_effects_dirty = true;
        true
    }

    pub fn clear_post_effects(&mut self) -> bool {
        if self.post_effects.is_empty() {
            return false;
        }
        self.post_effects.clear();
        self.post_effects_dirty = true;
        true
    }

    pub fn get_post_effects(&self) -> &[String] {
        &self.post_effects
    }

    pub fn remove_post_effect(&mut self, post_effect: &str) -> bool {
        match self.post_effects.iter().position(|p| p == post_effect) {
            Some(index) => {
                self.post_effects.remove(index);
                self.post_effects_dirty = true;
                true
            }
            None => false,
        }
    }

    pub fn pick_up(&mut self, stack: ItemStack) -> i32 {
        self.inventory.add(stack).1
    }

    pub fn reset_last_action_time(&mut self) {
        self.last_action_time = Instant::now();
    }

    pub fn disconnect(&mut self, translation_key: &str) {
        self.disconnect_with(crate::network::chat::translate(translation_key));
    }

    pub fn disconnect_with(&mut self, reason: crate::network::chat::Component) {
        self.connection.send(clientbound::DISCONNECT, |p| reason.write_trusted_context_free(p));
        self.disconnected = true;
        self.disconnect_reason = Some(reason.get_string());
    }
}

fn did_not_move(dx: f64, dy: f64, dz: f64) -> bool {
    dx == 0.0 && dy == 0.0 && dz == 0.0
}

impl ServerLevel {
    pub fn server_player_teleport_same_dimension(&mut self, id: EntityId, transition: TeleportTransition) -> EntityId {
        if transition.missing_respawn_block {
            self.player(id).connection.send(clientbound::GAME_EVENT, |w| {
                w.u8(0);
                w.f32(0.0);
            });
        }
        if !transition.as_passenger {
            self.remove_vehicle(id);
        }
        self.server_player_teleport_position(id, transition);
        id
    }

    fn server_player_teleport_position(&mut self, id: EntityId, transition: TeleportTransition) {
        let teleport = self.player_mut(id).teleport_set_position(transition.position_move_rotation(), transition.relatives);
        self.entity_moved(id);
        let player = self.player_mut(id);
        player.send_teleport(teleport, transition.position_move_rotation(), transition.relatives);
        player.reset_position();
        transition.post_teleport_transition.on_transition(self, id);
    }

    pub fn save_player(&self, id: EntityId) -> Nbt {
        self.save_player_with_ender_pearls(id, &|pearl| self.other_levels.iter().find(|level| level.entities.contains_key(&pearl)))
    }

    pub fn save_player_with_ender_pearls<'a>(&'a self, id: EntityId, other_level_pearl: &dyn Fn(EntityId) -> Option<&'a ServerLevel>) -> Nbt {
        let Nbt::Compound(mut fields) = self.player(id).to_nbt(self.dimension) else { unreachable!("player data is a compound") };
        self.save_vehicle_position(id, &mut fields);
        if let Some(root_vehicle) = self.save_parent_vehicle(id) {
            let index = fields.iter().position(|(k, _)| k == crate::stats::server_recipe_book::RECIPE_BOOK_TAG).unwrap_or(fields.len());
            fields.insert(index, ("RootVehicle".into(), root_vehicle));
        }
        if let Some(ender_pearls) = self.save_ender_pearls(id, other_level_pearl) {
            let index = fields.iter().position(|(k, _)| k == "post_effects").unwrap_or(fields.len());
            fields.insert(index, ("ender_pearls".into(), ender_pearls));
        }
        Nbt::Compound(fields)
    }

    fn save_parent_vehicle(&self, id: EntityId) -> Option<Nbt> {
        let root_vehicle = self.root_vehicle(id);
        let vehicle = self.vehicle(id)?;
        if root_vehicle == id || !self.has_exactly_one_player_passenger(root_vehicle) {
            return None;
        }
        let entity = self.save_entity(root_vehicle).unwrap_or(Nbt::Compound(Vec::new().into()));
        Some(Nbt::Compound(vec![("Attach".into(), crate::world::level::storage::uuid_ints(self.entity(vehicle).uuid)), ("Entity".into(), entity)].into()))
    }

    pub fn load_and_spawn_parent_vehicle(&mut self, id: EntityId, player_input: &Nbt) {
        let Some((root_tag, _root_scope)) = tag_value_input::child(player_input, "RootVehicle") else { return };
        let loaded = match tag_value_input::child(root_tag, "Entity") {
            Some((entity_tag, _entity_scope)) => {
                let site = crate::world::entity::entity_save::player_spawn_site(&LOAD_AND_SPAWN_PARENT_VEHICLE_SITE);
                self.load_entity_recursive_in_scope(entity_tag, &site, &mut |_| {})
            }
            None => Vec::new(),
        };
        let mut added = Vec::new();
        for entity in loaded {
            let entity_id = entity.entity().id;
            if self.add_entity(entity) {
                added.push(entity_id);
            }
        }
        let Some(&vehicle) = added.first() else { return };
        let attach_to = tag_value_input::read(root_tag, "Attach", crate::world::entity::entity_save::decode_uuid);
        if attach_to == Some(self.entity(vehicle).uuid) {
            self.start_riding(id, vehicle, true, false);
        } else {
            for entity in self.indirect_passengers(vehicle) {
                if Some(self.entity(entity).uuid) == attach_to {
                    self.start_riding(id, entity, true, false);
                    break;
                }
            }
        }
        if !self.is_passenger(id) {
            warn!("Couldn't reattach entity to player");
            self.discard(vehicle);
            for entity in self.indirect_passengers(vehicle) {
                self.discard(entity);
            }
        }
    }

    pub fn server_player_drop(&mut self, id: EntityId, all: bool) {
        let used_item_hand = self.used_item_hand(id);
        let p = self.player_mut(id);
        let removed = p.inventory.remove_from_selected(all);
        let selected_slot = p.inventory.selected;
        let selected_item = p.inventory.held().cloned().unwrap_or_else(ItemStack::empty);
        if let Some(menu) = p.container_menu.as_mut().or(p.inventory_menu.as_mut())
            && let Some(slot_index) = menu.find_slot(crate::world::inventory::slot::SlotContainer::Inventory, selected_slot)
        {
            menu.set_remote_slot(slot_index, &selected_item);
        }
        if p.living.use_item.is_none() || used_item_hand == InteractionHand::MainHand && selected_item.is_empty() {
            self.stop_using_item(id);
        }
        self.player_drop(id, removed, true, crate::util::Prediction::Predicted);
        self.reset_attack_strength_ticker(id);
    }

    pub fn swing_and_reset_attack_strength(&mut self, id: EntityId, hand: InteractionHand, animation: crate::world::item::component::swing_animation::SwingAnimation, send_to_swinging_entity: bool) {
        if self.swing(id, hand, animation, send_to_swinging_entity) {
            self.reset_attack_strength_ticker(id);
        }
    }

    pub fn handle_extra_items_created_on_use(&mut self, id: EntityId, extra_items: ItemStack) {
        let left = self.player_mut(id).pick_up(extra_items.clone());
        if left > 0 {
            self.player_drop(id, extra_items.copy_with_count(left), false, crate::util::Prediction::Predicted);
        }
    }

    fn synchronize_special_item_updates(&mut self, id: EntityId, map_id: Option<MapId>) {
        let Some(data) = map_item::get_saved_data_by_id(map_id, self) else { return };
        let Some(map_id) = map_id else { return };
        if let Some(packet) = data.borrow_mut().get_update_packet(map_id, id) {
            self.player(id).connection.send(clientbound::MAP_ITEM_DATA, |w| packet.write(w));
        }
    }

    pub fn server_player_do_tick(&mut self, id: EntityId) {
        let ticks = !self.is_spectator(id) || !self.touching_unloaded_chunk(id);
        if ticks {
            self.guard_player_tick(id, |level| level.player_tick(id));
        }
        self.then_tick(move |level| level.server_player_do_tick_end(id, ticks));
    }

    fn server_player_do_tick_end(&mut self, id: EntityId, ticks: bool) {
        self.guard_player_tick(id, |level| level.server_player_do_tick_end_unguarded(id, ticks));
    }

    fn server_player_do_tick_end_unguarded(&mut self, id: EntityId, ticks: bool) {
        if ticks {
            self.tick_food(id);
            self.award_stat(id, stats::PLAY_TIME, 1);
            self.award_stat(id, stats::TOTAL_WORLD_TIME, 1);
            if self.is_alive(id) {
                self.award_stat(id, stats::TIME_SINCE_DEATH, 1);
            }
            if self.entity(id).is_discrete() {
                self.award_stat(id, stats::CROUCH_TIME, 1);
            }
            if !self.is_sleeping(id) {
                self.award_stat(id, stats::TIME_SINCE_REST, 1);
            }
        }
        for i in 0..41 {
            let slot = crate::world::item::inventory_slot(i);
            let map_id = self.player(id).inventory.slots[slot].as_ref().filter(|stack| !stack.is_empty()).and_then(|stack| stack.get(dc::MAP_ID)).and_then(MapId::from_nbt);
            if map_id.is_some() {
                self.synchronize_special_item_updates(id, map_id);
            }
        }
        let p = self.player_mut(id);
        let food = p.food_data;
        let saturation_zero = food.saturation_level == 0.0;
        if p.health != p.last_sent_health || p.last_sent_food != food.food_level || saturation_zero != p.last_food_saturation_zero {
            let health = p.health;
            p.connection.send(clientbound::SET_HEALTH, |w| {
                w.f32(health);
                w.varint(food.food_level);
                w.f32(food.saturation_level);
            });
            p.last_sent_health = health;
            p.last_sent_food = food.food_level;
            p.last_food_saturation_zero = saturation_zero;
        }
        let health_and_absorption = self.player(id).health + self.absorption_amount(id);
        if health_and_absorption != self.player(id).last_recorded.health_and_absorption {
            self.player_mut(id).last_recorded.health_and_absorption = health_and_absorption;
            self.update_score_for_criteria(id, &ObjectiveCriteria::HEALTH, mth::ceil(health_and_absorption as f64));
        }
        let food = self.player(id).food_data.food_level;
        if food != self.player(id).last_recorded.food_level {
            self.player_mut(id).last_recorded.food_level = food;
            self.update_score_for_criteria(id, &ObjectiveCriteria::FOOD, food);
        }
        let air = self.player(id).entity.air_supply();
        if air != self.player(id).last_recorded.air_level {
            self.player_mut(id).last_recorded.air_level = air;
            self.update_score_for_criteria(id, &ObjectiveCriteria::AIR, air);
        }
        let armor = self.armor_value(id);
        if armor != self.player(id).last_recorded.armor {
            self.player_mut(id).last_recorded.armor = armor;
            self.update_score_for_criteria(id, &ObjectiveCriteria::ARMOR, armor);
        }
        let experience = self.player(id).experience;
        if experience.total != self.player(id).last_recorded.experience {
            self.player_mut(id).last_recorded.experience = experience.total;
            self.update_score_for_criteria(id, &ObjectiveCriteria::EXPERIENCE, experience.total);
        }
        if experience.level != self.player(id).last_recorded.level {
            self.player_mut(id).last_recorded.level = experience.level;
            self.update_score_for_criteria(id, &ObjectiveCriteria::LEVEL, experience.level);
        }
        let p = self.player_mut(id);
        if p.experience.total != p.last_sent_exp {
            p.last_sent_exp = p.experience.total;
            p.send_experience();
        }
        if self.entity(id).tick_count % 20 == 0 {
            criteria_triggers::LOCATION.trigger(self, id);
        }
    }

    pub fn is_receiving_waypoints(&self, id: EntityId) -> bool {
        self.attribute_value(id, Attribute::WaypointReceiveRange) > 0.0
    }

    pub fn server_player_on_attribute_updated(&mut self, id: EntityId, attribute: Attribute) {
        if attribute == Attribute::WaypointReceiveRange {
            if self.living(id).attributes.value(attribute) > 0.0 {
                self.get_waypoint_manager().add_player(self, id);
            } else {
                self.get_waypoint_manager().remove_player(self, id);
            }
        }
        self.living_entity_on_attribute_updated(id, attribute);
    }

    pub fn server_player_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        if self.is_invulnerable_to(id, &source) {
            return false;
        }
        if let Some(attacker) = source.causing_entity.filter(|&e| self.is_player(e))
            && !self.can_harm_player(id, attacker)
        {
            return false;
        }
        if let Some(arrow_owner) = source
            .causing_entity
            .filter(|&e| self.as_projectile(e).is_some_and(|projectile| projectile.abstract_arrow().is_some()))
            .and_then(|arrow| self.projectile_get_owner(arrow))
            .filter(|&owner| self.is_player(owner))
            && !self.can_harm_player(id, arrow_owner)
        {
            return false;
        }
        let _scope = crate::server::main_thread_frames::enter(["net.minecraft.server.level.ServerPlayer.hurtServer(ServerPlayer.java:1078)"]);
        self.player_hurt_server(id, source, damage)
    }

    pub fn server_player_is_invulnerable_to(&mut self, id: EntityId, source: &DamageSource) -> bool {
        self.player_is_invulnerable_to(id, source)
            || self.player(id).is_changing_dimension && !source.is_type(wafer_data::damage_types::ENDER_PEARL)
            || !self.player(id).movement.has_client_loaded()
    }

    pub fn can_harm_player(&self, id: EntityId, target: EntityId) -> bool {
        self.game_rules.get_bool(game_rule::PVP) && self.do_teams_allow_damage(id, target)
    }

    pub fn handle_shoulder_entities(&mut self, id: EntityId) {
        let left = self.player(id).shoulder_entity_left.clone();
        self.play_shoulder_entity_ambient_sound(id, &left);
        let right = self.player(id).shoulder_entity_right.clone();
        self.play_shoulder_entity_ambient_sound(id, &right);
        let p = self.player(id);
        if p.entity.fall_distance > 0.5 || p.entity.is_in_water() || p.abilities_flying() || self.is_sleeping(id) || p.entity.is_in_powder_snow {
            let _scope = crate::server::main_thread_frames::enter(HANDLE_SHOULDER_ENTITIES_FRAMES);
            self.remove_entities_on_shoulder(id);
        }
    }

    fn play_shoulder_entity_ambient_sound(&mut self, id: EntityId, shoulder_entity_tag: &Nbt) {
        if is_empty_compound(shoulder_entity_tag) || shoulder_entity_tag.get("Silent").and_then(Nbt::as_i64).is_some_and(|silent| silent != 0) {
            return;
        }
        if self.entity_mut(id).random.next_int_bounded(200) != 0 {
            return;
        }
        if shoulder_entity_tag.get("id").and_then(Nbt::as_str) == Some("minecraft:parrot") && !crate::world::entity::animal::parrot::parrot::imitate_nearby_mobs(self, id) {
            let e = self.entity(id);
            let (x, y, z) = (e.x(), e.y(), e.z());
            let sound = crate::world::entity::animal::parrot::parrot::get_ambient(self);
            let source = self.sound_source(id);
            let pitch = crate::world::entity::animal::parrot::parrot::get_pitch(&mut self.entity_mut(id).random);
            self.play_sound_at(None, x, y, z, crate::world::entity::sound_event(sound), source, 1.0, pitch);
        }
    }

    pub fn set_entity_on_shoulder_of(&mut self, id: EntityId, entity_tag: Nbt) -> bool {
        let e = self.entity(id);
        if self.is_passenger(id) || !e.on_ground || e.is_in_water() || e.is_in_powder_snow {
            return false;
        }
        let game_time = self.game_time;
        let p = self.player_mut(id);
        if is_empty_compound(&p.shoulder_entity_left) {
            p.set_shoulder_entity_left(entity_tag);
            p.time_entity_sat_on_shoulder = game_time;
            true
        } else if is_empty_compound(&p.shoulder_entity_right) {
            p.set_shoulder_entity_right(entity_tag);
            p.time_entity_sat_on_shoulder = game_time;
            true
        } else {
            false
        }
    }

    pub fn remove_entities_on_shoulder(&mut self, id: EntityId) {
        if self.player(id).time_entity_sat_on_shoulder + 20 < self.game_time {
            let left = self.player(id).shoulder_entity_left.clone();
            {
                let _scope = crate::server::main_thread_frames::enter([REMOVE_ENTITIES_ON_SHOULDER_LEFT_FRAME]);
                self.respawn_entity_on_shoulder(id, &left);
            }
            self.player_mut(id).set_shoulder_entity_left(Nbt::Compound(TagMap::new()));
            let right = self.player(id).shoulder_entity_right.clone();
            {
                let _scope = crate::server::main_thread_frames::enter([REMOVE_ENTITIES_ON_SHOULDER_RIGHT_FRAME]);
                self.respawn_entity_on_shoulder(id, &right);
            }
            self.player_mut(id).set_shoulder_entity_right(Nbt::Compound(TagMap::new()));
        }
    }

    fn respawn_entity_on_shoulder(&mut self, id: EntityId, tag: &Nbt) {
        if is_empty_compound(tag) {
            return;
        }
        let _scope = crate::server::main_thread_frames::enter([RESPAWN_ENTITY_ON_SHOULDER_FRAME]);
        let _collector = crate::world::level::storage::tag_value_input::ScopedCollector::new(PathElement::Text(self.entity_to_string(id)), SERVER_PLAYER_LOGGER);
        let child = crate::world::level::storage::tag_value_input::enter_child(PathElement::Text(".shoulder".to_string()));
        let site = crate::server::main_thread_frames::current();
        let loaded = self.create_entity_from_tag(tag, &site);
        drop(child);
        let Some(mut entity) = loaded else { return };
        let e = self.entity(id);
        let (x, y, z, uuid) = (e.x(), e.y(), e.z(), e.uuid);
        if let crate::world::entity::LevelEntity::Mob(mob) = &mut entity
            && mob.tamable_animal().is_some()
        {
            crate::world::entity::tamable_animal::set_owner_reference(&mut mob.entity, Some(crate::world::entity::entity_reference::EntityReference::of(uuid)));
        }
        entity.entity_mut().set_pos(Vec3::new(x, y + 0.7f32 as f64, z));
        self.add_fresh_entity(entity);
    }

    pub fn server_player_die(&mut self, id: EntityId, source: DamageSource) {
        self.entity_game_event(id, crate::world::level::gameevent::game_event::ENTITY_DIE);
        if self.game_rules.get_bool(game_rule::SHOW_DEATH_MESSAGES) {
            let message = self.death_message(id);
            let display_name = self.display_name(id);
            let fallback = || {
                let truncated_death_message = message.get_string_with_limit(256);
                let explanation = Component::translatable_with("death.attack.message_too_long", vec![Arg::Component(Component::literal(truncated_death_message).with_color(ChatFormatting::Yellow))]);
                let fake_death_message = Component::translatable_with("death.attack.even_more_magic", vec![Arg::Component(display_name)])
                    .with_hover(crate::network::chat::HoverEvent::ShowText(Box::new(explanation)));
                Some(crate::network::connection::encode(clientbound::PLAYER_COMBAT_KILL, |p| clientbound_player_combat_kill_packet::write(p, id, &fake_death_message)))
            };
            self.player(id).connection.send_with_fallback(
                clientbound::PLAYER_COMBAT_KILL,
                |p| clientbound_player_combat_kill_packet::write(p, id, &message),
                Some(clientbound::PLAYER_COMBAT_KILL),
                fallback,
            );
            match self.team_of(id).map(|t| t.death_message_visibility()) {
                None | Some(Visibility::Always) => self.broadcast_system_message(&message),
                Some(Visibility::HideForOtherTeams) => self.broadcast_system_to_team(id, &message),
                Some(Visibility::HideForOwnTeam) => self.broadcast_system_to_all_except_team(id, &message),
                Some(Visibility::Never) => {}
            }
        } else {
            self.player(id).connection.send(clientbound::PLAYER_COMBAT_KILL, |p| clientbound_player_combat_kill_packet::write(p, id, &crate::network::chat::Component::empty()));
        }
        self.remove_entities_on_shoulder(id);
        if self.game_rules.get_bool(game_rule::FORGIVE_DEAD_PLAYERS) {
            self.tell_neutral_mobs_that_i_died(id);
        }
        if !self.is_spectator(id) {
            self.drop_all_death_loot(id, &source);
        }
        let holder = ScoreHolder::for_entity(self, id);
        self.for_all_objectives(&ObjectiveCriteria::DEATH_COUNT, &holder, |level, score| {
            let _ = score.increment(level);
        });
        if let Some(killer) = self.kill_credit(id) {
            let killer_type = self.entity(killer).entity_type;
            self.award_stat(id, stats::ENTITY_KILLED_BY.get(killer_type as u16), 1);
            self.award_kill_score(killer, id, &source);
        }
        self.broadcast_entity_event(id, entity_event::DEATH);
        self.award_stat(id, stats::DEATHS, 1);
        self.reset_stat(id, stats::TIME_SINCE_DEATH);
        self.reset_stat(id, stats::TIME_SINCE_REST);
        self.clear_fire(id);
        self.entity_mut(id).set_ticks_frozen(0);
        self.entity_mut(id).set_shared_flag(flags::ON_FIRE, false);
        self.recheck_combat_status(id);
        let pos = self.entity(id).block_position();
        let dimension = self.dimension;
        let p = self.player_mut(id);
        p.last_death_location = Some((dimension.to_owned(), pos));
        p.movement.waiting_for_respawn = true;
    }

    fn tell_neutral_mobs_that_i_died(&mut self, id: EntityId) {
        let aabb = wafer_core::AABB::of_block(self.entity(id).block_position()).inflate(32.0, 10.0, 32.0);
        let neutral_mobs: Vec<EntityId> = self
            .get_entities(None, &aabb)
            .into_iter()
            .filter(|&entity| !self.is_spectator(entity) && self.mob(entity).is_some_and(|mob| mob.neutral_mob().is_some()))
            .collect();
        for mob in neutral_mobs {
            self.player_died(mob, id);
        }
    }

    pub fn server_player_award_kill_score(&mut self, id: EntityId, victim: EntityId, killing_blow: &DamageSource) {
        if victim == id {
            return;
        }
        self.entity_award_kill_score(id, victim, killing_blow);
        let holder = ScoreHolder::for_entity(self, id);
        self.for_all_objectives(&ObjectiveCriteria::KILL_COUNT_ALL, &holder, |level, score| {
            let _ = score.increment(level);
        });
        if self.is_player(victim) {
            self.award_stat(id, stats::PLAYER_KILLS, 1);
            self.for_all_objectives(&ObjectiveCriteria::KILL_COUNT_PLAYERS, &holder, |level, score| {
                let _ = score.increment(level);
            });
        } else {
            self.award_stat(id, stats::MOB_KILLS, 1);
        }
        let victim_holder = ScoreHolder::for_entity(self, victim);
        self.handle_team_kill(&holder, &victim_holder, ObjectiveCriteria::team_kill);
        self.handle_team_kill(&victim_holder, &holder, ObjectiveCriteria::killed_by_team);
        criteria_triggers::PLAYER_KILLED_ENTITY.trigger(self, id, victim, killing_blow);
    }

    fn handle_team_kill(&mut self, source: &ScoreHolder, target: &ScoreHolder, criteria_by_team: fn(crate::world::scores::team_color::TeamColor) -> ObjectiveCriteria) {
        let color = self.scoreboard.borrow().get_players_team(target.scoreboard_name()).and_then(|own_team| own_team.color());
        if let Some(color) = color {
            let criteria = criteria_by_team(color);
            self.for_all_objectives(&criteria, source, |level, score| {
                let _ = score.increment(level);
            });
        }
    }

    pub fn get_enchanted_damage(&mut self, id: EntityId, entity: EntityId, damage: f32, damage_source: &DamageSource) -> f32 {
        let weapon = self.get_weapon_item(id);
        self.modify_damage(&weapon, entity, damage_source, damage)
    }

    pub fn crit(&self, id: EntityId, entity: EntityId) {
        let packet = crate::network::connection::encode(clientbound::ANIMATE, |w| {
            w.varint(entity);
            w.u8(1);
        });
        self.send_to_tracking_players_and_self(id, &packet);
    }

    pub fn magic_crit(&self, id: EntityId, entity: EntityId) {
        let packet = crate::network::connection::encode(clientbound::ANIMATE, |w| {
            w.varint(entity);
            w.u8(2);
        });
        self.send_to_tracking_players_and_self(id, &packet);
    }

    pub fn server_player_indicate_damage(&mut self, id: EntityId, xd: f64, zd: f64) {
        let p = self.player_mut(id);
        let hurt_dir = (mth::atan2(zd, xd) * mth::RAD_TO_DEG as f64 - p.entity.y_rot as f64) as f32;
        p.living.hurt_dir = hurt_dir;
        p.connection.send(clientbound::HURT_ANIMATION, |w| {
            w.varint(id);
            w.f32(hurt_dir);
        });
    }

    pub fn check_riding_statistics(&mut self, id: EntityId, dx: f64, dy: f64, dz: f64) {
        let Some(vehicle) = self.vehicle(id) else { return };
        if did_not_move(dx, dy, dz) {
            return;
        }
        let distance = mth::java_round_f32((((dx * dx + dy * dy + dz * dz).sqrt()) as f32) * 100.0);
        if self.is_minecart(vehicle) {
            self.award_stat(id, stats::MINECART_ONE_CM, distance);
        } else if self.is_boat(vehicle) {
            self.award_stat(id, stats::BOAT_ONE_CM, distance);
        } else if self.is_pig(vehicle) {
            self.award_stat(id, stats::PIG_ONE_CM, distance);
        } else if self.is_abstract_horse(vehicle) {
            self.award_stat(id, stats::HORSE_ONE_CM, distance);
        } else if self.is_strider(vehicle) {
            self.award_stat(id, stats::STRIDER_ONE_CM, distance);
        } else if self.is_happy_ghast(vehicle) {
            self.award_stat(id, stats::HAPPY_GHAST_ONE_CM, distance);
        } else if self.is_abstract_nautilus(vehicle) {
            self.award_stat(id, stats::NAUTILUS_ONE_CM, distance);
        }
    }

    pub fn track_start_falling_position(&mut self, id: EntityId) {
        let p = self.player_mut(id);
        if p.entity.fall_distance > 0.0 && p.starting_to_fall_position.is_none() {
            let starting_to_fall_position = p.entity.position();
            p.starting_to_fall_position = Some(starting_to_fall_position);
            if let Some(current_explosion_impact_pos) = p.current_explosion_impact_pos
                && current_explosion_impact_pos.y <= starting_to_fall_position.y
            {
                let current_explosion_cause = p.current_explosion_cause;
                criteria_triggers::FALL_AFTER_EXPLOSION.trigger(self, id, current_explosion_impact_pos, current_explosion_cause);
            }
            let p = self.player_mut(id);
            p.current_explosion_impact_pos = None;
            p.current_explosion_cause = None;
        }
    }

    pub fn entity_on_explosion_hit(&mut self, id: EntityId, explosion_caused_by: Option<EntityId>) {
        if !self.is_player(id) {
            return;
        }
        let ignore_fall_damage = explosion_caused_by.is_some_and(|cause| self.entity(cause).entity_type == registry_id(registry::ENTITY_TYPE, "minecraft:wind_charge"));
        let p = self.player_mut(id);
        let position = p.entity.position();
        p.current_explosion_impact_pos = Some(position);
        p.current_explosion_cause = explosion_caused_by;
        p.living.set_ignore_fall_damage_from_current_impulse(ignore_fall_damage, position);
    }

    pub fn track_entered_or_exited_lava_on_vehicle(&mut self, id: EntityId) {
        if let Some(vehicle) = self.vehicle(id)
            && self.entity(vehicle).is_in_lava()
        {
            match self.player(id).entered_lava_on_vehicle_position {
                None => {
                    let position = self.entity(id).position();
                    self.player_mut(id).entered_lava_on_vehicle_position = Some(position);
                }
                Some(position) => crate::advancements::triggers::criteria_triggers::RIDE_ENTITY_IN_LAVA_TRIGGER.trigger(self, id, position),
            }
        }
        if self.player(id).entered_lava_on_vehicle_position.is_some() && !self.vehicle(id).is_some_and(|vehicle| self.entity(vehicle).is_in_lava()) {
            self.player_mut(id).entered_lava_on_vehicle_position = None;
        }
    }

    pub fn check_movement_statistics(&mut self, id: EntityId, dx: f64, dy: f64, dz: f64) {
        if self.is_passenger(id) || did_not_move(dx, dy, dz) {
            return;
        }
        let distance = |d: f64| mth::java_round_f32((d.sqrt() as f32) * 100.0);
        let e = self.entity(id);
        if e.is_swimming() {
            let distance = distance(dx * dx + dy * dy + dz * dz);
            if distance > 0 {
                self.award_stat(id, stats::SWIM_ONE_CM, distance);
                self.cause_food_exhaustion(id, EXHAUSTION_SWIM * distance as f32 * 0.01);
            }
        } else if e.is_eye_in_fluid(&wafer_data::fluid_tags::WATER) {
            let distance = distance(dx * dx + dy * dy + dz * dz);
            if distance > 0 {
                self.award_stat(id, stats::WALK_UNDER_WATER_ONE_CM, distance);
                self.cause_food_exhaustion(id, EXHAUSTION_SWIM * distance as f32 * 0.01);
            }
        } else if e.is_in_water() {
            let horizontal_distance = distance(dx * dx + dz * dz);
            if horizontal_distance > 0 {
                self.award_stat(id, stats::WALK_ON_WATER_ONE_CM, horizontal_distance);
                self.cause_food_exhaustion(id, EXHAUSTION_SWIM * horizontal_distance as f32 * 0.01);
            }
        } else if self.on_climbable(id) {
            if dy > 0.0 {
                self.award_stat(id, stats::CLIMB_ONE_CM, mth::java_round(dy * 100.0) as i32);
            }
        } else if self.entity(id).on_ground {
            let horizontal_distance = distance(dx * dx + dz * dz);
            if horizontal_distance > 0 {
                let e = self.entity(id);
                if e.is_sprinting() {
                    self.award_stat(id, stats::SPRINT_ONE_CM, horizontal_distance);
                    self.cause_food_exhaustion(id, EXHAUSTION_SPRINT * horizontal_distance as f32 * 0.01);
                } else if e.is_crouching() {
                    self.award_stat(id, stats::CROUCH_ONE_CM, horizontal_distance);
                    self.cause_food_exhaustion(id, 0.0);
                } else {
                    self.award_stat(id, stats::WALK_ONE_CM, horizontal_distance);
                    self.cause_food_exhaustion(id, 0.0);
                }
            }
        } else if self.entity(id).is_fall_flying() {
            let distance = distance(dx * dx + dy * dy + dz * dz);
            self.award_stat(id, stats::AVIATE_ONE_CM, distance);
        } else {
            let horizontal_distance = distance(dx * dx + dz * dz);
            if horizontal_distance > 25 {
                self.award_stat(id, stats::FLY_ONE_CM, horizontal_distance);
            }
        }
    }

    pub fn award_stat(&mut self, id: EntityId, stat: Stat, count: i32) {
        let Some(player) = self.find_player_mut(id) else { return };
        player.stats.increment(stat, count);
        let criteria = stat.criteria();
        if self.scoreboard.borrow().objectives_by_criteria(&criteria).is_empty() {
            return;
        }
        let holder = ScoreHolder::for_entity(self, id);
        self.for_all_objectives(&criteria, &holder, |level, score| {
            let _ = score.add(level, count);
        });
    }

    pub fn reset_stat(&mut self, id: EntityId, stat: Stat) {
        let Some(player) = self.find_player_mut(id) else { return };
        player.stats.set_value(stat, 0);
        let criteria = stat.criteria();
        if self.scoreboard.borrow().objectives_by_criteria(&criteria).is_empty() {
            return;
        }
        let holder = ScoreHolder::for_entity(self, id);
        self.for_all_objectives(&criteria, &holder, |level, score| {
            let _ = score.reset(level);
        });
    }

    pub fn server_player_jump_from_ground(&mut self, id: EntityId) {
        self.living_entity_jump_from_ground(id);
        self.award_stat(id, stats::JUMP, 1);
        let exhaustion = if self.entity(id).is_sprinting() { crate::world::food::constants::EXHAUSTION_SPRINT_JUMP } else { crate::world::food::constants::EXHAUSTION_JUMP };
        self.cause_food_exhaustion(id, exhaustion);
    }

    pub fn server_player_on_effect_added(&mut self, id: EntityId, effect: MobEffect, source: Option<EntityId>) {
        self.living_entity_on_effect_added(id, effect, source);
        let p = self.player(id);
        let instance = p.living.effect(effect).expect("added effect");
        p.connection.send_raw(update_mob_effect(id, instance, true));
        if effect == wafer_data::mob_effects::LEVITATION {
            let tick_count = self.entity(id).tick_count;
            let p = self.player_mut(id);
            p.levitation_start_time = tick_count;
            p.levitation_start_pos = Some(p.entity.position());
        }
        criteria_triggers::EFFECTS_CHANGED.trigger(self, id, source);
    }

    pub fn server_player_on_effect_updated(&mut self, id: EntityId, effect: MobEffect, refresh_attributes: bool, source: Option<EntityId>) {
        self.living_entity_on_effect_updated(id, effect, refresh_attributes, source);
        let p = self.player(id);
        let instance = p.living.effect(effect).expect("updated effect");
        p.connection.send_raw(update_mob_effect(id, instance, false));
        criteria_triggers::EFFECTS_CHANGED.trigger(self, id, source);
    }

    pub fn server_player_on_effects_removed(&mut self, id: EntityId, effects: &[MobEffectInstance]) {
        self.living_entity_on_effects_removed(id, effects);
        let p = self.player_mut(id);
        for effect in effects {
            p.connection.send_raw(remove_mob_effect(id, effect.effect));
            if effect.effect == wafer_data::mob_effects::LEVITATION {
                p.levitation_start_pos = None;
            }
        }
        criteria_triggers::EFFECTS_CHANGED.trigger(self, id, None);
    }

    pub fn show_end_credits(&mut self, id: EntityId) {
        self.player_remove(id, crate::world::entity::RemovalReason::ChangedDimension);
        self.update_sleeping_player_list();
        let p = self.player_mut(id);
        if !p.won_game {
            p.won_game = true;
            p.connection.send(clientbound::GAME_EVENT, |w| {
                w.u8(GAME_EVENT_WIN_GAME);
                w.f32(0.0);
            });
            p.seen_credits = true;
        }
    }

    pub fn send_active_player_effects(&self, id: EntityId) {
        let p = self.player(id);
        for effect in &p.living.active_effects {
            p.connection.send_raw(update_mob_effect(id, effect, false));
        }
    }

    pub fn send_system_message(&self, id: EntityId, message: &crate::network::chat::Component) {
        self.player(id).send_system_message(message, false);
    }

    pub fn get_camera(&self, id: EntityId) -> EntityId {
        self.player(id).camera.unwrap_or(id)
    }

    pub fn set_camera(&mut self, id: EntityId, new_camera: Option<EntityId>) {
        let old_camera = self.get_camera(id);
        let camera = new_camera.unwrap_or(id);
        self.player_mut(id).camera = Some(camera);
        if old_camera != camera {
            if let Some(pos) = self.entity_ref(camera).map(Entity::position) {
                self.server_player_teleport_in_level(id, pos);
            }
            self.on_camera_changed(id, camera, new_camera.is_some());
        }
    }

    fn server_player_teleport_in_level(&mut self, id: EntityId, pos: Vec3) {
        if self.is_sleeping(id) {
            self.stop_sleep_in_bed(id, true, true);
        }
        let player = self.player_mut(id);
        let (y_rot, x_rot) = (player.entity.y_rot, player.entity.x_rot);
        let destination = crate::world::entity::relative::PositionMoveRotation { position: pos, delta_movement: Vec3::ZERO, y_rot, x_rot };
        player.teleport_relative(destination, 0);
        player.reset_position();
        player.entity.y_head_rot = y_rot;
        player.movement.above_ground_tick_count = 0;
    }

    pub(crate) fn on_camera_changed(&mut self, id: EntityId, camera: EntityId, moved: bool) {
        if moved {
            self.move_player(id);
        }
        let player = self.player_mut(id);
        player.connection.send(clientbound::SET_CAMERA, |w| w.varint(camera));
        player.reset_position();
    }

    pub fn tick_camera(&mut self, id: EntityId) {
        let camera = self.get_camera(id);
        if camera == id {
            return;
        }
        if self.entity_ref(camera).is_some() && self.is_alive(camera) {
            let e = self.entity(camera);
            let (pos, y_rot, x_rot) = (e.position(), e.y_rot, e.x_rot);
            let player = &mut self.player_mut(id).entity;
            player.abs_snap_to(pos.x, pos.y, pos.z);
            player.abs_snap_rotation_to(y_rot, x_rot);
            self.move_player(id);
            if self.player(id).entity.is_shift_key_down() {
                self.set_camera(id, Some(id));
            }
        } else {
            self.set_camera(id, Some(id));
        }
    }
}

impl crate::server::Server {
    pub fn set_camera(&mut self, id: EntityId, new_camera: Option<EntityId>) {
        let level = self.level_of(id).expect("player on this server");
        let camera = new_camera.unwrap_or(id);
        match self.level_of(camera) {
            Some(camera_level) if camera_level != level && self.levels[level].get_camera(id) != camera => {
                self.player_mut(id).camera = Some(camera);
                let pos = self.levels[camera_level].entity(camera).position();
                let (y_rot, x_rot) = (self.player(id).entity.y_rot, self.player(id).entity.x_rot);
                self.teleport_to(id, camera_level, pos, 0, y_rot, x_rot, false);
                let level = self.level_of(id).expect("player on this server");
                self.levels[level].on_camera_changed(id, camera, new_camera.is_some());
            }
            _ => self.levels[level].set_camera(id, new_camera),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn server_player_teleport_to(&mut self, id: EntityId, level: usize, pos: Vec3, relatives: u16, y_rot: f32, x_rot: f32, reset_camera: bool) -> bool {
        let current = self.level_of(id).expect("player on this server");
        if self.levels[current].is_sleeping(id) {
            self.levels[current].stop_sleep_in_bed(id, true, true);
        }
        if reset_camera {
            self.set_camera(id, Some(id));
        }
        let transition = TeleportTransition::with_relatives(level, pos, Vec3::ZERO, y_rot, x_rot, relatives, PostTeleportTransition::DO_NOTHING);
        let success = self.server_player_teleport(id, transition).is_some();
        if success {
            let p = self.player_mut(id);
            p.entity.y_head_rot = if relatives & crate::world::entity::relative::relative::Y_ROT != 0 { p.entity.y_head_rot + y_rot } else { y_rot };
            p.movement.above_ground_tick_count = 0;
        }
        success
    }

    pub fn server_player_teleport(&mut self, id: EntityId, transition: TeleportTransition) -> Option<EntityId> {
        let old_level = self.level_of(id)?;
        if self.levels[old_level].entity(id).is_removed() {
            return None;
        }
        let out = self.player(id).connection.clone();
        if transition.missing_respawn_block {
            out.send(clientbound::GAME_EVENT, |w| {
                w.u8(0);
                w.f32(0.0);
            });
        }
        let new_level = transition.new_level;
        if !transition.as_passenger {
            self.levels[old_level].remove_vehicle(id);
        }
        if new_level == old_level {
            let level = &mut self.levels[old_level];
            level.server_player_teleport_position(id, transition);
            return Some(id);
        }
        self.player_mut(id).is_changing_dimension = true;
        out.send(clientbound::RESPAWN, |w| {
            self.create_common_spawn_info(w, new_level, self.player(id));
            w.u8(3);
        });
        out.send(clientbound::CHANGE_DIFFICULTY, |w| {
            w.varint(self.difficulty.id());
            w.bool(self.difficulty_locked);
        });
        self.send_player_permission_level(id);
        let mut player = self.levels[old_level].remove_player_immediately(id, crate::world::entity::RemovalReason::ChangedDimension);
        player.entity.removal = None;
        if self.levels[old_level].dimension == crate::server::OVERWORLD && self.levels[new_level].dimension == crate::server::THE_NETHER {
            player.entered_nether_position = Some(player.entity.position());
        }
        let level = &mut self.levels[new_level];
        level.players.insert(id, player);
        let teleport = level.player_mut(id).teleport_set_position(transition.position_move_rotation(), transition.relatives);
        level.entity_moved(id);
        let player = level.player_mut(id);
        player.send_teleport(teleport, transition.position_move_rotation(), transition.relatives);
        player.reset_position();
        let player = level.players.shift_remove(&id).expect("teleported player");
        level.add_player(player);
        self.trigger_dimension_change_triggers(id, old_level);
        let level = &mut self.levels[new_level];
        level.stop_using_item(id);
        let player = level.player(id);
        player.connection.send(clientbound::PLAYER_ABILITIES, |w| player.abilities.write(w));
        self.send_level_info(&out, new_level);
        self.send_all_player_info(id);
        let level = &mut self.levels[new_level];
        level.send_active_player_effects(id);
        level.player_mut(id).send_post_effects();
        transition.post_teleport_transition.on_transition(level, id);
        let player = level.player_mut(id);
        player.last_sent_exp = -1;
        player.last_sent_health = -1.0;
        player.last_sent_food = -1;
        self.teleport_spectators(id, transition, old_level);
        Some(id)
    }

    pub fn teleport_spectators(&mut self, entity: EntityId, transition: TeleportTransition, old_level: usize) {
        let spectators: Vec<EntityId> = self.levels[old_level].players.values().filter(|p| p.camera == Some(entity)).map(|p| p.entity.id).collect();
        for spectator in spectators {
            self.teleport(spectator, transition);
            self.set_camera(spectator, None);
        }
    }

    fn trigger_dimension_change_triggers(&mut self, id: EntityId, old_level: usize) {
        let new_level = self.level_of(id).expect("player on this server");
        let old_key = self.levels[old_level].dimension;
        let new_key = self.levels[new_level].dimension;
        let level = &mut self.levels[new_level];
        criteria_triggers::CHANGED_DIMENSION.trigger(level, id, old_key, new_key);
        if old_key == crate::server::THE_NETHER
            && new_key == crate::server::OVERWORLD
            && let Some(entered_nether_position) = level.player(id).entered_nether_position
        {
            criteria_triggers::NETHER_TRAVEL.trigger(level, id, entered_nether_position);
        }
        if self.levels[new_level].dimension != crate::server::THE_NETHER {
            self.player_mut(id).entered_nether_position = None;
        }
    }

    pub fn send_all_player_info(&mut self, id: EntityId) {
        let level = self.entity_level_mut(id);
        if let Some(mut menu) = level.player_mut(id).inventory_menu.take() {
            level.menu_send_all_data_to_remote(&mut menu, id);
            level.player_mut(id).inventory_menu = Some(menu);
        }
        let player = level.player_mut(id);
        player.last_sent_health = -1.0e8;
        let selected = player.inventory.selected as i32;
        player.connection.send(clientbound::SET_HELD_SLOT, |w| w.varint(selected));
    }
}
