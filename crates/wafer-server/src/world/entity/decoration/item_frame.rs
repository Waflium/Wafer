use wafer_core::random::RandomSource;
use wafer_core::{AABB, Axis, BlockPos, Direction, Vec3};
use wafer_data::{blocks, damage_type_tags, game_rule, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::block_attached_entity::{self, BlockAttachedEntity, BlockAttachedKind};
use super::hanging_entity::{self, HangingView};
use crate::server::level::ServerLevel;
use crate::util::codec;
use crate::world::damagesource::DamageSource;
use crate::world::entity::data::{DataValue, entity_defaults};
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, flags, sound_event};
use crate::world::item::component::{self as dc};
use crate::world::item::{ItemStack, map_item};
use crate::world::level::BlockGetter;
use crate::world::level::block::entity::{byte_or, float_or};
use crate::world::level::block::is_diode;
use crate::world::level::saveddata::maps::map_id::MapId;
use crate::world::{InteractionHand, InteractionResult};
use crate::world::level::storage::tag_value_input;

const DATA_ITEM: usize = 9;
const DATA_ROTATION: usize = 10;
pub const NUM_ROTATIONS: i32 = 8;
const DEFAULT_DROP_CHANCE: f32 = 1.0;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.5, 0.5).with_eye_height(0.0);

pub struct ItemFrame {
    pub entity: Entity,
    pub glow: bool,
    drop_chance: f32,
    fixed: bool,
    pending_add_item_sound: Option<Vec3>,
}

pub fn get_item(entity: &Entity) -> Option<&ItemStack> {
    match entity.data.get(DATA_ITEM) {
        DataValue::ItemStack(item) => item.as_ref(),
        _ => None,
    }
}

fn get_framed_map_id(item_stack: &ItemStack) -> Option<MapId> {
    item_stack.get(dc::MAP_ID).and_then(MapId::from_nbt)
}

pub fn has_framed_map(entity: &Entity) -> bool {
    get_item(entity).is_some_and(|item| item.has(dc::MAP_ID))
}

pub fn calculate_bounding_box(entity: &Entity, pos: BlockPos, direction: Direction) -> AABB {
    create_bounding_box(pos, direction, has_framed_map(entity))
}

fn create_bounding_box(block_pos: BlockPos, direction: Direction, has_framed_map: bool) -> AABB {
    let position = Vec3::at_center_of(block_pos).relative(direction, -0.46875);
    let width = if has_framed_map { 1.0f32 } else { 0.75 };
    let height = if has_framed_map { 1.0f32 } else { 0.75 };
    let axis = direction.axis();
    let x_size = if axis == Axis::X { 0.0625 } else { width as f64 };
    let y_size = if axis == Axis::Y { 0.0625 } else { height as f64 };
    let z_size = if axis == Axis::Z { 0.0625 } else { width as f64 };
    AABB::of_size(position, x_size, y_size, z_size)
}

fn set_direction(entity: &mut Entity, direction: Direction) {
    hanging_entity::set_direction_raw(entity, direction);
    if direction.axis().is_horizontal() {
        entity.x_rot = 0.0;
        entity.y_rot = (direction.get_2d_data_value() * 90) as f32;
    } else {
        entity.x_rot = (-90 * direction.axis_direction().step()) as f32;
        entity.y_rot = 0.0;
    }
    entity.x_rot_o = entity.x_rot;
    entity.y_rot_o = entity.y_rot;
    block_attached_entity::recalculate_bounding_box(entity);
}

fn get_rotation(entity: &Entity) -> i32 {
    match entity.data.get(DATA_ROTATION) {
        DataValue::Int(rotation) => *rotation,
        _ => 0,
    }
}

