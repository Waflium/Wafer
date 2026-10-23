use std::borrow::Cow;

use wafer_core::{AABB, BlockPos, Direction, Vec3, mth};

use crate::fluid::{Fluid, FluidState};
use crate::properties::{NoteBlockInstrument, Property, PropertyId, PropertyValue};
use crate::{BLOCKS, Item, PROPERTIES, SHAPES, STATES, Tag};

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Block(pub u16);

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockState(pub u16);

pub type VoxelShape = &'static [AABB];

pub struct SoundType {
    pub volume: f32,
    pub pitch: f32,
    pub break_sound: i32,
    pub step_sound: i32,
    pub place_sound: i32,
    pub hit_sound: i32,
    pub fall_sound: i32,
}

pub struct BlockInfo {
    pub name: &'static str,
    pub description_id: &'static str,
    pub destroy_time: f32,
    pub explosion_resistance: f32,
    pub friction: f32,
    pub speed_factor: f32,
    pub jump_factor: f32,
    pub bounce_restitution: f32,
    pub fall_distance_reduction: f32,
    pub has_collision: bool,
    pub dynamic_shape: bool,
    pub item: Item,
    pub first_state: u16,
    pub state_count: u16,
    pub default_state: BlockState,
    pub loot_table: Option<&'static str>,
    pub sound: SoundType,
    pub classes: &'static [&'static str],
    pub fields: &'static str,
    pub properties: &'static [BlockProperty],
    pub offset_type: OffsetType,
    pub max_horizontal_offset: f32,
    pub max_vertical_offset: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OffsetType {
    None,
    Xz,
    Xyz,
}

pub struct BlockProperty {
    pub id: PropertyId,
    pub stride: u16,
    pub count: u16,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SupportType {
    Full,
    Center,
    Rigid,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PushReaction {
    PushPull,
    Push,
    Popped,
    Immoveable,
    IgnoreEntity,
}

pub(crate) struct StateData {
    pub block: u16,
    pub flags: u32,
    pub light_emission: u8,
    pub light_dampening: u8,
    pub face_sturdy: u32,
    pub push_reaction: u8,
    pub map_color: u8,
    pub instrument: u8,
    pub fluid: u8,
    pub fluid_amount: u8,
    pub shape: u16,
    pub collision: u16,
    pub occlusion: u16,
    pub support: u16,
    pub interaction: u16,
}

pub(crate) mod flags {
    pub const AIR: u32 = 1 << 0;
    pub const SOLID: u32 = 1 << 1;
    pub const SOLID_RENDER: u32 = 1 << 2;
    pub const CAN_OCCLUDE: u32 = 1 << 3;
    pub const REPLACEABLE: u32 = 1 << 4;
    pub const REQUIRES_TOOL: u32 = 1 << 5;
    pub const PROPAGATES_SKYLIGHT: u32 = 1 << 6;
    pub const USE_SHAPE_FOR_LIGHT_OCCLUSION: u32 = 1 << 7;
    pub const RANDOM_TICKING: u32 = 1 << 8;
    pub const REDSTONE_CONDUCTOR: u32 = 1 << 9;
    pub const SUFFOCATING: u32 = 1 << 10;
    pub const SIGNAL_SOURCE: u32 = 1 << 11;
    pub const ANALOG_OUTPUT: u32 = 1 << 12;
    pub const BLOCK_ENTITY: u32 = 1 << 13;
    pub const IGNITED_BY_LAVA: u32 = 1 << 14;
    pub const LIQUID: u32 = 1 << 15;
    pub const LARGE_COLLISION: u32 = 1 << 16;
    pub const COLLISION_FULL_BLOCK: u32 = 1 << 17;
    pub const HAS_OFFSET: u32 = 1 << 18;
    pub const FLUID_FALLING: u32 = 1 << 19;
    pub const SUFFOCATING_BY_TAG: u32 = 1 << 20;
    pub const PATHFINDABLE_LAND: u32 = 1 << 21;
    pub const PATHFINDABLE_WATER: u32 = 1 << 22;
    pub const SHAPE_OFFSET: u32 = 1 << 23;
    pub const COLLISION_OFFSET: u32 = 1 << 24;
    pub const SUPPORT_OFFSET: u32 = 1 << 25;
    pub const DYNAMIC_SHAPE: u32 = 1 << 26;
}

impl Block {
    pub fn info(self) -> &'static BlockInfo {
        &BLOCKS[self.0 as usize]
    }

    pub fn name(self) -> &'static str {
        self.info().name
    }

