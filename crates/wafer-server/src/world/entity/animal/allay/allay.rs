use wafer_core::{BlockPos, Vec3};
use wafer_data::{EquipmentSlot, blocks, item_tags, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::allay_ai;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::behavior::behavior_utils;
use crate::world::entity::ai::brain::{self, Brain, Provider};
use crate::world::entity::ai::control::flying_move_control::FlyingMoveControl;
use crate::world::entity::ai::control::move_control::MoveControlKind;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::navigation::path_navigation::{PathNavigation, PathNavigationKind};
use crate::world::entity::ai::sensing::sensor_type::SensorType;
use crate::world::entity::attributes::ALLAY_ATTRIBUTES;
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::entity_event;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::npc::inventory_carrier;
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, sound_event};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::BlockGetter;
use crate::world::simple_container::{SimpleContainer, SimpleContainerKind};
use crate::world::level::gameevent::dynamic_game_event_listener::DynamicGameEventListener;
use crate::world::level::gameevent::game_event::{self, Context, GameEvent};
use crate::world::level::gameevent::game_event_listener::{EntityListenerKind, GameEventListener};
use crate::world::level::gameevent::position_source::PositionSource;
use crate::world::level::gameevent::vibrations::vibration_system::{Data, VibrationUser};
use crate::world::{InteractionHand, InteractionResult};
use crate::world::level::storage::tag_value_input;

pub const ITEM_PICKUP_REACH: (f64, f64, f64) = (1.0, 1.0, 1.0);
const DEFAULT_DUPLICATION_COOLDOWN: i64 = 0;
const DUPLICATION_COOLDOWN_TICKS: i64 = 6000;
pub const MAX_NOTEBLOCK_DISTANCE: i32 = 1024;
pub const VIBRATION_EVENT_LISTENER_RANGE: i32 = 16;
const JUKEBOX_PLAY_NOTIFICATION_RADIUS: f64 = 10.0;
pub const DATA_DANCING: usize = 16;
pub const DATA_CAN_DUPLICATE: usize = 17;
pub const THROW_SOUND_PITCHES: [f32; 16] = [0.5625, 0.625, 0.75, 0.9375, 1.0, 1.0, 1.125, 1.25, 1.5, 1.875, 2.0, 2.25, 2.5, 3.0, 3.75, 4.0];
const MOVE_CONTROL: FlyingMoveControl = FlyingMoveControl::new(20, true);
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.35, 0.6).with_eye_height(0.36).with_vehicle_attachment(&[Vec3::new(0.0, -0.04f32 as f64, 0.0)]);

pub static BRAIN_PROVIDER: Provider = brain::provider_with_memories(
    &[memory_module_type::LIKED_PLAYER.erase(), memory_module_type::LIKED_NOTEBLOCK_POSITION.erase(), memory_module_type::LIKED_NOTEBLOCK_COOLDOWN_TICKS.erase()],
    &[SensorType::NearestLivingEntities, SensorType::NearestPlayers, SensorType::HurtBy, SensorType::NearestItems],
    |_| allay_ai::get_activities(),
);

pub struct Allay {
    pub dynamic_vibration_listener: DynamicGameEventListener,
    vibration_data: Data,
    pub dynamic_jukebox_listener: DynamicGameEventListener,
    pub inventory: SimpleContainer,
    jukebox_pos: Option<BlockPos>,
    duplication_cooldown: i64,
}

impl Allay {
    fn new(id: EntityId) -> Allay {
        Allay {
            dynamic_vibration_listener: DynamicGameEventListener::new(GameEventListener::Entity { entity: id, kind: EntityListenerKind::Vibration }),
            vibration_data: Data::default(),
            dynamic_jukebox_listener: DynamicGameEventListener::new(GameEventListener::Entity { entity: id, kind: EntityListenerKind::Jukebox }),
            inventory: SimpleContainer::new(SimpleContainerKind::Plain, 1),
            jukebox_pos: None,
            duplication_cooldown: DEFAULT_DUPLICATION_COOLDOWN,
        }
    }
}

