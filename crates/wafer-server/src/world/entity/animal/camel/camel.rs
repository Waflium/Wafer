use wafer_core::random::RandomSource;
use wafer_core::{Vec3, mth};
use wafer_data::{block_tags, item_tags, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::{camel_ai, camel_husk};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::brain::{self, Brain, Provider};
use crate::world::entity::ai::control::look_control::LookControlKind;
use crate::world::entity::ai::control::move_control::{MoveControlKind, Operation};
use crate::world::entity::ai::sensing::sensor_type::SensorType;
use crate::world::entity::animal::equine::abstract_horse::{AbstractHorse, abstract_horse_defaults};
use crate::world::entity::attributes::{Attribute, CAMEL_ATTRIBUTES};
use crate::world::entity::data::{DataValue, Pose};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{Entity, EntityDimensions, EntityId, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::block::entity::long_value;
use crate::world::{InteractionHand, InteractionResult};

pub const BABY_SCALE: f32 = 0.6;
pub const DASH_COOLDOWN_TICKS: i32 = 55;
pub const MAX_HEAD_Y_ROT: i32 = 30;
const RUNNING_SPEED_BONUS: f32 = 0.1;
const DASH_VERTICAL_MOMENTUM: f32 = 1.4285;
const DASH_HORIZONTAL_MOMENTUM: f32 = 22.2222;
const SITDOWN_DURATION_TICKS: i32 = 40;
const STANDUP_DURATION_TICKS: i32 = 52;
const SITTING_HEIGHT_DIFFERENCE: f32 = 1.43;
const DEFAULT_LAST_POSE_CHANGE_TICK: i64 = 0;
pub const DATA_DASH: usize = 19;
pub const DATA_LAST_POSE_CHANGE_TICK: usize = 20;

pub static BRAIN_PROVIDER: Provider =
    brain::provider(&[SensorType::NearestLivingEntities, SensorType::HurtBy, SensorType::FoodTemptations, SensorType::NearestAdult], |_| camel_ai::get_activities());

pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(1.7, 2.375).with_eye_height(2.275);
const BABY_STANDING_DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.95, 1.4).with_eye_height(1.38);
const BABY_SITTING_DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.95, 0.425).with_eye_height(0.41);
const ADULT_SITTING_DIMENSIONS: EntityDimensions = EntityDimensions::scalable(1.7, 2.375 - SITTING_HEIGHT_DIFFERENCE).with_eye_height(0.845);

#[derive(Default)]
pub struct Camel {
    pub abstract_horse: AbstractHorse,
    pub dash_cooldown: i32,
}

pub fn camel(mob: &Mob) -> &Camel {
    match &mob.kind {
        MobKind::Camel(camel) | MobKind::CamelHusk(camel) => camel,
        _ => unreachable!("a camel"),
    }
}

pub fn camel_mut(mob: &mut Mob) -> &mut Camel {
    match &mut mob.kind {
        MobKind::Camel(camel) | MobKind::CamelHusk(camel) => camel,
        _ => unreachable!("a camel"),
    }
}

pub fn is_dashing(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_DASH), DataValue::Boolean(true))
}

fn last_pose_change_tick(entity: &Entity) -> i64 {
    match entity.data.get(DATA_LAST_POSE_CHANGE_TICK) {
        DataValue::Long(tick) => *tick,
        _ => DEFAULT_LAST_POSE_CHANGE_TICK,
    }
}

pub fn reset_last_pose_change_tick(entity: &mut Entity, synced_pose_tick_time: i64) {
    entity.data.set(DATA_LAST_POSE_CHANGE_TICK, DataValue::Long(synced_pose_tick_time));
}

fn reset_last_pose_change_tick_to_full_stand(entity: &mut Entity, current_time: i64) {
    reset_last_pose_change_tick(entity, 0.max(current_time - STANDUP_DURATION_TICKS as i64 - 1));
}

pub fn is_camel_sitting(entity: &Entity) -> bool {
    last_pose_change_tick(entity) < 0
}

pub fn get_pose_time(entity: &Entity, game_time: i64) -> i64 {
    game_time - last_pose_change_tick(entity).abs()
}

