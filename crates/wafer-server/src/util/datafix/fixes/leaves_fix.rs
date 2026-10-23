use std::collections::HashMap;
use std::sync::Arc;

use wafer_core::fastutil::{Int2ObjectOpenHashMap, IntOpenHashSet};
use wafer_core::mth;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{OpticFinder, Typed, field_finder, find_field, type_finder};
use crate::datafixers::types::{Type, equals, list, named, remainder_type};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::{BLOCK_STATE, CHUNK};
use crate::util::datafix::fixes::world_gen_settings_fix::java_parse_int;
use crate::util::datafix::packed_bit_storage::PackedBitStorage;

const DIRECTIONS: [[i32; 3]; 6] = [[-1, 0, 0], [1, 0, 0], [0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1]];
const DECAY_DISTANCE: i32 = 7;
const SIZE_BITS: i32 = 12;
const SIZE: i32 = 4096;

fn leaves(block_name: &str) -> Option<i32> {
    match block_name {
        "minecraft:acacia_leaves" => Some(0),
        "minecraft:birch_leaves" => Some(1),
        "minecraft:dark_oak_leaves" => Some(2),
        "minecraft:jungle_leaves" => Some(3),
        "minecraft:oak_leaves" => Some(4),
        "minecraft:spruce_leaves" => Some(5),
        _ => None,
    }
}

const LOGS: [&str; 18] = [
    "minecraft:acacia_bark",
    "minecraft:birch_bark",
    "minecraft:dark_oak_bark",
    "minecraft:jungle_bark",
    "minecraft:oak_bark",
    "minecraft:spruce_bark",
    "minecraft:acacia_log",
    "minecraft:birch_log",
    "minecraft:dark_oak_log",
    "minecraft:jungle_log",
    "minecraft:oak_log",
    "minecraft:spruce_log",
    "minecraft:stripped_acacia_log",
    "minecraft:stripped_birch_log",
    "minecraft:stripped_dark_oak_log",
    "minecraft:stripped_jungle_log",
    "minecraft:stripped_oak_log",
    "minecraft:stripped_spruce_log",
];

pub struct LeavesFix {
    base: DataFixBase,
}