pub fn allay_mut(mob: &mut Mob) -> &mut Allay {
    match &mut mob.kind {
        MobKind::Allay(allay) => allay,
        _ => unreachable!("an allay"),
    }
}

pub fn allay(mob: &Mob) -> &Allay {
    match &mob.kind {
        MobKind::Allay(allay) => allay,
        _ => unreachable!("an allay"),
    }
}

pub fn is_dancing(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_DANCING), DataValue::Boolean(true))
}

fn can_duplicate(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_CAN_DUPLICATE), DataValue::Boolean(true))
}

pub fn has_item_in_hand(mob: &Mob) -> bool {
    mob.item_by_slot(EquipmentSlot::Mainhand).is_some_and(|stack| !stack.is_empty())
}

fn is_on_pickup_cooldown(mob: &Mob) -> bool {
    mob.living.brain.has_memory_value(memory_module_type::ITEM_PICKUP_COOLDOWN_TICKS)
}

pub fn can_pick_up_loot(mob: &Mob) -> bool {
    !is_on_pickup_cooldown(mob) && has_item_in_hand(mob)
}

pub fn get_ambient_sound(mob: &Mob) -> &'static str {
    if has_item_in_hand(mob) { "minecraft:entity.allay.ambient_with_item" } else { "minecraft:entity.allay.ambient_without_item" }
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.allay.hurt"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.allay.death"
}

pub fn get_sound_volume() -> f32 {
    0.4
}

fn allay_considers_item_equal(item1: &ItemStack, item2: &ItemStack) -> bool {
    ItemStack::is_same_item(item1, item2) && !has_non_matching_potion(item1, item2)
}

fn has_non_matching_potion(item_in_hand: &ItemStack, pickup_item: &ItemStack) -> bool {
    item_in_hand.get(dc::POTION_CONTENTS) != pickup_item.get(dc::POTION_CONTENTS)
}

pub fn equipment_has_changed(previous: &ItemStack, current: &ItemStack) -> bool {
    !allay_considers_item_equal(previous, current)
}

fn set_duplication_cooldown(mob: &mut Mob, duplication_cooldown: i64) {
    allay_mut(mob).duplication_cooldown = duplication_cooldown;
    mob.entity.data.set(DATA_CAN_DUPLICATE, DataValue::Boolean(duplication_cooldown == 0));
}

fn reset_duplication_cooldown(mob: &mut Mob) {
    set_duplication_cooldown(mob, DUPLICATION_COOLDOWN_TICKS);
}

