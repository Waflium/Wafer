use wafer_core::joml::{Quaternionf, Vector3f};
use wafer_core::math::Transformation;
use wafer_data::{BlockState, blocks, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::data::{DataValue, entity_defaults};
use super::{Entity, EntityDimensions, EntityId, LevelEntity};
use crate::commands::arguments::selector::entity_world::LevelWorld;
use crate::commands::brigadier::CommandSyntaxException;
use crate::commands::source::{Anchor, CommandResultCallback, CommandSource, CommandSourceStack, Names};
use crate::network::chat::{Component, component_utils};
use crate::server::Server;
use crate::server::level::ServerLevel;
use crate::util::FastIndexMap;
use crate::util::brightness::Brightness;
use crate::util::codec::{self, DataResult};
use crate::util::extra_codecs;
use crate::world::item::ItemStack;
use crate::world::item::item_display_context::{self, ItemDisplayContext};
use crate::world::level::block::entity::{byte_or, float_or, int_or};
use crate::world::level::chunk::serialization::{state_from_nbt, state_to_nbt};

const DATA_TRANSFORMATION_INTERPOLATION_START_DELTA_TICKS_ID: usize = 8;
const DATA_TRANSFORMATION_INTERPOLATION_DURATION_ID: usize = 9;
const DATA_POS_ROT_INTERPOLATION_DURATION_ID: usize = 10;
const DATA_TRANSLATION_ID: usize = 11;
const DATA_SCALE_ID: usize = 12;
const DATA_LEFT_ROTATION_ID: usize = 13;
const DATA_RIGHT_ROTATION_ID: usize = 14;
const DATA_BILLBOARD_RENDER_CONSTRAINTS_ID: usize = 15;
const DATA_BRIGHTNESS_OVERRIDE_ID: usize = 16;
const DATA_VIEW_RANGE_ID: usize = 17;
const DATA_SHADOW_RADIUS_ID: usize = 18;
const DATA_SHADOW_STRENGTH_ID: usize = 19;
const DATA_WIDTH_ID: usize = 20;
const DATA_HEIGHT_ID: usize = 21;
const DATA_GLOW_COLOR_OVERRIDE_ID: usize = 22;
const DATA_BLOCK_STATE_ID: usize = 23;
const DATA_ITEM_STACK_ID: usize = 23;
const DATA_ITEM_DISPLAY_ID: usize = 24;
const DATA_TEXT_ID: usize = 23;
const DATA_LINE_WIDTH_ID: usize = 24;
const DATA_BACKGROUND_COLOR_ID: usize = 25;
const DATA_TEXT_OPACITY_ID: usize = 26;
const DATA_STYLE_FLAGS_ID: usize = 27;
const INITIAL_SHADOW_RADIUS: f32 = 0.0;
const INITIAL_SHADOW_STRENGTH: f32 = 1.0;
const INITIAL_VIEW_RANGE: f32 = 1.0;
const INITIAL_WIDTH: f32 = 0.0;
const INITIAL_HEIGHT: f32 = 0.0;
const NO_GLOW_COLOR_OVERRIDE: i32 = -1;
const FLAG_SHADOW: i8 = 1;
const FLAG_SEE_THROUGH: i8 = 2;
const FLAG_USE_DEFAULT_BACKGROUND: i8 = 4;
const FLAG_ALIGN_LEFT: i8 = 8;
const FLAG_ALIGN_RIGHT: i8 = 16;
const INITIAL_TEXT_OPACITY: i8 = -1;
const INITIAL_BACKGROUND: i32 = 1073741824;
const INITIAL_LINE_WIDTH: i32 = 200;

pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.0, 0.0);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BillboardConstraints {
    Fixed,
    Vertical,
    Horizontal,
    Center,
}

