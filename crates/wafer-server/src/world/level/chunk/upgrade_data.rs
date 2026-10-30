use std::cell::RefCell;
use std::sync::LazyLock;

use wafer_core::fastutil::OpenHashMap;
use wafer_core::{BlockPos, ChunkPos, Direction};
use wafer_data::properties::{AGE_7, CHEST_TYPE, ChestType, DISTANCE, HORIZONTAL_FACING};
use wafer_data::{Block, BlockState, Fluid, blocks};
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::level::block::{self, entity::BlockEntityKind};
use crate::world::level::chunk::block_entities::EntityCreationType;
use crate::world::level::BlockGetter;
use crate::world::level::chunk::serialization;
use crate::world::level::levelgen::blending::blending_data::Direction8;
use crate::world::ticks::SavedTick;

#[derive(Clone, Copy, PartialEq, Eq)]
enum BlockFixer {
    Blacklist,
    Default,
    Chest,
    Leaves,
    StemBlock,
}

const BLACKLIST: &[Block] = &[
    blocks::OBSERVER,
    blocks::NETHER_PORTAL,
    blocks::WHITE_CONCRETE_POWDER,
    blocks::ORANGE_CONCRETE_POWDER,
    blocks::MAGENTA_CONCRETE_POWDER,
    blocks::LIGHT_BLUE_CONCRETE_POWDER,
    blocks::YELLOW_CONCRETE_POWDER,
    blocks::LIME_CONCRETE_POWDER,
    blocks::PINK_CONCRETE_POWDER,
    blocks::GRAY_CONCRETE_POWDER,
    blocks::LIGHT_GRAY_CONCRETE_POWDER,
    blocks::CYAN_CONCRETE_POWDER,
    blocks::PURPLE_CONCRETE_POWDER,
    blocks::BLUE_CONCRETE_POWDER,
    blocks::BROWN_CONCRETE_POWDER,
    blocks::GREEN_CONCRETE_POWDER,
    blocks::RED_CONCRETE_POWDER,
    blocks::BLACK_CONCRETE_POWDER,
    blocks::ANVIL,
    blocks::CHIPPED_ANVIL,
    blocks::DAMAGED_ANVIL,
    blocks::DRAGON_EGG,
    blocks::GRAVEL,
    blocks::SAND,
    blocks::RED_SAND,
    blocks::OAK_SIGN,
    blocks::SPRUCE_SIGN,
    blocks::BIRCH_SIGN,
    blocks::ACACIA_SIGN,
    blocks::CHERRY_SIGN,
    blocks::JUNGLE_SIGN,
    blocks::DARK_OAK_SIGN,
    blocks::PALE_OAK_SIGN,
    blocks::POPLAR_SIGN,
    blocks::OAK_WALL_SIGN,
    blocks::SPRUCE_WALL_SIGN,
    blocks::BIRCH_WALL_SIGN,
    blocks::ACACIA_WALL_SIGN,
    blocks::JUNGLE_WALL_SIGN,
    blocks::DARK_OAK_WALL_SIGN,
    blocks::PALE_OAK_WALL_SIGN,
    blocks::POPLAR_WALL_SIGN,
    blocks::OAK_HANGING_SIGN,
    blocks::SPRUCE_HANGING_SIGN,
    blocks::BIRCH_HANGING_SIGN,
    blocks::ACACIA_HANGING_SIGN,
    blocks::JUNGLE_HANGING_SIGN,
    blocks::DARK_OAK_HANGING_SIGN,
    blocks::PALE_OAK_HANGING_SIGN,
    blocks::POPLAR_HANGING_SIGN,
    blocks::OAK_WALL_HANGING_SIGN,
    blocks::SPRUCE_WALL_HANGING_SIGN,
    blocks::BIRCH_WALL_HANGING_SIGN,
    blocks::ACACIA_WALL_HANGING_SIGN,
    blocks::JUNGLE_WALL_HANGING_SIGN,
    blocks::DARK_OAK_WALL_HANGING_SIGN,
    blocks::PALE_OAK_WALL_HANGING_SIGN,
    blocks::POPLAR_WALL_HANGING_SIGN,
];