impl ItemFrame {
    pub fn create_empty(glow: bool) -> ItemFrame {
        let mut data = entity_defaults();
        data.push(DataValue::Direction(hanging_entity::DEFAULT_DIRECTION));
        data.push(DataValue::ItemStack(None));
        data.push(DataValue::Int(0));
        let entity_type = if glow { "minecraft:glow_item_frame" } else { "minecraft:item_frame" };
        let mut entity = Entity::new(registry_id(registry::ENTITY_TYPE, entity_type), DIMENSIONS, data);
        entity.block_attached = Some(BlockAttachedEntity::new(BlockAttachedKind::ItemFrame, BlockPos::ZERO));
        entity.set_shared_flag(flags::INVISIBLE, false);
        ItemFrame { entity, glow, drop_chance: DEFAULT_DROP_CHANCE, fixed: false, pending_add_item_sound: None }
    }

    pub fn new(glow: bool, pos: BlockPos, direction: Direction) -> ItemFrame {
        let mut frame = ItemFrame::create_empty(glow);
        frame.entity.block_attached = Some(BlockAttachedEntity::new(BlockAttachedKind::ItemFrame, pos));
        set_direction(&mut frame.entity, direction);
        frame.entity.set_shared_flag(flags::INVISIBLE, false);
        frame
    }

    pub fn is_fixed(&self) -> bool {
        self.fixed
    }

    pub fn item_mut(&mut self) -> &mut DataValue {
        self.entity.data.get_mut(DATA_ITEM)
    }

    pub fn pop_box(&self) -> AABB {
        create_bounding_box(self.entity.block_attached_pos(), hanging_entity::get_direction(&self.entity), false)
    }

    pub fn set_item_data(&mut self, item_stack: ItemStack) {
        let item_stack = if item_stack.is_empty() { None } else { Some(item_stack.copy_with_count(1)) };
        let added = item_stack.is_some();
        self.entity.data.set(DATA_ITEM, DataValue::ItemStack(item_stack));
        block_attached_entity::recalculate_bounding_box(&mut self.entity);
        if added {
            self.pending_add_item_sound = Some(self.entity.position());
        }
    }

    pub fn add_additional_save_data(&self, output: &mut Vec<(String, Nbt)>) {
        block_attached_entity::add_additional_save_data(&self.entity, output);
        if let Some(current_item) = get_item(&self.entity) {
            output.push(("Item".into(), current_item.to_nbt()));
        }
        output.push(("ItemRotation".into(), Nbt::Byte(get_rotation(&self.entity) as i8)));
        output.push(("ItemDropChance".into(), Nbt::Float(self.drop_chance)));
        output.push(("Facing".into(), Nbt::Byte(hanging_entity::get_direction(&self.entity).ordinal() as i8)));
        output.push(("Invisible".into(), Nbt::Byte(self.entity.shared_flag(flags::INVISIBLE) as i8)));
        output.push(("Fixed".into(), Nbt::Byte(self.fixed as i8)));
    }

    pub fn read_additional_save_data(&mut self, input: &Nbt) {
        block_attached_entity::read_additional_save_data(&mut self.entity, input);
        let item_stack = tag_value_input::read(input, "Item", ItemStack::from_nbt_partial).unwrap_or_else(ItemStack::empty);
        self.set_item_data(item_stack);
        self.entity.data.set(DATA_ROTATION, DataValue::Int(byte_or(input, "ItemRotation", 0) as i32 % NUM_ROTATIONS));
        self.drop_chance = float_or(input, "ItemDropChance", DEFAULT_DROP_CHANCE);
        let facing = tag_value_input::read_or_error(input, "Facing", |tag| codec::int(tag)).map_or(Direction::Down, |value| Direction::from_3d_data_value(value as i8 as i32));
        set_direction(&mut self.entity, facing);
        self.entity.set_shared_flag(flags::INVISIBLE, byte_or(input, "Invisible", 0) != 0);
        self.fixed = byte_or(input, "Fixed", 0) != 0;
    }

    pub fn get_pick_result(&self) -> ItemStack {
        match get_item(&self.entity) {
            Some(framed_stack) => framed_stack.copy(),
            None => self.get_frame_item_stack_with_data(),
        }
    }

