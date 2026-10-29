use wafer_core::BlockPos;
use wafer_core::random::LegacyRandomSource;
use wafer_data::properties::{STRUCTUREBLOCK_MODE, StructureMode};
use wafer_data::{Block, BlockState, blocks};
use wafer_protocol::nbt::Nbt;

use super::BlockEntityKind;
use crate::server::level::ServerLevel;
use crate::util::codec;
use crate::world::entity::EntityId;
use crate::world::level::BlockGetter;
use crate::world::level::block::mirror::Mirror;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::templatesystem::structure_place_settings::{PlaceRandom, StructurePlaceSettings};
use crate::world::level::levelgen::structure::templatesystem::structure_processor::StructureProcessor;
use crate::world::level::levelgen::structure::templatesystem::structure_template::StructureTemplate;
use crate::world::level::levelgen::structure::templatesystem::structure_template_manager;
use crate::world::level::storage::tag_value_input;

const SCAN_CORNER_BLOCKS_RANGE: i32 = 80;
pub const MAX_OFFSET_PER_AXIS: i32 = 48;
pub const MAX_SIZE_PER_AXIS: i32 = 48;
const DEFAULT_POS: BlockPos = BlockPos::new(0, 1, 0);
const DEFAULT_SIZE: BlockPos = BlockPos::ZERO;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum UpdateType {
    UpdateData,
    SaveArea,
    LoadArea,
    ScanArea,
}

impl UpdateType {
    pub const VALUES: [UpdateType; 4] = [UpdateType::UpdateData, UpdateType::SaveArea, UpdateType::LoadArea, UpdateType::ScanArea];
}

pub fn mode_legacy_name(mode: StructureMode) -> &'static str {
    match mode {
        StructureMode::Save => "SAVE",
        StructureMode::Load => "LOAD",
        StructureMode::Corner => "CORNER",
        StructureMode::Data => "DATA",
    }
}

#[derive(Clone)]
pub struct StructureBlockEntity {
    structure_name: Option<String>,
    pub author: String,
    pub meta_data: String,
    pub structure_pos: BlockPos,
    pub structure_size: BlockPos,
    pub mirror: Mirror,
    pub rotation: Rotation,
    pub mode: StructureMode,
    pub ignore_entities: bool,
    pub strict: bool,
    pub powered: bool,
    pub show_air: bool,
    pub show_bounding_box: bool,
    pub integrity: f32,
    pub seed: i64,
}