const LEAVES: &[Block] = &[
    blocks::ACACIA_LEAVES,
    blocks::CHERRY_LEAVES,
    blocks::BIRCH_LEAVES,
    blocks::PALE_OAK_LEAVES,
    blocks::RED_POPLAR_LEAVES,
    blocks::ORANGE_POPLAR_LEAVES,
    blocks::YELLOW_POPLAR_LEAVES,
    blocks::DARK_OAK_LEAVES,
    blocks::JUNGLE_LEAVES,
    blocks::OAK_LEAVES,
    blocks::SPRUCE_LEAVES,
];

static MAP: LazyLock<Vec<BlockFixer>> = LazyLock::new(|| {
    let mut map = vec![BlockFixer::Default; Block::count()];
    for (fixer, blocks) in [
        (BlockFixer::Blacklist, BLACKLIST),
        (BlockFixer::Chest, &[blocks::CHEST, blocks::TRAPPED_CHEST][..]),
        (BlockFixer::Leaves, LEAVES),
        (BlockFixer::StemBlock, &[blocks::MELON_STEM, blocks::PUMPKIN_STEM][..]),
    ] {
        for block in blocks {
            map[block.0 as usize] = fixer;
        }
    }
    map
});

type LeavesQueue = Vec<OpenHashMap<BlockPos, ()>>;

thread_local! {
    static LEAVES_QUEUE: RefCell<LeavesQueue> = RefCell::new(Vec::with_capacity(7));
}

#[derive(Clone, Default)]
pub struct UpgradeData {
    sides: u8,
    neighbor_block_ticks: Vec<SavedTick<Block>>,
    neighbor_fluid_ticks: Vec<SavedTick<Fluid>>,
    index: Vec<Option<Vec<i32>>>,
}

impl UpgradeData {
    pub fn new(tag: &Nbt, sections_count: usize) -> UpgradeData {
        let mut data = UpgradeData { index: vec![None; sections_count], ..UpgradeData::default() };
        if let Some(indices_tag @ Nbt::Compound(_)) = tag.get("Indices") {
            for (i, slot) in data.index.iter_mut().enumerate() {
                if let Some(Nbt::IntArray(indices)) = indices_tag.get(&i.to_string()) {
                    *slot = Some(indices.clone());
                }
            }
        }
        data.sides = tag.get("Sides").and_then(Nbt::as_i64).map_or(0, |sides| sides as i32 as u8);
        data.neighbor_block_ticks = serialization::saved_ticks_from_nbt(tag.get("neighbor_block_ticks"), |name| Some(name?.as_str().and_then(Block::by_name).unwrap_or(blocks::AIR)));
        data.neighbor_fluid_ticks = serialization::saved_ticks_from_nbt(tag.get("neighbor_fluid_ticks"), |name| Some(name?.as_str().and_then(serialization::fluid_by_name).unwrap_or(Fluid::Empty)));
        data
    }

    pub fn is_empty(&self) -> bool {
        self.index.iter().all(Option::is_none) && self.sides == 0
    }

    pub fn write(&self) -> Nbt {
        let mut tag: Vec<(String, Nbt)> = Vec::new();
        let indices_tag: Vec<(String, Nbt)> =
            self.index.iter().enumerate().filter_map(|(i, indices)| indices.as_ref().filter(|indices| !indices.is_empty()).map(|indices| (i.to_string(), Nbt::IntArray(indices.clone())))).collect();
        if !indices_tag.is_empty() {
            tag.push(("Indices".to_string(), Nbt::Compound(indices_tag.into())));
        }
        tag.push(("Sides".to_string(), Nbt::Byte(self.sides as i8)));
        if !self.neighbor_block_ticks.is_empty() {
            tag.push(("neighbor_block_ticks".to_string(), serialization::saved_ticks_to_nbt(&self.neighbor_block_ticks, Block::name)));
        }
        if !self.neighbor_fluid_ticks.is_empty() {
            tag.push(("neighbor_fluid_ticks".to_string(), serialization::saved_ticks_to_nbt(&self.neighbor_fluid_ticks, |fluid| wafer_data::registry::FLUID[fluid as usize])));
        }
        Nbt::Compound(tag.into())
    }