const BILLBOARD_CONSTRAINTS: [(&str, BillboardConstraints); 4] = [
    ("fixed", BillboardConstraints::Fixed),
    ("vertical", BillboardConstraints::Vertical),
    ("horizontal", BillboardConstraints::Horizontal),
    ("center", BillboardConstraints::Center),
];

impl BillboardConstraints {
    fn get_id(self) -> i8 {
        self as i8
    }

    fn by_id(id: i8) -> BillboardConstraints {
        BILLBOARD_CONSTRAINTS.get(id as usize).map_or(BillboardConstraints::Fixed, |(_, constraints)| *constraints)
    }

    fn get_serialized_name(self) -> &'static str {
        BILLBOARD_CONSTRAINTS[self as usize].0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Align {
    Center,
    Left,
    Right,
}

const ALIGNS: [(&str, Align); 3] = [("center", Align::Center), ("left", Align::Left), ("right", Align::Right)];

pub enum DisplayKind {
    BlockDisplay,
    ItemDisplay,
    TextDisplay { pending_text: Option<Component> },
}

pub struct Display {
    pub entity: Entity,
    pub kind: DisplayKind,
}

fn display_defaults() -> Vec<DataValue> {
    let mut data = entity_defaults();
    data.extend([
        DataValue::Int(0),
        DataValue::Int(0),
        DataValue::Int(0),
        DataValue::Vector3(Vector3f::new(0.0, 0.0, 0.0)),
        DataValue::Vector3(Vector3f::new(1.0, 1.0, 1.0)),
        DataValue::Quaternion(Quaternionf::IDENTITY),
        DataValue::Quaternion(Quaternionf::IDENTITY),
        DataValue::Byte(BillboardConstraints::Fixed.get_id()),
        DataValue::Int(-1),
        DataValue::Float(INITIAL_VIEW_RANGE),
        DataValue::Float(INITIAL_SHADOW_RADIUS),
        DataValue::Float(INITIAL_SHADOW_STRENGTH),
        DataValue::Float(INITIAL_WIDTH),
        DataValue::Float(INITIAL_HEIGHT),
        DataValue::Int(NO_GLOW_COLOR_OVERRIDE),
    ]);
    data
}

fn decode_transformation(tag: &Nbt) -> DataResult<Transformation> {
    let decomposed = |tag: &Nbt| -> DataResult<Transformation> {
        Ok(Transformation::new(
            codec::decode_field(tag, "translation", extra_codecs::vector3f)?,
            codec::decode_field(tag, "left_rotation", extra_codecs::quaternionf)?,
            codec::decode_field(tag, "scale", extra_codecs::vector3f)?,
            codec::decode_field(tag, "right_rotation", extra_codecs::quaternionf)?,
        ))
    };
    Ok(match codec::either(tag, decomposed, extra_codecs::matrix4f)? {
        Ok(transformation) => transformation,
        Err(matrix) => Transformation::from_matrix(&matrix),
    })
}

fn encode_transformation(transformation: &Transformation) -> Nbt {
    Nbt::Compound(vec![
        ("translation".into(), extra_codecs::vector3f_to_nbt(transformation.translation)),
        ("left_rotation".into(), extra_codecs::quaternionf_to_nbt(transformation.left_rotation)),
        ("scale".into(), extra_codecs::vector3f_to_nbt(transformation.scale)),
        ("right_rotation".into(), extra_codecs::quaternionf_to_nbt(transformation.right_rotation)),
    ])
}

impl Display {
    fn create(entity_type: &str, kind: DisplayKind, extra: Vec<DataValue>) -> Display {
        let mut data = display_defaults();
        data.extend(extra);
        let mut entity = Entity::new(registry_id(registry::ENTITY_TYPE, entity_type), DIMENSIONS, data);
        entity.no_physics = true;
        Display { entity, kind }
    }

    pub fn create_block_display() -> Display {
        Display::create("minecraft:block_display", DisplayKind::BlockDisplay, vec![DataValue::BlockState(blocks::AIR.default_state())])
    }

