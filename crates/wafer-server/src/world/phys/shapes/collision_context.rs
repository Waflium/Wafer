use std::sync::LazyLock;

use wafer_core::shapes::VoxelShape;
use wafer_core::{AABB, Axis, BlockPos};
use wafer_data::properties::{BOTTOM, LEVEL, RailShape, STABILITY_DISTANCE};
use wafer_data::{BlockState, FluidState, blocks};

use crate::server::level::ServerLevel;
use crate::world::entity::{Entity, EntityId, LevelEntity};

static EMPTY: LazyLock<VoxelShape> = LazyLock::new(VoxelShape::empty);
static BLOCK: LazyLock<VoxelShape> = LazyLock::new(VoxelShape::block);
static SCAFFOLDING_SHAPE_UNSTABLE_BOTTOM: LazyLock<VoxelShape> = LazyLock::new(|| VoxelShape::from_boxes(&[AABB::new(0.0, 0.0, 0.0, 1.0, 0.125, 1.0)]));
static POWDER_SNOW_FALLING_COLLISION_SHAPE: LazyLock<VoxelShape> =
    LazyLock::new(|| VoxelShape::from_boxes(&[AABB::new(0.0, 0.0, 0.0, 1.0, 0.9f32 as f64, 1.0)]));

#[derive(Clone, Copy, Debug)]
struct EntityProperties {
    fall_distance: f64,
    is_falling_block: bool,
    can_walk_on_powder_snow: bool,
    liquid_collision_shape: Option<&'static VoxelShape>,
    can_stand_on_lava: bool,
    is_player: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct CollisionContext {
    descending: bool,
    placement: bool,
    position: bool,
    entity_bottom: f64,
    entity: Option<EntityProperties>,
    ignore_below: Option<BlockPos>,
    slope_ignore: Option<BlockPos>,
}

impl CollisionContext {
    pub const EMPTY: CollisionContext =
        CollisionContext { descending: false, placement: false, position: false, entity_bottom: -f64::MAX, entity: None, ignore_below: None, slope_ignore: None };

    pub fn position_context(y: f64) -> CollisionContext {
        CollisionContext { position: true, entity_bottom: y, ..CollisionContext::EMPTY }
    }

    pub fn of(level: &ServerLevel, entity: EntityId) -> CollisionContext {
        let mut context = CollisionContext::new(level, entity, false, level.entity(entity).y());
        if level.is_minecart(entity) && crate::world::entity::vehicle::minecart::abstract_minecart::use_experimental_movement(level) {
            context.setup_minecart_context(level, entity);
        }
        context
    }

    fn setup_minecart_context(&mut self, level: &ServerLevel, minecart: EntityId) {
        let e = level.entity(minecart);
        let block_state = |pos: BlockPos| {
            level.chunks.chunk_without_loading(pos.chunk()).filter(|_| !level.chunks.is_outside_build_height(pos.y)).map_or(blocks::AIR.default_state(), |chunk| chunk.block_state(pos))
        };
        let (xt, mut yt, zt) = (wafer_core::mth::floor(e.x()), wafer_core::mth::floor(e.y()), wafer_core::mth::floor(e.z()));
        let y = e.y() - 0.1 - 1.0e-5f32 as f64;
        if block_state(BlockPos::containing(xt as f64, y, zt as f64)).is(&wafer_data::block_tags::RAILS) {
            yt = wafer_core::mth::floor(y);
        }
        let current_rail_pos = BlockPos::new(xt, yt, zt);
        let current_state = block_state(current_rail_pos);
        if crate::world::level::block::base_rail_block::is_rail(current_state) {
            self.ignore_below = Some(current_rail_pos.below());
            self.slope_ignore = match crate::world::entity::vehicle::minecart::abstract_minecart::rail_shape(current_state) {
                RailShape::AscendingEast => Some(current_rail_pos.east()),
                RailShape::AscendingWest => Some(current_rail_pos.west()),
                RailShape::AscendingNorth => Some(current_rail_pos.north()),
                RailShape::AscendingSouth => Some(current_rail_pos.south()),
                _ => None,
            };
        }
    }

    pub fn of_detached(entity: &Entity) -> CollisionContext {
        let properties = EntityProperties {
            fall_distance: entity.fall_distance,
            is_falling_block: false,
            can_walk_on_powder_snow: false,
            liquid_collision_shape: None,
            can_stand_on_lava: false,
            is_player: entity.entity_type == crate::world::entity::entity_types::PLAYER.id,
        };
        CollisionContext { descending: entity.is_shift_key_down(), entity_bottom: entity.y(), entity: Some(properties), ..CollisionContext::EMPTY }
    }

    pub fn placement_context(level: &ServerLevel, player: Option<EntityId>) -> CollisionContext {
        match player {
            Some(player) => CollisionContext::new(level, player, true, level.entity(player).y()),
            None => CollisionContext { placement: true, ..CollisionContext::EMPTY },
        }
    }