    pub fn upgrade(level: &mut ServerLevel, chunk_pos: ChunkPos) {
        let mut data = std::mem::take(&mut level.chunks.loaded_full_chunk(chunk_pos).upgrade_data);
        data.upgrade_inside(level, chunk_pos);
        for direction8 in Direction8::VALUES {
            data.upgrade_sides(level, chunk_pos, direction8);
        }
        for tick in &data.neighbor_block_ticks {
            let ty = if tick.ty == blocks::AIR { level.block_state(tick.pos).block() } else { tick.ty };
            level.schedule_tick_with_priority(tick.pos, ty, tick.delay, tick.priority);
        }
        for tick in &data.neighbor_fluid_ticks {
            let ty = if tick.ty == Fluid::Empty { level.fluid_state(tick.pos).fluid } else { tick.ty };
            level.schedule_fluid_tick_with_priority(tick.pos, ty, tick.delay, tick.priority);
        }
        process_leaves_chunk(level);
        level.chunks.loaded_full_chunk(chunk_pos).upgrade_data = data;
    }

    fn upgrade_sides(&mut self, level: &mut ServerLevel, chunk_pos: ChunkPos, direction8: Direction8) {
        let bit = 1u8 << direction8 as u8;
        if self.sides & bit == 0 {
            return;
        }
        self.sides &= !bit;
        let directions = direction8.directions();
        let east = directions.contains(&Direction::East);
        let west = directions.contains(&Direction::West);
        let south = directions.contains(&Direction::South);
        let north = directions.contains(&Direction::North);
        let singular = directions.len() == 1;
        let min_x = chunk_pos.min_block_x() + if !singular || (!north && !south) { if west { 0 } else { 15 } } else { 1 };
        let max_x = chunk_pos.min_block_x() + if !singular || (!north && !south) { if west { 0 } else { 15 } } else { 14 };
        let min_z = chunk_pos.min_block_z() + if !singular || (!east && !west) { if north { 0 } else { 15 } } else { 1 };
        let max_z = chunk_pos.min_block_z() + if !singular || (!east && !west) { if north { 0 } else { 15 } } else { 14 };
        let min_y = level.chunks.min_y();
        let max_y = min_y + level.chunks.height() - 1;
        for pos in BlockPos::between_closed(BlockPos::new(min_x, min_y, min_z), BlockPos::new(max_x, max_y, max_z)) {
            let state = level.block_state(pos);
            let mut new_state = state;
            for direction in Direction::VALUES {
                let neighbour_pos = pos.relative(direction);
                new_state = update_state(new_state, direction, level, pos, neighbour_pos);
            }
            level.update_or_destroy(state, new_state, pos, 18, 512);
        }
    }

    fn upgrade_inside(&mut self, level: &mut ServerLevel, chunk_pos: ChunkPos) {
        let min_section_y = level.chunks.min_y() >> 4;
        for section_index in 0..self.index.len() {
            let upgrade_index = self.index[section_index].take();
            let Some(upgrade_index) = upgrade_index.filter(|upgrade_index| !upgrade_index.is_empty()) else { continue };
            let section_y = min_section_y + section_index as i32;
            let bottom_y_in_section = section_y << 4;
            for coordinate in upgrade_index {
                let x = coordinate & 15;
                let y = (coordinate >> 8) & 15;
                let z = (coordinate >> 4) & 15;
                let pos = BlockPos::new(chunk_pos.min_block_x() + x, bottom_y_in_section + y, chunk_pos.min_block_z() + z);
                let state = level.chunks.loaded_full_chunk(chunk_pos).sections[section_index].states.get(x as usize, y as usize, z as usize);
                let mut new_state = state;
                for direction in Direction::VALUES {
                    let neighbour_pos = pos.relative(direction);
                    if (pos.x >> 4) == chunk_pos.x && (pos.z >> 4) == chunk_pos.z {
                        new_state = update_state(new_state, direction, level, pos, neighbour_pos);
                    }
                }
                level.update_or_destroy(state, new_state, pos, 18, 512);
            }
        }
        for (i, indices) in self.index.iter_mut().enumerate() {
            if indices.is_some() {
                warn!("Discarding update data for section {} for chunk ({} {})", min_section_y + i as i32, chunk_pos.x, chunk_pos.z);
            }
            *indices = None;
        }
    }
}