    pub fn create_item_display() -> Display {
        Display::create("minecraft:item_display", DisplayKind::ItemDisplay, vec![DataValue::ItemStack(None), DataValue::Byte(ItemDisplayContext::None.get_id())])
    }

    pub fn create_text_display() -> Display {
        Display::create(
            "minecraft:text_display",
            DisplayKind::TextDisplay { pending_text: None },
            vec![
                DataValue::Component(Component::empty().to_nbt()),
                DataValue::Int(INITIAL_LINE_WIDTH),
                DataValue::Int(INITIAL_BACKGROUND),
                DataValue::Byte(INITIAL_TEXT_OPACITY),
                DataValue::Byte(0),
            ],
        )
    }

    fn int(&self, id: usize) -> i32 {
        match self.entity.data.get(id) {
            DataValue::Int(v) => *v,
            _ => 0,
        }
    }

    fn float(&self, id: usize) -> f32 {
        match self.entity.data.get(id) {
            DataValue::Float(v) => *v,
            _ => 0.0,
        }
    }

    fn byte(&self, id: usize) -> i8 {
        match self.entity.data.get(id) {
            DataValue::Byte(v) => *v,
            _ => 0,
        }
    }

    fn vector(&self, id: usize) -> Vector3f {
        match self.entity.data.get(id) {
            DataValue::Vector3(v) => *v,
            _ => Vector3f::default(),
        }
    }

    fn quaternion(&self, id: usize) -> Quaternionf {
        match self.entity.data.get(id) {
            DataValue::Quaternion(q) => *q,
            _ => Quaternionf::IDENTITY,
        }
    }

    fn create_transformation(&self) -> Transformation {
        Transformation::new(self.vector(DATA_TRANSLATION_ID), self.quaternion(DATA_LEFT_ROTATION_ID), self.vector(DATA_SCALE_ID), self.quaternion(DATA_RIGHT_ROTATION_ID))
    }

    fn set_transformation(&mut self, transformation: Transformation) {
        self.entity.data.set(DATA_TRANSLATION_ID, DataValue::Vector3(transformation.translation));
        self.entity.data.set(DATA_LEFT_ROTATION_ID, DataValue::Quaternion(transformation.left_rotation));
        self.entity.data.set(DATA_SCALE_ID, DataValue::Vector3(transformation.scale));
        self.entity.data.set(DATA_RIGHT_ROTATION_ID, DataValue::Quaternion(transformation.right_rotation));
    }

    fn get_billboard_constraints(&self) -> BillboardConstraints {
        BillboardConstraints::by_id(self.byte(DATA_BILLBOARD_RENDER_CONSTRAINTS_ID))
    }

    fn get_brightness_override(&self) -> Option<Brightness> {
        let value = self.int(DATA_BRIGHTNESS_OVERRIDE_ID);
        (value != -1).then(|| Brightness::unpack(value))
    }