impl Allay {
    pub fn create() -> Mob {
        let mut data = mob_defaults();
        data.extend([DataValue::Boolean(false), DataValue::Boolean(true)]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:allay"), DIMENSIONS, data);
        let id = entity.id;
        let mut mob = Mob::new(entity, LivingEntity::new(&ALLAY_ATTRIBUTES), MobKind::Allay(Allay::new(id)));
        mob.ai.move_control.kind = MoveControlKind::Flying(MOVE_CONTROL);
        mob.can_pick_up_loot = can_pick_up_loot(&mob);
        mob
    }

    pub fn create_navigation(follow_range_base: f64) -> PathNavigation {
        let mut flying_path_navigation = PathNavigation::of_kind(PathNavigationKind::Flying, follow_range_base);
        flying_path_navigation.path_finder.node_evaluator.node_evaluator_mut().can_open_doors = false;
        flying_path_navigation.set_can_float(true);
        flying_path_navigation.set_required_path_length_detached(follow_range_base, 48.0);
        flying_path_navigation
    }

    pub fn make_brain(mob: &mut Mob, packed_brain: brain::Packed) -> Brain {
        BRAIN_PROVIDER.make_brain(mob, packed_brain)
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        let allay = allay(mob);
        inventory_carrier::write_inventory_to_tag(&allay.inventory, output);
        output.push(("listener".into(), allay.vibration_data.to_nbt()));
        output.push(("DuplicationCooldown".into(), Nbt::Long(allay.duplication_cooldown)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        inventory_carrier::read_inventory_from_tag(&mut allay_mut(mob).inventory, input);
        allay_mut(mob).vibration_data = Data::load(input.get("listener"));
        let duplication_cooldown = tag_value_input::get_i64(input, "DuplicationCooldown").map_or(DEFAULT_DUPLICATION_COOLDOWN, |value| value as i32 as i64);
        set_duplication_cooldown(mob, duplication_cooldown);
    }
}

impl ServerLevel {
    pub fn allay_vibration_data(&self, id: EntityId) -> Option<&Data> {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Allay(allay)) => Some(&allay.vibration_data),
            _ => None,
        }
    }

    pub fn allay_vibration_data_mut(&mut self, id: EntityId) -> Option<&mut Data> {
        match self.mob_mut(id).map(|mob| &mut mob.kind) {
            Some(MobKind::Allay(allay)) => Some(&mut allay.vibration_data),
            _ => None,
        }
    }

    pub fn allay_position_source(&self, id: EntityId) -> PositionSource {
        PositionSource::Entity { entity: id, y_offset: DIMENSIONS.eye_height }
    }

    pub fn allay_listener_source(&self, id: EntityId, _kind: EntityListenerKind) -> PositionSource {
        self.allay_position_source(id)
    }

    pub fn allay_listener_radius(&self, kind: EntityListenerKind) -> i32 {
        match kind {
            EntityListenerKind::Vibration => VIBRATION_EVENT_LISTENER_RANGE,
            EntityListenerKind::Jukebox => game_event::JUKEBOX_PLAY.notification_radius(),
        }
    }

    pub fn allay_handle_game_event(&mut self, id: EntityId, kind: EntityListenerKind, event: GameEvent, context: Context, source_position: Vec3) -> bool {
        match kind {
            EntityListenerKind::Vibration => self.vibration_listener_handle_game_event(VibrationUser::Allay(id), event, context, source_position),
            EntityListenerKind::Jukebox if event == game_event::JUKEBOX_PLAY => {
                self.allay_set_jukebox_playing(id, BlockPos::containing_vec(source_position), true);
                true
            }
            EntityListenerKind::Jukebox if event == game_event::JUKEBOX_STOP_PLAY => {
                self.allay_set_jukebox_playing(id, BlockPos::containing_vec(source_position), false);
                true
            }
            EntityListenerKind::Jukebox => false,
        }
    }

    pub fn allay_can_receive_vibration(&self, id: EntityId, pos: BlockPos) -> bool {
        if self.mob(id).is_none_or(|mob| mob.is_no_ai()) {
            return false;
        }
        match self.get_brain(id).get_memory(memory_module_type::LIKED_NOTEBLOCK_POSITION) {
            None => true,
            Some(global_pos) => global_pos.is_close_enough(self.dimension, self.entity(id).block_position(), MAX_NOTEBLOCK_DISTANCE) && global_pos.pos() == pos,
        }
    }

    pub fn allay_on_receive_vibration(&mut self, id: EntityId, pos: BlockPos, event: GameEvent) {
        if event == game_event::NOTE_BLOCK_PLAY {
            allay_ai::hear_noteblock(self, id, pos);
        }
    }

    pub fn is_allay(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Allay(_)))
    }

    pub fn allay_travel(&mut self, id: EntityId, input: wafer_core::Vec3) {
        let speed = self.get_speed(id);
        self.travel_flying(id, input, speed);
    }

    pub fn allay_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        if self.allay_is_liked_player(id, source.causing_entity) { false } else { self.living_entity_hurt_server(id, source, damage) }
    }

    pub fn allay_considers_entity_as_ally(&self, id: EntityId, other: EntityId) -> bool {
        self.allay_is_liked_player(id, Some(other)) || self.entity_considers_entity_as_ally(id, other)
    }

    fn allay_is_liked_player(&self, id: EntityId, other: Option<EntityId>) -> bool {
        let Some(player) = other.filter(|&other| self.is_player(other)) else { return false };
        self.get_brain(id).get_memory(memory_module_type::LIKED_PLAYER).is_some_and(|&liked_player| self.entity(player).uuid == liked_player)
    }

    pub fn allay_custom_server_ai_step(&mut self, id: EntityId) {
        Brain::tick(self, id);
        if self.mob(id).is_none() {
            return;
        }
        allay_ai::update_activity(self.get_brain_mut(id));
    }

    pub fn allay_ai_step(&mut self, id: EntityId) {
        self.mob_ai_step(id);
        if self.mob(id).is_none() {
            return;
        }
        if self.is_alive(id) && self.entity(id).tick_count % 10 == 0 {
            self.heal(id, 1.0);
        }
        if is_dancing(self.entity(id)) && self.allay_should_stop_dancing(id) && self.entity(id).tick_count % 20 == 0 {
            self.allay_set_dancing(id, false);
            allay_mut(self.mob_mut(id).expect("an allay")).jukebox_pos = None;
        }
        self.allay_update_duplication_cooldown(id);
    }

    pub fn allay_tick(&mut self, id: EntityId) {
        self.mob_tick(id);
        if self.mob(id).is_none() {
            return;
        }
        self.vibration_ticker_tick(VibrationUser::Allay(id));
        if self.mob(id).is_none() {
            return;
        }
        if self.is_panicking(id) {
            self.allay_set_dancing(id, false);
        }
    }

    pub fn allay_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let mut interaction_item = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        let item_in_hand = self.item_by_slot(id, EquipmentSlot::Mainhand).unwrap_or_else(ItemStack::empty);
        if is_dancing(self.entity(id)) && interaction_item.get_item().is(&item_tags::DUPLICATES_ALLAYS) && can_duplicate(self.entity(id)) {
            self.allay_duplicate_allay(id);
            self.broadcast_entity_event(id, entity_event::IN_LOVE_HEARTS);
            self.play_sound_from_entity(Some(player), id, sound_event("minecraft:block.amethyst_block.chime"), SoundSource::Neutral, 2.0, 1.0);
            self.allay_remove_interaction_item(player, hand, &mut interaction_item);
            return InteractionResult::SUCCESS;
        }
        if item_in_hand.is_empty() && !interaction_item.is_empty() {
            let item_to_give = interaction_item.copy_with_count(1);
            self.set_item_in_hand(id, InteractionHand::MainHand, item_to_give);
            self.allay_remove_interaction_item(player, hand, &mut interaction_item);
            self.play_sound_from_entity(Some(player), id, sound_event("minecraft:entity.allay.item_given"), SoundSource::Neutral, 2.0, 1.0);
            let uuid = self.entity(player).uuid;
            self.get_brain_mut(id).set_memory(memory_module_type::LIKED_PLAYER, uuid);
            return InteractionResult::SUCCESS;
        }
        if !item_in_hand.is_empty() && hand == InteractionHand::MainHand && interaction_item.is_empty() {
            self.living_set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::empty());
            self.play_sound_from_entity(Some(player), id, sound_event("minecraft:entity.allay.item_taken"), SoundSource::Neutral, 2.0, 1.0);
            self.swing(id, InteractionHand::MainHand, item_in_hand.interact_animation(), false);
            let items = allay_mut(self.mob_mut(id).expect("an allay")).inventory.remove_all_items();
            for item_stack in items {
                let position = self.entity(id).position();
                behavior_utils::throw_item(self, id, item_stack, position);
            }
            self.get_brain_mut(id).erase_memory(memory_module_type::LIKED_PLAYER);
            self.player_inventory_add(player, item_in_hand);
            InteractionResult::SUCCESS
        } else {
            self.mob_mob_interact(id, player, hand)
        }
    }

    pub fn allay_set_jukebox_playing(&mut self, id: EntityId, jukebox: BlockPos, is_playing: bool) {
        if is_playing {
            if !is_dancing(self.entity(id)) {
                allay_mut(self.mob_mut(id).expect("an allay")).jukebox_pos = Some(jukebox);
                self.allay_set_dancing(id, true);
            }
        } else {
            let jukebox_pos = allay(self.mob(id).expect("an allay")).jukebox_pos;
            if jukebox_pos == Some(jukebox) || jukebox_pos.is_none() {
                allay_mut(self.mob_mut(id).expect("an allay")).jukebox_pos = None;
                self.allay_set_dancing(id, false);
            }
        }
    }

    pub fn allay_wants_to_pick_up(&self, id: EntityId, item_stack: &ItemStack) -> bool {
        let mob = self.mob(id).expect("an allay");
        let Some(item_in_hand) = mob.item_by_slot(EquipmentSlot::Mainhand).filter(|stack| !stack.is_empty()) else { return false };
        self.game_rules.get_bool(wafer_data::game_rule::MOB_GRIEFING) && allay(mob).inventory.can_add_item(item_stack) && allay_considers_item_equal(item_in_hand, item_stack)
    }

    pub fn allay_pick_up_item(&mut self, id: EntityId, entity: EntityId) {
        self.inventory_carrier_pick_up_item(id, entity, move |level| &mut allay_mut(level.mob_mut(id).expect("an allay")).inventory);
    }

    fn allay_set_dancing(&mut self, id: EntityId, is_dancing: bool) {
        if self.mob(id).is_some_and(|mob| !mob.is_no_ai()) && (!is_dancing || !self.is_panicking(id)) {
            self.entity_mut(id).data.set(DATA_DANCING, DataValue::Boolean(is_dancing));
        }
    }

    fn allay_should_stop_dancing(&mut self, id: EntityId) -> bool {
        let Some(jukebox_pos) = allay(self.mob(id).expect("an allay")).jukebox_pos else { return true };
        let position = self.entity(id).position();
        !jukebox_pos.closer_to_center_than(position, JUKEBOX_PLAY_NOTIFICATION_RADIUS) || !self.block_state(jukebox_pos).is_block(blocks::JUKEBOX)
    }

    pub fn allay_drop_equipment(&mut self, id: EntityId) {
        let items = allay_mut(self.mob_mut(id).expect("an allay")).inventory.remove_all_items();
        for item_stack in items {
            self.spawn_at_location(id, item_stack, 0.0);
        }
        let item_stack = self.item_by_slot(id, EquipmentSlot::Mainhand).unwrap_or_else(ItemStack::empty);
        if !item_stack.is_empty() && !crate::world::item::enchantment::enchantment_helper::has_prevent_equipment_drop(&item_stack) {
            self.spawn_at_location(id, item_stack, 0.0);
            self.living_set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::empty());
        }
    }

    fn allay_update_duplication_cooldown(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("an allay");
        let duplication_cooldown = allay(mob).duplication_cooldown;
        if duplication_cooldown > 0 {
            set_duplication_cooldown(mob, duplication_cooldown - 1);
        }
    }

    fn allay_duplicate_allay(&mut self, id: EntityId) {
        let mut allay = Allay::create();
        let position = self.entity(id).position();
        allay.entity.snap_to(position.x, position.y, position.z, allay.entity.y_rot, allay.entity.x_rot);
        allay.persistence_required = true;
        reset_duplication_cooldown(&mut allay);
        reset_duplication_cooldown(self.mob_mut(id).expect("an allay"));
        self.add_fresh_entity(LevelEntity::Mob(Box::new(allay)));
    }

    fn allay_remove_interaction_item(&mut self, player: EntityId, hand: InteractionHand, interaction_item: &mut ItemStack) {
        interaction_item.consume(1, self, Some(player));
        self.player_mut(player).update_item_in_hand(hand, interaction_item);
    }
}
