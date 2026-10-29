use std::sync::LazyLock;

use wafer_core::{AABB, BlockPos};
use wafer_data::blocks;
use wafer_protocol::nbt::Nbt;
use wafer_protocol::{Reader, Writer};

use super::BlockEntityKind;
use crate::gametest::framework::game_test_instance::GameTestInstance;
use crate::gametest::framework::structure_utils;
use crate::network::chat::{Arg, ChatFormatting, Component, component_serialization};
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DecodeResult};
use crate::util::extra_codecs;
use crate::world::item::component::codec::Codec;
use crate::world::item::component::types::{self, Pair, enumeration, identifier, nbt, opt, record, req};
use crate::world::level::storage::tag_value_input;
use crate::world::level::BlockGetter;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;
use crate::world::level::levelgen::structure::templatesystem::structure_template_manager;

const STRUCTURE_OFFSET: BlockPos = BlockPos::new(0, 1, 1);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Cleared,
    Running,
    Finished,
}

impl Status {
    const VALUES: [Status; 3] = [Status::Cleared, Status::Running, Status::Finished];

    fn name(self) -> &'static str {
        match self {
            Status::Cleared => "cleared",
            Status::Running => "running",
            Status::Finished => "finished",
        }
    }

    fn by_name(name: &str) -> Option<Status> {
        Status::VALUES.into_iter().find(|status| status.name() == name)
    }

    pub fn by_index(index: i32) -> Status {
        usize::try_from(index).ok().and_then(|index| Status::VALUES.get(index).copied()).unwrap_or(Status::Cleared)
    }
}

pub fn rotation_name(rotation: Rotation) -> &'static str {
    match rotation {
        Rotation::None => "none",
        Rotation::Clockwise90 => "clockwise_90",
        Rotation::Clockwise180 => "180",
        Rotation::Counterclockwise90 => "counterclockwise_90",
    }
}

fn int_array(tag: &Nbt) -> Option<BlockPos> {
    let values: Vec<i32> = match tag {
        Nbt::IntArray(values) => values.clone(),
        Nbt::List(values) => values.iter().map(|v| v.as_i64().map(|v| v as i32)).collect::<Option<_>>()?,
        _ => return None,
    };
    (values.len() == 3).then(|| BlockPos::new(values[0], values[1], values[2]))
}

#[derive(Clone)]
pub struct Data {
    pub test: Option<String>,
    pub size: BlockPos,
    pub rotation: Rotation,
    pub ignore_entities: bool,
    pub status: Status,
    pub error_message: Option<Component>,
}

impl Data {
    pub fn with_status(&self, status: Status) -> Data {
        Data { status, error_message: None, ..self.clone() }
    }

    pub fn with_error(&self, error: Component) -> Data {
        Data { status: Status::Finished, error_message: Some(error), ..self.clone() }
    }

    fn to_nbt(&self) -> Nbt {
        let mut fields = Vec::new();
        if let Some(test) = &self.test {
            fields.push(("test".into(), Nbt::String(test.clone())));
        }
        fields.push(("size".into(), Nbt::IntArray(vec![self.size.x, self.size.y, self.size.z])));
        fields.push(("rotation".into(), Nbt::String(rotation_name(self.rotation).into())));
        fields.push(("ignore_entities".into(), Nbt::Byte(self.ignore_entities as i8)));
        fields.push(("status".into(), Nbt::String(self.status.name().into())));
        if let Some(error_message) = &self.error_message {
            fields.push(("error_message".into(), error_message.to_nbt()));
        }
        Nbt::Compound(fields)
    }

    fn decode(tag: &Nbt) -> DecodeResult<Data> {
        static CODEC: LazyLock<Pair> = LazyLock::new(|| {
            record(vec![
                opt("test", identifier()),
                req("size", extra_codecs::vec3i_codec()),
                req("rotation", enumeration(&["none", "clockwise_90", "180", "counterclockwise_90"])),
                req("ignore_entities", types::bool()),
                req("status", enumeration(&["cleared", "running", "finished"])),
                opt("error_message", nbt(Codec::Func(component_serialization::decode_custom_name))),
            ])
        });
        let DecodeResult { value, error } = CODEC.0.decode_partial(tag);
        DecodeResult { value: value.and_then(|decoded| Data::from_nbt(&decoded)), error }
    }

