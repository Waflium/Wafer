use std::sync::Arc;

use wafer_core::{AABB, BlockPos, mth};
use wafer_data::DimensionType;

use super::LevelAccessor;
use super::chunk::{Biome, HeightmapType};
use super::levelgen::structure::structure::StructureHolder;
use super::levelgen::structure::structure_start::StructureStart;
use super::light_layer::LightLayer;
use crate::world::attribute::{AttributeValue, EnvironmentAttribute};
use crate::world::difficulty::Difficulty;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::{EntityId, LevelEntity};

pub trait ServerLevelAccessor: LevelAccessor {
    fn get_difficulty(&self) -> Difficulty;

    fn get_current_difficulty_at(&mut self, pos: BlockPos) -> DifficultyInstance;

    fn get_height(&mut self, kind: HeightmapType, x: i32, z: i32) -> i32;

    fn get_biome(&mut self, pos: BlockPos) -> Biome;

    fn get_environment_attribute<T: AttributeValue>(&mut self, attribute: EnvironmentAttribute<T>, pos: BlockPos) -> T;

    fn dimension_type(&self) -> &'static DimensionType;

    fn get_sky_darken(&self) -> i32;

    fn is_thundering(&self) -> bool;

    fn is_within_world_border(&self, pos: BlockPos) -> bool;

    fn get_nearest_player(&self, x: f64, y: f64, z: f64, max_dist: f64, filter_out_creative: bool) -> Option<EntityId>;

    fn no_collision(&mut self, aabb: &AABB) -> bool;

    fn is_unobstructed(&mut self, aabb: &AABB) -> bool;

    fn add_fresh_entity(&mut self, entity: LevelEntity) -> bool;

    fn get_structure_with_piece_at(&mut self, pos: BlockPos, predicate: &dyn Fn(StructureHolder) -> bool) -> Option<Arc<StructureStart>>;

    fn add_fresh_entity_with_passengers(&mut self, entity: LevelEntity) {
        self.add_fresh_entity(entity);
    }

    fn get_heightmap_pos(&mut self, kind: HeightmapType, pos: BlockPos) -> BlockPos {
        BlockPos::new(pos.x, self.get_height(kind, pos.x, pos.z), pos.z)
    }

    fn get_max_local_raw_brightness(&mut self, pos: BlockPos) -> i32 {
        let sky_darken = self.get_sky_darken();
        self.get_max_local_raw_brightness_with(pos, sky_darken)
    }

    fn get_max_local_raw_brightness_with(&mut self, pos: BlockPos, sky_darkening: i32) -> i32 {
        if pos.x >= -30_000_000 && pos.z >= -30_000_000 && pos.x < 30_000_000 && pos.z < 30_000_000 { self.raw_brightness(pos, sky_darkening) } else { 15 }
    }

    fn get_light_level_dependent_magic_value(&mut self, pos: BlockPos) -> f32 {
        let v = self.get_max_local_raw_brightness(pos) as f32 / 15.0;
        let curved_v = v / (4.0 - 3.0 * v);
        mth::lerp_f(self.dimension_type().ambient_light, curved_v, 1.0)
    }

    fn get_pathfinding_cost_from_light_levels(&mut self, pos: BlockPos) -> f32 {
        self.get_light_level_dependent_magic_value(pos) - 0.5
    }

    fn can_see_sky(&mut self, pos: BlockPos) -> bool {
        self.get_brightness(LightLayer::Sky, pos) >= 15
    }

    fn can_see_sky_from_below_water(&mut self, pos: BlockPos) -> bool {
        let sea_level = self.get_sea_level();
        if pos.y >= sea_level {
            return self.can_see_sky(pos);
        }
        let scan_point = BlockPos::new(pos.x, sea_level, pos.z);
        if !self.can_see_sky(scan_point) {
            return false;
        }
        let mut check = scan_point.below();
        while check.y > pos.y {
            let state = self.block_state(check);
            if state.light_dampening() > 0 && !state.liquid() {
                return false;
            }
            check = check.below();
        }
        true
    }

    fn contains_any_liquid(&mut self, aabb: &AABB) -> bool {
        let from = BlockPos::containing(aabb.min_x, aabb.min_y, aabb.min_z);
        let to = BlockPos::containing(aabb.max_x, aabb.max_y, aabb.max_z);
        for z in from.z..=to.z {
            for y in from.y..=to.y {
                for x in from.x..=to.x {
                    if !self.block_state(BlockPos::new(x, y, z)).fluid_state().is_empty() {
                        return true;
                    }
                }
            }
        }
        false
    }
}
