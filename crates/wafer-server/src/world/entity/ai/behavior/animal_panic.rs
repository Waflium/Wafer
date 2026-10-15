use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::{BlockState, Tag, damage_type_tags, fluid_tags};

use super::behavior::{Behavior, BehaviorState};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::entity::ai::util::land_random_pos;
use crate::world::level::path_navigation_region::PathNavigationRegion;

const PANIC_MIN_DURATION: i32 = 100;
const PANIC_MAX_DURATION: i32 = 120;
const PANIC_DISTANCE_HORIZONTAL: i32 = 5;
const PANIC_DISTANCE_VERTICAL: i32 = 4;

pub struct AnimalPanic {
    behavior: BehaviorState,
    speed_multiplier: f32,
    panic_causing_damage_types: fn(&ServerLevel, EntityId) -> &'static Tag,
    position_getter: fn(&mut ServerLevel, EntityId) -> Option<Vec3>,
}

impl AnimalPanic {
    pub fn new(speed_multiplier: f32) -> AnimalPanic {
        AnimalPanic::with_position_getter(speed_multiplier, |_, _| &damage_type_tags::PANIC_CAUSES, |level, mob| land_random_pos::get_pos(level, mob, PANIC_DISTANCE_HORIZONTAL, PANIC_DISTANCE_VERTICAL))
    }

    pub fn with_position_getter(
        speed_multiplier: f32,
        panic_causing_damage_types: fn(&ServerLevel, EntityId) -> &'static Tag,
        position_getter: fn(&mut ServerLevel, EntityId) -> Option<Vec3>,
    ) -> AnimalPanic {
        AnimalPanic {
            behavior: BehaviorState::with_durations(
                vec![(memory_module_type::IS_PANICKING.erase(), MemoryStatus::Registered), (memory_module_type::HURT_BY.erase(), MemoryStatus::Registered)],
                PANIC_MIN_DURATION,
                PANIC_MAX_DURATION,
            ),
            speed_multiplier,
            panic_causing_damage_types,
            position_getter,
        }
    }

    fn get_panic_pos(&self, level: &mut ServerLevel, body: EntityId) -> Option<Vec3> {
        if level.is_on_fire(body)
            && let Some(nearest_water) = look_for_water(level, body)
        {
            return Some(Vec3::at_bottom_center_of(nearest_water));
        }
        (self.position_getter)(level, body)
    }
}

fn look_for_water(level: &ServerLevel, mob: EntityId) -> Option<BlockPos> {
    let mob_position = level.entity(mob).block_position();
    let region = PathNavigationRegion::new(level);
    if !region.get_block_state(mob_position).collision_voxel_shape().is_empty() {
        return None;
    }
    let mut block_matcher = BlockPos::within_box_by_manhattan_distance(mob_position, 5, 1, 5).filter(|&pos| is_water(region.get_block_state(pos)));
    if mth::ceil(level.entity(mob).dimensions.width as f64) == 2 {
        block_matcher.find(|&pos| is_water(region.get_block_state(pos.south())) && is_water(region.get_block_state(pos.east())) && is_water(region.get_block_state(pos.south().east())))
    } else {
        block_matcher.next()
    }
}

fn is_water(state: BlockState) -> bool {
    state.fluid_state().is(&fluid_tags::WATER)
}

impl Behavior for AnimalPanic {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        let panic_causing_damage_types = (self.panic_causing_damage_types)(level, body);
        let brain = level.get_brain(body);
        brain.get_memory(memory_module_type::HURT_BY).is_some_and(|damage_source| damage_source.is(panic_causing_damage_types)) || brain.has_memory_value(memory_module_type::IS_PANICKING)
    }

    fn can_still_use(&mut self, _level: &mut ServerLevel, _body: EntityId, _timestamp: i64) -> bool {
        true
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let brain = level.get_brain_mut(body);
        brain.set_memory(memory_module_type::IS_PANICKING, true);
        brain.erase_memory(memory_module_type::WALK_TARGET);
        level.get_navigation(body, |navigation, _| navigation.stop());
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        level.get_brain_mut(body).erase_memory(memory_module_type::IS_PANICKING);
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        if level.mob(body).expect("a mob").ai.navigation.is_done()
            && let Some(panic_to_pos) = self.get_panic_pos(level, body)
        {
            level.get_brain_mut(body).set_memory(memory_module_type::WALK_TARGET, WalkTarget::of_vec(panic_to_pos, self.speed_multiplier, 0));
        }
    }
}
