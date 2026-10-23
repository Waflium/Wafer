use wafer_core::{AABB, BlockPos, Vec3, mth};
use wafer_data::properties::{POWERED, RailShape};
use wafer_data::{BlockState, blocks, registry, registry_id};

use super::abstract_minecart::{exits, rail_shape};
use crate::server::level::ServerLevel;
use crate::world::entity::{EntityId, MoverType};
use crate::world::level::BlockGetter;
use crate::world::level::block::base_rail_block;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MinecartStep {
    pub position: Vec3,
    pub movement: Vec3,
    pub y_rot: f32,
    pub x_rot: f32,
    pub weight: f32,
}

#[derive(Default)]
pub struct NewMinecartBehavior {
    pub lerp_steps: Vec<MinecartStep>,
}

struct TrackIteration {
    movement_left: f64,
    first_iteration: bool,
    has_gained_slope_speed: bool,
    has_halted: bool,
    has_boosted: bool,
}

impl TrackIteration {
    fn should_iterate(&self) -> bool {
        self.first_iteration || self.movement_left > 1.0e-5f32 as f64
    }
}

fn vec3(v: [i32; 3]) -> Vec3 {
    Vec3::new(v[0] as f64, v[1] as f64, v[2] as f64)
}

fn is_decending(movement: Vec3, shape: RailShape) -> bool {
    match shape {
        RailShape::AscendingEast => movement.x < 0.0,
        RailShape::AscendingWest => movement.x > 0.0,
        RailShape::AscendingNorth => movement.z > 0.0,
        RailShape::AscendingSouth => movement.z < 0.0,
        _ => false,
    }
}

impl ServerLevel {
    pub fn new_minecart_behavior_tick(&mut self, id: EntityId) {
        let pos = self.get_current_block_pos_or_rail_below(id);
        let state = self.block_state(pos);
        if self.entity(id).first_tick {
            self.minecart_mut(id).set_on_rails(base_rail_block::is_rail(state));
            self.new_minecart_behavior_adjust_to_rails(id, pos, state, true);
        }
        self.apply_gravity(id);
        self.new_minecart_behavior_move_along_track(id);
    }

    pub fn new_minecart_behavior_adjust_to_rails(&mut self, id: EntityId, target_block_pos: BlockPos, current_state: BlockState, instant: bool) {
        if !base_rail_block::is_rail(current_state) {
            return;
        }
        let shape = rail_shape(current_state);
        let (first, second) = exits(shape);
        let exit0 = vec3(first).scale(0.5);
        let exit1 = vec3(second).scale(0.5);
        let mut horizontal_out_direction = exit0.horizontal();
        let mut horizontal_in_direction = exit1.horizontal();
        let delta_movement = self.entity(id).delta_movement;
        if delta_movement.length() > 1.0e-5f32 as f64 && delta_movement.dot(horizontal_out_direction) < delta_movement.dot(horizontal_in_direction)
            || is_decending(horizontal_in_direction, shape)
        {
            std::mem::swap(&mut horizontal_out_direction, &mut horizontal_in_direction);
        }
        let flipped = self.minecart(id).is_flipped();
        let mut y_rot = 180.0 - (mth::atan2(horizontal_out_direction.z, horizontal_out_direction.x) * 180.0 / std::f64::consts::PI) as f32;
        y_rot += if flipped { 180.0 } else { 0.0 };
        let previous_position = self.entity(id).position();
        let in_corner = exit0.x != exit1.x && exit0.z != exit1.z;
        let target_position = if in_corner {
            let from0_to1 = exit1.subtract(exit0);
            let from0_to_pos = previous_position.subtract(Vec3::at_bottom_center_of(target_block_pos)).subtract(exit0);
            let travel_vector_from0 = from0_to1.scale(from0_to1.dot(from0_to_pos) / from0_to1.dot(from0_to1));
            y_rot = 180.0 - (mth::atan2(travel_vector_from0.z, travel_vector_from0.x) * 180.0 / std::f64::consts::PI) as f32;
            y_rot += if flipped { 180.0 } else { 0.0 };
            Vec3::at_bottom_center_of(target_block_pos).add_vec(exit0).add_vec(travel_vector_from0)
        } else {
            let z_snap = exit0.subtract(exit1).x != 0.0;
            let x_snap = exit0.subtract(exit1).z != 0.0;
            Vec3::new(
                if x_snap { Vec3::at_center_of(target_block_pos).x } else { previous_position.x },
                target_block_pos.y as f64,
                if z_snap { Vec3::at_center_of(target_block_pos).z } else { previous_position.z },
            )
        };
        let diff_from_block = target_position.subtract(previous_position);
        self.entity_mut(id).set_pos(previous_position.add_vec(diff_from_block));
        let mut x_rot = 0.0;
        let in_hill = exit0.y != exit1.y;
        if in_hill {
            let in_position = Vec3::at_bottom_center_of(target_block_pos).add_vec(horizontal_in_direction);
            let horizontal_distance_from_in = in_position.distance_to(self.entity(id).position());
            let position = self.entity(id).position().add(0.0, horizontal_distance_from_in + 0.1, 0.0);
            self.entity_mut(id).set_pos(position);
            x_rot = if flipped { 45.0 } else { -45.0 };
        } else {
            let position = self.entity(id).position().add(0.0, 0.1, 0.0);
            self.entity_mut(id).set_pos(position);
        }
        self.new_minecart_behavior_set_rotation(id, y_rot, x_rot);
        let e = self.entity(id);
        let adjust_distance = previous_position.distance_to(e.position());
        if adjust_distance > 0.0 {
            let step = MinecartStep {
                position: e.position(),
                movement: e.delta_movement,
                y_rot: e.y_rot,
                x_rot: e.x_rot,
                weight: if instant { 0.0 } else { adjust_distance as f32 },
            };
            self.minecart_mut(id).new_behavior.lerp_steps.push(step);
        }
    }