pub fn is_in_pose_transition(entity: &Entity, game_time: i64) -> bool {
    let pose_time = get_pose_time(entity, game_time);
    pose_time < if is_camel_sitting(entity) { SITDOWN_DURATION_TICKS } else { STANDUP_DURATION_TICKS } as i64
}

pub fn refuse_to_move(entity: &Entity, game_time: i64) -> bool {
    is_camel_sitting(entity) || is_in_pose_transition(entity, game_time)
}

pub fn finalize_spawn(game_time: i64, mob: &mut Mob) {
    camel_ai::init_memories(mob);
    reset_last_pose_change_tick_to_full_stand(&mut mob.entity, game_time);
}

pub fn get_default_dimensions(baby: bool, pose: Pose) -> EntityDimensions {
    if pose == Pose::Sitting {
        if baby { BABY_SITTING_DIMENSIONS } else { ADULT_SITTING_DIMENSIONS }
    } else if baby {
        BABY_STANDING_DIMENSIONS
    } else {
        DIMENSIONS
    }
}

pub fn get_ambient_sound(mob: &Mob) -> &'static str {
    match mob.kind {
        MobKind::CamelHusk(_) => camel_husk::get_ambient_sound(),
        _ => "minecraft:entity.camel.ambient",
    }
}

pub fn get_death_sound(mob: &Mob) -> &'static str {
    match mob.kind {
        MobKind::CamelHusk(_) => camel_husk::get_death_sound(),
        _ => "minecraft:entity.camel.death",
    }
}

pub fn get_hurt_sound(mob: &Mob) -> &'static str {
    match mob.kind {
        MobKind::CamelHusk(_) => camel_husk::get_hurt_sound(),
        _ => "minecraft:entity.camel.hurt",
    }
}

fn get_dashing_sound(mob: &Mob) -> &'static str {
    match mob.kind {
        MobKind::CamelHusk(_) => camel_husk::get_dashing_sound(),
        _ => "minecraft:entity.camel.dash",
    }
}

fn get_dash_ready_sound(mob: &Mob) -> &'static str {
    match mob.kind {
        MobKind::CamelHusk(_) => camel_husk::get_dash_ready_sound(),
        _ => "minecraft:entity.camel.dash_ready",
    }
}

pub fn get_eating_sound(mob: &Mob) -> &'static str {
    match mob.kind {
        MobKind::CamelHusk(_) => camel_husk::get_eating_sound(),
        _ => "minecraft:entity.camel.eat",
    }
}

fn get_stand_up_sound(mob: &Mob) -> &'static str {
    match mob.kind {
        MobKind::CamelHusk(_) => camel_husk::get_stand_up_sound(),
        _ => "minecraft:entity.camel.stand",
    }
}

fn get_sit_down_sound(mob: &Mob) -> &'static str {
    match mob.kind {
        MobKind::CamelHusk(_) => camel_husk::get_sit_down_sound(),
        _ => "minecraft:entity.camel.sit",
    }
}

pub fn get_saddle_sound(mob: &Mob) -> &'static str {
    match mob.kind {
        MobKind::CamelHusk(_) => camel_husk::get_saddle_sound(),
        _ => "minecraft:entity.camel.saddle",
    }
}

pub fn is_food(mob: &Mob, item_stack: &ItemStack) -> bool {
    match mob.kind {
        MobKind::CamelHusk(_) => camel_husk::is_food(item_stack),
        _ => item_stack.get_item().is(&item_tags::CAMEL_FOOD),
    }
}

pub fn get_max_head_y_rot() -> i32 {
    MAX_HEAD_Y_ROT
}

pub fn get_age_scale(baby: bool) -> f32 {
    if baby { BABY_SCALE } else { 1.0 }
}

pub fn is_tamed() -> bool {
    true
}

impl Camel {
    pub fn create() -> Mob {
        Camel::create_with("minecraft:camel", MobKind::Camel)
    }