impl StructureBlockEntity {
    pub fn new(state: BlockState) -> StructureBlockEntity {
        StructureBlockEntity {
            structure_name: None,
            author: String::new(),
            meta_data: String::new(),
            structure_pos: DEFAULT_POS,
            structure_size: DEFAULT_SIZE,
            mirror: Mirror::None,
            rotation: Rotation::None,
            mode: state.get(STRUCTUREBLOCK_MODE),
            ignore_entities: true,
            strict: false,
            powered: false,
            show_air: false,
            show_bounding_box: true,
            integrity: 1.0,
            seed: 0,
        }
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("name".into(), Nbt::String(self.get_structure_name())));
        output.push(("author".into(), Nbt::String(self.author.clone())));
        output.push(("metadata".into(), Nbt::String(self.meta_data.clone())));
        output.push(("posX".into(), Nbt::Int(self.structure_pos.x)));
        output.push(("posY".into(), Nbt::Int(self.structure_pos.y)));
        output.push(("posZ".into(), Nbt::Int(self.structure_pos.z)));
        output.push(("sizeX".into(), Nbt::Int(self.structure_size.x)));
        output.push(("sizeY".into(), Nbt::Int(self.structure_size.y)));
        output.push(("sizeZ".into(), Nbt::Int(self.structure_size.z)));
        output.push(("rotation".into(), Nbt::String(self.rotation.legacy_name().into())));
        output.push(("mirror".into(), Nbt::String(self.mirror.legacy_name().into())));
        output.push(("mode".into(), Nbt::String(mode_legacy_name(self.mode).into())));
        output.push(("ignoreEntities".into(), Nbt::Byte(self.ignore_entities as i8)));
        output.push(("strict".into(), Nbt::Byte(self.strict as i8)));
        output.push(("powered".into(), Nbt::Byte(self.powered as i8)));
        output.push(("showair".into(), Nbt::Byte(self.show_air as i8)));
        output.push(("showboundingbox".into(), Nbt::Byte(self.show_bounding_box as i8)));
        output.push(("integrity".into(), Nbt::Float(self.integrity)));
        output.push(("seed".into(), Nbt::Long(self.seed)));
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        let string_or = |key: &str| tag_value_input::get_string(input, key).unwrap_or_default().to_owned();
        let int_or = |key: &str, default: i32| super::int_or(input, key, default);
        let bool_or = |key: &str, default: bool| super::byte_or(input, key, default as i8) != 0;
        self.set_structure_name(&string_or("name"));
        self.author = string_or("author");
        self.meta_data = string_or("metadata");
        let offset = |key: &str, default: i32| int_or(key, default).clamp(-MAX_OFFSET_PER_AXIS, MAX_OFFSET_PER_AXIS);
        self.structure_pos = BlockPos::new(offset("posX", DEFAULT_POS.x), offset("posY", DEFAULT_POS.y), offset("posZ", DEFAULT_POS.z));
        let size = |key: &str, default: i32| int_or(key, default).clamp(0, MAX_SIZE_PER_AXIS);
        self.structure_size = BlockPos::new(size("sizeX", DEFAULT_SIZE.x), size("sizeY", DEFAULT_SIZE.y), size("sizeZ", DEFAULT_SIZE.z));
        self.rotation = tag_value_input::read_or_error(input, "rotation", |tag| codec::legacy_enum(tag, Rotation::by_legacy_name)).unwrap_or(Rotation::None);
        self.mirror = tag_value_input::read_or_error(input, "mirror", |tag| codec::legacy_enum(tag, Mirror::by_legacy_name)).unwrap_or(Mirror::None);
        self.mode = tag_value_input::read_or_error(input, "mode", |tag| codec::legacy_enum(tag, |name| StructureMode::VALUES.into_iter().find(|&mode| mode_legacy_name(mode) == name))).unwrap_or(StructureMode::Data);
        self.ignore_entities = bool_or("ignoreEntities", true);
        self.strict = bool_or("strict", false);
        self.powered = bool_or("powered", false);
        self.show_air = bool_or("showair", false);
        self.show_bounding_box = bool_or("showboundingbox", true);
        self.integrity = super::float_or(input, "integrity", 1.0);
        self.seed = tag_value_input::get_numeric(input, "seed").and_then(super::long_value).unwrap_or(0);
    }

    pub fn get_structure_name(&self) -> String {
        self.structure_name.clone().unwrap_or_default()
    }

    pub fn has_structure_name(&self) -> bool {
        self.structure_name.is_some()
    }

    pub fn set_structure_name(&mut self, structure_name: &str) {
        self.structure_name = if structure_name.is_empty() { None } else { crate::commands::arguments::parse_identifier(structure_name) };
    }
}

pub fn create_random(seed: i64) -> LegacyRandomSource {
    if seed == 0 { LegacyRandomSource::create() } else { LegacyRandomSource::new(seed) }
}

#[allow(clippy::too_many_arguments)]
pub fn save_structure(
    level: &mut ServerLevel,
    structure_name: &str,
    pos: BlockPos,
    structure_size: BlockPos,
    ignore_entities: bool,
    author: &str,
    save_to_disk: bool,
    ignore_blocks: &[Block],
) -> bool {
    let mut structure_template = (*structure_template_manager::get_or_create(structure_name)).clone();
    let ignore_blocks: Vec<Block> = ignore_blocks.iter().copied().chain([blocks::STRUCTURE_VOID]).collect();
    structure_template.fill_from_world(level, pos, structure_size, !ignore_entities, &ignore_blocks);
    structure_template.set_author(author.to_owned());
    structure_template_manager::put(structure_name, structure_template);
    if save_to_disk { structure_template_manager::save(structure_name) } else { true }
}