    fn new_minecart_behavior_set_rotation(&mut self, id: EntityId, mut y_rot: f32, mut x_rot: f32) {
        let y_rot_diff = (y_rot - self.entity(id).y_rot).abs() as f64;
        if (175.0..=185.0).contains(&y_rot_diff) {
            let flipped = self.minecart(id).is_flipped();
            self.minecart_mut(id).set_flipped(!flipped);
            y_rot -= 180.0;
            x_rot *= -1.0;
        }
        x_rot = x_rot.clamp(-45.0, 45.0);
        let e = self.entity_mut(id);
        e.set_x_rot(x_rot % 360.0);
        e.set_y_rot(y_rot % 360.0);
    }

    pub fn new_minecart_behavior_move_along_track(&mut self, id: EntityId) {
        let mut track_iteration = TrackIteration { movement_left: 0.0, first_iteration: true, has_gained_slope_speed: false, has_halted: false, has_boosted: false };
        while track_iteration.should_iterate() && self.is_alive(id) {
            let initial_step_delta_movement = self.entity(id).delta_movement;
            let current_pos = self.get_current_block_pos_or_rail_below(id);
            let current_state = self.block_state(current_pos);
            let on_rails = base_rail_block::is_rail(current_state);
            if self.minecart(id).is_on_rails() != on_rails {
                self.minecart_mut(id).set_on_rails(on_rails);
                self.new_minecart_behavior_adjust_to_rails(id, current_pos, current_state, false);
            }
            if on_rails {
                self.reset_fall_distance(id);
                self.entity_mut(id).set_old_pos_and_rot();
                if current_state.is_block(blocks::ACTIVATOR_RAIL) {
                    self.activate_minecart(id, current_pos.x, current_pos.y, current_pos.z, current_state.get(POWERED));
                    if self.entity_ref(id).is_none() {
                        return;
                    }
                }
                let shape = rail_shape(current_state);
                let new_delta_movement = self.new_minecart_behavior_calculate_track_speed(id, initial_step_delta_movement.horizontal(), &mut track_iteration, current_pos, current_state, shape);
                if track_iteration.first_iteration {
                    track_iteration.movement_left = new_delta_movement.horizontal_distance();
                } else {
                    track_iteration.movement_left += new_delta_movement.horizontal_distance() - initial_step_delta_movement.horizontal_distance();
                }
                self.entity_mut(id).set_delta_movement(new_delta_movement);
                track_iteration.movement_left = self.new_minecart_behavior_step_along_track(id, current_pos, shape, track_iteration.movement_left);
            } else {
                self.come_off_track(id);
                track_iteration.movement_left = 0.0;
            }
            if self.entity_ref(id).is_none() {
                return;
            }
            let e = self.entity(id);
            let step_position = e.position();
            let step_delta = step_position.subtract(e.old_position());
            let step_length = step_delta.length();
            if step_length > 1.0e-5f32 as f64 {
                if step_delta.horizontal_distance_sqr() <= 1.0e-5f32 as f64 {
                    if !self.minecart(id).is_on_rails() {
                        let e = self.entity_mut(id);
                        let x_rot = if e.on_ground { 0.0 } else { mth::rot_lerp(0.2, e.x_rot, 0.0) };
                        e.set_x_rot(x_rot);
                    }
                } else {
                    let flipped = self.minecart(id).is_flipped();
                    let on_rails = self.minecart(id).is_on_rails();
                    let mut y_rot = 180.0 - (mth::atan2(step_delta.z, step_delta.x) * 180.0 / std::f64::consts::PI) as f32;
                    let mut x_rot = if self.entity(id).on_ground && !on_rails {
                        0.0
                    } else {
                        90.0 - (mth::atan2(step_delta.horizontal_distance(), step_delta.y) * 180.0 / std::f64::consts::PI) as f32
                    };
                    y_rot += if flipped { 180.0 } else { 0.0 };
                    x_rot *= if flipped { -1.0 } else { 1.0 };
                    self.new_minecart_behavior_set_rotation(id, y_rot, x_rot);
                }
                let max_speed = self.new_minecart_behavior_max_speed(id);
                let e = self.entity(id);
                let step = MinecartStep { position: step_position, movement: e.delta_movement, y_rot: e.y_rot, x_rot: e.x_rot, weight: step_length.min(max_speed) as f32 };
                self.minecart_mut(id).new_behavior.lerp_steps.push(step);
            } else if initial_step_delta_movement.horizontal_distance_sqr() > 0.0 {
                let e = self.entity(id);
                let step = MinecartStep { position: step_position, movement: e.delta_movement, y_rot: e.y_rot, x_rot: e.x_rot, weight: 1.0 };
                self.minecart_mut(id).new_behavior.lerp_steps.push(step);
            }
            if step_length > 1.0e-5f32 as f64 || track_iteration.first_iteration {
                self.apply_effects_from_blocks(id);
                if self.entity_ref(id).is_none() {
                    return;
                }
                self.apply_effects_from_blocks(id);
                if self.entity_ref(id).is_none() {
                    return;
                }
            }
            track_iteration.first_iteration = false;
        }
    }