    pub fn create_with(entity_type: &str, kind: fn(Camel) -> MobKind) -> Mob {
        let mut data = abstract_horse_defaults();
        data.push(DataValue::Boolean(false));
        data.push(DataValue::Long(DEFAULT_LAST_POSE_CHANGE_TICK));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, entity_type), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&CAMEL_ATTRIBUTES), kind(Camel::default()));
        AbstractHorse::init(&mut mob);
        mob.ai.move_control.kind = MoveControlKind::Camel;
        mob.ai.look_control.kind = LookControlKind::Camel;
        mob.ai.navigation.set_can_float(true);
        mob.ai.navigation.set_can_walk_over_fences(true);
        mob
    }

    pub fn make_brain(mob: &mut Mob, packed_brain: brain::Packed) -> Brain {
        BRAIN_PROVIDER.make_brain(mob, packed_brain)
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        AbstractHorse::add_additional_save_data(mob, output);
        output.push(("LastPoseTick".into(), Nbt::Long(last_pose_change_tick(&mob.entity))));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        AbstractHorse::read_additional_save_data(mob, input);
        let pose_tick = input.get("LastPoseTick").and_then(long_value).unwrap_or(DEFAULT_LAST_POSE_CHANGE_TICK);
        if pose_tick < 0 && mob.entity.pose() != Pose::Sitting {
            mob.entity.set_pose(Pose::Sitting);
            mob.refresh_dimensions();
        }
        reset_last_pose_change_tick(&mut mob.entity, pose_tick);
    }
}

