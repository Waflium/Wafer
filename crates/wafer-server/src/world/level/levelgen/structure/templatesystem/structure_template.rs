use std::sync::Arc;

use wafer_core::random::RandomSource;
use wafer_core::shapes::BitSetDiscreteVoxelShape;
use wafer_core::{Axis, BlockPos, Direction, Vec3, mth};
use wafer_data::{BlockState, blocks};
use wafer_protocol::nbt::Nbt;

use super::structure_place_settings::StructurePlaceSettings;
use crate::server::level::{ServerLevel, UPDATE_SHAPE_ORDER};
use crate::world::entity::decoration::block_attached_entity::BlockAttachedKind;
use crate::world::entity::decoration::hanging_entity;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::entity_type::EntityType;
use crate::world::entity::{Entity, LevelEntity, neutral_mob};
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::block;
use crate::world::level::block::mirror::Mirror;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::levelgen::structure::BoundingBox;

#[derive(Clone)]
pub struct StructureTemplate {
    palettes: Vec<Palette>,
    entity_info_list: Vec<StructureEntityInfo>,
    size: BlockPos,
    author: String,
}

#[derive(Clone)]
pub struct StructureEntityInfo {
    pub pos: Vec3,
    pub block_pos: BlockPos,
    pub nbt: Nbt,
}

#[derive(Clone)]
pub struct Palette {
    blocks: Vec<StructureBlockInfo>,
    cached_jigsaws: std::sync::OnceLock<Vec<JigsawBlockInfo>>,
}

impl Palette {
    pub fn blocks(&self) -> &[StructureBlockInfo] {
        &self.blocks
    }