impl ServerLevel {
    pub fn structure_block_entity(&mut self, pos: BlockPos) -> Option<&mut StructureBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::StructureBlock(structure_block) => Some(structure_block),
            _ => None,
        }
    }

    pub fn structure_block_entity_update_block_state(&mut self, pos: BlockPos) {
        let Some(mode) = self.structure_block_entity(pos).map(|entity| entity.mode) else { return };
        let block_state = self.block_state(pos);
        if block_state.is_block(blocks::STRUCTURE_BLOCK) {
            self.set_block(pos, block_state.set(STRUCTUREBLOCK_MODE, mode), 2);
        }
    }

    pub fn structure_block_entity_set_mode(&mut self, pos: BlockPos, mode: StructureMode) {
        let Some(entity) = self.structure_block_entity(pos) else { return };
        entity.mode = mode;
        let state = self.block_state(pos);
        if state.is_block(blocks::STRUCTURE_BLOCK) {
            self.set_block(pos, state.set(STRUCTUREBLOCK_MODE, mode), 2);
        }
    }

    pub fn structure_block_entity_created_by(&mut self, pos: BlockPos, creator: EntityId) {
        let name = match self.find_player(creator) {
            Some(player) => player.profile.name.clone(),
            None => self.find_entity(creator).map_or_else(String::new, |entity| entity.name().get_string()),
        };
        if let Some(entity) = self.structure_block_entity(pos) {
            entity.author = name;
        }
    }

    pub fn structure_block_entity_detect_size(&mut self, pos: BlockPos) -> bool {
        let Some(entity) = self.structure_block_entity(pos) else { return false };
        if entity.mode != StructureMode::Save {
            return false;
        }
        let structure_name = entity.structure_name.clone();
        let Some(bb) = self.get_enclosing_bounding_box(pos, structure_name.as_deref()) else { return false };
        let delta_x = bb.max.x - bb.min.x;
        let delta_y = bb.max.y - bb.min.y;
        let delta_z = bb.max.z - bb.min.z;
        if delta_x > 1 && delta_y > 1 && delta_z > 1 {
            let entity = self.structure_block_entity(pos).unwrap();
            entity.structure_pos = BlockPos::new(bb.min.x - pos.x + 1, bb.min.y - pos.y + 1, bb.min.z - pos.z + 1);
            entity.structure_size = BlockPos::new(delta_x - 1, delta_y - 1, delta_z - 1);
            self.block_entity_set_changed(pos);
            let state = self.block_state(pos);
            self.send_block_updated(pos, state, state);
            true
        } else {
            false
        }
    }

    fn get_enclosing_bounding_box(&mut self, self_pos: BlockPos, structure_name: Option<&str>) -> Option<BoundingBox> {
        let mut corner_count = 0;
        let mut result: Option<BoundingBox> = None;
        let corner1 = BlockPos::new(self_pos.x - SCAN_CORNER_BLOCKS_RANGE, self.chunks.min_y(), self_pos.z - SCAN_CORNER_BLOCKS_RANGE);
        let corner2 = BlockPos::new(self_pos.x + SCAN_CORNER_BLOCKS_RANGE, self.chunks.max_y(), self_pos.z + SCAN_CORNER_BLOCKS_RANGE);
        self.find_blocks_in(corner1, corner2).filter_state(|state| state.is_block(blocks::STRUCTURE_BLOCK)).for_each(self, |level, pos, _| {
            if let Some(entity) = level.structure_block_entity(pos)
                && entity.mode == StructureMode::Corner
                && entity.structure_name.as_deref() == structure_name
            {
                corner_count += 1;
                result = Some(match result {
                    None => BoundingBox::of(pos),
                    Some(bounding_box) => bounding_box.encapsulate(pos),
                });
            }
        });
        match corner_count {
            0 => None,
            1 => result.map(|bounding_box| bounding_box.encapsulate(self_pos)),
            _ => result,
        }
    }

    pub fn structure_block_entity_save_structure(&mut self, pos: BlockPos) -> bool {
        match self.structure_block_entity(pos) {
            Some(entity) if entity.mode == StructureMode::Save => self.structure_block_entity_save_structure_to(pos, true),
            _ => false,
        }
    }

    pub fn structure_block_entity_save_structure_to(&mut self, pos: BlockPos, save_to_disk: bool) -> bool {
        let Some(entity) = self.structure_block_entity(pos) else { return false };
        let Some(structure_name) = entity.structure_name.clone() else { return false };
        let (structure_pos, structure_size, ignore_entities, author) = (entity.structure_pos, entity.structure_size, entity.ignore_entities, entity.author.clone());
        let origin = pos.offset(structure_pos.x, structure_pos.y, structure_pos.z);
        save_structure(self, &structure_name, origin, structure_size, ignore_entities, &author, save_to_disk, &[])
    }

    pub fn structure_block_entity_place_structure_if_same_size(&mut self, pos: BlockPos) -> bool {
        let Some(entity) = self.structure_block_entity(pos) else { return false };
        if entity.mode != StructureMode::Load {
            return false;
        }
        let Some(structure_name) = entity.structure_name.clone() else { return false };
        let Some(template) = structure_template_manager::get(&structure_name) else { return false };
        if template.get_size() == entity.structure_size {
            self.structure_block_entity_place(pos, &template);
            true
        } else {
            self.structure_block_entity_load_structure_info(pos, &template);
            false
        }
    }

    pub fn structure_block_entity_load_structure_info_from_manager(&mut self, pos: BlockPos) -> bool {
        let Some(template) = self.structure_block_entity_get_structure_template(pos) else { return false };
        self.structure_block_entity_load_structure_info(pos, &template);
        true
    }

    fn structure_block_entity_load_structure_info(&mut self, pos: BlockPos, structure_template: &StructureTemplate) {
        let Some(entity) = self.structure_block_entity(pos) else { return };
        entity.author = structure_template.get_author().to_owned();
        entity.structure_size = structure_template.get_size();
        self.block_entity_set_changed(pos);
    }

    pub fn structure_block_entity_place_structure(&mut self, pos: BlockPos) {
        if let Some(template) = self.structure_block_entity_get_structure_template(pos) {
            self.structure_block_entity_place(pos, &template);
        }
    }

    fn structure_block_entity_get_structure_template(&mut self, pos: BlockPos) -> Option<std::sync::Arc<StructureTemplate>> {
        let structure_name = self.structure_block_entity(pos)?.structure_name.clone()?;
        structure_template_manager::get(&structure_name)
    }

    fn structure_block_entity_place(&mut self, pos: BlockPos, template: &StructureTemplate) {
        self.structure_block_entity_load_structure_info(pos, template);
        let Some(entity) = self.structure_block_entity(pos) else { return };
        let (mirror, rotation, ignore_entities, strict, integrity, seed, structure_pos) =
            (entity.mirror, entity.rotation, entity.ignore_entities, entity.strict, entity.integrity, entity.seed, entity.structure_pos);
        let block_rot_processor = StructureProcessor::BlockRot { rottable_blocks: None, integrity: integrity.clamp(0.0, 1.0) };
        let mut place_settings = StructurePlaceSettings { mirror, rotation, ignore_entities, known_shape: strict, ..StructurePlaceSettings::default() };
        if integrity < 1.0 {
            place_settings.processors = vec![&block_rot_processor];
            place_settings.random = PlaceRandom::Own(std::cell::RefCell::new(create_random(seed)));
        }
        let origin = pos.offset(structure_pos.x, structure_pos.y, structure_pos.z);
        template.place_in_world(self, origin, origin, &place_settings, &mut create_random(seed), 2 | if strict { 816 } else { 0 });
    }

    pub fn structure_block_entity_unload_structure(&mut self, pos: BlockPos) {
        if let Some(structure_name) = self.structure_block_entity(pos).and_then(|entity| entity.structure_name.clone()) {
            structure_template_manager::remove(&structure_name);
        }
    }

    pub fn structure_block_entity_is_structure_loadable(&mut self, pos: BlockPos) -> bool {
        match self.structure_block_entity(pos) {
            Some(entity) if entity.mode == StructureMode::Load => entity.structure_name.clone().is_some_and(|name| structure_template_manager::get(&name).is_some()),
            _ => false,
        }
    }
}