    pub fn by_name(name: &str) -> Option<Block> {
        BLOCKS.iter().position(|b| b.name == name).map(|i| Block(i as u16))
    }

    pub fn default_state(self) -> BlockState {
        self.info().default_state
    }

    pub fn item(self) -> Item {
        self.info().item
    }

    pub fn possible_states(self) -> impl Iterator<Item = BlockState> {
        let info = self.info();
        (info.first_state..info.first_state + info.state_count).map(BlockState)
    }

    pub fn is_instance_of(self, class: &str) -> bool {
        self.info().classes.contains(&class)
    }

    pub fn is(self, tag: &Tag) -> bool {
        tag.contains(self.0 as usize)
    }

    pub fn count() -> usize {
        BLOCKS.len()
    }

    pub fn get_max_horizontal_offset(self) -> f32 {
        self.info().max_horizontal_offset
    }

    pub fn get_max_vertical_offset(self) -> f32 {
        self.info().max_vertical_offset
    }

    pub fn property_by_name(self, name: &str) -> Option<(&'static str, &'static [&'static str])> {
        let p = self.info().properties.iter().find(|p| PROPERTIES[p.id.0 as usize].name == name)?;
        let data = &PROPERTIES[p.id.0 as usize];
        Some((data.name, data.values))
    }
}

impl std::fmt::Debug for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl BlockState {
    fn data(self) -> &'static StateData {
        &STATES[self.0 as usize]
    }

    pub fn by_id(id: u32) -> Option<BlockState> {
        (id < STATES.len() as u32).then_some(BlockState(id as u16))
    }

    pub fn id(self) -> u32 {
        self.0 as u32
    }

    pub fn count() -> usize {
        STATES.len()
    }

    pub fn block(self) -> Block {
        Block(self.data().block)
    }

    pub fn is_block(self, block: Block) -> bool {
        self.data().block == block.0
    }

    pub fn is(self, tag: &Tag) -> bool {
        tag.contains(self.data().block as usize)
    }

    fn flag(self, flag: u32) -> bool {
        self.data().flags & flag != 0
    }

    pub fn is_air(self) -> bool {
        self.flag(flags::AIR)
    }
    pub fn is_solid(self) -> bool {
        self.flag(flags::SOLID)
    }
    pub fn is_solid_render(self) -> bool {
        self.flag(flags::SOLID_RENDER)
    }
    pub fn can_occlude(self) -> bool {
        self.flag(flags::CAN_OCCLUDE)
    }
    pub fn can_be_replaced(self) -> bool {
        self.flag(flags::REPLACEABLE)
    }
    pub fn requires_correct_tool_for_drops(self) -> bool {
        self.flag(flags::REQUIRES_TOOL)
    }
    pub fn propagates_skylight_down(self) -> bool {
        self.flag(flags::PROPAGATES_SKYLIGHT)
    }
    pub fn use_shape_for_light_occlusion(self) -> bool {
        self.flag(flags::USE_SHAPE_FOR_LIGHT_OCCLUSION)
    }
    pub fn is_randomly_ticking(self) -> bool {
        self.flag(flags::RANDOM_TICKING)
    }
    pub fn is_redstone_conductor(self) -> bool {
        self.flag(flags::REDSTONE_CONDUCTOR)
    }
    pub fn is_suffocating(self) -> bool {
        if self.flag(flags::SUFFOCATING_BY_TAG) {
            self.is(&crate::block_tags::CAUSES_SUFFOCATION) && self.is_collision_shape_full_block()
        } else {
            self.flag(flags::SUFFOCATING)
        }
    }
    pub fn is_signal_source(self) -> bool {
        self.flag(flags::SIGNAL_SOURCE)
    }
    pub fn has_analog_output_signal(self) -> bool {
        self.flag(flags::ANALOG_OUTPUT)
    }
    pub fn has_block_entity(self) -> bool {
        self.flag(flags::BLOCK_ENTITY)
    }
    pub fn ignited_by_lava(self) -> bool {
        self.flag(flags::IGNITED_BY_LAVA)
    }
    pub fn liquid(self) -> bool {
        self.flag(flags::LIQUID)
    }
    pub fn has_large_collision_shape(self) -> bool {
        self.flag(flags::LARGE_COLLISION)
    }
    pub fn is_collision_shape_full_block(self) -> bool {
        self.flag(flags::COLLISION_FULL_BLOCK)
    }
    pub fn has_offset_function(self) -> bool {
        self.flag(flags::HAS_OFFSET)
    }

