use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::{Arc, LazyLock};

use wafer_protocol::nbt::Nbt;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::data_fix_utils;
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::block_state_data;
use crate::util::datafix::fixes::references::CHUNK;
use crate::util::datafix::packed_bit_storage::PackedBitStorage;

const SIZE: usize = 4096;

#[derive(Default)]
struct PointerHasher(u64);

impl Hasher for PointerHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0 ^ *byte as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        }
    }

    fn write_usize(&mut self, value: usize) {
        self.0 = (value as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
}

type PointerMap<V> = HashMap<usize, V, BuildHasherDefault<PointerHasher>>;

fn identity(state: &Arc<Dynamic>) -> usize {
    Arc::as_ptr(state) as usize
}

struct Palette {
    by_id: Vec<Arc<Dynamic>>,
    ids: PointerMap<i32>,
}

impl Palette {
    fn create() -> Palette {
        Palette { by_id: Vec::new(), ids: PointerMap::default() }
    }

    fn get_id(&self, thing: &Arc<Dynamic>) -> i32 {
        self.ids.get(&identity(thing)).copied().unwrap_or(-1)
    }

    fn by_id(&self, id: i32) -> Option<&Arc<Dynamic>> {
        if id >= 0 { self.by_id.get(id as usize) } else { None }
    }

    fn add(&mut self, key: &Arc<Dynamic>) -> i32 {
        let value = self.by_id.len() as i32;
        self.by_id.push(key.clone());
        self.ids.insert(identity(key), value);
        value
    }

    fn size(&self) -> usize {
        self.by_id.len()
    }
}

fn legacy_block_state(id: &str, properties: &[(&str, &str)]) -> Arc<Dynamic> {
    let dynamic = Dynamic::new(Nbt::Compound(Vec::new()));
    let mut block_state = dynamic.clone().set("Name", dynamic.create_string(id));
    if !properties.is_empty() {
        let map = dynamic.create_map(properties.iter().map(|(key, value)| (dynamic.create_string(*key), dynamic.create_string(*value))).collect::<Vec<_>>());
        block_state = block_state.set("Properties", map);
    }
    Arc::new(block_state)
}

pub fn get_name(state: &Dynamic) -> String {
    state.get("Name").as_string_or("")
}

pub fn get_property(state: &Dynamic, property: &str) -> String {
    state.get("Properties").get(property).as_string_or("")
}

fn id_for(states: &mut Palette, state: &Arc<Dynamic>) -> i32 {
    let mut id = states.get_id(state);
    if id == -1 {
        id = states.add(state);
    }
    id
}

pub const NORTH_WEST_MASK: i32 = 128;
pub const WEST_MASK: i32 = 64;
pub const SOUTH_WEST_MASK: i32 = 32;
pub const SOUTH_MASK: i32 = 16;
pub const SOUTH_EAST_MASK: i32 = 8;
pub const EAST_MASK: i32 = 4;
pub const NORTH_EAST_MASK: i32 = 2;
pub const NORTH_MASK: i32 = 1;

pub fn get_side_mask(west: bool, east: bool, north: bool, south: bool) -> i32 {
    let mut s = 0;
    if north {
        if east {
            s |= NORTH_EAST_MASK;
        } else if west {
            s |= NORTH_WEST_MASK;
        } else {
            s |= NORTH_MASK;
        }
    } else if south {
        if west {
            s |= SOUTH_WEST_MASK;
        } else if east {
            s |= SOUTH_EAST_MASK;
        } else {
            s |= SOUTH_MASK;
        }
    } else if east {
        s |= EAST_MASK;
    } else if west {
        s |= WEST_MASK;
    }
    s
}

struct DataLayer {
    data: Vec<i8>,
}

impl DataLayer {
    const SIZE: usize = 2048;

    fn new() -> DataLayer {
        DataLayer { data: vec![0; DataLayer::SIZE] }
    }

    fn from_data(data: Vec<i8>) -> DataLayer {
        if data.len() != DataLayer::SIZE {
            panic!("ChunkNibbleArrays should be 2048 bytes not: {}", data.len());
        }
        DataLayer { data }
    }

    fn get(&self, x: i32, y: i32, z: i32) -> i32 {
        let position = DataLayer::get_position(y << 8 | z << 4 | x);
        if DataLayer::is_first(y << 8 | z << 4 | x) { (self.data[position] & 15) as i32 } else { (self.data[position] >> 4 & 15) as i32 }
    }

    fn is_first(position: i32) -> bool {
        (position & 1) == 0
    }

    fn get_position(position: i32) -> usize {
        (position >> 1) as usize
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AxisDirection {
    Positive,
    Negative,
}

impl AxisDirection {
    pub fn get_step(self) -> i32 {
        match self {
            AxisDirection::Positive => 1,
            AxisDirection::Negative => -1,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Direction {
    pub fn get_axis_direction(self) -> AxisDirection {
        match self {
            Direction::Down | Direction::North | Direction::West => AxisDirection::Negative,
            Direction::Up | Direction::South | Direction::East => AxisDirection::Positive,
        }
    }

    pub fn get_axis(self) -> Axis {
        match self {
            Direction::Down | Direction::Up => Axis::Y,
            Direction::North | Direction::South => Axis::Z,
            Direction::West | Direction::East => Axis::X,
        }
    }
}

struct MappingConstants {
    virtual_: [bool; 256],
    fix: [bool; 256],
    pumpkin: Arc<Dynamic>,
    snowy_podzol: Arc<Dynamic>,
    snowy_grass: Arc<Dynamic>,
    snowy_mycelium: Arc<Dynamic>,
    upper_sunflower: Arc<Dynamic>,
    upper_lilac: Arc<Dynamic>,
    upper_tall_grass: Arc<Dynamic>,
    upper_large_fern: Arc<Dynamic>,
    upper_rose_bush: Arc<Dynamic>,
    upper_peony: Arc<Dynamic>,
    flower_pot_map: HashMap<String, Arc<Dynamic>>,
    skull_map: HashMap<String, Arc<Dynamic>>,
    door_map: HashMap<String, Arc<Dynamic>>,
    note_block_map: HashMap<String, Arc<Dynamic>>,
    bed_block_map: HashMap<String, Arc<Dynamic>>,
    banner_block_map: HashMap<String, Arc<Dynamic>>,
    air: Arc<Dynamic>,
}

const DYE_COLOR_MAP: [(i32, &str); 16] = [
    (0, "white"),
    (1, "orange"),
    (2, "magenta"),
    (3, "light_blue"),
    (4, "yellow"),
    (5, "lime"),
    (6, "pink"),
    (7, "gray"),
    (8, "light_gray"),
    (9, "cyan"),
    (10, "purple"),
    (11, "blue"),
    (12, "brown"),
    (13, "green"),
    (14, "red"),
    (15, "black"),
];

const FLOWER_POTS: [(&str, &str); 22] = [
    ("minecraft:air0", "minecraft:flower_pot"),
    ("minecraft:red_flower0", "minecraft:potted_poppy"),
    ("minecraft:red_flower1", "minecraft:potted_blue_orchid"),
    ("minecraft:red_flower2", "minecraft:potted_allium"),
    ("minecraft:red_flower3", "minecraft:potted_azure_bluet"),
    ("minecraft:red_flower4", "minecraft:potted_red_tulip"),
    ("minecraft:red_flower5", "minecraft:potted_orange_tulip"),
    ("minecraft:red_flower6", "minecraft:potted_white_tulip"),
    ("minecraft:red_flower7", "minecraft:potted_pink_tulip"),
    ("minecraft:red_flower8", "minecraft:potted_oxeye_daisy"),
    ("minecraft:yellow_flower0", "minecraft:potted_dandelion"),
    ("minecraft:sapling0", "minecraft:potted_oak_sapling"),
    ("minecraft:sapling1", "minecraft:potted_spruce_sapling"),
    ("minecraft:sapling2", "minecraft:potted_birch_sapling"),
    ("minecraft:sapling3", "minecraft:potted_jungle_sapling"),
    ("minecraft:sapling4", "minecraft:potted_acacia_sapling"),
    ("minecraft:sapling5", "minecraft:potted_dark_oak_sapling"),
    ("minecraft:red_mushroom0", "minecraft:potted_red_mushroom"),
    ("minecraft:brown_mushroom0", "minecraft:potted_brown_mushroom"),
    ("minecraft:deadbush0", "minecraft:potted_dead_bush"),
    ("minecraft:tallgrass2", "minecraft:potted_fern"),
    ("minecraft:cactus0", "minecraft:potted_cactus"),
];

const DOOR_TYPES: [&str; 7] = ["oak_door", "iron_door", "spruce_door", "birch_door", "jungle_door", "acacia_door", "dark_oak_door"];

const FIX_IDS: [usize; 18] = [2, 3, 110, 140, 144, 25, 86, 26, 176, 177, 175, 64, 71, 193, 194, 195, 196, 197];

const VIRTUAL_IDS: [usize; 42] = [
    54, 146, 25, 26, 51, 53, 67, 108, 109, 114, 128, 134, 135, 136, 156, 163, 164, 180, 203, 55, 85, 113, 188, 189, 190, 191, 192, 93, 94, 101, 102, 160, 106, 107, 183, 184, 185, 186, 187, 132, 139, 199,
];

fn map_skull(map: &mut HashMap<String, Arc<Dynamic>>, i: i32, name: &str, ty: &str) {
    for facing in ["north", "east", "south", "west"] {
        map.insert(format!("{i}{facing}"), legacy_block_state(&format!("minecraft:{name}_wall_{ty}"), &[("facing", facing)]));
    }
    for rot in 0..16 {
        map.insert(format!("{i}{rot}"), legacy_block_state(&format!("minecraft:{name}_{ty}"), &[("rotation", &rot.to_string())]));
    }
}

fn map_door(map: &mut HashMap<String, Arc<Dynamic>>, ty: &str) {
    let id = format!("minecraft:{ty}");
    for facing in ["east", "north", "south", "west"] {
        for half in ["lower", "upper"] {
            for hinge in ["left", "right"] {
                for open in ["false", "true"] {
                    for powered in ["false", "true"] {
                        map.insert(
                            format!("minecraft:{ty}{facing}{half}{hinge}{open}{powered}"),
                            legacy_block_state(&id, &[("facing", facing), ("half", half), ("hinge", hinge), ("open", open), ("powered", powered)]),
                        );
                    }
                }
            }
        }
    }
}

fn add_beds(map: &mut HashMap<String, Arc<Dynamic>>, color_id: i32, color: &str) {
    let bed = format!("minecraft:{color}_bed");
    for part in ["foot", "head"] {
        for facing in ["south", "west", "north", "east"] {
            map.insert(format!("{facing}false{part}{color_id}"), legacy_block_state(&bed, &[("facing", facing), ("occupied", "false"), ("part", part)]));
        }
    }
    for facing in ["south", "west", "north", "east"] {
        map.insert(format!("{facing}truehead{color_id}"), legacy_block_state(&bed, &[("facing", facing), ("occupied", "true"), ("part", "head")]));
    }
}

fn add_banners(map: &mut HashMap<String, Arc<Dynamic>>, color_id: i32, color: &str) {
    for i in 0..16 {
        map.insert(format!("{i}_{color_id}"), legacy_block_state(&format!("minecraft:{color}_banner"), &[("rotation", &i.to_string())]));
    }
    for facing in ["north", "south", "west", "east"] {
        map.insert(format!("{facing}_{color_id}"), legacy_block_state(&format!("minecraft:{color}_wall_banner"), &[("facing", facing)]));
    }
}

static MAPPING_CONSTANTS: LazyLock<MappingConstants> = LazyLock::new(|| {
    let mut virtual_ = [false; 256];
    let mut fix = [false; 256];
    for id in FIX_IDS {
        fix[id] = true;
    }
    for id in VIRTUAL_IDS {
        virtual_[id] = true;
    }
    let mut flower_pot_map = HashMap::new();
    for (key, name) in FLOWER_POTS {
        flower_pot_map.insert(key.to_string(), legacy_block_state(name, &[]));
    }
    let mut skull_map = HashMap::new();
    map_skull(&mut skull_map, 0, "skeleton", "skull");
    map_skull(&mut skull_map, 1, "wither_skeleton", "skull");
    map_skull(&mut skull_map, 2, "zombie", "head");
    map_skull(&mut skull_map, 3, "player", "head");
    map_skull(&mut skull_map, 4, "creeper", "head");
    map_skull(&mut skull_map, 5, "dragon", "head");
    let mut door_map = HashMap::new();
    for ty in DOOR_TYPES {
        map_door(&mut door_map, ty);
    }
    let mut note_block_map = HashMap::new();
    for i in 0..26 {
        note_block_map.insert(format!("true{i}"), legacy_block_state("minecraft:note_block", &[("powered", "true"), ("note", &i.to_string())]));
        note_block_map.insert(format!("false{i}"), legacy_block_state("minecraft:note_block", &[("powered", "false"), ("note", &i.to_string())]));
    }
    let mut bed_block_map = HashMap::new();
    for (key, value) in DYE_COLOR_MAP {
        if value != "red" {
            add_beds(&mut bed_block_map, key, value);
        }
    }
    let mut banner_block_map = HashMap::new();
    for (key, value) in DYE_COLOR_MAP {
        if value != "white" {
            add_banners(&mut banner_block_map, 15 - key, value);
        }
    }
    let upper = |name: &str| legacy_block_state(name, &[("half", "upper")]);
    MappingConstants {
        virtual_,
        fix,
        pumpkin: legacy_block_state("minecraft:pumpkin", &[]),
        snowy_podzol: legacy_block_state("minecraft:podzol", &[("snowy", "true")]),
        snowy_grass: legacy_block_state("minecraft:grass_block", &[("snowy", "true")]),
        snowy_mycelium: legacy_block_state("minecraft:mycelium", &[("snowy", "true")]),
        upper_sunflower: upper("minecraft:sunflower"),
        upper_lilac: upper("minecraft:lilac"),
        upper_tall_grass: upper("minecraft:tall_grass"),
        upper_large_fern: upper("minecraft:large_fern"),
        upper_rose_bush: upper("minecraft:rose_bush"),
        upper_peony: upper("minecraft:peony"),
        flower_pot_map,
        skull_map,
        door_map,
        note_block_map,
        bed_block_map,
        banner_block_map,
        air: legacy_block_state("minecraft:air", &[]),
    }
});

struct Section {
    palette: Palette,
    list_tag: Vec<Arc<Dynamic>>,
    section: Dynamic,
    has_data: bool,
    to_fix: Vec<(i32, Vec<i32>)>,
    update: Vec<i32>,
    y: i32,
    buffer: Box<[i32; SIZE]>,
}

impl Section {
    fn new(section: Dynamic) -> Section {
        let y = section.get("Y").as_int(0);
        let has_data = section.get("Blocks").result().is_some();
        Section { palette: Palette::create(), list_tag: Vec::new(), section, has_data, to_fix: Vec::new(), update: Vec::new(), y, buffer: Box::new([0; SIZE]) }
    }

    fn get_block(&self, pos: i32) -> Arc<Dynamic> {
        if (0..=4095).contains(&pos) {
            match self.palette.by_id(self.buffer[pos as usize]) {
                Some(tag) => tag.clone(),
                None => MAPPING_CONSTANTS.air.clone(),
            }
        } else {
            MAPPING_CONSTANTS.air.clone()
        }
    }

    fn set_block(&mut self, idx: i32, block_state: &Arc<Dynamic>) {
        if self.palette.get_id(block_state) == -1 {
            self.list_tag.push(if get_name(block_state) == block_state_data::FILTER_ME { MAPPING_CONSTANTS.air.clone() } else { block_state.clone() });
        }
        self.buffer[idx as usize] = id_for(&mut self.palette, block_state);
    }

    fn upgrade(&mut self, mut sides: i32) -> i32 {
        if !self.has_data {
            return sides;
        }
        let blocks = match self.section.get("Blocks").result().map(|blocks| blocks.as_byte_buffer_opt().into_result()) {
            Some(Ok(blocks)) => blocks,
            _ => panic!("No value present"),
        };
        let data = match self.section.get("Data").result().map(|data| data.as_byte_buffer_opt().into_result()) {
            Some(Ok(data)) => DataLayer::from_data(data),
            _ => DataLayer::new(),
        };
        let add_blocks = match self.section.get("Add").result().map(|add| add.as_byte_buffer_opt().into_result()) {
            Some(Ok(add)) => DataLayer::from_data(add),
            _ => DataLayer::new(),
        };
        let air = MAPPING_CONSTANTS.air.clone();
        id_for(&mut self.palette, &air);
        self.list_tag.push(air);
        for idx in 0..SIZE as i32 {
            let xx = idx & 15;
            let yy = idx >> 8 & 15;
            let zz = idx >> 4 & 15;
            let block = match blocks.get(idx as usize) {
                Some(block) => *block,
                None => panic!("null"),
            };
            let id = add_blocks.get(xx, yy, zz) << 12 | (block as i32 & 255) << 4 | data.get(xx, yy, zz);
            if bit(&MAPPING_CONSTANTS.fix, id >> 4) {
                self.add_fix(id >> 4, idx);
            }
            if bit(&MAPPING_CONSTANTS.virtual_, id >> 4) {
                let s = get_side_mask(xx == 0, xx == 15, zz == 0, zz == 15);
                if s == 0 {
                    self.update.push(idx);
                } else {
                    sides |= s;
                }
            }
            self.set_block(idx, &block_state_data::get_tag(id));
        }
        sides
    }

    fn add_fix(&mut self, id: i32, position: i32) {
        match self.to_fix.iter_mut().find(|(key, _)| *key == id) {
            Some((_, list)) => list.push(position),
            None => self.to_fix.push((id, vec![position])),
        }
    }

    fn write(&mut self) -> Dynamic {
        let section = std::mem::take(&mut self.section);
        if !self.has_data {
            return section;
        }
        let palette = section.create_list(self.list_tag.iter().map(|state| (**state).clone()).collect::<Vec<_>>());
        let section = section.set("Palette", palette);
        let size = 4.max(data_fix_utils::ceillog2(self.palette.size() as i32));
        let mut storage = PackedBitStorage::new(size, SIZE as i32);
        for (j, value) in self.buffer.iter().enumerate() {
            storage.set(j as i32, *value);
        }
        let block_states = section.create_long_list(storage.get_raw().iter().copied());
        let section = section.set("BlockStates", block_states);
        let section = section.remove("Blocks");
        let section = section.remove("Data");
        section.remove("Add")
    }
}

fn bit(bits: &[bool; 256], index: i32) -> bool {
    (0..256).contains(&index) && bits[index as usize]
}

struct BlockEntities {
    entries: Vec<Option<(i32, Dynamic)>>,
    positions: HashMap<i32, usize>,
}

impl BlockEntities {
    fn new() -> BlockEntities {
        BlockEntities { entries: Vec::with_capacity(16), positions: HashMap::with_capacity(16) }
    }

    fn put(&mut self, key: i32, entity: Dynamic) -> Option<Dynamic> {
        match self.positions.get(&key) {
            Some(&position) => self.entries[position].replace((key, entity)).map(|(_, previous)| previous),
            None => {
                self.positions.insert(key, self.entries.len());
                self.entries.push(Some((key, entity)));
                None
            }
        }
    }

    fn get(&self, key: i32) -> Option<&Dynamic> {
        self.positions.get(&key).and_then(|&position| self.entries[position].as_ref().map(|(_, entity)| entity))
    }

    fn remove(&mut self, key: i32) -> Option<Dynamic> {
        let position = self.positions.remove(&key)?;
        self.entries[position].take().map(|(_, entity)| entity)
    }

    fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    fn values(&self) -> impl Iterator<Item = &Dynamic> {
        self.entries.iter().flatten().map(|(_, entity)| entity)
    }
}

struct UpgradeChunk {
    sides: i32,
    sections: [Option<Section>; 16],
    level: Dynamic,
    block_entities: BlockEntities,
}

impl UpgradeChunk {
    fn new(level: Dynamic) -> UpgradeChunk {
        let x = level.get("xPos").as_int(0) << 4;
        let z = level.get("zPos").as_int(0) << 4;
        let mut block_entities = BlockEntities::new();
        if let Ok(entities) = level.get("TileEntities").as_stream_opt().into_result() {
            for entity in entities {
                let entity_x = entity.get("x").as_int(0).wrapping_sub(x) & 15;
                let entity_y = entity.get("y").as_int(0);
                let entity_z = entity.get("z").as_int(0).wrapping_sub(z) & 15;
                let key = entity_y << 8 | entity_z << 4 | entity_x;
                if block_entities.put(key, entity).is_some() {
                    warn!("In chunk: {}x{} found a duplicate block entity at position: [{}, {}, {}]", x, z, entity_x, entity_y, entity_z);
                }
            }
        }
        let converted_from_alpha_format = level.get("convertedFromAlphaFormat").as_boolean_or(false);
        let mut chunk = UpgradeChunk { sides: 0, sections: std::array::from_fn(|_| None), level, block_entities };
        if let Ok(sections) = chunk.level.get("Sections").as_stream_opt().into_result() {
            for sec in sections {
                let mut section = Section::new(sec);
                chunk.sides = section.upgrade(chunk.sides);
                if !(0..16).contains(&section.y) {
                    panic!("Index {} out of bounds for length 16", section.y);
                }
                let y = section.y as usize;
                chunk.sections[y] = Some(section);
            }
        }
        for section_index in 0..16 {
            let Some(section) = chunk.sections[section_index].as_mut() else { continue };
            let to_fix = std::mem::take(&mut section.to_fix);
            let dy = section.y << 12;
            for (key, positions) in to_fix {
                chunk.fix_block(key, &positions, dy, converted_from_alpha_format);
            }
        }
        chunk
    }

    fn fix_block(&mut self, key: i32, positions: &[i32], dy: i32, converted_from_alpha_format: bool) {
        let constants = &*MAPPING_CONSTANTS;
        match key {
            2 => {
                for &pos in positions {
                    let pos = pos | dy;
                    let state = self.get_block(pos);
                    if get_name(&state) == "minecraft:grass_block" {
                        let name = get_name(&self.get_block(relative(pos, Direction::Up)));
                        if name == "minecraft:snow" || name == "minecraft:snow_layer" {
                            self.set_block(pos, &constants.snowy_grass);
                        }
                    }
                }
            }
            3 => {
                for &pos in positions {
                    let pos = pos | dy;
                    let state = self.get_block(pos);
                    if get_name(&state) == "minecraft:podzol" {
                        let name = get_name(&self.get_block(relative(pos, Direction::Up)));
                        if name == "minecraft:snow" || name == "minecraft:snow_layer" {
                            self.set_block(pos, &constants.snowy_podzol);
                        }
                    }
                }
            }
            25 => {
                for &pos in positions {
                    let pos = pos | dy;
                    if let Some(entity) = self.remove_block_entity(pos) {
                        let key = format!("{}{}", entity.get("powered").as_boolean_or(false), entity.get("note").as_int(0).max(0).min(24));
                        let state = constants.note_block_map.get(&key).unwrap_or_else(|| &constants.note_block_map["false0"]);
                        self.set_block(pos, state);
                    }
                }
            }
            26 => {
                for &pos in positions {
                    let pos = pos | dy;
                    let state = self.get_block(pos);
                    if let Some(entity) = self.get_block_entity(pos) {
                        let color = entity.get("color").as_int(0);
                        if color != 14 && (0..16).contains(&color) {
                            let key = format!("{}{}{}{}", get_property(&state, "facing"), get_property(&state, "occupied"), get_property(&state, "part"), color);
                            if let Some(bed) = constants.bed_block_map.get(&key) {
                                self.set_block(pos, bed);
                            }
                        }
                    }
                }
            }
            64 | 71 | 193 | 194 | 195 | 196 | 197 => {
                for &pos in positions {
                    let pos = pos | dy;
                    let state = self.get_block(pos);
                    if get_name(&state).ends_with("_door") {
                        let lower = self.get_block(pos);
                        if get_property(&lower, "half") == "lower" {
                            let above_pos = relative(pos, Direction::Up);
                            let upper = self.get_block(above_pos);
                            let name = get_name(&lower);
                            if name == get_name(&upper) {
                                let facing = get_property(&lower, "facing");
                                let open = get_property(&lower, "open");
                                let hinge = if converted_from_alpha_format { "left".to_string() } else { get_property(&upper, "hinge") };
                                let powered = if converted_from_alpha_format { "false".to_string() } else { get_property(&upper, "powered") };
                                let lower_state = constants.door_map.get(&format!("{name}{facing}lower{hinge}{open}{powered}")).expect("a door block state");
                                self.set_block(pos, lower_state);
                                let upper_state = constants.door_map.get(&format!("{name}{facing}upper{hinge}{open}{powered}")).expect("a door block state");
                                self.set_block(above_pos, upper_state);
                            }
                        }
                    }
                }
            }
            86 => {
                for &pos in positions {
                    let pos = pos | dy;
                    let state = self.get_block(pos);
                    if get_name(&state) == "minecraft:carved_pumpkin" {
                        let name = get_name(&self.get_block(relative(pos, Direction::Down)));
                        if name == "minecraft:grass_block" || name == "minecraft:dirt" {
                            self.set_block(pos, &constants.pumpkin);
                        }
                    }
                }
            }
            110 => {
                for &pos in positions {
                    let pos = pos | dy;
                    let state = self.get_block(pos);
                    if get_name(&state) == "minecraft:mycelium" {
                        let name = get_name(&self.get_block(relative(pos, Direction::Up)));
                        if name == "minecraft:snow" || name == "minecraft:snow_layer" {
                            self.set_block(pos, &constants.snowy_mycelium);
                        }
                    }
                }
            }
            140 => {
                for &pos in positions {
                    let pos = pos | dy;
                    if let Some(entity) = self.remove_block_entity(pos) {
                        let key = format!("{}{}", entity.get("Item").as_string_or(""), entity.get("Data").as_int(0));
                        let state = constants.flower_pot_map.get(&key).unwrap_or_else(|| &constants.flower_pot_map["minecraft:air0"]);
                        self.set_block(pos, state);
                    }
                }
            }
            144 => {
                for &pos in positions {
                    let pos = pos | dy;
                    if let Some(entity) = self.get_block_entity(pos) {
                        let ty = entity.get("SkullType").as_int(0).to_string();
                        let facing = get_property(&self.get_block(pos), "facing");
                        let key = if facing != "up" && facing != "down" { format!("{ty}{facing}") } else { format!("{ty}{}", entity.get("Rot").as_int(0)) };
                        let state = constants.skull_map.get(&key).unwrap_or_else(|| &constants.skull_map["0north"]);
                        self.set_block(pos, state);
                    }
                }
            }
            175 => {
                for &pos in positions {
                    let pos = pos | dy;
                    let block = self.get_block(pos);
                    if get_property(&block, "half") == "upper" {
                        let below = self.get_block(relative(pos, Direction::Down));
                        let variant = get_name(&below);
                        match variant.as_str() {
                            "minecraft:sunflower" => self.set_block(pos, &constants.upper_sunflower),
                            "minecraft:lilac" => self.set_block(pos, &constants.upper_lilac),
                            "minecraft:tall_grass" => self.set_block(pos, &constants.upper_tall_grass),
                            "minecraft:large_fern" => self.set_block(pos, &constants.upper_large_fern),
                            "minecraft:rose_bush" => self.set_block(pos, &constants.upper_rose_bush),
                            "minecraft:peony" => self.set_block(pos, &constants.upper_peony),
                            _ => {}
                        }
                    }
                }
            }
            176 | 177 => {
                for &pos in positions {
                    let pos = pos | dy;
                    let state = self.get_block(pos);
                    if let Some(entity) = self.get_block_entity(pos) {
                        let color = entity.get("Base").as_int(0);
                        if color != 15 && (0..16).contains(&color) {
                            let key = format!("{}_{}", get_property(&state, if key == 176 { "rotation" } else { "facing" }), color);
                            if let Some(banner) = constants.banner_block_map.get(&key) {
                                self.set_block(pos, banner);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn get_block_entity(&self, pos: i32) -> Option<Dynamic> {
        self.block_entities.get(pos).cloned()
    }

    fn remove_block_entity(&mut self, pos: i32) -> Option<Dynamic> {
        self.block_entities.remove(pos)
    }

    fn set_block(&mut self, pos: i32, block: &Arc<Dynamic>) {
        if (0..=65535).contains(&pos) {
            if let Some(section) = self.get_section_mut(pos) {
                section.set_block(pos & 4095, block);
            }
        }
    }

    fn get_section(&self, pos: i32) -> Option<&Section> {
        let section_y = (pos >> 12) as usize;
        if section_y < self.sections.len() { self.sections[section_y].as_ref() } else { None }
    }

    fn get_section_mut(&mut self, pos: i32) -> Option<&mut Section> {
        let section_y = (pos >> 12) as usize;
        if section_y < self.sections.len() { self.sections[section_y].as_mut() } else { None }
    }

    fn get_block(&self, pos: i32) -> Arc<Dynamic> {
        if (0..=65535).contains(&pos) {
            match self.get_section(pos) {
                Some(section) => section.get_block(pos & 4095),
                None => MAPPING_CONSTANTS.air.clone(),
            }
        } else {
            MAPPING_CONSTANTS.air.clone()
        }
    }

    fn write(mut self) -> Dynamic {
        let mut level = self.level.clone();
        if self.block_entities.is_empty() {
            level = level.remove("TileEntities");
        } else {
            let tile_entities = level.create_list(self.block_entities.values().cloned().collect::<Vec<_>>());
            level = level.set("TileEntities", tile_entities);
        }
        let mut indices = level.empty_map();
        let mut sections = Vec::new();
        for section in self.sections.iter_mut().flatten() {
            sections.push(section.write());
            let update = indices.create_int_list(section.update.iter().copied());
            indices = indices.set(&section.y.to_string(), update);
        }
        let tag = level.empty_map();
        let sides = tag.create_byte(self.sides as i8);
        let tag = tag.set("Sides", sides);
        let tag = tag.set("Indices", indices);
        let sections = tag.create_list(sections);
        level.set("UpgradeData", tag).set("Sections", sections)
    }
}

pub fn relative(pos: i32, direction: Direction) -> i32 {
    match direction.get_axis() {
        Axis::X => {
            let x = (pos & 15) + direction.get_axis_direction().get_step();
            if (0..=15).contains(&x) { pos & -16 | x } else { -1 }
        }
        Axis::Y => {
            let y = (pos >> 8) + direction.get_axis_direction().get_step();
            if (0..=255).contains(&y) { pos & 0xFF | y << 8 } else { -1 }
        }
        Axis::Z => {
            let z = (pos >> 4 & 15) + direction.get_axis_direction().get_step();
            if (0..=15).contains(&z) { pos & -241 | z << 4 } else { -1 }
        }
    }
}

pub struct ChunkPalettedStorageFix {
    base: DataFixBase,
}

impl ChunkPalettedStorageFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> ChunkPalettedStorageFix {
        ChunkPalettedStorageFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

fn fix(input: Dynamic) -> Dynamic {
    let level = input.get("Level").result();
    match level {
        Some(level) if level.get("Sections").as_stream_opt().into_result().is_ok() => {
            let upgraded = UpgradeChunk::new(level).write();
            input.set("Level", upgraded)
        }
        _ => input,
    }
}

impl DataFix for ChunkPalettedStorageFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let old_type = self.get_input_schema().get_type(CHUNK);
        let new_type = self.get_output_schema().get_type(CHUNK);
        self.write_fix_and_read("ChunkPalettedStorageFix", old_type, new_type, fix)
    }
}