impl ServerLevel {
    pub fn is_camel(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Camel(_) | MobKind::CamelHusk(_)))
    }

    pub fn camel_is_camel_sitting(&self, id: EntityId) -> bool {
        is_camel_sitting(self.entity(id))
    }

    pub fn camel_get_pose_time(&self, id: EntityId) -> i64 {
        get_pose_time(self.entity(id), self.game_time)
    }

    pub fn camel_is_in_pose_transition(&self, id: EntityId) -> bool {
        is_in_pose_transition(self.entity(id), self.game_time)
    }

    pub fn camel_refuse_to_move(&self, id: EntityId) -> bool {
        refuse_to_move(self.entity(id), self.game_time)
    }

    pub fn camel_custom_server_ai_step(&mut self, id: EntityId) {
        Brain::tick(self, id);
        if self.mob(id).is_none() {
            return;
        }
        camel_ai::update_activity(self.get_brain_mut(id));
        self.animal_custom_server_ai_step(id);
    }

    pub fn camel_tick(&mut self, id: EntityId) {
        self.abstract_horse_tick(id);
        let Some(mob) = self.mob(id) else { return };
        let e = &mob.entity;
        if is_dashing(e) && camel(mob).dash_cooldown < 50 && (e.on_ground || e.is_in_liquid() || self.is_passenger(id)) {
            self.camel_set_dashing(id, false);
        }
        let camel = camel_mut(self.mob_mut(id).expect("a camel"));
        if camel.dash_cooldown > 0 {
            camel.dash_cooldown -= 1;
            if camel.dash_cooldown == 0 {
                let mob = self.mob(id).expect("a camel");
                let sound = get_dash_ready_sound(mob);
                let pos = mob.entity.block_position();
                self.play_sound(None, pos, sound_event(sound), SoundSource::Neutral, 1.0, 1.0);
            }
        }
        if self.camel_refuse_to_move(id) {
            self.clamp_head_rotation_to_body(id);
        }
        if self.camel_is_camel_sitting(id) && self.entity(id).is_in_water() {
            self.camel_stand_up_instantly(id);
        }
    }

    pub fn camel_travel(&mut self, id: EntityId, input: Vec3) {
        let mut input = input;
        if self.camel_refuse_to_move(id) && self.entity(id).on_ground {
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.multiply(0.0, 1.0, 0.0));
            input = input.multiply(0.0, 1.0, 0.0);
        }
        self.living_entity_travel(id, input);
    }

    pub fn camel_tick_ridden(&mut self, id: EntityId, controller: EntityId, ridden_input: Vec3) {
        self.abstract_horse_tick_ridden(id, controller, ridden_input);
        if self.living(controller).zza > 0.0 && self.camel_is_camel_sitting(id) && !self.camel_is_in_pose_transition(id) {
            self.camel_stand_up(id);
        }
    }

    pub fn camel_get_ridden_speed(&self, id: EntityId, controller: EntityId) -> f32 {
        let movement_bonus = if self.entity(controller).is_sprinting() && camel(self.mob(id).expect("a camel")).dash_cooldown == 0 { RUNNING_SPEED_BONUS } else { 0.0 };
        self.attribute_value(id, Attribute::MovementSpeed) as f32 + movement_bonus
    }

    pub fn camel_get_ridden_rotation(&self, id: EntityId, controller: EntityId) -> (f32, f32) {
        if self.camel_refuse_to_move(id) {
            let e = self.entity(id);
            (e.x_rot, e.y_rot)
        } else {
            let c = self.entity(controller);
            (c.x_rot * 0.5, c.y_rot)
        }
    }

    pub fn camel_get_ridden_input(&self, id: EntityId, controller: EntityId, self_input: Vec3) -> Vec3 {
        if self.camel_refuse_to_move(id) { Vec3::ZERO } else { self.abstract_horse_get_ridden_input(id, controller, self_input) }
    }

    pub fn camel_can_jump(&self, id: EntityId) -> bool {
        !self.camel_refuse_to_move(id) && self.abstract_horse_can_jump(id)
    }

    pub fn camel_execute_riders_jump(&mut self, id: EntityId, amount: f32) {
        let jump_momentum = self.get_jump_power(id) as f64;
        let speed = self.attribute_value(id, Attribute::MovementSpeed);
        let block_speed_factor = self.block_speed_factor(id) as f64;
        let e = self.entity_mut(id);
        let horizontal = e.look_angle().multiply(1.0, 0.0, 1.0).normalize().scale((DASH_HORIZONTAL_MOMENTUM * amount) as f64 * speed * block_speed_factor);
        e.add_delta_movement(horizontal.add(0.0, (DASH_VERTICAL_MOMENTUM * amount) as f64 * jump_momentum, 0.0));
        camel_mut(self.mob_mut(id).expect("a camel")).dash_cooldown = DASH_COOLDOWN_TICKS;
        self.camel_set_dashing(id, true);
        self.entity_mut(id).needs_sync = true;
    }

    pub fn camel_set_dashing(&mut self, id: EntityId, dashing: bool) {
        let mob = self.mob_mut(id).expect("a camel");
        if is_dashing(&mob.entity) == dashing {
            return;
        }
        mob.entity.data.set(DATA_DASH, DataValue::Boolean(dashing));
        if !mob.entity.first_tick {
            let camel = camel_mut(mob);
            camel.dash_cooldown = if camel.dash_cooldown == 0 { DASH_COOLDOWN_TICKS } else { camel.dash_cooldown };
        }
    }

    pub fn camel_handle_start_jump(&mut self, id: EntityId) {
        let sound = get_dashing_sound(self.mob(id).expect("a camel"));
        self.make_sound(id, sound_event(sound));
        self.camel_set_dashing(id, true);
    }

    pub fn camel_get_jump_cooldown(&self, id: EntityId) -> i32 {
        camel(self.mob(id).expect("a camel")).dash_cooldown
    }

    pub fn camel_play_step_sound(&mut self, id: EntityId, block_state: wafer_data::BlockState) {
        let husk = matches!(self.mob(id).expect("a camel").kind, MobKind::CamelHusk(_));
        let sand = block_state.is(&block_tags::CAMEL_SAND_STEP_SOUND_BLOCKS);
        if husk {
            camel_husk::play_step_sound(self, id, sand);
        } else {
            let sound = if sand { "minecraft:entity.camel.step_sand" } else { "minecraft:entity.camel.step" };
            self.entity_play_sound(id, sound_event(sound), 1.0, 1.0);
        }
    }

    pub fn camel_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if self.player(player).is_secondary_use_active() && !self.is_baby(id) {
            self.camel_open_custom_inventory_screen(id, player);
            return InteractionResult::SUCCESS;
        }
        if !item_stack.is_empty() {
            let interaction_result = crate::world::item::interact_living_entity(self, &mut item_stack, player, id, hand);
            self.player_mut(player).update_item_in_hand(hand, &item_stack);
            if interaction_result.consumes_action() {
                return interaction_result;
            }
        }
        if self.is_food(id, &item_stack) {
            return self.fed_food(id, player, hand, &mut item_stack);
        }
        if self.passengers(id).len() < 2 && !self.is_baby(id) {
            self.do_player_ride(id, player);
            return InteractionResult::CONSUME;
        }
        if self.is_baby(id) && self.is_holding(player, items::GOLDEN_DANDELION) { self.abstract_horse_mob_interact(id, player, hand) } else { InteractionResult::Fail }
    }

    pub fn camel_on_elastic_leash_pull(&mut self, id: EntityId) {
        if self.camel_is_camel_sitting(id) && !self.camel_is_in_pose_transition(id) && self.camel_can_camel_change_pose(id) {
            self.camel_stand_up(id);
        }
    }

    pub fn camel_can_camel_change_pose(&self, id: EntityId) -> bool {
        let pose = if self.camel_is_camel_sitting(id) { Pose::Standing } else { Pose::Sitting };
        self.would_not_suffocate_at_target_pose(id, pose)
    }

    pub fn camel_handle_eating(&mut self, id: EntityId, player: EntityId, item_stack: &ItemStack) -> bool {
        if !self.is_food(id, item_stack) {
            return false;
        }
        let could_heal = self.health(id) < self.max_health(id);
        if could_heal {
            self.heal(id, 2.0);
        }
        let could_set_in_love = self.is_tamed(id) && self.ageable_age(id) == 0 && self.can_fall_in_love(id);
        if could_set_in_love {
            self.set_in_love(id, Some(player));
        }
        let could_age_up = self.can_age_up(id);
        if could_age_up {
            let e = self.entity_mut(id);
            e.random_x(1.0);
            e.random_y();
            e.random_z(1.0);
            self.age_up(id, 10, false);
        }
        if !could_heal && !could_set_in_love && !could_age_up {
            return false;
        }
        if !self.entity(id).is_silent() {
            let eating_sound = get_eating_sound(self.mob(id).expect("a camel"));
            let source = self.sound_source(id);
            let e = self.entity_mut(id);
            let (x, y, z) = (e.x(), e.y(), e.z());
            let pitch = 1.0 + (e.random.next_float() - e.random.next_float()) * 0.2;
            self.play_sound_at(None, x, y, z, sound_event(eating_sound), source, 1.0, pitch);
        }
        true
    }

    pub fn camel_can_mate(&self, id: EntityId, partner: EntityId) -> bool {
        partner != id && self.is_camel(partner) && self.can_parent(id) && self.can_parent(partner)
    }

    pub fn camel_get_breed_offspring(&mut self, _id: EntityId, _partner: EntityId) -> Option<Box<Mob>> {
        Some(Box::new(Camel::create()))
    }

    pub fn camel_actually_hurt(&mut self, id: EntityId, source: &DamageSource, dmg: f32) {
        self.camel_stand_up_instantly(id);
        self.animal_actually_hurt(id, source, dmg);
    }

    pub fn camel_passenger_attachment_point(&self, id: EntityId, passenger: EntityId) -> Vec3 {
        let e = self.entity(id);
        let scale = self.attribute_value(id, Attribute::Scale) as f32 * get_age_scale(self.is_baby(id));
        let index = e.passengers.iter().position(|&p| p == passenger).unwrap_or(0);
        let driver = index == 0;
        let mut offset = 0.5f32;
        let height = if e.is_removed() { 0.01f32 as f64 } else { self.camel_get_body_anchor_animation_y_offset(id, driver, 0.0, e.dimensions, scale) } as f32;
        if e.passengers.len() > 1 {
            if !driver {
                offset = -0.7;
            }
            if self.mob(passenger).is_some_and(|mob| mob.animal().is_some()) {
                offset += 0.2;
            }
        }
        Vec3::new(0.0, height as f64, (offset * scale) as f64).y_rot(-e.y_rot * mth::DEG_TO_RAD)
    }

    fn camel_get_body_anchor_animation_y_offset(&self, id: EntityId, is_front: bool, partial_ticks: f32, dimensions: EntityDimensions, scale: f32) -> f64 {
        let age_sit_y_offset = if self.is_baby(id) { 0.09375 } else { 0.375 };
        let mut base_sit_offset = dimensions.height as f64 - age_sit_y_offset;
        let sitting_height_difference = scale * SITTING_HEIGHT_DIFFERENCE;
        let vertical_drop = sitting_height_difference - scale * 0.2;
        let bottom_point = sitting_height_difference - vertical_drop;
        let is_in_transition = self.camel_is_in_pose_transition(id);
        let is_sitting = self.camel_is_camel_sitting(id);
        if is_in_transition {
            let animation_duration = if is_sitting { SITDOWN_DURATION_TICKS } else { STANDUP_DURATION_TICKS };
            let (half_point, flex_point_offset) = if is_sitting {
                (28, if is_front { 0.5 } else { 0.1 })
            } else {
                (if is_front { 24 } else { 32 }, if is_front { 0.6 } else { 0.35 })
            };
            let pose_time = mth::clamp_f(self.camel_get_pose_time(id) as f32 + partial_ticks, 0.0, animation_duration as f32);
            let is_first_part = pose_time < half_point as f32;
            let part = if is_first_part { pose_time / half_point as f32 } else { (pose_time - half_point as f32) / (animation_duration - half_point) as f32 };
            let flex_point = sitting_height_difference - flex_point_offset * vertical_drop;
            base_sit_offset += if is_sitting {
                mth::lerp_f(part, if is_first_part { sitting_height_difference } else { flex_point }, if is_first_part { flex_point } else { bottom_point })
            } else {
                mth::lerp_f(part, if is_first_part { bottom_point - sitting_height_difference } else { bottom_point - flex_point }, if is_first_part { bottom_point - flex_point } else { 0.0 })
            } as f64;
        }
        if is_sitting && !is_in_transition {
            base_sit_offset += bottom_point as f64;
        }
        base_sit_offset
    }

    pub fn camel_can_add_passenger(&self, id: EntityId) -> bool {
        self.passengers(id).len() <= 2
    }

    pub fn camel_sit_down(&mut self, id: EntityId) {
        if !self.camel_is_camel_sitting(id) {
            let sound = get_sit_down_sound(self.mob(id).expect("a camel"));
            self.make_sound(id, sound_event(sound));
            self.set_entity_pose(id, Pose::Sitting);
            let game_time = self.game_time;
            reset_last_pose_change_tick(self.entity_mut(id), -game_time);
        }
    }

    pub fn camel_stand_up(&mut self, id: EntityId) {
        if self.camel_is_camel_sitting(id) {
            let sound = get_stand_up_sound(self.mob(id).expect("a camel"));
            self.make_sound(id, sound_event(sound));
            self.set_entity_pose(id, Pose::Standing);
            let game_time = self.game_time;
            reset_last_pose_change_tick(self.entity_mut(id), game_time);
        }
    }

    pub fn camel_stand_up_instantly(&mut self, id: EntityId) {
        self.set_entity_pose(id, Pose::Standing);
        let game_time = self.game_time;
        reset_last_pose_change_tick_to_full_stand(self.entity_mut(id), game_time);
    }

    pub fn camel_open_custom_inventory_screen(&mut self, id: EntityId, player: EntityId) {
        self.open_horse_inventory(player, id);
    }

    pub fn camel_body_rotation_control_client_tick(&mut self, id: EntityId) {
        if !self.camel_refuse_to_move(id) {
            self.body_rotation_control_tick(id);
        }
    }

    pub fn camel_look_control_tick(&mut self, id: EntityId) {
        if !self.has_controlling_passenger(id) {
            self.look_control_look_control_tick(id);
        }
    }

    pub fn camel_move_control_tick(&mut self, id: EntityId) {
        let mob = self.mob(id).expect("a camel");
        if mob.ai.move_control.operation == Operation::MoveTo
            && !self.is_leashed(id)
            && self.camel_is_camel_sitting(id)
            && !self.camel_is_in_pose_transition(id)
            && self.camel_can_camel_change_pose(id)
        {
            self.camel_stand_up(id);
        }
        self.move_control_tick(id);
    }
}