    pub fn get_offset(self, pos: BlockPos) -> Vec3 {
        let block = self.block();
        match block.info().offset_type {
            OffsetType::None => Vec3::ZERO,
            OffsetType::Xz => {
                let seed = mth::get_seed(pos.x, 0, pos.z);
                let max_horizontal_offset = block.get_max_horizontal_offset();
                let x = mth::clamp((((seed & 15) as f32 / 15.0) as f64 - 0.5) * 0.5, -max_horizontal_offset as f64, max_horizontal_offset as f64);
                let z = mth::clamp((((seed >> 8 & 15) as f32 / 15.0) as f64 - 0.5) * 0.5, -max_horizontal_offset as f64, max_horizontal_offset as f64);
                Vec3::new(x, 0.0, z)
            }
            OffsetType::Xyz => {
                let seed = mth::get_seed(pos.x, 0, pos.z);
                let y = (((seed >> 4 & 15) as f32 / 15.0) as f64 - 1.0) * block.get_max_vertical_offset() as f64;
                let max_horizontal_offset = block.get_max_horizontal_offset();
                let x = mth::clamp((((seed & 15) as f32 / 15.0) as f64 - 0.5) * 0.5, -max_horizontal_offset as f64, max_horizontal_offset as f64);
                let z = mth::clamp((((seed >> 8 & 15) as f32 / 15.0) as f64 - 0.5) * 0.5, -max_horizontal_offset as f64, max_horizontal_offset as f64);
                Vec3::new(x, y, z)
            }
        }
    }

    fn offset_if(self, flag: u32, pos: BlockPos) -> Vec3 {
        if self.flag(flag) { self.get_offset(pos) } else { Vec3::ZERO }
    }

    pub fn shape_offset(self, pos: BlockPos) -> Vec3 {
        self.offset_if(flags::SHAPE_OFFSET, pos)
    }

    pub fn collision_offset(self, pos: BlockPos) -> Vec3 {
        self.offset_if(flags::COLLISION_OFFSET, pos)
    }

    pub fn support_offset(self, pos: BlockPos) -> Vec3 {
        self.offset_if(flags::SUPPORT_OFFSET, pos)
    }

    fn moved_by_offset(self, shape: &'static wafer_core::shapes::VoxelShape, flag: u32, pos: BlockPos) -> Cow<'static, wafer_core::shapes::VoxelShape> {
        if self.flag(flag) && !shape.is_empty() {
            let offset = self.get_offset(pos);
            Cow::Owned(shape.moved(offset.x, offset.y, offset.z))
        } else {
            Cow::Borrowed(shape)
        }
    }

    pub fn get_shape(self, pos: BlockPos) -> Cow<'static, wafer_core::shapes::VoxelShape> {
        self.moved_by_offset(self.voxel_shape(), flags::SHAPE_OFFSET, pos)
    }