    pub fn with_position(level: &ServerLevel, entity: EntityId, position: f64) -> CollisionContext {
        CollisionContext::new(level, entity, true, position)
    }

    fn new(level: &ServerLevel, entity: EntityId, placement: bool, entity_bottom: f64) -> CollisionContext {
        let e = level.entity(entity);
        let properties = EntityProperties {
            fall_distance: e.fall_distance,
            is_falling_block: matches!(level.entities.get(&entity), Some(LevelEntity::FallingBlock(_))),
            can_walk_on_powder_snow: level.can_entity_walk_on_powder_snow(entity),
            liquid_collision_shape: level.get_liquid_collision_shape(entity),
            can_stand_on_lava: level.mob(entity).is_some_and(|mob| mob.can_stand_on_fluid(wafer_data::Fluid::Lava)),
            is_player: level.is_player(entity),
        };
        CollisionContext { descending: e.is_shift_key_down(), placement, entity_bottom, entity: Some(properties), ..CollisionContext::EMPTY }
    }

    pub fn is_descending(&self) -> bool {
        self.descending
    }

    pub fn is_placement(&self) -> bool {
        self.placement
    }

    pub fn is_player(&self) -> bool {
        self.entity.is_some_and(|entity| entity.is_player)
    }

    pub fn is_above(&self, shape: &VoxelShape, pos: BlockPos, default_value: bool) -> bool {
        self.is_above_max_y(shape.max(Axis::Y), pos, default_value)
    }

    fn is_above_max_y(&self, max_y: f64, pos: BlockPos, default_value: bool) -> bool {
        if self.entity.is_none() && !self.placement && !self.position {
            return default_value;
        }
        self.entity_bottom > pos.y as f64 + max_y - 1.0e-5f32 as f64
    }

    pub fn get_liquid_block_collision_shape(&self, state: BlockState, pos: BlockPos, fluid_above: impl FnOnce() -> FluidState) -> Option<&'static VoxelShape> {
        let entity = self.entity?;
        let liquid_stable_shape = entity.liquid_collision_shape?;
        if !(state.is_block(blocks::WATER) || state.is_block(blocks::LAVA)) || state.get(LEVEL) != 0 {
            return None;
        }
        let fluid = state.fluid_state().fluid;
        let can_stand_on_fluid = entity.can_stand_on_lava && fluid.is(&wafer_data::fluid_tags::LAVA) && !fluid_above().fluid.is_same(fluid);
        (self.is_above(liquid_stable_shape, pos, true) && can_stand_on_fluid).then_some(liquid_stable_shape)
    }

    pub fn is_ignored(&self, pos: BlockPos) -> bool {
        Some(pos) == self.ignore_below || Some(pos) == self.slope_ignore
    }

    pub fn get_collision_shape(&self, state: BlockState, pos: BlockPos) -> (&'static VoxelShape, [f64; 3]) {
        if self.is_ignored(pos) { (&EMPTY, [0.0; 3]) } else { self.get_state_collision_shape(state, pos) }
    }

    pub fn get_block_collision_shape(&self, state: BlockState, pos: BlockPos, fluid_above: impl FnOnce() -> FluidState) -> (&'static VoxelShape, [f64; 3]) {
        match self.get_liquid_block_collision_shape(state, pos, fluid_above) {
            Some(shape) => (shape, [0.0; 3]),
            None => self.get_state_collision_shape(state, pos),
        }
    }

    fn get_state_collision_shape(&self, state: BlockState, pos: BlockPos) -> (&'static VoxelShape, [f64; 3]) {
        if state.is_block(blocks::SCAFFOLDING) {
            (self.scaffolding_collision_shape(state, pos), [0.0; 3])
        } else if state.is_block(blocks::POWDER_SNOW) {
            (self.powder_snow_collision_shape(pos), [0.0; 3])
        } else {
            let offset = state.collision_offset(pos);
            (state.collision_voxel_shape(), [offset.x, offset.y, offset.z])
        }
    }

    fn scaffolding_collision_shape(&self, state: BlockState, pos: BlockPos) -> &'static VoxelShape {
        if self.is_placement() {
            &EMPTY
        } else if self.is_above(&BLOCK, pos, true) && !self.is_descending() {
            state.collision_voxel_shape()
        } else if state.get(STABILITY_DISTANCE) != 0 && state.get(BOTTOM) && self.is_above_max_y(0.0, pos, true) {
            &SCAFFOLDING_SHAPE_UNSTABLE_BOTTOM
        } else {
            &EMPTY
        }
    }

    fn powder_snow_collision_shape(&self, pos: BlockPos) -> &'static VoxelShape {
        if !self.is_placement()
            && let Some(entity) = self.entity
        {
            if entity.fall_distance > 2.5 {
                return &POWDER_SNOW_FALLING_COLLISION_SHAPE;
            }
            if entity.is_falling_block || entity.can_walk_on_powder_snow && self.is_above(&BLOCK, pos, false) && !self.is_descending() {
                return &BLOCK;
            }
        }
        &EMPTY
    }
}