    fn new_minecart_behavior_calculate_track_speed(
        &mut self,
        id: EntityId,
        delta_movement: Vec3,
        track_iteration: &mut TrackIteration,
        current_pos: BlockPos,
        current_state: BlockState,
        shape: RailShape,
    ) -> Vec3 {
        let mut new_delta_movement = delta_movement;
        if !track_iteration.has_gained_slope_speed {
            let sloped_delta_movement = self.new_minecart_behavior_calculate_slope_speed(id, new_delta_movement, shape);
            if sloped_delta_movement.horizontal_distance_sqr() != new_delta_movement.horizontal_distance_sqr() {
                track_iteration.has_gained_slope_speed = true;
                new_delta_movement = sloped_delta_movement;
            }
        }
        if track_iteration.first_iteration {
            let player_input_movement = self.new_minecart_behavior_calculate_player_input_speed(id, new_delta_movement);
            if player_input_movement.horizontal_distance_sqr() != new_delta_movement.horizontal_distance_sqr() {
                track_iteration.has_halted = true;
                new_delta_movement = player_input_movement;
            }
        }
        if !track_iteration.has_halted {
            let halted_delta_movement = calculate_halt_track_speed(new_delta_movement, current_state);
            if halted_delta_movement.horizontal_distance_sqr() != new_delta_movement.horizontal_distance_sqr() {
                track_iteration.has_halted = true;
                new_delta_movement = halted_delta_movement;
            }
        }
        if track_iteration.first_iteration {
            new_delta_movement = self.apply_natural_slowdown(id, new_delta_movement);
            if new_delta_movement.length_sqr() > 0.0 {
                let speed = new_delta_movement.length().min(self.minecart_max_speed(id));
                new_delta_movement = new_delta_movement.normalize().scale(speed);
            }
        }
        if !track_iteration.has_boosted {
            let boosted_delta_movement = self.new_minecart_behavior_calculate_boost_track_speed(new_delta_movement, current_pos, current_state);
            if boosted_delta_movement.horizontal_distance_sqr() != new_delta_movement.horizontal_distance_sqr() {
                track_iteration.has_boosted = true;
                new_delta_movement = boosted_delta_movement;
            }
        }
        new_delta_movement
    }

