use std::rc::Rc;

use wafer_core::BlockPos;
use wafer_core::random::java_string_hash;
use wafer_data::{EquipmentSlot, item_tags};
use wafer_protocol::Writer;
use wafer_protocol::nbt::Nbt;
use indexmap::IndexMap;

use super::map_banner::MapBanner;
use super::map_decoration::MapDecoration;
use super::map_decoration_type::MapDecorationTypeHolder;
use super::map_decoration_types::{FRAME, PLAYER, PLAYER_OFF_LIMITS, PLAYER_OFF_MAP};
use super::map_frame::MapFrame;
use super::map_id::MapId;
use crate::network::protocol::game::clientbound_map_item_data_packet::ClientboundMapItemDataPacket;
use crate::server::THE_NETHER;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DecodeResult, Record};
use crate::util::java_collections::JavaHashMap;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;

const MAP_SIZE: usize = 128;
pub const MAX_SCALE: i8 = 4;
pub const TRACKED_DECORATION_LIMIT: i32 = 256;
const FRAME_PREFIX: &str = "frame-";

pub struct MapItemSavedData {
    pub center_x: i32,
    pub center_z: i32,
    pub dimension: String,
    tracking_position: bool,
    unlimited_tracking: bool,
    pub scale: i8,
    pub colors: Vec<u8>,
    pub locked: bool,
    carried_by: Vec<HoldingPlayer>,
    banner_markers: JavaHashMap<MapBanner>,
    decorations: IndexMap<String, MapDecoration>,
    frame_markers: JavaHashMap<MapFrame>,
    tracked_decoration_count: i32,
    pub dirty: bool,
}

pub struct HoldingPlayer {
    pub player: EntityId,
    name: Rc<str>,
    removed: bool,
    dirty_data: bool,
    min_dirty_x: i32,
    min_dirty_y: i32,
    max_dirty_x: i32,
    max_dirty_y: i32,
    dirty_decorations: bool,
    tick: i32,
    pub step: i32,
}

struct MapDecorationLocation {
    ty: MapDecorationTypeHolder,
    x: i8,
    y: i8,
    rot: i8,
}

#[derive(Clone, PartialEq, Debug)]
pub struct MapPatch {
    pub start_x: i32,
    pub start_y: i32,
    pub width: i32,
    pub height: i32,
    pub map_colors: Vec<u8>,
}

impl MapPatch {
    pub fn write(patch: Option<&MapPatch>, w: &mut Writer) {
        match patch {
            Some(patch) => {
                w.u8(patch.width as u8);
                w.u8(patch.height as u8);
                w.u8(patch.start_x as u8);
                w.u8(patch.start_y as u8);
                w.varint(patch.map_colors.len() as i32);
                w.bytes(&patch.map_colors);
            }
            None => w.u8(0),
        }
    }
}

impl HoldingPlayer {
    fn new(player: EntityId, name: Rc<str>) -> HoldingPlayer {
        HoldingPlayer {
            player,
            name,
            removed: false,
            dirty_data: true,
            min_dirty_x: 0,
            min_dirty_y: 0,
            max_dirty_x: 127,
            max_dirty_y: 127,
            dirty_decorations: true,
            tick: 0,
            step: 0,
        }
    }

    fn create_patch(&self, colors: &[u8]) -> MapPatch {
        let (start_x, start_y) = (self.min_dirty_x, self.min_dirty_y);
        let width = self.max_dirty_x + 1 - self.min_dirty_x;
        let height = self.max_dirty_y + 1 - self.min_dirty_y;
        let mut patch = vec![0u8; (width * height) as usize];
        for x in 0..width {
            for y in 0..height {
                patch[(x + y * width) as usize] = colors[(start_x + x + (start_y + y) * MAP_SIZE as i32) as usize];
            }
        }
        MapPatch { start_x, start_y, width, height, map_colors: patch }
    }

    fn mark_colors_dirty(&mut self, x: i32, y: i32) {
        if self.dirty_data {
            self.min_dirty_x = self.min_dirty_x.min(x);
            self.min_dirty_y = self.min_dirty_y.min(y);
            self.max_dirty_x = self.max_dirty_x.max(x);
            self.max_dirty_y = self.max_dirty_y.max(y);
        } else {
            self.dirty_data = true;
            self.min_dirty_x = x;
            self.min_dirty_y = y;
            self.max_dirty_x = x;
            self.max_dirty_y = y;
        }
    }