    pub fn read_additional_save_data(&mut self, input: &Nbt) {
        let transformation = input.get("transformation").and_then(|tag| decode_transformation(tag).ok()).unwrap_or(Transformation::IDENTITY);
        self.set_transformation(transformation);
        self.entity.data.set(DATA_TRANSFORMATION_INTERPOLATION_DURATION_ID, DataValue::Int(int_or(input, "interpolation_duration", 0)));
        self.entity.data.set_forced(DATA_TRANSFORMATION_INTERPOLATION_START_DELTA_TICKS_ID, DataValue::Int(int_or(input, "start_interpolation", 0)));
        let teleport_duration = int_or(input, "teleport_duration", 0);
        self.entity.data.set(DATA_POS_ROT_INTERPOLATION_DURATION_ID, DataValue::Int(teleport_duration.clamp(0, 59)));
        let billboard = input.get("billboard").and_then(|tag| codec::string_enum(tag, &BILLBOARD_CONSTRAINTS).ok()).unwrap_or(BillboardConstraints::Fixed);
        self.entity.data.set(DATA_BILLBOARD_RENDER_CONSTRAINTS_ID, DataValue::Byte(billboard.get_id()));
        self.entity.data.set(DATA_VIEW_RANGE_ID, DataValue::Float(float_or(input, "view_range", INITIAL_VIEW_RANGE)));
        self.entity.data.set(DATA_SHADOW_RADIUS_ID, DataValue::Float(float_or(input, "shadow_radius", INITIAL_SHADOW_RADIUS)));
        self.entity.data.set(DATA_SHADOW_STRENGTH_ID, DataValue::Float(float_or(input, "shadow_strength", INITIAL_SHADOW_STRENGTH)));
        self.entity.data.set(DATA_WIDTH_ID, DataValue::Float(float_or(input, "width", INITIAL_WIDTH)));
        self.entity.data.set(DATA_HEIGHT_ID, DataValue::Float(float_or(input, "height", INITIAL_HEIGHT)));
        self.entity.data.set(DATA_GLOW_COLOR_OVERRIDE_ID, DataValue::Int(int_or(input, "glow_color_override", NO_GLOW_COLOR_OVERRIDE)));
        let brightness = input.get("brightness").and_then(|tag| Brightness::decode(tag).ok());
        self.entity.data.set(DATA_BRIGHTNESS_OVERRIDE_ID, DataValue::Int(brightness.map_or(-1, Brightness::pack)));
        match self.kind {
            DisplayKind::BlockDisplay => {
                let block_state = input.get("block_state").and_then(state_from_nbt).unwrap_or_else(|| blocks::AIR.default_state());
                self.entity.data.set(DATA_BLOCK_STATE_ID, DataValue::BlockState(block_state));
            }
            DisplayKind::ItemDisplay => {
                let item = input.get("item").and_then(|tag| ItemStack::from_nbt(tag).ok()).filter(|item| !item.is_empty());
                self.entity.data.set(DATA_ITEM_STACK_ID, DataValue::ItemStack(item));
                let item_display = input.get("item_display").and_then(|tag| codec::string_enum(tag, &item_display_context::VALUES).ok()).unwrap_or(ItemDisplayContext::None);
                self.entity.data.set(DATA_ITEM_DISPLAY_ID, DataValue::Byte(item_display.get_id()));
            }
            DisplayKind::TextDisplay { .. } => self.text_display_read_additional_save_data(input),
        }
    }

    fn text_display_read_additional_save_data(&mut self, input: &Nbt) {
        self.entity.data.set(DATA_LINE_WIDTH_ID, DataValue::Int(int_or(input, "line_width", INITIAL_LINE_WIDTH)));
        self.entity.data.set(DATA_TEXT_OPACITY_ID, DataValue::Byte(byte_or(input, "text_opacity", INITIAL_TEXT_OPACITY)));
        self.entity.data.set(DATA_BACKGROUND_COLOR_ID, DataValue::Int(int_or(input, "background", INITIAL_BACKGROUND)));
        let load_flag = |flags: i8, id: &str, mask: i8| if byte_or(input, id, 0) != 0 { flags | mask } else { flags };
        let mut flags = load_flag(0, "shadow", FLAG_SHADOW);
        flags = load_flag(flags, "see_through", FLAG_SEE_THROUGH);
        flags = load_flag(flags, "default_background", FLAG_USE_DEFAULT_BACKGROUND);
        if let Some(alignment) = input.get("alignment").and_then(|tag| codec::string_enum(tag, &ALIGNS).ok()) {
            flags = match alignment {
                Align::Center => flags,
                Align::Left => flags | FLAG_ALIGN_LEFT,
                Align::Right => flags | FLAG_ALIGN_RIGHT,
            };
        }
        self.entity.data.set(DATA_STYLE_FLAGS_ID, DataValue::Byte(flags));
        if let Some(text) = input.get("text") {
            let text = Component::from_nbt(text);
            if component_utils::needs_resolution(&text) {
                if let DisplayKind::TextDisplay { pending_text } = &mut self.kind {
                    *pending_text = Some(text);
                }
            } else {
                self.entity.data.set(DATA_TEXT_ID, DataValue::Component(text.to_nbt()));
            }
        }
    }