    fn new_minecart_behavior_calculate_slope_speed(&self, id: EntityId, delta_movement: Vec3, shape: RailShape) -> Vec3 {
        let mut slide_speed = 0.0078125f64.max(delta_movement.horizontal_distance() * 0.02);
        if self.entity(id).is_in_water() {
            slide_speed *= 0.2;
        }
        match shape {
            RailShape::AscendingEast => delta_movement.add(-slide_speed, 0.0, 0.0),
            RailShape::AscendingWest => delta_movement.add(slide_speed, 0.0, 0.0),
            RailShape::AscendingNorth => delta_movement.add(0.0, 0.0, slide_speed),
            RailShape::AscendingSouth => delta_movement.add(0.0, 0.0, -slide_speed),
            _ => delta_movement,
        }
    }

    fn new_minecart_behavior_calculate_player_input_speed(&self, id: EntityId, delta_movement: Vec3) -> Vec3 {
        let Some(player) = self.first_passenger(id).and_then(|passenger| self.players.get(&passenger)) else { return delta_movement };
        let move_intent = player.get_last_client_move_intent();
        if move_intent.length_sqr() > 0.0 {
            let rider_movement = move_intent.normalize();
            let own_dist = delta_movement.horizontal_distance_sqr();
            if rider_movement.length_sqr() > 0.0 && own_dist < 0.01 {
                return delta_movement.add_vec(Vec3::new(rider_movement.x, 0.0, rider_movement.z).normalize().scale(0.001));
            }
        }
        delta_movement
    }

    fn new_minecart_behavior_calculate_boost_track_speed(&mut self, delta_movement: Vec3, pos: BlockPos, state: BlockState) -> Vec3 {
        if !state.is_block(blocks::POWERED_RAIL) || !state.get(POWERED) {
            return delta_movement;
        }
        if delta_movement.length() > 0.01 {
            return delta_movement.normalize().scale(delta_movement.length() + 0.06);
        }
        let power_direction = self.get_redstone_direction(pos);
        if power_direction.length_sqr() <= 0.0 { delta_movement } else { power_direction.scale(delta_movement.length() + 0.2) }
    }

    pub fn new_minecart_behavior_step_along_track(&mut self, id: EntityId, pos: BlockPos, shape: RailShape, mut movement_left: f64) -> f64 {
        if movement_left < 1.0e-5f32 as f64 {
            return 0.0;
        }
        let old_position = self.entity(id).position();
        let (exit0, exit1) = exits(shape);
        let mut movement = self.entity(id).delta_movement.horizontal();
        if movement.length() < 1.0e-5f32 as f64 {
            self.entity_mut(id).set_delta_movement(Vec3::ZERO);
            return 0.0;
        }
        let in_hill = exit0[1] != exit1[1];
        let horizontal_in_direction = vec3(exit1).scale(0.5).horizontal();
        let mut horizontal_out_direction = vec3(exit0).scale(0.5).horizontal();
        if movement.dot(horizontal_out_direction) < movement.dot(horizontal_in_direction) {
            horizontal_out_direction = horizontal_in_direction;
        }
        let mut out_position =
            Vec3::at_bottom_center_of(pos).add_vec(horizontal_out_direction).add(0.0, 0.1, 0.0).add_vec(horizontal_out_direction.normalize().scale(1.0e-5f32 as f64));
        if in_hill && !is_decending(movement, shape) {
            out_position = out_position.add(0.0, 1.0, 0.0);
        }
        let towards_out = out_position.subtract(self.entity(id).position()).normalize();
        movement = towards_out.scale(movement.length() / towards_out.horizontal_distance());
        let mut new_position = old_position.add_vec(movement.normalize().scale(movement_left * if in_hill { mth::SQRT_OF_TWO as f64 } else { 1.0 }));
        if old_position.distance_to_sqr(out_position) <= old_position.distance_to_sqr(new_position) {
            movement_left = out_position.subtract(new_position).horizontal_distance();
            new_position = out_position;
        } else {
            movement_left = 0.0;
        }
        self.minecart_move(id, MoverType::SelfMove, new_position.subtract(old_position));
        if self.entity_ref(id).is_none() {
            return 0.0;
        }
        let new_block_state = self.block_state(BlockPos::containing(new_position.x, new_position.y, new_position.z));
        if in_hill {
            if base_rail_block::is_rail(new_block_state) {
                let new_rail_shape = rail_shape(new_block_state);
                if self.new_minecart_behavior_rest_at_v_shape(id, shape, new_rail_shape) {
                    return 0.0;
                }
            }
            let position = self.entity(id).position();
            let horizontal_distance_from_out = out_position.horizontal().distance_to(position.horizontal());
            let project_y_pos = out_position.y + if is_decending(movement, shape) { horizontal_distance_from_out } else { -horizontal_distance_from_out };
            if position.y < project_y_pos {
                self.entity_mut(id).set_pos(Vec3::new(position.x, project_y_pos, position.z));
            }
        }
        if self.entity(id).position().distance_to(old_position) < 1.0e-5f32 as f64 && new_position.distance_to(old_position) > 1.0e-5f32 as f64 {
            self.entity_mut(id).set_delta_movement(Vec3::ZERO);
            0.0
        } else {
            self.entity_mut(id).set_delta_movement(movement);
            movement_left
        }
    }