    fn mark_decorations_dirty(&mut self) {
        self.dirty_decorations = true;
    }
}

fn get_frame_key(id: i32) -> String {
    format!("{FRAME_PREFIX}{id}")
}

fn is_inside_map(xd: f32, yd: f32) -> bool {
    xd >= -63.0 && yd >= -63.0 && xd <= 63.0 && yd <= 63.0
}

fn clamp_map_coordinate(delta_from_center: f32) -> i8 {
    if delta_from_center <= -63.0 {
        -128
    } else if delta_from_center >= 63.0 {
        127
    } else {
        ((delta_from_center * 2.0) as f64 + 0.5) as i32 as i8
    }
}

fn inventory_contains(level: &ServerLevel, player: EntityId, map_stack: &ItemStack) -> bool {
    let map_id = map_stack.get(dc::MAP_ID);
    let item = map_stack.get_item();
    level.player(player).inventory.slots[5..].iter().flatten().any(|stack| stack.is(item) && stack.get(dc::MAP_ID) == map_id)
}

fn has_map_invisibility_item_equipped(level: &ServerLevel, player: EntityId) -> bool {
    [EquipmentSlot::Feet, EquipmentSlot::Legs, EquipmentSlot::Chest, EquipmentSlot::Head, EquipmentSlot::Body, EquipmentSlot::Saddle]
        .into_iter()
        .any(|slot| level.item_by_slot_ref(player, slot).is_some_and(|stack| stack.get_item().is(&item_tags::MAP_INVISIBILITY_EQUIPMENT)))
}

impl MapItemSavedData {
    fn new(center_x: i32, center_z: i32, scale: i8, tracking_position: bool, unlimited_tracking: bool, locked: bool, dimension: String) -> MapItemSavedData {
        MapItemSavedData {
            center_x,
            center_z,
            dimension,
            tracking_position,
            unlimited_tracking,
            scale,
            colors: vec![0; MAP_SIZE * MAP_SIZE],
            locked,
            carried_by: Vec::new(),
            banner_markers: JavaHashMap::new(java_string_hash),
            decorations: IndexMap::new(),
            frame_markers: JavaHashMap::new(java_string_hash),
            tracked_decoration_count: 0,
            dirty: false,
        }
    }

