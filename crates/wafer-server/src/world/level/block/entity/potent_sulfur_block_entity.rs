use wafer_core::random::{RandomSource, XoroshiroRandomSource};
use wafer_core::{AABB, BlockPos, Vec3};
use wafer_data::properties::{POTENT_SULFUR_STATE, PotentSulfurState};
use wafer_data::{BlockState, Fluid, blocks, entity_type_tags, mob_effects};
use wafer_protocol::nbt::Nbt;

use super::BlockEntityKind;
use crate::server::level::ServerLevel;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::{EntityId, entity_selector};
use crate::world::level::BlockGetter;
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};
use crate::world::level::gameevent::game_event;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::phys::shapes::collision_context::CollisionContext;
use crate::world::level::storage::tag_value_input;

const EFFECT_APPLICATION_FREQUENCY_TICKS: i64 = 10;
const EFFECT_DURATION_IN_TICKS: i32 = 80;
const GEYSER_BASE_LAUNCH_SPEED: f32 = 0.3;
const GEYSER_LAUNCH_FORCE: f32 = 0.2;
pub const GEYSER_SALT: i64 = -904011478;

#[derive(Clone)]
pub struct PotentSulfurBlockEntity {
    pub waiting_countdown: i32,
}

impl Default for PotentSulfurBlockEntity {
    fn default() -> PotentSulfurBlockEntity {
        PotentSulfurBlockEntity { waiting_countdown: -1 }
    }
}

impl PotentSulfurBlockEntity {
    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("countdown".into(), Nbt::Int(self.waiting_countdown)));
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        if let Some(value) = tag_value_input::get_numeric(input, "countdown").and_then(super::int_value) {
            self.waiting_countdown = value;
        }
    }

    pub fn reset_countdown(&mut self) {
        self.waiting_countdown = -1;
    }
}

fn effect_predicate(level: &ServerLevel, entity: EntityId) -> bool {
    entity_selector::no_spectators(level, entity) && level.is_alive(entity)
}

impl ServerLevel {
    fn potent_sulfur(&mut self, pos: BlockPos) -> Option<&mut PotentSulfurBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::PotentSulfur(potent_sulfur) => Some(potent_sulfur),
            _ => None,
        }
    }

    pub fn potent_sulfur_reset_countdown(&mut self, pos: BlockPos) {
        if let Some(potent_sulfur) = self.potent_sulfur(pos) {
            potent_sulfur.reset_countdown();
        }
    }

    pub fn potent_sulfur_server_nausea_effect_tick(&mut self, pos: BlockPos) {
        if self.game_time % EFFECT_APPLICATION_FREQUENCY_TICKS == 0
            && let Some(source_block) = find_noxious_gas_source_block(self, pos)
        {
            for entity in get_nearby_living_entities(self, source_block) {
                let eye_position = self.entity(entity).eye_position();
                if can_be_reached_by_noxious_gas(self, source_block, eye_position) {
                    apply_nausea_effect(self, entity);
                }
            }
        }
    }

    pub fn potent_sulfur_server_waiting_countdown_tick(&mut self, pos: BlockPos, state: BlockState) {
        if self.game_time % 20 != 0 {
            return;
        }
        let Some(source_block) = find_noxious_gas_source_block(self, pos) else { return };
        let seed = self.seed;
        let Some(entity) = self.potent_sulfur(pos) else { return };
        if entity.waiting_countdown <= 0 {
            let water_blocks = source_block.y - pos.y - 1;
            let mut geyser_positional = geyser_positional(seed, pos);
            if state.get(POTENT_SULFUR_STATE) == PotentSulfurState::Dormant {
                entity.waiting_countdown = 10 * (water_blocks - 1) + geyser_positional.next_int_between_inclusive(15, 30);
            } else {
                geyser_positional.next_int();
                entity.waiting_countdown = water_blocks - 1 + geyser_positional.next_int_between_inclusive(1, 2);
            }
        }
        if entity.waiting_countdown > 0 {
            entity.waiting_countdown -= 1;
        }
        if entity.waiting_countdown == 0 {
            let state_to_set = if state.get(POTENT_SULFUR_STATE) == PotentSulfurState::Dormant { PotentSulfurState::Erupting } else { PotentSulfurState::Dormant };
            self.set_block(pos, state.set(POTENT_SULFUR_STATE, state_to_set), UPDATE_ALL);
            if state_to_set == PotentSulfurState::Dormant {
                self.game_event_at_with(game_event::BLOCK_DEACTIVATE, pos, game_event::Context::of_state(Some(state)));
            }
        }
    }

    pub fn potent_sulfur_launch_entity_tick(&mut self, pos: BlockPos) {
        let Some(source_block) = find_noxious_gas_source_block(self, pos) else { return };
        let water_blocks = source_block.y - pos.y - 1;
        let geyser_force_height = get_unobstructed_block_count(self, pos.above(), water_blocks);
        let above = pos.above();
        let aabb = AABB::new(above.x as f64, above.y as f64, above.z as f64, (above.x + 1) as f64, (above.y + 1) as f64, (above.z + 1) as f64).expand_towards(0.0, (geyser_force_height - 1) as f64, 0.0);
        for entity_to_be_launched in self.get_entities_of_type(|_, _| true, &aabb, effect_predicate) {
            let entity_velocity = self.entity(entity_to_be_launched).delta_movement;
            self.entity_mut(entity_to_be_launched).check_fall_distance_accumulation();
            if self.can_simulate_movement(entity_to_be_launched)
                && !(self.is_player(entity_to_be_launched) && self.player(entity_to_be_launched).abilities.flying)
                && !self.is_passenger(entity_to_be_launched)
                && !entity_type_tags::NOT_AFFECTED_BY_GEYSERS.contains(self.entity(entity_to_be_launched).entity_type as usize)
                && entity_velocity.y < (GEYSER_BASE_LAUNCH_SPEED as f64 + water_blocks as f64 * 0.1)
            {
                self.entity_mut(entity_to_be_launched).add_delta_movement(Vec3::new(0.0, GEYSER_LAUNCH_FORCE as f64, 0.0));
            }
        }
    }
}