    fn get_frame_item_stack(&self) -> ItemStack {
        ItemStack::new(if self.glow { items::GLOW_ITEM_FRAME } else { items::ITEM_FRAME }, 1)
    }

    fn get_frame_item_stack_with_data(&self) -> ItemStack {
        let mut item_stack = self.get_frame_item_stack();
        item_stack.set(dc::CUSTOM_NAME, self.entity.custom_name().map(|name| name.to_nbt()));
        item_stack
    }

    fn sound(&self, regular: &str, glow: &str) -> i32 {
        sound_event(if self.glow { glow } else { regular })
    }

    pub fn get_remove_item_sound(&self) -> i32 {
        self.sound("minecraft:entity.item_frame.remove_item", "minecraft:entity.glow_item_frame.remove_item")
    }

    pub fn get_break_sound(&self) -> i32 {
        self.sound("minecraft:entity.item_frame.break", "minecraft:entity.glow_item_frame.break")
    }

    pub fn get_place_sound(&self) -> i32 {
        self.sound("minecraft:entity.item_frame.place", "minecraft:entity.glow_item_frame.place")
    }

    pub fn get_add_item_sound(&self) -> i32 {
        self.sound("minecraft:entity.item_frame.add_item", "minecraft:entity.glow_item_frame.add_item")
    }

    pub fn get_rotate_item_sound(&self) -> i32 {
        self.sound("minecraft:entity.item_frame.rotate_item", "minecraft:entity.glow_item_frame.rotate_item")
    }

    pub fn get_analog_output(&self) -> i32 {
        if get_item(&self.entity).is_none() { 0 } else { get_rotation(&self.entity) % 8 + 1 }
    }
}

fn can_hurt_when_fixed(level: &ServerLevel, source: &DamageSource) -> bool {
    source.is(&damage_type_tags::BYPASSES_INVULNERABILITY) || source.is_creative_player(level)
}

impl ServerLevel {
    fn item_frame(&self, id: EntityId) -> &ItemFrame {
        match self.entities.get(&id) {
            Some(LevelEntity::ItemFrame(frame)) => frame,
            _ => unreachable!("an item frame"),
        }
    }

    fn item_frame_mut(&mut self, id: EntityId) -> &mut ItemFrame {
        match self.entities.get_mut(&id) {
            Some(LevelEntity::ItemFrame(frame)) => frame,
            _ => unreachable!("an item frame"),
        }
    }

    pub fn item_frame_flush_pending_sounds(&mut self, id: EntityId) {
        let Some(LevelEntity::ItemFrame(frame)) = self.entities.get_mut(&id) else { return };
        let Some(position) = frame.pending_add_item_sound.take() else { return };
        let sound = frame.get_add_item_sound();
        if !frame.entity.is_silent() {
            let source = self.sound_source(id);
            self.play_sound_at(None, position.x, position.y, position.z, sound, source, 1.0, 1.0);
        }
    }

    pub fn item_frame_item(&self, id: EntityId) -> Option<ItemStack> {
        match self.entities.get(&id) {
            Some(LevelEntity::ItemFrame(frame)) => get_item(&frame.entity).cloned(),
            _ => None,
        }
    }

    pub fn item_frame_loaded(&mut self, id: EntityId, previous_item: Option<ItemStack>) {
        if let Some(current_item) = previous_item
            && !self.item_frame_item(id).is_some_and(|item| ItemStack::matches(&item, &current_item))
        {
            self.item_frame_remove_framed_map(id, &current_item);
        }
        self.item_frame_flush_pending_sounds(id);
    }

    pub fn item_frame_is_fixed(&self, id: EntityId) -> bool {
        matches!(self.entities.get(&id), Some(LevelEntity::ItemFrame(frame)) if frame.fixed)
    }

    pub fn item_frame_survives(&mut self, id: EntityId) -> bool {
        let frame = self.item_frame(id);
        if frame.fixed {
            return true;
        }
        let view = HangingView::of(&frame.entity, frame.pop_box());
        self.item_frame_view_survives(&view)
    }