    pub fn get_collision_shape(self, pos: BlockPos) -> Cow<'static, wafer_core::shapes::VoxelShape> {
        self.moved_by_offset(self.collision_voxel_shape(), flags::COLLISION_OFFSET, pos)
    }

    pub fn get_block_support_shape(self, pos: BlockPos) -> Cow<'static, wafer_core::shapes::VoxelShape> {
        self.moved_by_offset(self.support_voxel_shape(), flags::SUPPORT_OFFSET, pos)
    }
    pub fn is_pathfindable_land(self) -> bool {
        self.flag(flags::PATHFINDABLE_LAND)
    }
    pub fn is_pathfindable_water(self) -> bool {
        self.flag(flags::PATHFINDABLE_WATER)
    }

    pub fn light_emission(self) -> u8 {
        self.data().light_emission
    }

    pub fn light_dampening(self) -> u8 {
        self.data().light_dampening
    }

    pub fn is_face_sturdy_cached(self, direction: Direction, support: SupportType) -> bool {
        self.data().face_sturdy & (1 << (direction as u32 * 3 + support as u32)) != 0
    }

    pub fn has_dynamic_shape(self) -> bool {
        self.flag(flags::DYNAMIC_SHAPE)
    }

    pub fn push_reaction(self) -> PushReaction {
        [PushReaction::PushPull, PushReaction::Push, PushReaction::Popped, PushReaction::Immoveable, PushReaction::IgnoreEntity]
            [self.data().push_reaction as usize]
    }

    pub fn map_color(self) -> u8 {
        self.data().map_color
    }

    pub fn instrument(self) -> NoteBlockInstrument {
        NoteBlockInstrument::from_ordinal(self.data().instrument)
    }

    pub fn fluid_state(self) -> FluidState {
        let d = self.data();
        FluidState { fluid: Fluid::from_id(d.fluid), amount: d.fluid_amount, falling: d.flags & flags::FLUID_FALLING != 0 }
    }

    pub fn shape(self) -> VoxelShape {
        SHAPES[self.data().shape as usize]
    }

    pub fn collision_shape(self) -> VoxelShape {
        SHAPES[self.data().collision as usize]
    }

    pub fn occlusion_shape(self) -> VoxelShape {
        SHAPES[self.data().occlusion as usize]
    }

    pub fn block_support_shape(self) -> VoxelShape {
        SHAPES[self.data().support as usize]
    }

    pub fn collision_voxel_shape(self) -> &'static wafer_core::shapes::VoxelShape {
        voxel(self.data().collision)
    }

    pub fn voxel_shape(self) -> &'static wafer_core::shapes::VoxelShape {
        voxel(self.data().shape)
    }

    pub fn support_voxel_shape(self) -> &'static wafer_core::shapes::VoxelShape {
        voxel(self.data().support)
    }

    pub fn occlusion_voxel_shape(self) -> &'static wafer_core::shapes::VoxelShape {
        voxel(self.data().occlusion)
    }

    pub fn interaction_shape(self) -> VoxelShape {
        SHAPES[self.data().interaction as usize]
    }

    pub fn destroy_speed(self) -> f32 {
        self.block().info().destroy_time
    }

    pub fn sound_type(self) -> &'static SoundType {
        &self.block().info().sound
    }

    fn property(self, id: PropertyId) -> Option<&'static BlockProperty> {
        self.block().info().properties.iter().find(|p| p.id == id)
    }

    fn value_index(self, p: &BlockProperty) -> u16 {
        (self.0 - self.block().info().first_state) / p.stride % p.count
    }

    pub fn has<T>(self, property: Property<T>) -> bool {
        self.property(property.id).is_some()
    }

    pub fn try_get<T: PropertyValue>(self, property: Property<T>) -> Option<T> {
        let p = self.property(property.id)?;
        Some(T::from_ordinal(PROPERTIES[p.id.0 as usize].ordinals[self.value_index(p) as usize]))
    }

    pub fn get<T: PropertyValue>(self, property: Property<T>) -> T {
        match self.try_get(property) {
            Some(v) => v,
            None => panic!("Cannot get property {} as it does not exist in {:?}", PROPERTIES[property.id.0 as usize].name, self.block()),
        }
    }

    pub fn set<T: PropertyValue>(self, property: Property<T>, value: T) -> BlockState {
        let Some(p) = self.property(property.id) else {
            panic!("Cannot set property {} as it does not exist in {:?}", PROPERTIES[property.id.0 as usize].name, self.block());
        };
        let ordinals = PROPERTIES[p.id.0 as usize].ordinals;
        let Some(new) = ordinals.iter().position(|&o| o == value.ordinal()) else {
            panic!("Cannot set property {} to an unsupported value", PROPERTIES[p.id.0 as usize].name);
        };
        let old = self.value_index(p);
        BlockState(self.0 - old * p.stride + new as u16 * p.stride)
    }

    pub fn try_set<T: PropertyValue>(self, property: Property<T>, value: T) -> BlockState {
        if self.has(property) { self.set(property, value) } else { self }
    }

    pub fn cycle<T>(self, property: Property<T>) -> BlockState {
        let p = self.property(property.id).expect("property of this block");
        let old = self.value_index(p);
        let new = (old + 1) % p.count;
        BlockState(self.0 - old * p.stride + new * p.stride)
    }

    pub fn with_properties_of(self, source: BlockState) -> BlockState {
        let mut result = self;
        for p in source.block().info().properties {
            if let Some(target) = result.property(p.id) {
                let old = result.value_index(target);
                result = BlockState(result.0 - old * target.stride + source.value_index(p) * target.stride);
            }
        }
        result
    }

    pub fn property_value_by_name(self, name: &str) -> Option<&'static str> {
        let p = self.block().info().properties.iter().find(|p| PROPERTIES[p.id.0 as usize].name == name)?;
        Some(PROPERTIES[p.id.0 as usize].values[self.value_index(p) as usize])
    }

    pub fn with_property_by_name(self, name: &str, value: &str) -> Option<BlockState> {
        let p = self.block().info().properties.iter().find(|p| PROPERTIES[p.id.0 as usize].name == name)?;
        let new = PROPERTIES[p.id.0 as usize].values.iter().position(|v| *v == value)? as u16;
        let old = self.value_index(p);
        Some(BlockState(self.0 - old * p.stride + new * p.stride))
    }

    pub fn properties(self) -> impl Iterator<Item = (&'static str, &'static str)> {
        self.block().info().properties.iter().map(move |p| {
            let data = &PROPERTIES[p.id.0 as usize];
            (data.name, data.values[self.value_index(p) as usize])
        })
    }
}