    fn new_minecart_behavior_rest_at_v_shape(&mut self, id: EntityId, current_rail_shape: RailShape, new_rail_shape: RailShape) -> bool {
        let delta_movement = self.entity(id).delta_movement;
        if delta_movement.length_sqr() < 0.005
            && base_rail_block::is_slope(new_rail_shape)
            && is_decending(delta_movement, current_rail_shape)
            && !is_decending(delta_movement, new_rail_shape)
        {
            self.entity_mut(id).set_delta_movement(Vec3::ZERO);
            true
        } else {
            false
        }
    }

    pub fn new_minecart_behavior_max_speed(&self, id: EntityId) -> f64 {
        self.game_rules.get_int(wafer_data::game_rule::MAX_MINECART_SPEED) as f64 * if self.entity(id).is_in_water() { 0.5 } else { 1.0 } / 20.0
    }

    pub fn new_minecart_behavior_slowdown_factor(&self, id: EntityId) -> f64 {
        if self.is_vehicle(id) { 0.997 } else { 0.975 }
    }

    pub fn new_minecart_behavior_push_and_pickup_entities(&mut self, id: EntityId) -> bool {
        let bounding_box = self.entity(id).bounding_box();
        let picked_up = self.new_minecart_behavior_pickup_entities(id, &bounding_box.inflate(0.2, 0.0, 0.2));
        let e = self.entity(id);
        if !e.horizontal_collision && !e.vertical_collision {
            return false;
        }
        let pushed = self.new_minecart_behavior_push_entities(id, &bounding_box.inflate_all(1.0e-7));
        picked_up && !pushed
    }

    fn new_minecart_behavior_pickup_entities(&mut self, id: EntityId, hitbox: &AABB) -> bool {
        if self.minecart(id).is_rideable() && !self.is_vehicle(id) {
            let entities: Vec<EntityId> = self.get_entities(Some(id), hitbox).into_iter().filter(|&e| self.pushable_by(id, e)).collect();
            for entity in entities {
                if !self.is_player(entity) && !self.is_iron_golem(entity) && !self.is_minecart(entity) && !self.is_vehicle(id) && !self.is_passenger(entity) && self.start_riding(entity, id, false, true) {
                    return true;
                }
            }
        }
        false
    }

    fn new_minecart_behavior_push_entities(&mut self, id: EntityId, hitbox: &AABB) -> bool {
        let mut pushed = false;
        if self.minecart(id).is_rideable() {
            let entities: Vec<EntityId> = self.get_entities(Some(id), hitbox).into_iter().filter(|&e| self.pushable_by(id, e)).collect();
            for entity in entities {
                if self.is_player(entity) || self.is_iron_golem(entity) || self.is_minecart(entity) || self.is_vehicle(id) || self.is_passenger(entity) {
                    self.push_entity(entity, id);
                    pushed = true;
                }
            }
        } else {
            for entity in self.get_entities(Some(id), hitbox) {
                if !self.has_passenger(id, entity) && self.is_pushable(entity) && self.is_minecart(entity) {
                    self.push_entity(entity, id);
                    pushed = true;
                }
            }
        }
        pushed
    }

    fn is_iron_golem(&self, id: EntityId) -> bool {
        self.entity_ref(id).is_some_and(|e| e.entity_type == registry_id(registry::ENTITY_TYPE, "minecraft:iron_golem"))
    }
}

fn calculate_halt_track_speed(delta_movement: Vec3, state: BlockState) -> Vec3 {
    if state.is_block(blocks::POWERED_RAIL) && !state.get(POWERED) {
        if delta_movement.length() < 0.03 { Vec3::ZERO } else { delta_movement.scale(0.5) }
    } else {
        delta_movement
    }
}