    pub fn item_frame_view_survives(&mut self, view: &HangingView) -> bool {
        if self.hanging_entity_has_level_collision(view, &view.pop_box) {
            return false;
        }
        let state = self.block_state(view.pos.relative(view.direction.opposite()));
        if state.is_solid() || view.direction.axis().is_horizontal() && is_diode(state) { self.hanging_entity_can_coexist(view, true) } else { false }
    }

    pub fn item_frame_on_killed(&mut self, id: EntityId) {
        let item = get_item(&self.item_frame(id).entity).cloned();
        if let Some(item) = item {
            self.item_frame_remove_framed_map(id, &item);
        }
    }

    fn should_damage_drop_item(&self, id: EntityId, source: &DamageSource) -> bool {
        !source.is(&damage_type_tags::IS_EXPLOSION) && get_item(&self.item_frame(id).entity).is_some()
    }

    pub fn item_frame_hurt_server(&mut self, id: EntityId, source: DamageSource) -> bool {
        if !self.item_frame(id).fixed {
            if self.is_invulnerable_to_base(id, &source) {
                false
            } else if self.should_damage_drop_item(id, &source) {
                self.item_frame_drop_item_with_frame(id, source.causing_entity, false);
                self.entity_game_event_by(id, crate::world::level::gameevent::game_event::BLOCK_CHANGE, source.causing_entity);
                let sound = self.item_frame(id).get_remove_item_sound();
                self.entity_play_sound(id, sound, 1.0, 1.0);
                true
            } else {
                self.block_attached_entity_hurt_server(id, source)
            }
        } else {
            can_hurt_when_fixed(self, &source) && self.block_attached_entity_hurt_server(id, source)
        }
    }

    pub fn item_frame_drop_item(&mut self, id: EntityId, caused_by: Option<EntityId>) {
        let sound = self.item_frame(id).get_break_sound();
        self.entity_play_sound(id, sound, 1.0, 1.0);
        self.item_frame_drop_item_with_frame(id, caused_by, true);
        self.entity_game_event_by(id, crate::world::level::gameevent::game_event::BLOCK_CHANGE, caused_by);
    }

    fn item_frame_drop_item_with_frame(&mut self, id: EntityId, caused_by: Option<EntityId>, with_frame: bool) {
        if self.item_frame(id).fixed {
            return;
        }
        let item_stack = get_item(&self.item_frame(id).entity).cloned().unwrap_or_else(ItemStack::empty);
        self.item_frame_set_item(id, ItemStack::empty(), true);
        if !self.game_rules.get_bool(game_rule::ENTITY_DROPS) {
            if caused_by.is_none() {
                self.item_frame_remove_framed_map(id, &item_stack);
            }
        } else if caused_by.is_some_and(|player| self.has_infinite_materials(player)) {
            self.item_frame_remove_framed_map(id, &item_stack);
        } else {
            if with_frame {
                let frame_item = self.item_frame(id).get_frame_item_stack_with_data();
                self.hanging_entity_spawn_at_location(id, frame_item, 0.0);
            }
            if !item_stack.is_empty() {
                let item_stack = item_stack.copy();
                self.item_frame_remove_framed_map(id, &item_stack);
                let frame = self.item_frame_mut(id);
                if frame.entity.random.next_float() < frame.drop_chance {
                    self.hanging_entity_spawn_at_location(id, item_stack, 0.0);
                }
            }
        }
    }

    fn item_frame_remove_framed_map(&mut self, id: EntityId, item_stack: &ItemStack) {
        let Some(map_id) = get_framed_map_id(item_stack) else { return };
        if let Some(map_item_saved_data) = map_item::get_saved_data_by_id(Some(map_id), self) {
            let pos = self.entity(id).block_attached_pos();
            map_item_saved_data.borrow_mut().removed_from_frame(pos, id);
        }
    }

    pub fn item_frame_set_item(&mut self, id: EntityId, item_stack: ItemStack, update_neighbours: bool) {
        self.item_frame_mut(id).set_item_data(item_stack);
        self.item_frame_flush_pending_sounds(id);
        if update_neighbours {
            let pos = self.entity(id).block_attached_pos();
            self.update_neighbour_for_output_signal(pos, blocks::AIR);
        }
    }