    pub fn jigsaws(&self) -> &[JigsawBlockInfo] {
        self.cached_jigsaws.get_or_init(|| self.blocks.iter().filter(|info| info.state.is_block(blocks::JIGSAW)).map(JigsawBlockInfo::parse).collect())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JointType {
    Rollable,
    Aligned,
}

#[derive(Clone)]
pub struct JigsawBlockInfo {
    pub pos: BlockPos,
    pub state: BlockState,
    pub joint_type: JointType,
    pub name: Option<Arc<str>>,
    pub pool: Arc<str>,
    pub target: Arc<str>,
    pub placement_priority: i32,
    pub selection_priority: i32,
}

pub fn get_front_facing(state: BlockState) -> Direction {
    state.get(wafer_data::properties::ORIENTATION).front()
}

pub fn get_top_facing(state: BlockState) -> Direction {
    state.get(wafer_data::properties::ORIENTATION).top()
}

pub fn can_attach(source: &JigsawBlockInfo, target: &JigsawBlockInfo) -> bool {
    let source_front = get_front_facing(source.state);
    let target_front = get_front_facing(target.state);
    let source_top = get_top_facing(source.state);
    let target_top = get_top_facing(target.state);
    let rollable = source.joint_type == JointType::Rollable;
    source_front == target_front.opposite() && (rollable || source_top == target_top) && target.name.as_ref().is_none_or(|name| source.target == *name)
}

impl JigsawBlockInfo {
    fn parse(info: &StructureBlockInfo) -> JigsawBlockInfo {
        let nbt = info.nbt.as_ref().expect("jigsaw nbt");
        let string = |key: &str| nbt.get(key).and_then(Nbt::as_str).map(crate::world::level::levelgen::blockpredicates::block_predicate::resource);
        let int = |key: &str| match nbt.get(key) {
            Some(Nbt::Int(v)) => *v,
            _ => 0,
        };
        let joint_type = match nbt.get("joint").and_then(Nbt::as_str) {
            Some("rollable") => JointType::Rollable,
            Some("aligned") => JointType::Aligned,
            _ => {
                if get_front_facing(info.state).axis().is_horizontal() { JointType::Aligned } else { JointType::Rollable }
            }
        };
        JigsawBlockInfo {
            pos: info.pos,
            state: info.state,
            joint_type,
            name: Some(string("name").unwrap_or_else(|| "minecraft:empty".to_owned()).into()),
            pool: string("pool").unwrap_or_else(|| "minecraft:empty".to_owned()).into(),
            target: string("target").unwrap_or_else(|| "minecraft:empty".to_owned()).into(),
            placement_priority: int("placement_priority"),
            selection_priority: int("selection_priority"),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct StructureBlockInfo {
    pub pos: BlockPos,
    pub state: BlockState,
    pub nbt: Option<Nbt>,
}

fn add_to_lists(info: StructureBlockInfo, full_block_list: &mut Vec<StructureBlockInfo>, block_entities_list: &mut Vec<StructureBlockInfo>, other_blocks_list: &mut Vec<StructureBlockInfo>) {
    if info.nbt.is_some() {
        block_entities_list.push(info);
    } else if !info.state.block().info().dynamic_shape && info.state.is_collision_shape_full_block() {
        full_block_list.push(info);
    } else {
        other_blocks_list.push(info);
    }
}

fn build_info_list(mut full_block_list: Vec<StructureBlockInfo>, mut block_entities_list: Vec<StructureBlockInfo>, mut other_blocks_list: Vec<StructureBlockInfo>) -> Vec<StructureBlockInfo> {
    let comparator = |o: &StructureBlockInfo| (o.pos.y, o.pos.x, o.pos.z);
    full_block_list.sort_by_key(comparator);
    other_blocks_list.sort_by_key(comparator);
    block_entities_list.sort_by_key(comparator);
    let mut block_info_list = full_block_list;
    block_info_list.append(&mut other_blocks_list);
    block_info_list.append(&mut block_entities_list);
    block_info_list
}

#[derive(Default)]
struct SimplePalette {
    ids: Vec<Option<BlockState>>,
    last_id: usize,
}

impl SimplePalette {
    fn id_for(&mut self, state: BlockState) -> usize {
        if let Some(id) = self.ids.iter().position(|&s| s == Some(state)) {
            return id;
        }
        let id = self.last_id;
        self.last_id += 1;
        self.add_mapping(state, id);
        id
    }

    fn add_mapping(&mut self, state: BlockState, id: usize) {
        if self.ids.len() <= id {
            self.ids.resize(id + 1, None);
        }
        self.ids[id] = Some(state);
    }
}

fn int_list_tag(x: i32, y: i32, z: i32) -> Nbt {
    Nbt::List(vec![Nbt::Int(x), Nbt::Int(y), Nbt::Int(z)])
}

fn int_list(tag: Option<&Nbt>) -> [i32; 3] {
    let list = tag.map(Nbt::as_list).unwrap_or(&[]);
    let int_or = |i: usize| match list.get(i) {
        Some(Nbt::Int(v)) => *v,
        _ => 0,
    };
    [int_or(0), int_or(1), int_or(2)]
}

impl StructureTemplate {
    pub fn empty() -> StructureTemplate {
        StructureTemplate { palettes: Vec::new(), entity_info_list: Vec::new(), size: BlockPos::ZERO, author: "?".into() }
    }

    pub fn get_author(&self) -> &str {
        &self.author
    }

    pub fn set_author(&mut self, author: String) {
        self.author = author;
    }

    pub fn fill_from_world(&mut self, level: &mut ServerLevel, position: BlockPos, size: BlockPos, inlude_entities: bool, ignore_blocks: &[wafer_data::Block]) {
        if size.x < 1 || size.y < 1 || size.z < 1 {
            return;
        }
        let corner2 = position.offset(size.x, size.y, size.z).offset(-1, -1, -1);
        let mut full_block_list = Vec::new();
        let mut block_entities_list = Vec::new();
        let mut other_blocks_list = Vec::new();
        let min_corner = BlockPos::new(position.x.min(corner2.x), position.y.min(corner2.y), position.z.min(corner2.z));
        let max_corner = BlockPos::new(position.x.max(corner2.x), position.y.max(corner2.y), position.z.max(corner2.z));
        self.size = size;
        level.find_blocks_in(min_corner, max_corner).filter_state(|state| !ignore_blocks.contains(&state.block())).for_each(level, |level, pos, state| {
            let relative_pos = BlockPos::new(pos.x - min_corner.x, pos.y - min_corner.y, pos.z - min_corner.z);
            let nbt = level.get_block_entity(pos).map(|block_entity| block_entity.save_with_id());
            add_to_lists(StructureBlockInfo { pos: relative_pos, state, nbt }, &mut full_block_list, &mut block_entities_list, &mut other_blocks_list);
        });
        let block_info_list = build_info_list(full_block_list, block_entities_list, other_blocks_list);
        self.palettes.clear();
        self.palettes.push(Palette { blocks: block_info_list, cached_jigsaws: std::sync::OnceLock::new() });
        if inlude_entities {
            self.fill_entity_list(level, min_corner, max_corner);
        } else {
            self.entity_info_list.clear();
        }
    }

    fn fill_entity_list(&mut self, level: &mut ServerLevel, min_corner: BlockPos, max_corner: BlockPos) {
        let area = wafer_core::AABB::new(min_corner.x as f64, min_corner.y as f64, min_corner.z as f64, (max_corner.x + 1) as f64, (max_corner.y + 1) as f64, (max_corner.z + 1) as f64);
        let entities = level.get_entities_matching(None, &area, |level, id| !level.players.contains_key(&id));
        self.entity_info_list.clear();
        for id in entities {
            let entity = level.entity_ref(id).unwrap();
            let pos = Vec3::new(entity.x() - min_corner.x as f64, entity.y() - min_corner.y as f64, entity.z() - min_corner.z as f64);
            let block_pos = if matches!(level.entities.get(&id), Some(LevelEntity::Painting(_))) {
                let painting_pos = entity.block_attached_pos();
                BlockPos::new(painting_pos.x - min_corner.x, painting_pos.y - min_corner.y, painting_pos.z - min_corner.z)
            } else {
                BlockPos::containing(pos.x, pos.y, pos.z)
            };
            let nbt = level.save_entity(id).unwrap_or(Nbt::Compound(Vec::new()));
            self.entity_info_list.push(StructureEntityInfo { pos, block_pos, nbt });
        }
    }

    pub fn save(&self) -> Nbt {
        let mut tag = Vec::new();
        if self.palettes.is_empty() {
            tag.push(("blocks".to_string(), Nbt::List(Vec::new())));
            tag.push(("palette".to_string(), Nbt::List(Vec::new())));
        } else {
            let mut palettes: Vec<SimplePalette> = (0..self.palettes.len()).map(|_| SimplePalette::default()).collect();
            let mut block_list = Vec::new();
            let main_palette_blocks = self.palettes[0].blocks();
            for (i, block_info) in main_palette_blocks.iter().enumerate() {
                let mut block_tag = vec![("pos".to_string(), int_list_tag(block_info.pos.x, block_info.pos.y, block_info.pos.z))];
                let id = palettes[0].id_for(block_info.state);
                block_tag.push(("state".to_string(), Nbt::Int(id as i32)));
                if let Some(nbt) = &block_info.nbt {
                    block_tag.push(("nbt".to_string(), nbt.clone()));
                }
                block_list.push(Nbt::Compound(block_tag));
                for p in 1..self.palettes.len() {
                    palettes[p].add_mapping(self.palettes[p].blocks()[i].state, id);
                }
            }
            tag.push(("blocks".to_string(), Nbt::List(block_list)));
            let palette_tag = |palette: &SimplePalette| Nbt::List(palette.ids.iter().flatten().map(|&state| crate::nbt::write_block_state(state)).collect());
            if palettes.len() == 1 {
                tag.push(("palette".to_string(), palette_tag(&palettes[0])));
            } else {
                tag.push(("palettes".to_string(), Nbt::List(palettes.iter().map(palette_tag).collect())));
            }
        }
        let entity_list = self
            .entity_info_list
            .iter()
            .map(|entity_info| {
                Nbt::Compound(vec![
                    ("pos".to_string(), Nbt::List(vec![Nbt::Double(entity_info.pos.x), Nbt::Double(entity_info.pos.y), Nbt::Double(entity_info.pos.z)])),
                    ("blockPos".to_string(), int_list_tag(entity_info.block_pos.x, entity_info.block_pos.y, entity_info.block_pos.z)),
                    ("nbt".to_string(), entity_info.nbt.clone()),
                ])
            })
            .collect();
        tag.push(("entities".to_string(), Nbt::List(entity_list)));
        tag.push(("size".to_string(), int_list_tag(self.size.x, self.size.y, self.size.z)));
        tag.push(("DataVersion".to_string(), Nbt::Int(wafer_data::WORLD_VERSION)));
        Nbt::Compound(tag)
    }

    pub fn get_size(&self) -> BlockPos {
        self.size
    }

    pub fn get_size_rotated(&self, rotation: Rotation) -> BlockPos {
        match rotation {
            Rotation::Counterclockwise90 | Rotation::Clockwise90 => BlockPos::new(self.size.z, self.size.y, self.size.x),
            _ => self.size,
        }
    }

    pub fn calculate_connected_position(&self, settings1: &StructurePlaceSettings, connection1: BlockPos, settings2: &StructurePlaceSettings, connection2: BlockPos) -> BlockPos {
        let marker_pos1 = StructureTemplate::calculate_relative_position(settings1, connection1);
        let marker_pos2 = StructureTemplate::calculate_relative_position(settings2, connection2);
        marker_pos1.offset(-marker_pos2.x, -marker_pos2.y, -marker_pos2.z)
    }

    pub fn calculate_relative_position(settings: &StructurePlaceSettings, pos: BlockPos) -> BlockPos {
        transform(pos, settings.mirror, settings.rotation, settings.rotation_pivot)
    }

    pub fn place_in_world<R: RandomSource>(
        &self,
        level: &mut impl WorldGenLevel,
        position: BlockPos,
        reference_pos: BlockPos,
        settings: &StructurePlaceSettings,
        random: &mut R,
        update_mode: i32,
    ) -> bool {
        if self.palettes.is_empty() {
            return false;
        }
        let block_info_list = settings.get_random_palette(&self.palettes, position, random).blocks();
        if !(!block_info_list.is_empty() || !settings.ignore_entities && !self.entity_info_list.is_empty()) || self.size.x < 1 || self.size.y < 1 || self.size.z < 1 {
            return false;
        }
        let bounding_box = settings.bounding_box;
        let mut to_fill = Vec::with_capacity(if settings.apply_waterlogging { block_info_list.len() } else { 0 });
        let mut locked_fluids = Vec::with_capacity(if settings.apply_waterlogging { block_info_list.len() } else { 0 });
        let mut placed: Vec<BlockPos> = Vec::with_capacity(block_info_list.len());
        let (mut min_x, mut min_y, mut min_z) = (i32::MAX, i32::MAX, i32::MAX);
        let (mut max_x, mut max_y, mut max_z) = (i32::MIN, i32::MIN, i32::MIN);
        let processed_block_info_list = process_block_infos(level, position, reference_pos, settings, block_info_list, random);
        for block_info in processed_block_info_list {
            let block_pos = block_info.pos;
            if bounding_box.is_some_and(|bounding_box| !bounding_box.is_inside(block_pos)) {
                continue;
            }
            let previous_fluid_state = settings.apply_waterlogging.then(|| level.get_fluid_state(block_pos));
            let state = block::rotate(block::mirror(block_info.state, settings.mirror), settings.rotation);
            if block_info.nbt.is_some() {
                level.set_block(block_pos, blocks::BARRIER.default_state(), 820);
            }
            if !level.set_block(block_pos, state, update_mode) {
                continue;
            }
            min_x = min_x.min(block_pos.x);
            min_y = min_y.min(block_pos.y);
            min_z = min_z.min(block_pos.z);
            max_x = max_x.max(block_pos.x);
            max_y = max_y.max(block_pos.y);
            max_z = max_z.max(block_pos.z);
            placed.push(block_pos);
            if let Some(Nbt::Compound(mut nbt)) = block_info.nbt
                && let Some(block_entity) = level.get_block_entity(block_pos)
            {
                if block_entity.is_randomizable() {
                    let seed = Nbt::Long(random.next_long());
                    match nbt.iter_mut().find(|(key, _)| key == "LootTableSeed") {
                        Some((_, existing)) => *existing = seed,
                        None => nbt.push(("LootTableSeed".into(), seed)),
                    }
                }
                block_entity.load_with_components(&Nbt::Compound(nbt));
            }
            if let Some(previous_fluid_state) = previous_fluid_state {
                if state.fluid_state().is_source() {
                    locked_fluids.push(block_pos);
                } else if block::of(state).is_liquid_block_container(state) {
                    block::of(state).place_liquid(level, block_pos, state, previous_fluid_state);
                    if !previous_fluid_state.is_source() {
                        to_fill.push(block_pos);
                    }
                }
            }
        }
        let mut filled = true;
        let directions = [Direction::Up, Direction::North, Direction::East, Direction::South, Direction::West];
        while filled && !to_fill.is_empty() {
            filled = false;
            to_fill.retain(|&pos| {
                let mut to_place = level.get_fluid_state(pos);
                for direction in directions {
                    if to_place.is_source() {
                        break;
                    }
                    let neighbor_pos = pos.relative(direction);
                    let neighbor = level.get_fluid_state(neighbor_pos);
                    if neighbor.is_source() && !locked_fluids.contains(&neighbor_pos) {
                        to_place = neighbor;
                    }
                }
                if to_place.is_source() {
                    let state = level.get_block_state(pos);
                    if block::of(state).is_liquid_block_container(state) {
                        block::of(state).place_liquid(level, pos, state, to_place);
                        filled = true;
                        return false;
                    }
                }
                true
            });
        }
        if min_x <= max_x && !settings.known_shape {
            let mut shape = BitSetDiscreteVoxelShape::new(max_x - min_x + 1, max_y - min_y + 1, max_z - min_z + 1);
            for block_pos in &placed {
                shape.fill(block_pos.x - min_x, block_pos.y - min_y, block_pos.z - min_z);
            }
            update_shape_at_edge(level, update_mode, &shape, min_x, min_y, min_z);
            for &block_pos in &placed {
                let state = level.get_block_state(block_pos);
                let new_state = update_from_neighbour_shapes(state, level, block_pos);
                if state != new_state {
                    level.set_block(block_pos, new_state, (update_mode & -2) | 16);
                }
            }
        }
        if !settings.ignore_entities {
            self.place_entities(level, position, settings.mirror, settings.rotation, settings.rotation_pivot, bounding_box, settings.finalize_entities);
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn place_entities(&self, level: &mut impl WorldGenLevel, position: BlockPos, mirror: Mirror, rotation: Rotation, pivot: BlockPos, bounding_box: Option<BoundingBox>, finalize_entities: bool) {
        for entity_info in &self.entity_info_list {
            let block_pos = transform(entity_info.block_pos, mirror, rotation, pivot).offset(position.x, position.y, position.z);
            if bounding_box.is_some_and(|bounding_box| !bounding_box.is_inside(block_pos)) {
                continue;
            }
            let Nbt::Compound(mut tag) = entity_info.nbt.clone() else { continue };
            let pos = transform_vec(entity_info.pos, mirror, rotation, pivot).add(position.x as f64, position.y as f64, position.z as f64);
            set_tag(&mut tag, "Pos", Nbt::List(vec![Nbt::Double(pos.x), Nbt::Double(pos.y), Nbt::Double(pos.z)]));
            tag.retain(|(key, _)| key != "UUID");
            if tag.iter().any(|(key, _)| key == "block_pos") {
                set_tag(&mut tag, "block_pos", Nbt::IntArray(vec![block_pos.x, block_pos.y, block_pos.z]));
            }
            let Some(mut entity) = create_entity_ignore_exception(level, &Nbt::Compound(tag)) else { continue };
            let mut y_rot = entity_rotate(entity.entity_mut(), rotation);
            y_rot += entity_mirror(entity.entity_mut(), mirror) - entity.entity().y_rot;
            let x_rot = entity.entity().x_rot;
            entity.entity_mut().snap_to(pos.x, pos.y, pos.z, y_rot, x_rot);
            if finalize_entities && let LevelEntity::Mob(mob) = &mut entity {
                let difficulty = level.get_current_difficulty_at(BlockPos::containing(pos.x, pos.y, pos.z));
                crate::world::entity::mob::finalize_spawn(level, mob, &difficulty, EntitySpawnReason::Structure, None);
            }
            level.add_fresh_entity_with_passengers(entity);
        }
    }

    pub fn get_jigsaws(&self, position: BlockPos, rotation: Rotation) -> Vec<JigsawBlockInfo> {
        if self.palettes.is_empty() {
            return Vec::new();
        }
        let settings = StructurePlaceSettings { rotation, ..StructurePlaceSettings::default() };
        let palette = settings.get_random_palette(&self.palettes, position, &mut wafer_core::random::LegacyRandomSource::new(0));
        palette
            .jigsaws()
            .iter()
            .map(|jigsaw| {
                let pos = StructureTemplate::calculate_relative_position(&settings, jigsaw.pos).offset(position.x, position.y, position.z);
                JigsawBlockInfo { pos, state: block::rotate(jigsaw.state, rotation), ..jigsaw.clone() }
            })
            .collect()
    }

    pub fn filter_blocks<R: RandomSource>(&self, position: BlockPos, settings: &StructurePlaceSettings, block: wafer_data::Block, random: &mut R) -> Vec<StructureBlockInfo> {
        if self.palettes.is_empty() {
            return Vec::new();
        }
        let bounding_box = settings.bounding_box;
        settings
            .get_random_palette(&self.palettes, position, random)
            .blocks
            .iter()
            .filter(|block_info| block_info.state.is_block(block))
            .filter_map(|block_info| {
                let block_pos = StructureTemplate::calculate_relative_position(settings, block_info.pos).offset(position.x, position.y, position.z);
                bounding_box
                    .is_none_or(|bounding_box| bounding_box.is_inside(block_pos))
                    .then(|| StructureBlockInfo { pos: block_pos, state: block::rotate(block_info.state, settings.rotation), nbt: block_info.nbt.clone() })
            })
            .collect()
    }

    pub fn get_zero_position_with_transform(&self, zero_pos: BlockPos, mirror: Mirror, rotation: Rotation) -> BlockPos {
        get_zero_position_with_transform(zero_pos, mirror, rotation, self.size.x, self.size.z)
    }

    pub fn get_bounding_box(&self, settings: &StructurePlaceSettings, position: BlockPos) -> BoundingBox {
        get_bounding_box(position, settings.rotation, settings.rotation_pivot, settings.mirror, self.size)
    }

    pub fn load(tag: &Nbt) -> StructureTemplate {
        let [x, y, z] = int_list(tag.get("size"));
        let size = BlockPos::new(x, y, z);
        let block_list = tag.get("blocks").map(Nbt::as_list).unwrap_or(&[]);
        let palettes = match tag.get("palettes") {
            Some(Nbt::List(palette_list_list)) => palette_list_list.iter().map(|palette_list| load_palette(palette_list.as_list(), block_list)).collect(),
            _ => vec![load_palette(tag.get("palette").map(Nbt::as_list).unwrap_or(&[]), block_list)],
        };
        let entity_info_list = tag
            .get("entities")
            .map(Nbt::as_list)
            .unwrap_or(&[])
            .iter()
            .filter(|entity_tag| matches!(entity_tag, Nbt::Compound(_)))
            .filter_map(|entity_tag| {
                let pos_tag = entity_tag.get("pos").map(Nbt::as_list).unwrap_or(&[]);
                let double = |i: usize| match pos_tag.get(i) {
                    Some(Nbt::Double(v)) => *v,
                    _ => 0.0,
                };
                let pos = Vec3::new(double(0), double(1), double(2));
                let [x, y, z] = int_list(entity_tag.get("blockPos"));
                let nbt = entity_tag.get("nbt").filter(|nbt| matches!(nbt, Nbt::Compound(_))).cloned()?;
                Some(StructureEntityInfo { pos, block_pos: BlockPos::new(x, y, z), nbt })
            })
            .collect();
        StructureTemplate { palettes, entity_info_list, size, author: "?".into() }
    }
}

fn load_palette(palette_list: &[Nbt], block_list: &[Nbt]) -> Palette {
    let palette: Vec<BlockState> = palette_list.iter().map(crate::nbt::read_block_state).collect();
    let mut full_block_list = Vec::new();
    let mut block_entities_list = Vec::new();
    let mut other_blocks_list = Vec::new();
    for block_tag in block_list.iter().filter(|tag| matches!(tag, Nbt::Compound(_))) {
        let [x, y, z] = int_list(block_tag.get("pos"));
        let state_index = match block_tag.get("state") {
            Some(Nbt::Int(index)) => *index,
            _ => 0,
        };
        let state = usize::try_from(state_index).ok().and_then(|index| palette.get(index).copied()).unwrap_or(blocks::AIR.default_state());
        let nbt = block_tag.get("nbt").filter(|nbt| matches!(nbt, Nbt::Compound(_))).cloned();
        add_to_lists(StructureBlockInfo { pos: BlockPos::new(x, y, z), state, nbt }, &mut full_block_list, &mut block_entities_list, &mut other_blocks_list);
    }
    Palette { blocks: build_info_list(full_block_list, block_entities_list, other_blocks_list), cached_jigsaws: std::sync::OnceLock::new() }
}

pub fn update_shape_at_edge(level: &mut impl WorldGenLevel, update_mode: i32, shape: &BitSetDiscreteVoxelShape, start_x: i32, start_y: i32, start_z: i32) {
    shape.for_all_faces(|direction, x, y, z| {
        let pos = BlockPos::new(start_x + x, start_y + y, start_z + z);
        let neighbor_pos = pos.relative(direction);
        let state = level.get_block_state(pos);
        let neighbor_state = level.get_block_state(neighbor_pos);
        let new_state = block::of(state).update_shape(state, level, pos, direction, neighbor_pos, neighbor_state);
        if state != new_state {
            level.set_block(pos, new_state, update_mode & -2);
        }
        let new_neighbor_state = block::of(neighbor_state).update_shape(neighbor_state, level, neighbor_pos, direction.opposite(), pos, new_state);
        if neighbor_state != new_neighbor_state {
            level.set_block(neighbor_pos, new_neighbor_state, update_mode & -2);
        }
    });
}

fn update_from_neighbour_shapes(state: BlockState, level: &mut impl WorldGenLevel, pos: BlockPos) -> BlockState {
    let mut new_state = state;
    for direction in UPDATE_SHAPE_ORDER {
        let neighbor_pos = pos.relative(direction);
        let neighbor_state = level.get_block_state(neighbor_pos);
        new_state = block::of(new_state).update_shape(new_state, level, pos, direction, neighbor_pos, neighbor_state);
    }
    new_state
}

pub fn process_block_infos<R: RandomSource>(
    level: &mut impl WorldGenLevel,
    position: BlockPos,
    reference_pos: BlockPos,
    settings: &StructurePlaceSettings,
    block_info_list: &[StructureBlockInfo],
    random: &mut R,
) -> Vec<StructureBlockInfo> {
    let mut original_block_info_list: Vec<&StructureBlockInfo> = Vec::new();
    let mut processed_block_info_list = Vec::with_capacity(block_info_list.len());
    let process_only_in_current_chunk = !settings.processors.iter().any(|processor| processor.evaluates_entire_piece_state());
    let chunk_bb = settings.bounding_box;
    for block_info in block_info_list {
        let block_pos = StructureTemplate::calculate_relative_position(settings, block_info.pos);
        let block_pos = block_pos.offset(position.x, position.y, position.z);
        if process_only_in_current_chunk && chunk_bb.is_some_and(|chunk_bb| !chunk_bb.is_inside(block_pos)) {
            continue;
        }
        let mut processed_block_info = Some(StructureBlockInfo { pos: block_pos, state: block_info.state, nbt: block_info.nbt.clone() });
        for processor in &settings.processors {
            let Some(info) = processed_block_info else { break };
            processed_block_info = processor.process_block(level, position, reference_pos, block_info.pos, info, settings, random);
        }
        if let Some(processed_block_info) = processed_block_info {
            processed_block_info_list.push(processed_block_info);
            original_block_info_list.push(block_info);
        }
    }
    for processor in &settings.processors {
        processor.finalize_processing(level, position, reference_pos, &original_block_info_list, &mut processed_block_info_list, settings, random);
    }
    processed_block_info_list
}

fn set_tag(tag: &mut Vec<(String, Nbt)>, key: &str, value: Nbt) {
    match tag.iter_mut().find(|(k, _)| k == key) {
        Some((_, existing)) => *existing = value,
        None => tag.push((key.into(), value)),
    }
}

fn create_entity_ignore_exception(level: &mut impl WorldGenLevel, tag: &Nbt) -> Option<LevelEntity> {
    let id = tag.get("id").and_then(Nbt::as_str);
    let entity_type = id.and_then(EntityType::by_name).filter(|ty| ty.can_spawn(level.get_difficulty()));
    let Some(mut entity) = entity_type.and(id).and_then(ServerLevel::create_entity) else {
        warn!("Skipping Entity with id {}", id.unwrap_or("[invalid]"));
        return None;
    };
    entity.load(tag).ok()?;
    if let LevelEntity::Mob(mob) = &mut entity
        && let Some((neutral_mob, entity)) = mob.neutral_mob_mut()
    {
        let end_time = tag.get(neutral_mob::TAG_ANGER_END_TIME).and_then(Nbt::as_i64).unwrap_or(neutral_mob::NO_ANGER_END_TIME);
        neutral_mob.set_persistent_anger_end_time(entity, end_time);
    }
    Some(entity)
}

fn entity_rotate(entity: &mut Entity, rotation: Rotation) -> f32 {
    if entity.block_attached.is_some_and(|attached| attached.kind != BlockAttachedKind::LeashKnot && attached.kind != BlockAttachedKind::Cushion) {
        let direction = hanging_entity::get_direction(entity);
        if direction.axis() != Axis::Y {
            hanging_entity::set_direction(entity, rotation.rotate(direction));
        }
        let angle = mth::wrap_degrees(entity.y_rot);
        return match rotation {
            Rotation::Clockwise180 => angle + 180.0,
            Rotation::Counterclockwise90 => angle + 90.0,
            Rotation::Clockwise90 => angle + 270.0,
            Rotation::None => angle,
        };
    }
    let angle = mth::wrap_degrees(entity.y_rot);
    match rotation {
        Rotation::Clockwise180 => angle + 180.0,
        Rotation::Counterclockwise90 => angle + 270.0,
        Rotation::Clockwise90 => angle + 90.0,
        Rotation::None => angle,
    }
}

fn entity_mirror(entity: &mut Entity, mirror: Mirror) -> f32 {
    if entity.block_attached.is_some_and(|attached| attached.kind != BlockAttachedKind::LeashKnot && attached.kind != BlockAttachedKind::Cushion) {
        let rotation = mirror.get_rotation(hanging_entity::get_direction(entity));
        return entity_rotate(entity, rotation);
    }
    let angle = mth::wrap_degrees(entity.y_rot);
    match mirror {
        Mirror::FrontBack => -angle,
        Mirror::LeftRight => 180.0 - angle,
        Mirror::None => angle,
    }
}

pub fn transform_vec(pos: Vec3, mirror: Mirror, rotation: Rotation, pivot: BlockPos) -> Vec3 {
    let (mut x, y, mut z) = (pos.x, pos.y, pos.z);
    let was_mirrored = match mirror {
        Mirror::LeftRight => {
            z = 1.0 - z;
            true
        }
        Mirror::FrontBack => {
            x = 1.0 - x;
            true
        }
        Mirror::None => false,
    };
    let (pivot_x, pivot_z) = (pivot.x as f64, pivot.z as f64);
    match rotation {
        Rotation::Counterclockwise90 => Vec3::new(pivot_x - pivot_z + z, y, pivot_x + pivot_z + 1.0 - x),
        Rotation::Clockwise90 => Vec3::new(pivot_x + pivot_z + 1.0 - z, y, pivot_z - pivot_x + x),
        Rotation::Clockwise180 => Vec3::new(pivot_x + pivot_x + 1.0 - x, y, pivot_z + pivot_z + 1.0 - z),
        Rotation::None => if was_mirrored { Vec3::new(x, y, z) } else { pos },
    }
}

pub fn transform(pos: BlockPos, mirror: Mirror, rotation: Rotation, pivot: BlockPos) -> BlockPos {
    let (mut x, y, mut z) = (pos.x, pos.y, pos.z);
    match mirror {
        Mirror::LeftRight => z = -z,
        Mirror::FrontBack => x = -x,
        Mirror::None => {}
    }
    let (pivot_x, pivot_z) = (pivot.x, pivot.z);
    match rotation {
        Rotation::Counterclockwise90 => BlockPos::new(pivot_x - pivot_z + z, y, pivot_x + pivot_z - x),
        Rotation::Clockwise90 => BlockPos::new(pivot_x + pivot_z - z, y, pivot_z - pivot_x + x),
        Rotation::Clockwise180 => BlockPos::new(pivot_x + pivot_x - x, y, pivot_z + pivot_z - z),
        Rotation::None => BlockPos::new(x, y, z),
    }
}

pub fn get_zero_position_with_transform(zero_pos: BlockPos, mirror: Mirror, rotation: Rotation, size_x: i32, size_z: i32) -> BlockPos {
    let size_x = size_x - 1;
    let size_z = size_z - 1;
    let mirror_delta_x = if mirror == Mirror::FrontBack { size_x } else { 0 };
    let mirror_delta_z = if mirror == Mirror::LeftRight { size_z } else { 0 };
    match rotation {
        Rotation::Counterclockwise90 => zero_pos.offset(mirror_delta_z, 0, size_x - mirror_delta_x),
        Rotation::Clockwise90 => zero_pos.offset(size_z - mirror_delta_z, 0, mirror_delta_x),
        Rotation::Clockwise180 => zero_pos.offset(size_x - mirror_delta_x, 0, size_z - mirror_delta_z),
        Rotation::None => zero_pos.offset(mirror_delta_x, 0, mirror_delta_z),
    }
}

pub fn get_bounding_box(position: BlockPos, rotation: Rotation, pivot: BlockPos, mirror: Mirror, size: BlockPos) -> BoundingBox {
    let delta = size.offset(-1, -1, -1);
    let corner1 = transform(BlockPos::ZERO, mirror, rotation, pivot);
    let corner2 = transform(delta, mirror, rotation, pivot);
    BoundingBox::from_corners(corner1, corner2).move_by(position)
}