fn apply_nausea_effect(level: &mut ServerLevel, entity: EntityId) {
    level.add_effect(entity, MobEffectInstance::with(mob_effects::NAUSEA, EFFECT_DURATION_IN_TICKS, 0, true, true, true), None);
}

fn get_nearby_living_entities(level: &ServerLevel, pos: BlockPos) -> Vec<EntityId> {
    let aabb = AABB::new(pos.x as f64, pos.y as f64, pos.z as f64, (pos.x + 1) as f64, (pos.y + 1) as f64, (pos.z + 1) as f64).inflate(2.5, 0.0, 2.5);
    level.get_entities_matching(None, &aabb, |level, entity| level.is_living(entity) && effect_predicate(level, entity))
}

pub fn geyser_positional(seed: i64, pos: BlockPos) -> impl RandomSource {
    XoroshiroRandomSource::new(seed ^ GEYSER_SALT).fork_positional().at(pos.x, pos.y, pos.z)
}

fn get_unobstructed_block_count(level: &mut ServerLevel, pos: BlockPos, water_blocks: i32) -> i32 {
    let geyser_force_height = 6 * water_blocks;
    let geyser_position_context = CollisionContext::position_context(pos.below().y as f64);
    for i in 0..geyser_force_height {
        let current_pos = pos.relative_n(wafer_core::Direction::Up, i);
        let state = level.block_state(current_pos);
        if !is_geyser_passable_block(state, current_pos, &geyser_position_context) {
            return i;
        }
    }
    geyser_force_height
}

fn is_geyser_passable_block(state: BlockState, pos: BlockPos, context: &CollisionContext) -> bool {
    if !state.is_air() && !state.is_block(blocks::WATER) { context.get_collision_shape(state, pos).0.is_empty() } else { true }
}

fn find_noxious_gas_source_block(level: &mut ServerLevel, origin: BlockPos) -> Option<BlockPos> {
    let max_y = origin.y + 4 + 1;
    let geyser_position_context = CollisionContext::position_context(origin.y as f64);
    let mut pos = origin.above();
    while pos.y <= max_y {
        let state = level.block_state(pos);
        let is_water_logged = level.fluid_state(pos).is_source_of_type(Fluid::Water);
        if !is_water_logged || !state.is_block(blocks::WATER) && !is_geyser_passable_block(state, pos, &geyser_position_context) {
            if state.is_air() || is_geyser_passable_block(state, pos, &geyser_position_context) {
                return Some(pos);
            }
            break;
        }
        pos = pos.above();
    }
    None
}

pub fn can_be_reached_by_noxious_gas(level: &mut ServerLevel, source_block: BlockPos, pos: Vec3) -> bool {
    let block_pos = BlockPos::containing_vec(pos);
    let geyser_position_context = CollisionContext::position_context(block_pos.below().y as f64);
    let state = level.block_state(block_pos);
    if !is_geyser_passable_block(state, block_pos, &geyser_position_context) {
        return false;
    }
    if pos.distance_to_sqr(Vec3::at_center_of(source_block)) > 9.0 {
        return false;
    }
    let below_source = Vec3::at_center_of(source_block.below());
    let below_pos = Vec3::new(pos.x, pos.y - 1.0, pos.z);
    is_water(level, below_pos) && have_line_of_sight(level, below_source, below_pos)
}

fn is_water(level: &mut ServerLevel, pos: Vec3) -> bool {
    level.fluid_state(BlockPos::containing_vec(pos)).is_source_of_type(Fluid::Water)
}

fn have_line_of_sight(level: &mut ServerLevel, a: Vec3, b: Vec3) -> bool {
    level.clip(&ClipContext { from: a, to: b, block: ClipBlock::Collider, fluid: ClipFluid::None, collision_context: CollisionContext::EMPTY }).miss
}