    pub fn add_additional_save_data(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("transformation".into(), encode_transformation(&self.create_transformation())));
        output.push(("billboard".into(), Nbt::String(self.get_billboard_constraints().get_serialized_name().into())));
        output.push(("interpolation_duration".into(), Nbt::Int(self.int(DATA_TRANSFORMATION_INTERPOLATION_DURATION_ID))));
        output.push(("teleport_duration".into(), Nbt::Int(self.int(DATA_POS_ROT_INTERPOLATION_DURATION_ID))));
        output.push(("view_range".into(), Nbt::Float(self.float(DATA_VIEW_RANGE_ID))));
        output.push(("shadow_radius".into(), Nbt::Float(self.float(DATA_SHADOW_RADIUS_ID))));
        output.push(("shadow_strength".into(), Nbt::Float(self.float(DATA_SHADOW_STRENGTH_ID))));
        output.push(("width".into(), Nbt::Float(self.float(DATA_WIDTH_ID))));
        output.push(("height".into(), Nbt::Float(self.float(DATA_HEIGHT_ID))));
        output.push(("glow_color_override".into(), Nbt::Int(self.int(DATA_GLOW_COLOR_OVERRIDE_ID))));
        if let Some(brightness) = self.get_brightness_override() {
            output.push(("brightness".into(), brightness.encode()));
        }
        match self.kind {
            DisplayKind::BlockDisplay => output.push(("block_state".into(), state_to_nbt(self.get_block_state()))),
            DisplayKind::ItemDisplay => {
                if let Some(item) = self.get_item_stack() {
                    output.push(("item".into(), item.to_nbt()));
                }
                output.push(("item_display".into(), Nbt::String(ItemDisplayContext::by_id(self.byte(DATA_ITEM_DISPLAY_ID)).get_serialized_name().into())));
            }
            DisplayKind::TextDisplay { .. } => {
                let text = match (&self.kind, self.entity.data.get(DATA_TEXT_ID)) {
                    (DisplayKind::TextDisplay { pending_text: Some(pending) }, _) => pending.to_nbt(),
                    (_, DataValue::Component(text)) => text.clone(),
                    _ => Component::empty().to_nbt(),
                };
                output.push(("text".into(), text));
                output.push(("line_width".into(), Nbt::Int(self.int(DATA_LINE_WIDTH_ID))));
                output.push(("background".into(), Nbt::Int(self.int(DATA_BACKGROUND_COLOR_ID))));
                output.push(("text_opacity".into(), Nbt::Byte(self.byte(DATA_TEXT_OPACITY_ID))));
                let flags = self.byte(DATA_STYLE_FLAGS_ID);
                output.push(("shadow".into(), Nbt::Byte(((flags & FLAG_SHADOW) != 0) as i8)));
                output.push(("see_through".into(), Nbt::Byte(((flags & FLAG_SEE_THROUGH) != 0) as i8)));
                output.push(("default_background".into(), Nbt::Byte(((flags & FLAG_USE_DEFAULT_BACKGROUND) != 0) as i8)));
                output.push(("alignment".into(), Nbt::String(ALIGNS[get_align(flags) as usize].0.into())));
            }
        }
    }

    fn get_block_state(&self) -> BlockState {
        match self.entity.data.get(DATA_BLOCK_STATE_ID) {
            DataValue::BlockState(state) => *state,
            _ => blocks::AIR.default_state(),
        }
    }

    pub fn get_item_stack(&self) -> Option<&ItemStack> {
        match self.entity.data.get(DATA_ITEM_STACK_ID) {
            DataValue::ItemStack(item) => item.as_ref(),
            _ => None,
        }
    }