fn update_state(state: BlockState, direction: Direction, level: &mut ServerLevel, pos: BlockPos, neighbour_pos: BlockPos) -> BlockState {
    let neighbour = level.block_state(neighbour_pos);
    match MAP[state.block().0 as usize] {
        BlockFixer::Blacklist => state,
        BlockFixer::Default => block::of(state).update_shape(state, level, pos, direction, neighbour_pos, neighbour),
        BlockFixer::Chest => chest_update_shape(state, direction, neighbour, level, pos, neighbour_pos),
        BlockFixer::Leaves => leaves_update_shape(state, direction, level, pos, neighbour_pos, neighbour),
        BlockFixer::StemBlock => stem_update_shape(state, direction, neighbour),
    }
}

fn chest_update_shape(state: BlockState, direction: Direction, neighbour: BlockState, level: &mut ServerLevel, pos: BlockPos, neighbour_pos: BlockPos) -> BlockState {
    if neighbour.is_block(state.block())
        && direction.axis().is_horizontal()
        && state.get(CHEST_TYPE) == ChestType::Single
        && neighbour.get(CHEST_TYPE) == ChestType::Single
    {
        let facing = state.get(HORIZONTAL_FACING);
        if direction.axis() != facing.axis() && facing == neighbour.get(HORIZONTAL_FACING) {
            let new_type = if direction == facing.clockwise() { ChestType::Left } else { ChestType::Right };
            let opposite = if new_type == ChestType::Left { ChestType::Right } else { ChestType::Left };
            level.set_block(neighbour_pos, neighbour.set(CHEST_TYPE, opposite), 18);
            if facing == Direction::North || facing == Direction::East {
                let one = match level.chunks.block_entity(pos, EntityCreationType::Immediate).map(|entity| &mut entity.kind) {
                    Some(BlockEntityKind::Chest(chest)) => Some(chest.base.items.clone()),
                    _ => None,
                };
                let two = match level.chunks.block_entity(neighbour_pos, EntityCreationType::Immediate).map(|entity| &mut entity.kind) {
                    Some(BlockEntityKind::Chest(chest)) => Some(chest.base.items.clone()),
                    _ => None,
                };
                if let (Some(one_items), Some(two_items)) = (one, two) {
                    if let Some(BlockEntityKind::Chest(chest)) = level.chunks.block_entity(pos, EntityCreationType::Immediate).map(|entity| &mut entity.kind) {
                        chest.base.items = two_items;
                    }
                    if let Some(BlockEntityKind::Chest(chest)) = level.chunks.block_entity(neighbour_pos, EntityCreationType::Immediate).map(|entity| &mut entity.kind) {
                        chest.base.items = one_items;
                    }
                }
            }
            return state.set(CHEST_TYPE, new_type);
        }
    }
    state
}

fn leaves_update_shape(state: BlockState, direction: Direction, level: &mut ServerLevel, pos: BlockPos, neighbour_pos: BlockPos, neighbour: BlockState) -> BlockState {
    let new_state = block::of(state).update_shape(state, level, pos, direction, neighbour_pos, neighbour);
    if state != new_state {
        let distance = new_state.get(DISTANCE);
        LEAVES_QUEUE.with(|queue| {
            let mut queue = queue.borrow_mut();
            if queue.is_empty() {
                for _ in 0..7 {
                    queue.push(OpenHashMap::new());
                }
            }
            if distance < 0 || distance as usize >= queue.len() {
                panic!("java.lang.IndexOutOfBoundsException: Index {distance} out of bounds for length {}", queue.len());
            }
            queue[distance as usize].insert(pos, ());
        });
    }
    state
}

fn process_leaves_chunk(level: &mut ServerLevel) {
    let queue_size = LEAVES_QUEUE.with(|queue| queue.borrow().len());
    for neighbor_distance in 2..queue_size {
        let current_distance = neighbor_distance - 1;
        let positions: Vec<BlockPos> = LEAVES_QUEUE.with(|queue| queue.borrow()[current_distance].keys().collect());
        for pos in positions {
            let state = level.block_state(pos);
            if state.get(DISTANCE) >= current_distance as i32 {
                level.set_block(pos, state.set(DISTANCE, current_distance as i32), 18);
                if neighbor_distance != 7 {
                    for direction in Direction::VALUES {
                        let neighbor_pos = pos.relative(direction);
                        let neighbor = level.block_state(neighbor_pos);
                        if neighbor.try_get(DISTANCE).is_some() && state.get(DISTANCE) > neighbor_distance as i32 {
                            LEAVES_QUEUE.with(|queue| queue.borrow_mut()[neighbor_distance].insert(neighbor_pos, ()));
                        }
                    }
                }
            }
        }
    }
    LEAVES_QUEUE.with(|queue| queue.borrow_mut().clear());
}