    fn from_nbt(tag: &Nbt) -> Option<Data> {
        Some(Data {
            test: match tag.get("test") {
                Some(test) => Some(crate::resources::identifier::read(test.as_str()?).ok()?),
                None => None,
            },
            size: int_array(tag.get("size")?)?,
            rotation: Rotation::by_name(tag.get("rotation")?.as_str()?)?,
            ignore_entities: tag.get("ignore_entities")?.as_i64()? != 0,
            status: Status::by_name(tag.get("status")?.as_str()?)?,
            error_message: tag.get("error_message").map(Component::from_nbt),
        })
    }

    pub fn read(r: &mut Reader) -> anyhow::Result<Data> {
        let test = if r.bool()? { Some(crate::resources::identifier::read(r.string(32767)?).map_err(anyhow::Error::msg)?) } else { None };
        let size = BlockPos::new(r.varint()?, r.varint()?, r.varint()?);
        let rotation = Rotation::VALUES[r.varint()?.rem_euclid(4) as usize];
        let ignore_entities = r.bool()?;
        let status = Status::by_index(r.varint()?);
        let error_message = if r.bool()? { Some(Component::from_nbt(&Nbt::read(r)?)) } else { None };
        Ok(Data { test, size, rotation, ignore_entities, status, error_message })
    }
}

#[derive(Clone)]
pub struct ErrorMarker {
    pub pos: BlockPos,
    pub text: Component,
}

impl ErrorMarker {
    fn decode_list(tag: &Nbt) -> DecodeResult<Vec<ErrorMarker>> {
        static CODEC: LazyLock<Pair> = LazyLock::new(|| {
            record(vec![req("pos", extra_codecs::vec3i_codec()), req("text", nbt(Codec::Func(component_serialization::decode_custom_name)))])
        });
        codec::list_partial(tag, |marker| {
            let DecodeResult { value, error } = CODEC.0.decode_partial(marker);
            DecodeResult { value: value.and_then(|decoded| Some(ErrorMarker { pos: int_array(decoded.get("pos")?)?, text: Component::from_nbt(decoded.get("text")?) })), error }
        })
    }
}

#[derive(Clone)]
pub struct TestInstanceBlockEntity {
    pub data: Data,
    error_markers: Vec<ErrorMarker>,
}

impl Default for TestInstanceBlockEntity {
    fn default() -> TestInstanceBlockEntity {
        TestInstanceBlockEntity {
            data: Data { test: None, size: BlockPos::ZERO, rotation: Rotation::None, ignore_entities: false, status: Status::Cleared, error_message: None },
            error_markers: Vec::new(),
        }
    }
}

impl TestInstanceBlockEntity {
    pub fn load_additional(&mut self, input: &Nbt) {
        if let Some(data) = tag_value_input::read(input, "data", Data::decode) {
            self.data = data;
        }
        self.error_markers.clear();
        self.error_markers.extend(tag_value_input::read(input, "errors", ErrorMarker::decode_list).unwrap_or_default());
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("data".into(), self.data.to_nbt()));
        if !self.error_markers.is_empty() {
            let markers = self
                .error_markers
                .iter()
                .map(|marker| Nbt::Compound(vec![("pos".into(), Nbt::IntArray(vec![marker.pos.x, marker.pos.y, marker.pos.z])), ("text".into(), marker.text.to_nbt())]))
                .collect();
            output.push(("errors".into(), Nbt::List(markers)));
        }
    }
}

pub fn get_structure_size(test_key: &str) -> Option<BlockPos> {
    let (_, test) = GameTestInstance::get(test_key)?;
    structure_template_manager::get(test.structure()).map(|template| template.get_size())
}

pub fn get_structure_pos(block_pos: BlockPos) -> BlockPos {
    block_pos.offset(STRUCTURE_OFFSET.x, STRUCTURE_OFFSET.y, STRUCTURE_OFFSET.z)
}

pub fn aabb_of(bounding_box: &BoundingBox) -> AABB {
    AABB::new(
        bounding_box.min.x as f64,
        bounding_box.min.y as f64,
        bounding_box.min.z as f64,
        (bounding_box.max.x + 1) as f64,
        (bounding_box.max.y + 1) as f64,
        (bounding_box.max.z + 1) as f64,
    )
}

pub fn write_status(w: &mut Writer, status: &Component, size: Option<BlockPos>) {
    status.write(w);
    w.bool(size.is_some());
    if let Some(size) = size {
        w.varint(size.x);
        w.varint(size.y);
        w.varint(size.z);
    }
}