    fn item_frame_set_rotation(&mut self, id: EntityId, rotation: i32, update_neighbours: bool) {
        self.item_frame_mut(id).entity.data.set(DATA_ROTATION, DataValue::Int(rotation % NUM_ROTATIONS));
        if update_neighbours {
            let pos = self.entity(id).block_attached_pos();
            self.update_neighbour_for_output_signal(pos, blocks::AIR);
        }
    }

    pub fn item_frame_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        let frame_has_item = get_item(&self.item_frame(id).entity).is_some();
        let has_held_item = !item_stack.is_empty();
        if self.item_frame(id).fixed {
            return InteractionResult::Pass;
        }
        if !frame_has_item {
            if has_held_item && !self.entity(id).is_removed() {
                if map_item::get_saved_data(&item_stack, self).is_some_and(|data| data.borrow().is_tracked_count_over_limit(256)) {
                    return InteractionResult::Fail;
                }
                self.item_frame_set_item(id, item_stack.clone(), true);
                self.entity_game_event_by(id, crate::world::level::gameevent::game_event::BLOCK_CHANGE, Some(player));
                item_stack.consume(1, self, Some(player));
                self.player_mut(player).update_item_in_hand(hand, &item_stack);
                InteractionResult::SUCCESS
            } else {
                InteractionResult::Pass
            }
        } else {
            let sound = self.item_frame(id).get_rotate_item_sound();
            self.entity_play_sound(id, sound, 1.0, 1.0);
            let rotation = get_rotation(&self.item_frame(id).entity);
            self.item_frame_set_rotation(id, rotation + 1, true);
            self.entity_game_event_by(id, crate::world::level::gameevent::game_event::BLOCK_CHANGE, Some(player));
            InteractionResult::SUCCESS
        }
    }

    pub fn item_frame_play_placement_sound(&mut self, id: EntityId) {
        let sound = self.item_frame(id).get_place_sound();
        self.entity_play_sound(id, sound, 1.0, 1.0);
    }

    pub fn get_item_frame_for_comparator(&self, direction: Direction, pos: BlockPos) -> Option<EntityId> {
        let area = AABB::new(pos.x as f64, pos.y as f64, pos.z as f64, (pos.x + 1) as f64, (pos.y + 1) as f64, (pos.z + 1) as f64);
        let item_frames: Vec<EntityId> = self
            .get_entities(None, &area)
            .into_iter()
            .filter(|&id| matches!(self.entities.get(&id), Some(LevelEntity::ItemFrame(frame)) if hanging_entity::get_direction(&frame.entity) == direction))
            .collect();
        if item_frames.len() == 1 { Some(item_frames[0]) } else { None }
    }

    pub fn item_frame_tick_map(&mut self, id: EntityId) {
        let frame = self.item_frame(id);
        let Some(item_stack) = get_item(&frame.entity).cloned() else { return };
        if !item_stack.item.is_instance_of("MapItem") {
            return;
        }
        let map_id = get_framed_map_id(&item_stack);
        let Some(data) = map_item::get_saved_data_by_id(map_id, self) else { return };
        let Some(map_id) = map_id else { return };
        let placed_in_frame = Some((frame.entity.block_attached_pos(), hanging_entity::get_direction(&frame.entity), id));
        let players: Vec<EntityId> = self.players.keys().copied().collect();
        for player in players {
            data.borrow_mut().tick_carried_by(self, player, &item_stack, placed_in_frame);
            if let Some(packet) = data.borrow_mut().get_update_packet(map_id, player) {
                self.player(player).connection.send(wafer_data::packets::play::clientbound::MAP_ITEM_DATA, |w| packet.write(w));
            }
        }
    }

    pub fn item_frame_get_analog_output(&self, id: EntityId) -> i32 {
        self.item_frame(id).get_analog_output()
    }
}