fn stem_update_shape(state: BlockState, direction: Direction, neighbour: BlockState) -> BlockState {
    if state.get(AGE_7) == 7 {
        let fruit = if state.is_block(blocks::PUMPKIN_STEM) { blocks::PUMPKIN } else { blocks::MELON };
        if neighbour.is_block(fruit) {
            return (if state.is_block(blocks::PUMPKIN_STEM) { blocks::ATTACHED_PUMPKIN_STEM } else { blocks::ATTACHED_MELON_STEM }).default_state().set(HORIZONTAL_FACING, direction);
        }
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::ticks::TickPriority;

    fn tick(name: &str, x: i32, delay: i32) -> Nbt {
        Nbt::Compound(
            vec![
                ("i".to_string(), Nbt::String(name.to_string())),
                ("x".to_string(), Nbt::Int(x)),
                ("y".to_string(), Nbt::Int(64)),
                ("z".to_string(), Nbt::Int(7)),
                ("t".to_string(), Nbt::Int(delay)),
                ("p".to_string(), Nbt::Int(0)),
            ]
            .into(),
        )
    }

    fn upgrade_tag() -> Nbt {
        Nbt::Compound(
            vec![
                ("Indices".to_string(), Nbt::Compound(vec![("4".to_string(), Nbt::IntArray(vec![1, 256])), ("5".to_string(), Nbt::IntArray(Vec::new()))].into())),
                ("Sides".to_string(), Nbt::Byte(28)),
                ("neighbor_block_ticks".to_string(), Nbt::List(vec![tick("minecraft:stone", 3, 2), tick("minecraft:not_a_block", 4, 1)])),
                ("neighbor_fluid_ticks".to_string(), Nbt::List(vec![tick("minecraft:water", 3, 2)])),
            ]
            .into(),
        )
    }

    #[test]
    fn the_default_upgrade_data_is_empty() {
        let data = UpgradeData::default();
        assert!(data.is_empty());
        assert_eq!(data.write().get("Sides"), Some(&Nbt::Byte(0)));
    }

    #[test]
    fn indices_and_sides_decide_whether_the_data_is_empty() {
        let data = UpgradeData::new(&upgrade_tag(), 24);
        assert!(!data.is_empty());
        let only_ticks = Nbt::Compound(vec![("neighbor_block_ticks".to_string(), Nbt::List(vec![tick("minecraft:stone", 3, 2)]))].into());
        assert!(UpgradeData::new(&only_ticks, 24).is_empty());
    }

    #[test]
    fn upgrade_data_writes_what_it_read() {
        let written = UpgradeData::new(&upgrade_tag(), 24).write();
        let indices = written.get("Indices").expect("indices");
        assert_eq!(indices.get("4"), Some(&Nbt::IntArray(vec![1, 256])));
        assert!(indices.get("5").is_none());
        assert_eq!(written.get("Sides"), Some(&Nbt::Byte(28)));
        let block_ticks = written.get("neighbor_block_ticks").expect("block ticks").as_list();
        assert_eq!(block_ticks.len(), 2);
        assert_eq!(block_ticks[1].get("i").and_then(Nbt::as_str), Some("minecraft:air"));
        assert_eq!(written.get("neighbor_fluid_ticks").expect("fluid ticks").as_list().len(), 1);
    }

    #[test]
    fn neighbor_ticks_keep_their_priority() {
        let data = UpgradeData::new(&upgrade_tag(), 24);
        assert_eq!(data.neighbor_block_ticks[0].priority, TickPriority::NORMAL);
        assert_eq!(data.neighbor_block_ticks[0].delay, 2);
        assert_eq!(data.neighbor_fluid_ticks[0].ty, Fluid::Water);
    }

    #[test]
    fn indices_beyond_the_section_count_are_ignored() {
        let data = UpgradeData::new(&upgrade_tag(), 3);
        assert!(data.index.iter().all(Option::is_none));
    }
}