impl ServerLevel {
    pub fn test_instance_block_entity(&mut self, pos: BlockPos) -> Option<&mut TestInstanceBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::TestInstanceBlock(test_instance) => Some(test_instance),
            _ => None,
        }
    }

    fn test_instance(&mut self, pos: BlockPos) -> TestInstanceBlockEntity {
        self.test_instance_block_entity(pos).cloned().unwrap_or_default()
    }

    pub fn test_instance_block_entity_set(&mut self, pos: BlockPos, data: Data) {
        let Some(entity) = self.test_instance_block_entity(pos) else { return };
        entity.data = data;
        self.test_instance_block_entity_set_changed(pos);
    }

    fn test_instance_block_entity_set_changed(&mut self, pos: BlockPos) {
        self.block_entity_set_changed(pos);
        let state = self.block_state(pos);
        self.send_block_updated(pos, blocks::AIR.default_state(), state);
    }

    pub fn test_instance_block_entity_test_holder(&mut self, pos: BlockPos) -> Option<(&'static str, &'static GameTestInstance)> {
        let test = self.test_instance_block_entity(pos)?.data.test.clone()?;
        GameTestInstance::get(&test)
    }

    pub fn test_instance_block_entity_get_test_name(&mut self, pos: BlockPos) -> Component {
        match self.test_instance(pos).data.test {
            Some(test) => Component::literal(test),
            None => Component::translatable("test_instance_block.invalid_test"),
        }
    }

    pub fn test_instance_block_entity_get_rotation(&mut self, pos: BlockPos) -> Rotation {
        let rotation = self.test_instance(pos).data.rotation;
        self.test_instance_block_entity_test_holder(pos).map_or(Rotation::None, |(_, test)| test.rotation()).get_rotated(rotation)
    }

    fn test_instance_block_entity_get_padding(&mut self, pos: BlockPos) -> i32 {
        self.test_instance_block_entity_test_holder(pos).map_or(0, |(_, test)| test.padding())
    }

    pub fn test_instance_block_entity_set_error_message(&mut self, pos: BlockPos, error_message: Component) {
        let data = self.test_instance(pos).data.with_error(error_message);
        self.test_instance_block_entity_set(pos, data);
    }

    pub fn test_instance_block_entity_set_success(&mut self, pos: BlockPos) {
        let data = self.test_instance(pos).data.with_status(Status::Finished);
        self.test_instance_block_entity_set(pos, data);
    }

    pub fn test_instance_block_entity_set_running(&mut self, pos: BlockPos) {
        let data = self.test_instance(pos).data.with_status(Status::Running);
        self.test_instance_block_entity_set(pos, data);
    }

    pub fn test_instance_block_entity_get_structure_pos(&mut self, pos: BlockPos) -> BlockPos {
        let padding = self.test_instance_block_entity_get_padding(pos);
        get_structure_pos(pos.offset(padding, padding, padding))
    }

    fn test_instance_block_entity_get_transformed_size(&mut self, pos: BlockPos) -> BlockPos {
        let size = self.test_instance(pos).data.size;
        let rotation = self.test_instance_block_entity_get_rotation(pos);
        let axes_switched = rotation == Rotation::Clockwise90 || rotation == Rotation::Counterclockwise90;
        BlockPos::new(if axes_switched { size.z } else { size.x }, size.y, if axes_switched { size.x } else { size.z })
    }

    pub fn test_instance_block_entity_get_structure_bounding_box(&mut self, pos: BlockPos) -> BoundingBox {
        let corner1 = self.test_instance_block_entity_get_structure_pos(pos);
        let size = self.test_instance_block_entity_get_transformed_size(pos);
        let corner2 = corner1.offset(size.x, size.y, size.z).offset(-1, -1, -1);
        BoundingBox::from_corners(corner1, corner2)
    }

    pub fn test_instance_block_entity_get_test_bounding_box(&mut self, pos: BlockPos) -> BoundingBox {
        let padding = self.test_instance_block_entity_get_padding(pos);
        self.test_instance_block_entity_get_structure_bounding_box(pos).inflated_by(padding)
    }

    pub fn test_instance_block_entity_get_structure_bounds(&mut self, pos: BlockPos) -> AABB {
        aabb_of(&self.test_instance_block_entity_get_structure_bounding_box(pos))
    }

    pub fn test_instance_block_entity_get_test_bounds(&mut self, pos: BlockPos) -> AABB {
        let padding = self.test_instance_block_entity_get_padding(pos) as f64;
        self.test_instance_block_entity_get_structure_bounds(pos).inflate_all(padding)
    }

    pub fn test_instance_block_entity_get_start_corner(&mut self, pos: BlockPos) -> BlockPos {
        let structure_size = self.test_instance(pos).data.size;
        let rotation = self.test_instance_block_entity_get_rotation(pos);
        let north_west_corner = self.test_instance_block_entity_get_structure_pos(pos);
        match rotation {
            Rotation::None => north_west_corner,
            Rotation::Clockwise90 => north_west_corner.offset(structure_size.z - 1, 0, 0),
            Rotation::Clockwise180 => north_west_corner.offset(structure_size.x - 1, 0, structure_size.z - 1),
            Rotation::Counterclockwise90 => north_west_corner.offset(0, 0, structure_size.x - 1),
        }
    }

    pub fn test_instance_block_entity_reset_test(&mut self, pos: BlockPos, feedback: &mut Vec<Component>) {
        self.test_instance_block_entity_remove_barriers(pos);
        self.test_instance_block_entity_clear_error_markers(pos);
        if self.test_instance_block_entity_place_structure(pos) {
            let name = self.test_instance_block_entity_get_test_name(pos);
            feedback.push(Component::translatable_with("test_instance_block.reset_success", vec![name.into()]).with_color(ChatFormatting::Green));
        }
        let data = self.test_instance(pos).data.with_status(Status::Cleared);
        self.test_instance_block_entity_set(pos, data);
    }

    pub fn test_instance_block_entity_save_test(&mut self, pos: BlockPos, feedback: &mut Vec<Component>) -> Option<String> {
        let identifier = match self.test_instance_block_entity_test_holder(pos) {
            Some((_, test)) => Some(test.structure().to_string()),
            None => self.test_instance(pos).data.test,
        };
        let Some(identifier) = identifier else {
            feedback.push(
                Component::translatable_with("test_instance_block.error.unable_to_save", vec![Arg::Int(pos.x), Arg::Int(pos.y), Arg::Int(pos.z)]).with_color(ChatFormatting::Red),
            );
            return None;
        };
        let structure_pos = self.test_instance_block_entity_get_structure_pos(pos);
        let data = self.test_instance(pos).data;
        super::structure_block_entity::save_structure(self, &identifier, structure_pos, data.size, data.ignore_entities, "", true, &[blocks::AIR]);
        Some(identifier)
    }

    pub fn test_instance_block_entity_export_test(&mut self, pos: BlockPos, feedback: &mut Vec<Component>) -> bool {
        match self.test_instance_block_entity_save_test(pos, feedback) {
            Some(saved) => export(&saved, feedback),
            None => false,
        }
    }

    pub fn test_instance_block_entity_place_structure(&mut self, pos: BlockPos) -> bool {
        let Some(test) = self.test_instance(pos).data.test else { return false };
        let Some((_, test)) = GameTestInstance::get(&test) else { return false };
        let Some(template) = structure_template_manager::get(test.structure()) else { return false };
        let rotation = self.test_instance_block_entity_get_rotation(pos);
        let ignore_entities = self.test_instance(pos).data.ignore_entities;
        let place_settings = StructurePlaceSettings { rotation, ignore_entities, known_shape: true, ..StructurePlaceSettings::default() };
        let start = self.test_instance_block_entity_get_start_corner(pos);
        self.test_instance_block_entity_force_load_chunks(pos);
        let test_bounding_box = self.test_instance_block_entity_get_test_bounding_box(pos);
        structure_utils::clear_space_for_structure(&test_bounding_box, self);
        self.test_instance_block_entity_remove_entities(pos);
        let mut random = self.random.clone();
        template.place_in_world(self, start, start, &place_settings, &mut random, 818);
        self.random = random;
        true
    }

    fn test_instance_block_entity_remove_entities(&mut self, pos: BlockPos) {
        let bounds = self.test_instance_block_entity_get_test_bounds(pos);
        for entity in self.get_entities(None, &bounds) {
            if !self.is_player(entity) {
                self.discard(entity);
            }
        }
    }

    fn test_instance_block_entity_force_load_chunks(&mut self, pos: BlockPos) {
        let bounding_box = self.test_instance_block_entity_get_structure_bounding_box(pos);
        for x in bounding_box.min.x >> 4..=bounding_box.max.x >> 4 {
            for z in bounding_box.min.z >> 4..=bounding_box.max.z >> 4 {
                self.set_chunk_forced(x, z, true);
            }
        }
    }

    pub fn test_instance_block_entity_encase_structure(&mut self, pos: BlockPos) {
        for block_pos in self.test_instance_block_entity_structure_boundary(pos) {
            if !self.block_state(block_pos).is_block(blocks::TEST_INSTANCE_BLOCK) {
                self.set_block(block_pos, blocks::BARRIER.default_state(), 3);
            }
        }
    }

    pub fn test_instance_block_entity_remove_barriers(&mut self, pos: BlockPos) {
        for block_pos in self.test_instance_block_entity_structure_boundary(pos) {
            if self.block_state(block_pos).is_block(blocks::BARRIER) {
                self.set_block(block_pos, blocks::AIR.default_state(), 3);
            }
        }
    }

    fn test_instance_block_entity_structure_boundary(&mut self, pos: BlockPos) -> Vec<BlockPos> {
        let bounds = self.test_instance_block_entity_get_structure_bounds(pos);
        let has_ceiling = !self.test_instance_block_entity_test_holder(pos).is_some_and(|(_, test)| test.sky_access());
        let low = BlockPos::containing(bounds.min_x, bounds.min_y, bounds.min_z).offset(-1, -1, -1);
        let high = BlockPos::containing(bounds.max_x, bounds.max_y, bounds.max_z);
        let mut boundary = Vec::new();
        for block_pos in BlockPos::between_closed(low, high) {
            let is_non_ceiling_edge = block_pos.x == low.x || block_pos.x == high.x || block_pos.z == low.z || block_pos.z == high.z || block_pos.y == low.y;
            let is_ceiling = block_pos.y == high.y;
            if is_non_ceiling_edge || is_ceiling && has_ceiling {
                boundary.push(block_pos);
            }
        }
        boundary
    }

    pub fn test_instance_block_entity_mark_error(&mut self, pos: BlockPos, error_pos: BlockPos, text: Component) {
        let Some(entity) = self.test_instance_block_entity(pos) else { return };
        entity.error_markers.push(ErrorMarker { pos: error_pos, text });
        self.test_instance_block_entity_set_changed(pos);
    }

    pub fn test_instance_block_entity_clear_error_markers(&mut self, pos: BlockPos) {
        let Some(entity) = self.test_instance_block_entity(pos) else { return };
        if !entity.error_markers.is_empty() {
            entity.error_markers.clear();
            self.test_instance_block_entity_set_changed(pos);
        }
    }
}

