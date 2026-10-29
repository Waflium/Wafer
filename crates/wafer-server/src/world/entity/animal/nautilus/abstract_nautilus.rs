use wafer_core::random::RandomSource;
use wafer_core::{Vec3, mth};
use wafer_data::{EquipmentSlot, item_tags, items, mob_effects};

use super::nautilus_ai;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::effect::mob_effect_instance::MobEffectInstance;
use crate::world::entity::ai::control::look_control::LookControlKind;
use crate::world::entity::ai::control::move_control::MoveControlKind;
use crate::world::entity::ai::control::smooth_swimming_move_control::SmoothSwimmingMoveControl;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::data::DataValue;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::tamable_animal::{TamableAnimal, tamable_animal_defaults};
use crate::world::entity::{Entity, EntityId, MoverType, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::pathfinder::path_type::PathType;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::simple_container::{SimpleContainer, SimpleContainerKind};
use crate::world::{InteractionHand, InteractionResult};

pub const INVENTORY_SLOT_OFFSET: i32 = 500;
pub const SMALL_RESTRICTION_RADIUS: i32 = 16;
pub const LARGE_RESTRICTION_RADIUS: i32 = 32;
pub const RESTRICTION_RADIUS_BUFFER: i32 = 8;
const EFFECT_DURATION: i32 = 60;
const EFFECT_REFRESH_RATE: i64 = 40;
const NAUTILUS_WATER_RESISTANCE: f64 = 0.9;
const IN_WATER_SPEED_MODIFIER: f32 = 0.011;
const RIDDEN_SPEED_MODIFIER_IN_WATER: f32 = 0.0325;
const RIDDEN_SPEED_MODIFIER_ON_LAND: f32 = 0.02;
pub const DATA_DASH: usize = 20;
const DASH_COOLDOWN_TICKS: i32 = 40;
const DASH_MINIMUM_DURATION_TICKS: i32 = 5;
const DASH_MOMENTUM_IN_WATER: f32 = 1.2;
const DASH_MOMENTUM_ON_LAND: f32 = 0.5;
const BUBBLE_PROBABILITY_MULTIPLIER: f64 = 2.0;
const BUBBLE_PROBABILITY_MIN: f32 = 0.15;
const BUBBLE_PROBABILITY_MAX: f64 = 1.0;
pub const MOVE_CONTROL: SmoothSwimmingMoveControl = SmoothSwimmingMoveControl::new(85, 10, IN_WATER_SPEED_MODIFIER, 0.0, true);

pub struct AbstractNautilus {
    pub tamable_animal: TamableAnimal,
    pub dash_cooldown: i32,
    pub player_jump_pending_scale: f32,
    pub inventory: SimpleContainer,
    pub inventory_version: u32,
}

impl Default for AbstractNautilus {
    fn default() -> AbstractNautilus {
        AbstractNautilus {
            tamable_animal: TamableAnimal::default(),
            dash_cooldown: 0,
            player_jump_pending_scale: 0.0,
            inventory: SimpleContainer::new(SimpleContainerKind::Plain, 0),
            inventory_version: 0,
        }
    }
}

pub fn abstract_nautilus_defaults() -> Vec<DataValue> {
    let mut data = tamable_animal_defaults();
    data.push(DataValue::Boolean(false));
    data
}

pub fn abstract_nautilus(mob: &Mob) -> &AbstractNautilus {
    mob.abstract_nautilus().expect("an abstract nautilus")
}

pub fn abstract_nautilus_mut(mob: &mut Mob) -> &mut AbstractNautilus {
    mob.abstract_nautilus_mut().expect("an abstract nautilus")
}

pub fn is_dashing(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_DASH), DataValue::Boolean(true))
}

pub fn get_inventory_size(mob: &Mob) -> usize {
    crate::world::inventory::abstract_mount_inventory_menu::get_inventory_size(get_inventory_columns(mob) as usize)
}

pub fn get_inventory_columns(_mob: &Mob) -> i32 {
    0
}

pub fn is_pushed_by_fluid() -> bool {
    false
}

pub fn get_walk_target_value() -> f32 {
    0.0
}

pub fn remove_when_far_away() -> bool {
    true
}

pub fn check_spawn_obstruction(mob: &Mob, level: &mut (impl ServerLevelAccessor + ?Sized)) -> bool {
    level.is_unobstructed(&mob.entity.bounding_box())
}