impl LeavesFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> LeavesFix {
        LeavesFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

fn java_hash_map_key_order(keys: &[i32]) -> Vec<usize> {
    let mut capacity = 16usize;
    for size in 1..=keys.len() {
        if size > capacity * 3 / 4 {
            capacity *= 2;
        }
    }
    let bucket = |key: i32| (key ^ ((key as u32) >> 16) as i32) as u32 as usize & (capacity - 1);
    let mut order: Vec<usize> = (0..keys.len()).collect();
    order.sort_by_key(|&position| bucket(keys[position]));
    order
}

impl DataFix for LeavesFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let chunk_type = self.get_input_schema().get_type(CHUNK);
        let level_finder = find_field(&chunk_type, "Level");
        let sections_finder = find_field(&level_finder.ty(), "Sections");
        let sections_type = sections_finder.ty();
        let Type::List(section_type) = &*sections_type else { panic!("Expecting sections to be a list.") };
        let section_finder = type_finder(section_type.clone());
        let input_schema = self.get_input_schema().clone();
        self.fix_type_everywhere_typed("Leaves fix", chunk_type, move |chunk| {
            chunk.update_typed(&level_finder, None, |level| {
                let mut sides = 0;
                let mut new_level = level.update_typed(&sections_finder, None, |sections| {
                    let section_typeds = sections.get_all_typed(&section_finder);
                    let leaves_sections: Vec<LeavesSection> = section_typeds.iter().map(|section| LeavesSection::new(section, &input_schema)).collect();
                    let keys: Vec<i32> = leaves_sections.iter().map(LeavesSection::get_index).collect();
                    for (position, key) in keys.iter().enumerate() {
                        if keys[..position].contains(key) {
                            panic!("Duplicate key {key}");
                        }
                    }
                    let order = java_hash_map_key_order(&keys);
                    let mut leaves_sections: Vec<Option<LeavesSection>> = leaves_sections.into_iter().map(Some).collect();
                    let mut section_map: Int2ObjectOpenHashMap<LeavesSection> = Int2ObjectOpenHashMap::with_expected(keys.len(), 0.75);
                    for position in order {
                        section_map.insert(keys[position], leaves_sections[position].take().expect("a section"));
                    }
                    if section_map.values().all(LeavesSection::is_skippable) {
                        return sections;
                    }
                    let mut queue: Vec<IntOpenHashSet> = Vec::new();
                    for _ in 0..7 {
                        queue.push(IntOpenHashSet::new());
                    }
                    for section in section_map.values() {
                        if !section.is_skippable() {
                            for i in 0..SIZE {
                                let block = section.get_block(i);
                                if section.is_log(block) {
                                    queue[0].add(section.get_index() << SIZE_BITS | i);
                                } else if section.is_leaf(block) {
                                    let x = get_x(i);
                                    let z = get_z(i);
                                    sides |= get_side_mask(x == 0, x == 15, z == 0, z == 15);
                                }
                            }
                        }
                    }
                    for i in 1..DECAY_DISTANCE {
                        let positions: Vec<i32> = queue[(i - 1) as usize].iter().collect();
                        for pos_chunk in positions {
                            let x = get_x(pos_chunk);
                            let y = get_y(pos_chunk);
                            let z = get_z(pos_chunk);
                            for direction in DIRECTIONS {
                                let nx = x + direction[0];
                                let ny_chunk = y + direction[1];
                                let nz = z + direction[2];
                                if (0..=15).contains(&nx) && (0..=15).contains(&nz) && (0..=255).contains(&ny_chunk) {
                                    let Some(section) = section_map.get_mut(ny_chunk >> 4) else { continue };
                                    if !section.is_skippable() {
                                        let pos_section = get_index(nx, ny_chunk & 15, nz);
                                        let block = section.get_block(pos_section);
                                        if section.is_leaf(block) {
                                            let old_distance = section.get_distance(block);
                                            if old_distance > i {
                                                section.set_distance(pos_section, block, i);
                                                queue[i as usize].add(get_index(nx, ny_chunk, nz));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    sections.update_typed(&section_finder, None, |section| {
                        let y = section.get_remainder().get("Y").as_int(0);
                        section_map.get(y).expect("a leaves section").write(section)
                    })
                });
                if sides != 0 {
                    new_level = new_level.update_remainder(|tag| {
                        let upgrade_data = tag.get("UpgradeData").result().unwrap_or_else(|| tag.empty_map());
                        let sides_tag = tag.create_byte((upgrade_data.get("Sides").as_byte(0) as i32 | sides) as i8);
                        let upgrade_data = upgrade_data.set("Sides", sides_tag);
                        tag.set("UpgradeData", upgrade_data)
                    });
                }
                new_level
            })
        })
    }
}

pub fn get_index(x: i32, y: i32, z: i32) -> i32 {
    y << 8 | z << 4 | x
}

fn get_x(index: i32) -> i32 {
    index & 15
}

fn get_y(index: i32) -> i32 {
    index >> 8 & 0xFF
}

fn get_z(index: i32) -> i32 {
    index >> 4 & 15
}

pub fn get_side_mask(west: bool, east: bool, north: bool, south: bool) -> i32 {
    let mut s = 0;
    if north {
        if east {
            s |= 2;
        } else if west {
            s |= 128;
        } else {
            s |= 1;
        }
    } else if south {
        if west {
            s |= 32;
        } else if east {
            s |= 8;
        } else {
            s |= 16;
        }
    } else if east {
        s |= 4;
    } else if west {
        s |= 64;
    }
    s
}

pub struct LeavesSection {
    section: Section,
    leave_ids: IntOpenHashSet,
    log_ids: IntOpenHashSet,
    state_to_id_map: HashMap<i32, i32>,
}

impl LeavesSection {
    pub fn new(section: &Typed, input_schema: &Schema) -> LeavesSection {
        let mut leave_ids = IntOpenHashSet::new();
        let mut log_ids = IntOpenHashSet::new();
        let mut state_to_id_map = HashMap::new();
        let section = Section::new(section, input_schema, |palette| {
            for i in 0..palette.len() {
                let palette_tag = palette[i].clone();
                let block_name = palette_tag.get("Name").as_string_or("");
                if leaves(&block_name).is_some() {
                    let persistent = palette_tag.get("Properties").get("decayable").as_string_or("") == "false";
                    leave_ids.add(i as i32);
                    state_to_id_map.insert(get_state_id(&block_name, persistent, DECAY_DISTANCE), i as i32);
                    palette[i] = make_leaf_tag(&palette_tag, &block_name, persistent, DECAY_DISTANCE);
                }
                if LOGS.contains(&block_name.as_str()) {
                    log_ids.add(i as i32);
                }
            }
            leave_ids.is_empty() && log_ids.is_empty()
        });
        LeavesSection { section, leave_ids, log_ids, state_to_id_map }
    }

    pub fn is_log(&self, block: i32) -> bool {
        self.log_ids.contains(block)
    }

    pub fn is_leaf(&self, block: i32) -> bool {
        self.leave_ids.contains(block)
    }

    fn get_distance(&self, block: i32) -> i32 {
        if self.is_log(block) {
            0
        } else {
            let distance = self.section.palette[block as usize].get("Properties").get("distance").as_string_or("");
            java_parse_int(&distance).unwrap_or_else(|| panic!("For input string: \"{distance}\""))
        }
    }

    fn set_distance(&mut self, pos: i32, block: i32, distance: i32) {
        let base_tag = self.section.palette[block as usize].clone();
        let block_name = base_tag.get("Name").as_string_or("");
        let persistent = base_tag.get("Properties").get("persistent").as_string_or("") == "true";
        let state_id = get_state_id(&block_name, persistent, distance);
        if !self.state_to_id_map.contains_key(&state_id) {
            let id = self.section.palette.len() as i32;
            self.leave_ids.add(id);
            self.state_to_id_map.insert(state_id, id);
            self.section.palette.push(make_leaf_tag(&base_tag, &block_name, persistent, distance));
        }
        let id = self.state_to_id_map[&state_id];
        let storage = self.section.storage.as_mut().expect("a storage");
        if 1i32.wrapping_shl(storage.get_bits() as u32) <= id {
            let mut new_storage = PackedBitStorage::new(storage.get_bits() + 1, SIZE);
            for i in 0..SIZE {
                new_storage.set(i, storage.get(i));
            }
            *storage = new_storage;
        }
        storage.set(pos, id);
    }

    pub fn is_skippable(&self) -> bool {
        self.section.is_skippable()
    }

    pub fn get_block(&self, pos: i32) -> i32 {
        self.section.get_block(pos)
    }

    pub fn get_index(&self) -> i32 {
        self.section.get_index()
    }

    pub fn write(&self, section: Typed) -> Typed {
        self.section.write(section)
    }
}

fn make_leaf_tag(input: &Dynamic, block_name: &str, persistent: bool, distance: i32) -> Dynamic {
    let mut properties = input.empty_map();
    let persistent = properties.create_string(if persistent { "true" } else { "false" });
    properties = properties.set("persistent", persistent);
    let distance = properties.create_string(distance.to_string());
    properties = properties.set("distance", distance);
    let mut tag = input.empty_map();
    tag = tag.set("Properties", properties);
    let name = tag.create_string(block_name);
    tag.set("Name", name)
}

fn get_state_id(block_name: &str, persistent: bool, distance: i32) -> i32 {
    leaves(block_name).expect("a leaves block") << 5 | if persistent { 16 } else { 0 } | distance
}

pub struct Section {
    pub palette: Vec<Dynamic>,
    index: i32,
    pub storage: Option<PackedBitStorage>,
}

fn block_state_type() -> Arc<Type> {
    named(BLOCK_STATE.type_name(), remainder_type())
}

fn palette_finder() -> OpticFinder {
    field_finder("Palette", list(block_state_type()))
}

impl Section {
    pub fn new(section: &Typed, input_schema: &Schema, skippable: impl FnOnce(&mut Vec<Dynamic>) -> bool) -> Section {
        if !equals(&input_schema.get_type(BLOCK_STATE), &block_state_type(), false) {
            panic!("Block state type is not what was expected.");
        }
        let mut palette: Vec<Dynamic> = match section.get_optional(&palette_finder()) {
            Some(Value::List(entries)) => entries
                .into_iter()
                .map(|entry| match entry {
                    Value::Dynamic(dynamic) => dynamic,
                    other => panic!("Unexpected palette entry {other:?}"),
                })
                .collect(),
            _ => Vec::new(),
        };
        let tag = section.get_remainder();
        let index = tag.get("Y").as_int(0);
        let storage = if skippable(&mut palette) {
            None
        } else {
            let states = tag.get("BlockStates").as_long_stream();
            let size = 4.max(mth::ceillog2(palette.len() as i32));
            Some(PackedBitStorage::with_data(size, SIZE, states))
        };
        Section { palette, index, storage }
    }

    pub fn write(&self, section: Typed) -> Typed {
        match &self.storage {
            None => section,
            Some(storage) => {
                let block_states = storage.get_raw().to_vec();
                let palette_finder = palette_finder();
                let palette = Typed::new(palette_finder.ty(), Value::List(self.palette.iter().map(|block| Value::Dynamic(block.clone())).collect()));
                section.update_remainder(|tag| {
                    let states = tag.create_long_list(block_states.iter().copied());
                    tag.set("BlockStates", states)
                })
                .set(&palette_finder, palette)
            }
        }
    }

    pub fn is_skippable(&self) -> bool {
        self.storage.is_none()
    }

    pub fn get_block(&self, pos: i32) -> i32 {
        self.storage.as_ref().expect("a storage").get(pos)
    }

    pub fn get_index(&self) -> i32 {
        self.index
    }
}