pub fn export(_structure_id: &str, feedback: &mut Vec<Component>) -> bool {
    feedback.push(Component::literal("Test structure exporting is disabled").with_color(ChatFormatting::Red));
    true
}

pub fn run_test(server: &mut crate::server::Server, level: usize, pos: BlockPos, feedback: &mut Vec<Component>) {
    use crate::gametest::framework::{game_test_info::GameTestInfo, game_test_runner, retry_options::RetryOptions, test_command};
    let Some(test) = server.levels[level].test_instance_block_entity_test_holder(pos) else {
        feedback.push(Component::translatable_with("test_instance_block.error.no_test", vec![Arg::Int(pos.x), Arg::Int(pos.y), Arg::Int(pos.z)]).with_color(ChatFormatting::Red));
        return;
    };
    if !server.levels[level].test_instance_block_entity_place_structure(pos) {
        feedback.push(
            Component::translatable_with("test_instance_block.error.no_test_structure", vec![Arg::Int(pos.x), Arg::Int(pos.y), Arg::Int(pos.z)]).with_color(ChatFormatting::Red),
        );
        return;
    }
    server.levels[level].test_instance_block_entity_clear_error_markers(pos);
    test_command::stop_tests(server);
    server.game_test_ticker.forget_failed_tests();
    feedback.push(Component::translatable_with("test_instance_block.starting", vec![Arg::String(test.0.to_string())]));
    server.game_test_ticker.reset_arena_if_idle();
    let rotation = server.levels[level].test_instance_block_entity(pos).map_or(Rotation::None, |entity| entity.data.rotation);
    let mut info = GameTestInfo::new(test, rotation, level, RetryOptions::no_retries());
    info.set_test_block_pos(Some(pos));
    let info = game_test_runner::new_info(server, info);
    game_test_runner::Builder::from_info(server, vec![info]).build(server);
    let source = crate::commands::source::CommandSourceStack::server(server);
    test_command::track_and_start_runner(server, &source);
}