pub fn finalize_spawn(level: &mut (impl ServerLevelAccessor + ?Sized), mob: &mut Mob) {
    nautilus_ai::init_memories(mob, level.get_random());
}

fn get_dash_sound(mob: &Mob) -> &'static str {
    match mob.kind {
        MobKind::ZombieNautilus(_) => super::zombie_nautilus::get_dash_sound(&mob.entity),
        _ => super::nautilus::get_dash_sound(&mob.entity),
    }
}

fn get_dash_ready_sound(mob: &Mob) -> &'static str {
    match mob.kind {
        MobKind::ZombieNautilus(_) => super::zombie_nautilus::get_dash_ready_sound(&mob.entity),
        _ => super::nautilus::get_dash_ready_sound(&mob.entity),
    }
}

pub fn get_swim_sound(mob: &Mob) -> &'static str {
    match mob.kind {
        MobKind::ZombieNautilus(_) => super::zombie_nautilus::get_swim_sound(),
        _ => super::nautilus::get_swim_sound(mob.is_baby()),
    }
}

impl AbstractNautilus {
    pub fn init(mob: &mut Mob) {
        crate::world::entity::animal::Animal::init(mob);
        mob.ai.move_control.kind = MoveControlKind::SmoothSwimming(MOVE_CONTROL);
        mob.ai.look_control.kind = LookControlKind::SmoothSwimming(10);
        mob.set_pathfinding_malus(PathType::Water, 0.0);
        create_inventory(mob);
    }
}

pub fn create_inventory(mob: &mut Mob) {
    let size = get_inventory_size(mob);
    let nautilus = abstract_nautilus_mut(mob);
    let mut inventory = SimpleContainer::new(SimpleContainerKind::Plain, size);
    let max = nautilus.inventory.items.len().min(size);
    for slot in 0..max {
        let item_stack = &nautilus.inventory.items[slot];
        if !item_stack.is_empty() {
            inventory.items[slot] = item_stack.copy();
        }
    }
    nautilus.inventory = inventory;
    nautilus.inventory_version += 1;
}