fn voxel(index: u16) -> &'static wafer_core::shapes::VoxelShape {
    static CACHE: std::sync::LazyLock<Vec<std::sync::OnceLock<wafer_core::shapes::VoxelShape>>> =
        std::sync::LazyLock::new(|| (0..SHAPES.len()).map(|_| std::sync::OnceLock::new()).collect());
    CACHE[index as usize].get_or_init(|| wafer_core::shapes::VoxelShape::from_boxes(SHAPES[index as usize]))
}

impl std::fmt::Debug for BlockState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.block().name())?;
        let mut props = self.properties().peekable();
        if props.peek().is_some() {
            f.write_str("[")?;
            for (i, (name, value)) in props.enumerate() {
                if i > 0 {
                    f.write_str(",")?;
                }
                write!(f, "{name}={value}")?;
            }
            f.write_str("]")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::properties::{HALF, HORIZONTAL_FACING, Half, SLAB_TYPE, SNOWY, STAIRS_SHAPE, SlabType, StairsShape, WATERLOGGED};
    use super::{Block, BlockState, OffsetType};
    use crate::{SupportType, blocks};
    use wafer_core::{BlockPos, Direction, Vec3};

    #[test]
    fn properties_get_and_set() {
        let grass = blocks::GRASS_BLOCK.default_state();
        assert!(!grass.get(SNOWY));
        assert!(grass.set(SNOWY, true).get(SNOWY));
        assert_eq!(grass.set(SNOWY, true).set(SNOWY, false), grass);

        let stairs = blocks::OAK_STAIRS.default_state();
        assert_eq!(stairs.get(HORIZONTAL_FACING), Direction::North);
        assert_eq!(stairs.get(HALF), Half::Bottom);
        assert_eq!(stairs.get(STAIRS_SHAPE), StairsShape::Straight);
        let turned = stairs.set(HORIZONTAL_FACING, Direction::East).set(HALF, Half::Top);
        assert_eq!(turned.get(HORIZONTAL_FACING), Direction::East);
        assert_eq!(turned.get(HALF), Half::Top);
        assert!(!turned.get(WATERLOGGED));
        assert_eq!(format!("{turned:?}"), "minecraft:oak_stairs[facing=east,half=top,shape=straight,waterlogged=false]");
        assert!(!blocks::STONE.default_state().has(SNOWY));
    }

    #[test]
    fn cached_state_values() {
        let stone = blocks::STONE.default_state();
        assert!(stone.is_face_sturdy_cached(Direction::Up, SupportType::Full));
        assert!(stone.is_redstone_conductor());
        let slab = blocks::OAK_SLAB.default_state();
        assert_eq!(slab.get(SLAB_TYPE), SlabType::Bottom);
        assert!(!slab.is_face_sturdy_cached(Direction::Up, SupportType::Full));
        assert!(slab.is_face_sturdy_cached(Direction::Down, SupportType::Full));
        assert!(blocks::WATER.default_state().fluid_state().is_source());
        assert!(blocks::AIR.default_state().is_air());
    }

    const EXTREMES: [i32; 17] = [-30000000, -29999999, -16777217, -1048577, -65536, -4097, -1, 0, 1, 4096, 65535, 1048576, 16777216, 29999999, 30000000, i32::MIN, i32::MAX];

    fn positions() -> Vec<BlockPos> {
        let mut out = Vec::new();
        for x in -64..=64 {
            for z in -64..=64 {
                out.push(BlockPos::new(x, (x * 7 + z * 13) % 300, z));
            }
        }
        for x in EXTREMES {
            for z in EXTREMES {
                out.push(BlockPos::new(x, 64, z));
            }
        }
        out
    }

    fn mix(hash: i64, value: f64) -> i64 {
        ((hash ^ value.to_bits() as i64).wrapping_mul(0x9E3779B97F4A7C15u64 as i64)).rotate_left(27)
    }

    fn shape_hash(mut hash: i64, shape: &wafer_core::shapes::VoxelShape) -> i64 {
        for b in shape.to_aabbs() {
            hash = mix(mix(mix(mix(mix(mix(hash, b.min_x), b.min_y), b.min_z), b.max_x), b.max_y), b.max_z);
        }
        hash.wrapping_mul(31).wrapping_add(7)
    }

    #[test]
    fn offsets_match_vanilla() {
        let positions = positions();
        let mut offset_blocks = 0;
        for line in include_str!("offset_oracle.txt").lines() {
            let parts: Vec<&str> = line.split(' ').collect();
            let expected = u64::from_str_radix(parts[2], 16).unwrap() as i64;
            match parts[0] {
                "offset" => {
                    let state = Block::by_name(parts[1]).unwrap().default_state();
                    let hash = positions.iter().fold(0, |hash, &pos| {
                        let offset = state.get_offset(pos);
                        mix(mix(mix(hash, offset.x), offset.y), offset.z)
                    });
                    assert_eq!(hash, expected, "{}", parts[1]);
                    offset_blocks += 1;
                }
                _ => {
                    let state = BlockState(parts[1].parse().unwrap());
                    let shapes = positions.iter().fold(0, |hash, &pos| shape_hash(hash, &state.get_shape(pos)));
                    let collisions = positions.iter().fold(0, |hash, &pos| shape_hash(hash, &state.get_collision_shape(pos)));
                    assert_eq!(shapes, expected, "shape of {state:?}");
                    assert_eq!(collisions, u64::from_str_radix(parts[3], 16).unwrap() as i64, "collision of {state:?}");
                }
            }
        }
        assert_eq!(offset_blocks, (0..Block::count()).filter(|&b| Block(b as u16).default_state().has_offset_function()).count());
        for state in (0..BlockState::count()).map(|id| BlockState(id as u16)) {
            assert_eq!(state.has_offset_function(), state.block().info().offset_type != OffsetType::None, "{state:?}");
        }
    }

    #[test]
    fn bamboo_collision_moves_by_offset() {
        let bamboo = blocks::BAMBOO.default_state();
        let pos = BlockPos::new(3, 64, -7);
        let offset = bamboo.get_offset(pos);
        assert_ne!(offset, Vec3::ZERO);
        let shape = bamboo.get_collision_shape(pos);
        assert_eq!(shape.min(wafer_core::Axis::X), 6.5 / 16.0 + offset.x);
        assert_eq!(shape.min(wafer_core::Axis::Z), 6.5 / 16.0 + offset.z);
        assert_eq!(blocks::SHORT_GRASS.default_state().get_shape(pos).min(wafer_core::Axis::X), 2.0 / 16.0);
    }
}