    pub fn decode(tag: &Nbt) -> DecodeResult<MapItemSavedData> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let dimension = record.required(tag, "dimension", codec::identifier);
        let center_x = record.required(tag, "xCenter", codec::int);
        let center_z = record.required(tag, "zCenter", codec::int);
        let scale = record.with_default(tag, "scale", 0, |scale| codec::number(scale).map(|scale| crate::world::item::component::codec::as_i32(scale) as i8));
        let colors = record.required(tag, "colors", codec::byte_buffer);
        let tracking_position = record.with_default(tag, "trackingPosition", true, codec::bool_value);
        let unlimited_tracking = record.with_default(tag, "unlimitedTracking", false, codec::bool_value);
        let locked = record.with_default(tag, "locked", false, codec::bool_value);
        let banners = record.with_default(tag, "banners", Vec::new(), |banners| codec::list_partial(banners, MapBanner::decode));
        let frames = record.with_default(tag, "frames", Vec::new(), |frames| codec::list_partial(frames, MapFrame::decode));
        record.build(|| {
            let (dimension, center_x, center_z, colors) = (dimension?, center_x?, center_z?, colors?);
            Some(MapItemSavedData::from_fields(dimension, center_x, center_z, scale, colors, tracking_position, unlimited_tracking, locked, banners, frames))
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn from_fields(
        dimension: String,
        center_x: i32,
        center_z: i32,
        scale: i8,
        colors: Vec<i8>,
        tracking_position: bool,
        unlimited_tracking: bool,
        locked: bool,
        banners: Vec<MapBanner>,
        frames: Vec<MapFrame>,
    ) -> MapItemSavedData {
        let mut data = MapItemSavedData::new(center_x, center_z, scale.clamp(0, MAX_SCALE), tracking_position, unlimited_tracking, locked, dimension);
        if colors.len() == MAP_SIZE * MAP_SIZE {
            data.colors = colors.iter().map(|&c| c as u8).collect();
        }
        for banner in banners {
            let id = banner.get_id();
            data.banner_markers.insert(id.clone(), banner.clone());
            data.add_decoration(banner.get_decoration(), None, &id, banner.pos.x as f64, banner.pos.z as f64, 180.0, banner.name.clone());
        }
        for frame in frames {
            data.frame_markers.insert(frame.get_id(), frame.clone());
            data.add_decoration(FRAME, None, &get_frame_key(frame.entity_id), frame.pos.x as f64, frame.pos.z as f64, frame.rotation as f64, None);
        }
        data
    }

    pub fn encode(&self) -> Nbt {
        let mut fields = vec![
            ("dimension".to_string(), Nbt::String(self.dimension.clone())),
            ("xCenter".to_string(), Nbt::Int(self.center_x)),
            ("zCenter".to_string(), Nbt::Int(self.center_z)),
        ];
        if self.scale != 0 {
            fields.push(("scale".into(), Nbt::Byte(self.scale)));
        }
        fields.push(("colors".into(), Nbt::ByteArray(self.colors.iter().map(|&c| c as i8).collect())));
        if !self.tracking_position {
            fields.push(("trackingPosition".into(), Nbt::Byte(0)));
        }
        if self.unlimited_tracking {
            fields.push(("unlimitedTracking".into(), Nbt::Byte(1)));
        }
        if self.locked {
            fields.push(("locked".into(), Nbt::Byte(1)));
        }
        if self.banner_markers.len() > 0 {
            fields.push(("banners".into(), Nbt::List(self.banner_markers.iter().map(|(_, banner)| banner.encode()).collect())));
        }
        if self.frame_markers.len() > 0 {
            fields.push(("frames".into(), Nbt::List(self.frame_markers.iter().map(|(_, frame)| frame.encode()).collect())));
        }
        Nbt::Compound(fields)
    }

    pub fn set_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn create_fresh(origin_x: f64, origin_y: f64, scale: i8, tracking_position: bool, unlimited_tracking: bool, dimension: &str) -> MapItemSavedData {
        let size = 128 * (1 << scale);
        let area_x = wafer_core::mth::floor((origin_x + 64.0) / size as f64);
        let area_z = wafer_core::mth::floor((origin_y + 64.0) / size as f64);
        let x = area_x * size + size / 2 - 64;
        let z = area_z * size + size / 2 - 64;
        MapItemSavedData::new(x, z, scale, tracking_position, unlimited_tracking, false, dimension.to_string())
    }

    pub fn locked(&self) -> MapItemSavedData {
        let mut result = MapItemSavedData::new(self.center_x, self.center_z, self.scale, self.tracking_position, self.unlimited_tracking, true, self.dimension.clone());
        for (id, banner) in self.banner_markers.iter() {
            result.banner_markers.insert(id.to_string(), banner.clone());
        }
        result.decorations.extend(self.decorations.iter().map(|(key, decoration)| (key.clone(), decoration.clone())));
        result.tracked_decoration_count = self.tracked_decoration_count;
        result.colors.copy_from_slice(&self.colors);
        result
    }

    pub fn scaled(&self) -> MapItemSavedData {
        MapItemSavedData::create_fresh(
            self.center_x as f64,
            self.center_z as f64,
            (self.scale + 1).clamp(0, MAX_SCALE),
            self.tracking_position,
            self.unlimited_tracking,
            &self.dimension,
        )
    }

    #[cfg(test)]
    pub fn is_carried_by(&self, player: EntityId) -> bool {
        self.holding_player_index(player).is_some()
    }

    fn holding_player_index(&self, player: EntityId) -> Option<usize> {
        self.carried_by.iter().position(|holding_player| holding_player.player == player && !holding_player.removed)
    }

    pub fn tick_carried_by(&mut self, level: &ServerLevel, ticking_player: EntityId, item_stack: &ItemStack, placed_in_frame: Option<(BlockPos, wafer_core::Direction, EntityId)>) {
        if self.holding_player_index(ticking_player).is_none() {
            self.carried_by.push(HoldingPlayer::new(ticking_player, level.player(ticking_player).profile.name.as_str().into()));
        }
        if !inventory_contains(level, ticking_player, item_stack) {
            self.remove_decoration(&level.player(ticking_player).profile.name);
        }
        let mut i = 0;
        while i < self.carried_by.len() {
            let other_holding_player = &self.carried_by[i];
            let other_player = other_holding_player.player;
            let other_player_name = other_holding_player.name.clone();
            if other_holding_player.removed {
                self.carried_by.remove(i);
                self.remove_decoration(&other_player_name);
            } else if let Some(other_level) = level.level_with_player(other_player) {
                let other = other_level.player(other_player);
                if !other.entity.is_removed() && (placed_in_frame.is_some() || inventory_contains(other_level, other_player, item_stack)) {
                    if placed_in_frame.is_none() && other_level.dimension == self.dimension && self.tracking_position {
                        let (x, z, y_rot) = (other.entity.x(), other.entity.z(), other.entity.y_rot);
                        self.add_decoration(PLAYER, Some(other_level), &other_player_name, x, z, y_rot as f64, None);
                    }
                } else {
                    self.carried_by.remove(i);
                    self.remove_decoration(&other_player_name);
                }
                if other_player != ticking_player && has_map_invisibility_item_equipped(other_level, other_player) {
                    self.remove_decoration(&other_player_name);
                }
            }
            i += 1;
        }
        if let Some((pos, direction, frame_id)) = placed_in_frame
            && self.tracking_position
        {
            let existing_frame = self.frame_markers.get(&super::map_frame::frame_id(pos)).cloned();
            if let Some(existing_frame) = existing_frame
                && frame_id != existing_frame.entity_id
                && self.frame_markers.contains_key(&existing_frame.get_id())
            {
                self.remove_decoration(&get_frame_key(existing_frame.entity_id));
            }
            let rotation = direction.get_2d_data_value() * 90;
            let map_frame = MapFrame { pos, rotation, entity_id: frame_id };
            self.add_decoration(FRAME, Some(level), &get_frame_key(frame_id), pos.x as f64, pos.z as f64, rotation as f64, None);
            let old_frame = self.frame_markers.insert(map_frame.get_id(), map_frame.clone());
            if old_frame.as_ref() != Some(&map_frame) {
                self.set_dirty();
            }
        }
        if let Some(Nbt::Compound(static_decorations)) = item_stack.get(dc::MAP_DECORATIONS)
            && !static_decorations.iter().all(|(id, _)| self.decorations.contains_key(id))
        {
            for (id, entry) in static_decorations {
                if self.decorations.contains_key(id) {
                    continue;
                }
                let Some(ty) = entry.get("type").and_then(Nbt::as_str).and_then(MapDecorationTypeHolder::by_name) else { continue };
                let x = entry.get("x").and_then(Nbt::as_f64).unwrap_or(0.0);
                let z = entry.get("z").and_then(Nbt::as_f64).unwrap_or(0.0);
                let rotation = entry.get("rotation").and_then(Nbt::as_f64).unwrap_or(0.0) as f32;
                self.add_decoration(ty, Some(level), id, x, z, rotation as f64, None);
            }
        }
    }

    pub fn player_removed(&mut self, player: EntityId) {
        for holding_player in &mut self.carried_by {
            if holding_player.player == player {
                holding_player.removed = true;
            }
        }
    }

    fn remove_decoration(&mut self, key: &str) {
        if let Some(decoration) = self.decorations.shift_remove(key)
            && decoration.ty.value().track_count
        {
            self.tracked_decoration_count -= 1;
        }
        self.set_decorations_dirty();
    }

    pub fn add_target_decoration(item_stack: &mut ItemStack, position: BlockPos, key: &str, decoration_type: MapDecorationTypeHolder) {
        let new_decoration = Nbt::Compound(vec![
            ("type".into(), Nbt::String(decoration_type.key().into())),
            ("x".into(), Nbt::Double(position.x as f64)),
            ("z".into(), Nbt::Double(position.z as f64)),
            ("rotation".into(), Nbt::Float(180.0)),
        ]);
        let mut decorations = match item_stack.get(dc::MAP_DECORATIONS) {
            Some(Nbt::Compound(decorations)) => decorations.clone(),
            _ => Vec::new(),
        };
        match decorations.iter_mut().find(|(id, _)| id == key) {
            Some((_, entry)) => *entry = new_decoration,
            None => decorations.push((key.to_string(), new_decoration)),
        }
        item_stack.set(dc::MAP_DECORATIONS, Some(Nbt::Compound(decorations)));
    }

    #[allow(clippy::too_many_arguments)]
    fn add_decoration(&mut self, ty: MapDecorationTypeHolder, level: Option<&ServerLevel>, key: &str, x_pos: f64, z_pos: f64, y_rot: f64, name: Option<Nbt>) {
        let scaling = 1 << self.scale;
        let x_delta_from_center = (x_pos - self.center_x as f64) as f32 / scaling as f32;
        let y_delta_from_center = (z_pos - self.center_z as f64) as f32 / scaling as f32;
        let Some(location_and_type) = self.calculate_decoration_location_and_type(ty, level, y_rot, x_delta_from_center, y_delta_from_center) else {
            self.remove_decoration(key);
            return;
        };
        let new_decoration = MapDecoration::new(location_and_type.ty, location_and_type.x, location_and_type.y, location_and_type.rot, name);
        let previous_decoration = match self.decorations.get_mut(key) {
            Some(existing) if *existing == new_decoration => return,
            Some(existing) => Some(std::mem::replace(existing, new_decoration)),
            None => {
                self.decorations.insert(key.to_string(), new_decoration);
                None
            }
        };
        if previous_decoration.is_some_and(|previous| previous.ty.value().track_count) {
            self.tracked_decoration_count -= 1;
        }
        if location_and_type.ty.value().track_count {
            self.tracked_decoration_count += 1;
        }
        self.set_decorations_dirty();
    }

    fn calculate_decoration_location_and_type(
        &self,
        ty: MapDecorationTypeHolder,
        level: Option<&ServerLevel>,
        y_rot: f64,
        x_delta_from_center: f32,
        y_delta_from_center: f32,
    ) -> Option<MapDecorationLocation> {
        let x = clamp_map_coordinate(x_delta_from_center);
        let y = clamp_map_coordinate(y_delta_from_center);
        if ty.is(PLAYER) {
            let (ty, rot) = self.player_decoration_type_and_rotation(ty, level, y_rot, x_delta_from_center, y_delta_from_center)?;
            Some(MapDecorationLocation { ty, x, y, rot })
        } else if !is_inside_map(x_delta_from_center, y_delta_from_center) && !self.unlimited_tracking {
            None
        } else {
            Some(MapDecorationLocation { ty, x, y, rot: self.calculate_rotation(level, y_rot) })
        }
    }

    fn player_decoration_type_and_rotation(
        &self,
        ty: MapDecorationTypeHolder,
        level: Option<&ServerLevel>,
        y_rot: f64,
        x_delta_from_center: f32,
        y_delta_from_center: f32,
    ) -> Option<(MapDecorationTypeHolder, i8)> {
        if is_inside_map(x_delta_from_center, y_delta_from_center) {
            return Some((ty, self.calculate_rotation(level, y_rot)));
        }
        let outside_map_decoration_type = self.decoration_type_for_player_outside_map(x_delta_from_center, y_delta_from_center)?;
        Some((outside_map_decoration_type, self.calculate_rotation(level, y_rot)))
    }

    fn calculate_rotation(&self, level: Option<&ServerLevel>, y_rot: f64) -> i8 {
        match level {
            Some(level) if self.dimension == THE_NETHER => {
                let s = (level.game_time / 10) as i32;
                (s.wrapping_mul(s).wrapping_mul(34187121).wrapping_add(s.wrapping_mul(121)) >> 15 & 15) as i8
            }
            _ => {
                let adjusted_y_rot = if y_rot < 0.0 { y_rot - 8.0 } else { y_rot + 8.0 };
                (adjusted_y_rot * 16.0 / 360.0) as i32 as i8
            }
        }
    }

    fn decoration_type_for_player_outside_map(&self, x_delta_from_center: f32, y_delta_from_center: f32) -> Option<MapDecorationTypeHolder> {
        let is_within_limits = x_delta_from_center.abs() < 320.0 && y_delta_from_center.abs() < 320.0;
        if is_within_limits {
            Some(PLAYER_OFF_MAP)
        } else if self.unlimited_tracking {
            Some(PLAYER_OFF_LIMITS)
        } else {
            None
        }
    }

    pub fn get_update_packet(&mut self, id: MapId, player: EntityId) -> Option<ClientboundMapItemDataPacket> {
        let index = self.holding_player_index(player)?;
        self.next_update_packet(index, id)
    }

    fn next_update_packet(&mut self, index: usize, id: MapId) -> Option<ClientboundMapItemDataPacket> {
        let holding_player = &mut self.carried_by[index];
        let patch = if holding_player.dirty_data {
            holding_player.dirty_data = false;
            Some(holding_player.create_patch(&self.colors))
        } else {
            None
        };
        let decorations = if holding_player.dirty_decorations && {
            let tick = holding_player.tick;
            holding_player.tick = tick.wrapping_add(1);
            tick % 5 == 0
        } {
            holding_player.dirty_decorations = false;
            Some(self.decorations.values().cloned().collect())
        } else {
            None
        };
        if decorations.is_none() && patch.is_none() {
            return None;
        }
        Some(ClientboundMapItemDataPacket { map_id: id, scale: self.scale, locked: self.locked, decorations, color_patch: patch })
    }

    fn set_colors_dirty(&mut self, x: i32, y: i32) {
        self.set_dirty();
        for holding_player in &mut self.carried_by {
            holding_player.mark_colors_dirty(x, y);
        }
    }

    fn set_decorations_dirty(&mut self) {
        self.carried_by.iter_mut().for_each(HoldingPlayer::mark_decorations_dirty);
    }

    pub fn get_holding_player(&mut self, level: &ServerLevel, player: EntityId) -> &mut HoldingPlayer {
        let index = match self.holding_player_index(player) {
            Some(index) => index,
            None => {
                self.carried_by.push(HoldingPlayer::new(player, level.player(player).profile.name.as_str().into()));
                self.carried_by.len() - 1
            }
        };
        &mut self.carried_by[index]
    }

    pub fn toggle_banner(&mut self, level: &ServerLevel, pos: BlockPos) -> bool {
        let x_pos = pos.x as f64 + 0.5;
        let z_pos = pos.z as f64 + 0.5;
        let scale = 1 << self.scale;
        let xd = (x_pos - self.center_x as f64) / scale as f64;
        let yd = (z_pos - self.center_z as f64) / scale as f64;
        if xd >= -63.0 && yd >= -63.0 && xd <= 63.0 && yd <= 63.0 {
            let Some(banner) = MapBanner::from_world(level, pos) else { return false };
            let id = banner.get_id();
            if self.banner_markers.get(&id) == Some(&banner) {
                self.banner_markers.remove(&id);
                self.remove_decoration(&id);
                self.set_dirty();
                return true;
            }
            if !self.is_tracked_count_over_limit(TRACKED_DECORATION_LIMIT) {
                self.banner_markers.insert(id.clone(), banner.clone());
                self.add_decoration(banner.get_decoration(), Some(level), &id, x_pos, z_pos, 180.0, banner.name.clone());
                self.set_dirty();
                return true;
            }
        }
        false
    }

    pub fn check_banners(&mut self, level: &ServerLevel, x: i32, z: i32) {
        if self.banner_markers.len() == 0 {
            return;
        }
        let candidates: Vec<(String, MapBanner)> =
            self.banner_markers.iter().filter(|(_, expected)| expected.pos.x == x && expected.pos.z == z).map(|(id, expected)| (id.to_string(), expected.clone())).collect();
        for (id, expected) in candidates {
            let current = MapBanner::from_world(level, expected.pos);
            if current.as_ref() != Some(&expected) {
                self.banner_markers.remove(&id);
                self.remove_decoration(&expected.get_id());
                self.set_dirty();
            }
        }
    }

    pub fn update_color(&mut self, x: i32, y: i32, new_color: u8) -> bool {
        let old_color = self.colors[(x + y * MAP_SIZE as i32) as usize];
        if old_color != new_color {
            self.set_color(x, y, new_color);
            true
        } else {
            false
        }
    }

    pub fn set_color(&mut self, x: i32, y: i32, new_color: u8) {
        self.colors[(x + y * MAP_SIZE as i32) as usize] = new_color;
        self.set_colors_dirty(x, y);
    }

    pub fn removed_from_frame(&mut self, pos: BlockPos, entity_id: EntityId) {
        self.remove_decoration(&get_frame_key(entity_id));
        self.frame_markers.remove(&super::map_frame::frame_id(pos));
        self.set_dirty();
    }

    pub fn is_tracked_count_over_limit(&self, limit: i32) -> bool {
        self.tracked_decoration_count > limit
    }

    pub fn decorations(&self) -> impl Iterator<Item = (&str, &MapDecoration)> {
        self.decorations.iter().map(|(key, decoration)| (key.as_str(), decoration))
    }
}