    pub fn set_item_stack(&mut self, item: ItemStack) {
        self.entity.data.set(DATA_ITEM_STACK_ID, DataValue::ItemStack((!item.is_empty()).then_some(item)));
    }

    pub fn item_stack_mut(&mut self) -> &mut DataValue {
        self.entity.data.get_mut(DATA_ITEM_STACK_ID)
    }

    pub fn is_item_display(&self) -> bool {
        matches!(self.kind, DisplayKind::ItemDisplay)
    }

    pub fn is_ignoring_block_triggers() -> bool {
        true
    }

    pub fn hurt_server() -> bool {
        false
    }
}

pub fn get_align(flags: i8) -> Align {
    if (flags & FLAG_ALIGN_LEFT) != 0 {
        Align::Left
    } else if (flags & FLAG_ALIGN_RIGHT) != 0 {
        Align::Right
    } else {
        Align::Center
    }
}

impl ServerLevel {
    pub fn tick_display(&mut self, id: EntityId) {
        if let Some(vehicle) = self.entity(id).vehicle
            && self.entity_ref(vehicle).is_none_or(|vehicle| vehicle.is_removed())
        {
            self.stop_riding(id);
        }
    }
}

fn is_text_pending(entity: &LevelEntity) -> bool {
    matches!(entity, LevelEntity::Display(Display { kind: DisplayKind::TextDisplay { pending_text: Some(_) }, .. }))
}

fn take_pending_text(entities: &mut FastIndexMap<EntityId, LevelEntity>, id: EntityId, level: usize) -> Option<(Component, CommandSourceStack)> {
    let Some(LevelEntity::Display(display)) = entities.get_mut(&id) else { return None };
    let DisplayKind::TextDisplay { pending_text } = &mut display.kind else { return None };
    let text = pending_text.take()?;
    let entity = &display.entity;
    let source = CommandSourceStack {
        source: CommandSource::Null,
        position: entity.position(),
        rotation: (entity.x_rot, entity.y_rot),
        level,
        permission: 2,
        names: Names::ForEntity,
        entity: Some(id),
        silent: false,
        anchor: Anchor::Feet,
        callback: CommandResultCallback::default(),
        signing_context: None,
    };
    Some((text, source))
}

fn store_resolved_text(entities: &mut FastIndexMap<EntityId, LevelEntity>, id: EntityId, text: &Component, resolved: Result<Component, CommandSyntaxException>) {
    match resolved {
        Ok(resolved) => {
            if let Some(LevelEntity::Display(display)) = entities.get_mut(&id) {
                display.entity.data.set(DATA_TEXT_ID, DataValue::Component(resolved.to_nbt()));
            }
        }
        Err(e) => warn!("Failed to parse display entity text Optional[{}]\ncom.mojang.brigadier.exceptions.CommandSyntaxException: {}", text.java_to_string(), e.get_message()),
    }
}

impl ServerLevel {
    pub fn resolve_display_text_detached(&mut self, entity: LevelEntity) -> LevelEntity {
        if !is_text_pending(&entity) {
            return entity;
        }
        let id = entity.entity().id;
        self.entities.insert(id, entity);
        if let Some((text, source)) = take_pending_text(&mut self.entities, id, self.index) {
            let resolved = component_utils::resolve(&LevelWorld(self), &source, &text);
            store_resolved_text(&mut self.entities, id, &text, resolved);
        }
        self.entities.swap_remove(&id).expect("the display just inserted")
    }
}

impl Server {
    pub fn resolve_display_text(&mut self, level: usize, id: EntityId) {
        if let Some((text, source)) = take_pending_text(&mut self.levels[level].entities, id, level) {
            let resolved = component_utils::resolve(self, &source, &text);
            store_resolved_text(&mut self.levels[level].entities, id, &text, resolved);
        }
    }
}