impl ServerLevel {
    pub fn is_abstract_nautilus(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| mob.abstract_nautilus().is_some())
    }

    pub fn abstract_nautilus_is_food(&self, id: EntityId, item_stack: &ItemStack) -> bool {
        if !self.is_tame(id) && !self.is_baby(id) { item_stack.get_item().is(&item_tags::NAUTILUS_TAMING_ITEMS) } else { item_stack.get_item().is(&item_tags::NAUTILUS_FOOD) }
    }

    pub fn abstract_nautilus_use_player_item(&mut self, player: EntityId, hand: InteractionHand, item_stack: &mut ItemStack) {
        if item_stack.get_item().is(&item_tags::NAUTILUS_BUCKET_FOOD) {
            let result = crate::world::item::item_utils::create_filled_result(self, player, hand, item_stack.clone(), ItemStack::new(items::WATER_BUCKET, 1), true);
            self.player_mut(player).set_item_in_hand(hand, Some(result));
        } else {
            self.use_player_item(player, hand, item_stack);
        }
    }

    pub fn abstract_nautilus_can_use_slot(&self, id: EntityId, slot: EquipmentSlot) -> bool {
        if slot != EquipmentSlot::Saddle && slot != EquipmentSlot::Body { true } else { self.is_alive(id) && !self.is_baby(id) && self.is_tame(id) }
    }

    pub fn abstract_nautilus_can_add_passenger(&self, id: EntityId) -> bool {
        !self.is_vehicle(id)
    }

    pub fn abstract_nautilus_get_controlling_passenger(&self, id: EntityId) -> Option<EntityId> {
        if self.is_saddled(id)
            && let Some(passenger) = self.first_passenger(id).filter(|&passenger| self.is_player(passenger))
        {
            return Some(passenger);
        }
        self.mob_get_controlling_passenger(id)
    }

    pub fn abstract_nautilus_get_ridden_input(&self, controller: EntityId) -> Vec3 {
        let c = self.living(controller);
        let strafe = c.xxa;
        let mut forward = 0.0f32;
        let mut up = 0.0f32;
        if c.zza != 0.0 {
            let x_rot = self.entity(controller).x_rot * mth::DEG_TO_RAD;
            let mut forward_look = mth::cos(x_rot as f64) as f32;
            let mut up_look = -mth::sin(x_rot as f64) as f32;
            if c.zza < 0.0 {
                forward_look *= -0.5;
                up_look *= -0.5;
            }
            up = up_look;
            forward = forward_look;
        }
        Vec3::new(strafe as f64, up as f64, forward as f64)
    }

    fn abstract_nautilus_get_ridden_rotation(&self, controller: EntityId) -> (f32, f32) {
        let c = self.entity(controller);
        (c.x_rot * 0.5, c.y_rot)
    }

    pub fn abstract_nautilus_tick_ridden(&mut self, id: EntityId, controller: EntityId, ridden_input: Vec3) {
        self.living_entity_tick_ridden(id, controller, ridden_input);
        let (rotation_x, rotation_y) = self.abstract_nautilus_get_ridden_rotation(controller);
        let e = self.entity_mut(id);
        let mut y_rot = e.y_rot;
        let diff = mth::wrap_degrees(rotation_y - y_rot);
        y_rot += diff * 0.5;
        e.set_rot(y_rot, rotation_x);
        e.y_rot_o = y_rot;
        e.y_head_rot = y_rot;
        self.living_mut(id).y_body_rot = y_rot;
        if self.is_local_instance_authoritative(id) {
            let pending = abstract_nautilus(self.mob(id).expect("an abstract nautilus")).player_jump_pending_scale;
            if pending > 0.0 && !self.living(id).jumping {
                self.abstract_nautilus_execute_riders_jump(id, pending, controller);
            }
            abstract_nautilus_mut(self.mob_mut(id).expect("an abstract nautilus")).player_jump_pending_scale = 0.0;
        }
    }

    pub fn abstract_nautilus_travel_in_water(&mut self, id: EntityId, input: Vec3) {
        let speed = self.living(id).speed;
        self.entity_mut(id).move_relative(speed, input);
        let delta = self.entity(id).delta_movement;
        self.move_entity(id, MoverType::SelfMove, delta);
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.scale(NAUTILUS_WATER_RESISTANCE));
    }

    pub fn abstract_nautilus_get_ridden_speed(&self, id: EntityId) -> f32 {
        let speed = self.attribute_value(id, Attribute::MovementSpeed) as f32;
        if self.entity(id).is_in_water() { RIDDEN_SPEED_MODIFIER_IN_WATER * speed } else { RIDDEN_SPEED_MODIFIER_ON_LAND * speed }
    }

    fn abstract_nautilus_do_player_ride(&mut self, id: EntityId, player: EntityId) {
        self.start_riding(player, id, false, true);
        if !self.is_vehicle(id) {
            self.mob_mut(id).expect("an abstract nautilus").clear_home();
        }
    }

    fn get_nautilus_restriction_radius(&self, id: EntityId) -> i32 {
        if !self.is_baby(id) && self.item_by_slot_ref(id, EquipmentSlot::Saddle).is_none() { LARGE_RESTRICTION_RADIUS } else { SMALL_RESTRICTION_RADIUS }
    }

    fn abstract_nautilus_check_restriction(&mut self, id: EntityId) {
        if !self.is_leashed(id) && !self.is_vehicle(id) && self.is_tame(id) {
            let radius = self.get_nautilus_restriction_radius(id);
            let block_position = self.entity(id).block_position();
            let mob = self.mob_mut(id).expect("an abstract nautilus");
            let close = (mob.home_position.dist_sqr(block_position)) < ((radius + RESTRICTION_RADIUS_BUFFER) as f64).powi(2);
            if !mob.has_home() || !close || radius != mob.home_radius {
                mob.set_home_to(block_position, radius);
            }
        }
    }

    pub fn abstract_nautilus_custom_server_ai_step(&mut self, id: EntityId) {
        self.abstract_nautilus_check_restriction(id);
        self.animal_custom_server_ai_step(id);
    }

    fn abstract_nautilus_apply_effects(&mut self, id: EntityId) {
        if let Some(player) = self.first_passenger(id).filter(|&passenger| self.is_player(passenger)) {
            let has_effect = self.living(player).has_effect(mob_effects::BREATH_OF_THE_NAUTILUS);
            let should_refresh = self.game_time % EFFECT_REFRESH_RATE == 0;
            if !has_effect || should_refresh {
                self.add_effect(player, MobEffectInstance::with(mob_effects::BREATH_OF_THE_NAUTILUS, EFFECT_DURATION, 0, true, true, true), None);
            }
        }
    }

    fn abstract_nautilus_spawn_bubbles(&mut self, id: EntityId) {
        let e = self.entity_mut(id);
        let speed = e.delta_movement.length();
        let bubble_probability = mth::clamp(speed * BUBBLE_PROBABILITY_MULTIPLIER, BUBBLE_PROBABILITY_MIN as f64, BUBBLE_PROBABILITY_MAX);
        if (e.random.next_float() as f64) < bubble_probability {
            e.random.next_double();
            e.random.next_float();
            e.random.next_float();
            e.random.next_float();
        }
    }

    pub fn abstract_nautilus_tick(&mut self, id: EntityId) {
        self.mob_tick(id);
        self.then_tick(move |level| level.abstract_nautilus_tick_end(id));
    }

    fn abstract_nautilus_tick_end(&mut self, id: EntityId) {
        self.abstract_nautilus_apply_effects(id);
        let mob = self.mob(id).expect("an abstract nautilus");
        if is_dashing(&mob.entity) && abstract_nautilus(mob).dash_cooldown < DASH_COOLDOWN_TICKS - DASH_MINIMUM_DURATION_TICKS {
            self.abstract_nautilus_set_dashing(id, false);
        }
        let nautilus = abstract_nautilus_mut(self.mob_mut(id).expect("an abstract nautilus"));
        if nautilus.dash_cooldown > 0 {
            nautilus.dash_cooldown -= 1;
            if nautilus.dash_cooldown == 0 {
                let sound = get_dash_ready_sound(self.mob(id).expect("an abstract nautilus"));
                self.make_sound(id, sound_event(sound));
            }
        }
        if self.entity(id).is_in_water() {
            self.abstract_nautilus_spawn_bubbles(id);
        }
    }

    pub fn abstract_nautilus_can_jump(&self, id: EntityId) -> bool {
        self.is_saddled(id)
    }

    pub fn abstract_nautilus_set_dashing(&mut self, id: EntityId, dashing: bool) {
        let mob = self.mob_mut(id).expect("an abstract nautilus");
        if is_dashing(&mob.entity) == dashing {
            return;
        }
        mob.entity.data.set(DATA_DASH, DataValue::Boolean(dashing));
        if !mob.entity.first_tick {
            let nautilus = abstract_nautilus_mut(mob);
            nautilus.dash_cooldown = if nautilus.dash_cooldown == 0 { DASH_COOLDOWN_TICKS } else { nautilus.dash_cooldown };
        }
    }

    fn abstract_nautilus_execute_riders_jump(&mut self, id: EntityId, amount: f32, controller: EntityId) {
        let momentum = if self.entity(id).is_in_water() { DASH_MOMENTUM_IN_WATER } else { DASH_MOMENTUM_ON_LAND };
        let speed = self.attribute_value(id, Attribute::MovementSpeed);
        let block_speed_factor = self.block_speed_factor(id) as f64;
        let look_angle = self.entity(controller).look_angle();
        self.entity_mut(id).add_delta_movement(look_angle.scale((momentum * amount) as f64 * speed * block_speed_factor));
        abstract_nautilus_mut(self.mob_mut(id).expect("an abstract nautilus")).dash_cooldown = DASH_COOLDOWN_TICKS;
        self.abstract_nautilus_set_dashing(id, true);
        self.entity_mut(id).needs_sync = true;
    }

    pub fn abstract_nautilus_handle_start_jump(&mut self, id: EntityId) {
        let sound = get_dash_sound(self.mob(id).expect("an abstract nautilus"));
        self.make_sound(id, sound_event(sound));
        self.entity_game_event(id, crate::world::level::gameevent::game_event::ENTITY_ACTION);
        self.abstract_nautilus_set_dashing(id, true);
    }

    pub fn abstract_nautilus_get_jump_cooldown(&self, id: EntityId) -> i32 {
        abstract_nautilus(self.mob(id).expect("an abstract nautilus")).dash_cooldown
    }

    pub fn abstract_nautilus_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if self.is_baby(id) {
            return self.animal_mob_interact(id, player, hand);
        }
        if self.is_tame(id) && self.player(player).is_secondary_use_active() {
            self.abstract_nautilus_open_custom_inventory_screen(id, player);
            return InteractionResult::SUCCESS;
        }
        if !item_stack.is_empty() {
            if !self.is_tame(id) && self.is_food(id, &item_stack) {
                self.abstract_nautilus_use_player_item(player, hand, &mut item_stack);
                self.abstract_nautilus_try_to_tame(id, player);
                return InteractionResult::SUCCESS_SERVER;
            }
            if self.is_food(id, &item_stack) && self.health(id) < self.max_health(id) {
                self.tamable_animal_feed(id, player, hand, &mut item_stack, 2.0, 1.0);
                return InteractionResult::SUCCESS;
            }
            let interaction_result = crate::world::item::interact_living_entity(self, &mut item_stack, player, id, hand);
            self.player_mut(player).update_item_in_hand(hand, &item_stack);
            if interaction_result.consumes_action() {
                return interaction_result;
            }
        }
        if self.is_tame(id) && !self.player(player).is_secondary_use_active() && !self.is_food(id, &item_stack) {
            self.abstract_nautilus_do_player_ride(id, player);
            InteractionResult::SUCCESS
        } else {
            self.animal_mob_interact(id, player, hand)
        }
    }

    fn abstract_nautilus_try_to_tame(&mut self, id: EntityId, player: EntityId) {
        if self.entity_mut(id).random.next_int_bounded(3) == 0 {
            self.tame(id, player);
            self.get_navigation(id, |navigation, _| navigation.stop());
            self.broadcast_entity_event(id, crate::world::entity::entity_event::TAMING_SUCCEEDED);
        } else {
            self.broadcast_entity_event(id, crate::world::entity::entity_event::TAMING_FAILED);
        }
        self.play_eating_sound(id);
    }

    pub fn abstract_nautilus_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        let source_entity = source.causing_entity;
        let was_hurt = self.living_entity_hurt_server(id, source, damage);
        if was_hurt
            && let Some(source_entity) = source_entity.filter(|&entity| self.is_living(entity))
            && self.mob(id).is_some()
        {
            nautilus_ai::set_anger_target(self, id, source_entity);
        }
        was_hurt
    }

    pub fn abstract_nautilus_can_be_affected(&self, id: EntityId, effect: &MobEffectInstance) -> bool {
        if effect.effect == mob_effects::POISON { false } else { self.living_entity_can_be_affected(id, effect) }
    }

    pub fn abstract_nautilus_get_equip_sound_for_saddle(&self, id: EntityId) -> &'static str {
        if self.entity(id).is_under_water() { "minecraft:item.nautilus_saddle_underwater_equip" } else { "minecraft:item.nautilus_saddle_equip" }
    }

    pub fn abstract_nautilus_open_custom_inventory_screen(&mut self, id: EntityId, player: EntityId) {
        if (!self.is_vehicle(id) || self.has_passenger(id, player)) && self.is_tame(id) {
            self.open_nautilus_inventory(player, id);
        }
    }

    pub fn abstract_nautilus_get_slot(&self, id: EntityId, slot: i32) -> Option<crate::world::entity::slot_access::LevelSlot> {
        let inventory_slot = slot - INVENTORY_SLOT_OFFSET;
        let size = abstract_nautilus(self.mob(id).expect("an abstract nautilus")).inventory.items.len() as i32;
        if inventory_slot >= 0 && inventory_slot < size {
            return Some(crate::world::entity::slot_access::LevelSlot::Container(crate::world::container::ContainerRef::MountInventory(id), inventory_slot as usize));
        }
        None
    }

    pub fn abstract_nautilus_is_mob_controlled(&self, id: EntityId) -> bool {
        self.first_passenger(id).is_some_and(|passenger| self.is_mob(passenger))
    }

    pub fn abstract_nautilus_is_aggravated(&self, id: EntityId) -> bool {
        let brain = self.get_brain(id);
        brain.has_memory_value(memory_module_type::ANGRY_AT) || brain.has_memory_value(memory_module_type::ATTACK_TARGET)
    }

    pub fn abstract_nautilus_requires_custom_persistence(&self, id: EntityId) -> bool {
        self.mob_requires_custom_persistence(id) || self.is_tame(id)
    }
}
