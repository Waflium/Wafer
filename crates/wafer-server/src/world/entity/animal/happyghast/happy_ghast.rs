use wafer_core::{AABB, Vec3, mth};
use wafer_data::{EquipmentSlot, item_tags, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::happy_ghast_ai;
use crate::server::level::ServerLevel;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::brain::{self, Brain, Provider};
use crate::world::entity::ai::control::flying_move_control::FlyingMoveControl;
use crate::world::entity::ai::control::look_control::LookControlKind;
use crate::world::entity::ai::control::move_control::MoveControlKind;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::goal_selector::GoalSelector;
use crate::world::entity::ai::goal::tempt_goal::TemptGoal;
use crate::world::entity::ai::navigation::path_navigation::{PathNavigation, PathNavigationKind};
use crate::world::entity::ai::sensing::sensor_type::SensorType;
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::{Attribute, HAPPY_GHAST_ATTRIBUTES};
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::monster::ghast::{self, GhastMoveControl, RandomFloatAroundGoal};
use crate::world::entity::{Entity, EntityDimensions, EntityId, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::biome::Precipitation;
use crate::world::level::block::entity::int_value;
use crate::world::{InteractionHand, InteractionResult};
use crate::world::level::storage::tag_value_input;

pub const BABY_SCALE: f32 = 0.2375;
pub const WANDER_GROUND_DISTANCE: i32 = 16;
pub const SMALL_RESTRICTION_RADIUS: i32 = 32;
pub const LARGE_RESTRICTION_RADIUS: i32 = 64;
pub const RESTRICTION_RADIUS_BUFFER: i32 = 16;
pub const FAST_HEALING_TICKS: i32 = 20;
pub const SLOW_HEALING_TICKS: i32 = 600;
pub const MAX_PASSANGERS: usize = 4;
const STILL_TIMEOUT_ON_LOAD_GRACE_PERIOD: i32 = 60;
const MAX_STILL_TIMEOUT: i32 = 10;
pub const SPEED_MULTIPLIER_WHEN_PANICKING: f32 = 2.0;
pub const DATA_IS_LEASH_HOLDER: usize = 18;
pub const DATA_STAYS_STILL: usize = 19;
const MAX_SCALE: f32 = 1.0;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(4.0, 4.0)
    .with_eye_height(2.6)
    .with_passenger_attachments(&[Vec3::new(0.0, 4.0, 1.7), Vec3::new(-1.7, 4.0, 0.0), Vec3::new(0.0, 4.0, -1.7), Vec3::new(1.7, 4.0, 0.0)])
    .with_vehicle_attachment(&[Vec3::new(0.0, -0.5f32 as f64, 0.0)]);
const MOVE_CONTROL: GhastMoveControl = GhastMoveControl::new(true);
const BABY_MOVE_CONTROL: FlyingMoveControl = FlyingMoveControl::new(180, true);

pub static BRAIN_PROVIDER: Provider = brain::provider(
    &[SensorType::NearestLivingEntities, SensorType::HurtBy, SensorType::FoodTemptations, SensorType::NearestAdultAnyType, SensorType::NearestPlayers],
    |_| happy_ghast_ai::get_activities(),
);

#[derive(Default)]
pub struct HappyGhast {
    pub animal: Animal,
    leash_holder_time: i32,
    server_still_timeout: i32,
}

pub fn happy_ghast(mob: &Mob) -> &HappyGhast {
    match &mob.kind {
        MobKind::HappyGhast(happy_ghast) => happy_ghast,
        _ => unreachable!("a happy ghast"),
    }
}

pub fn happy_ghast_mut(mob: &mut Mob) -> &mut HappyGhast {
    match &mut mob.kind {
        MobKind::HappyGhast(happy_ghast) => happy_ghast,
        _ => unreachable!("a happy ghast"),
    }
}

pub fn baby_dimensions() -> EntityDimensions {
    DIMENSIONS.scale(BABY_SCALE).with_eye_height(0.46875)
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::HAPPY_GHAST_FOOD)
}

pub fn sanitize_scale(scale: f32) -> f32 {
    scale.min(MAX_SCALE)
}

pub fn get_ambient_sound(baby: bool) -> &'static str {
    if baby { "minecraft:entity.ghastling.ambient" } else { "minecraft:entity.happy_ghast.ambient" }
}

pub fn get_hurt_sound(baby: bool) -> &'static str {
    if baby { "minecraft:entity.ghastling.hurt" } else { "minecraft:entity.happy_ghast.hurt" }
}

pub fn get_death_sound(baby: bool) -> &'static str {
    if baby { "minecraft:entity.ghastling.death" } else { "minecraft:entity.happy_ghast.death" }
}

pub fn get_sound_volume(baby: bool) -> f32 {
    if baby { 1.0 } else { 4.0 }
}

pub fn get_voice_pitch() -> f32 {
    1.0
}

pub fn get_max_spawn_cluster_size() -> i32 {
    1
}

pub fn can_fall_in_love() -> bool {
    false
}

pub fn on_climbable() -> bool {
    false
}

pub fn leash_elastic_distance() -> f64 {
    10.0
}

pub fn leash_snap_distance() -> f64 {
    16.0
}

pub fn stays_still(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_STAYS_STILL), DataValue::Boolean(true))
}

pub fn is_on_still_timeout(mob: &Mob) -> bool {
    stays_still(&mob.entity) || happy_ghast(mob).server_still_timeout > 0
}

fn sync_stay_still_flag(mob: &mut Mob) {
    let stays_still = happy_ghast(mob).server_still_timeout > 0;
    mob.entity.data.set(DATA_STAYS_STILL, DataValue::Boolean(stays_still));
}

fn tempt_items(level: &crate::server::level::ServerLevel, id: EntityId, item_stack: &ItemStack) -> bool {
    if !level.is_wearing_body_armor(id) && !level.is_baby(id) { item_stack.get_item().is(&item_tags::HAPPY_GHAST_TEMPT_ITEMS) } else { is_food(item_stack) }
}

impl HappyGhast {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        data.extend([DataValue::Boolean(false), DataValue::Boolean(false)]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:happy_ghast"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&HAPPY_GHAST_ATTRIBUTES), MobKind::HappyGhast(HappyGhast::default()));
        Animal::init(&mut mob);
        mob.ai.move_control.kind = MoveControlKind::Ghast(MOVE_CONTROL);
        mob.ai.look_control.kind = LookControlKind::HappyGhast;
        mob
    }

    pub fn make_brain(mob: &mut Mob, packed_brain: brain::Packed) -> Brain {
        BRAIN_PROVIDER.make_brain(mob, packed_brain)
    }

    pub fn register_goals(ai: &mut MobAi) {
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(3, Goal::HappyGhastFloat(float_goal));
        goal_selector.add_goal(4, Goal::Tempt(TemptGoal::for_non_pathfinders(1.0, tempt_items, false, 7.0)));
        goal_selector.add_goal(5, Goal::RandomFloatAround(RandomFloatAroundGoal::new(WANDER_GROUND_DISTANCE)));
    }

    fn adult_ghast_setup(mob: &mut Mob) {
        mob.ai.move_control.kind = MoveControlKind::Ghast(MOVE_CONTROL);
        mob.ai.look_control.kind = LookControlKind::HappyGhast;
        let follow_range_base = mob.living.attributes.base_value(Attribute::FollowRange);
        mob.ai.navigation = PathNavigation::of_kind(PathNavigationKind::Ground, follow_range_base);
        mob.ai.goal_selector = GoalSelector::default();
        HappyGhast::register_goals(&mut mob.ai);
    }

    fn baby_ghast_setup(mob: &mut Mob) {
        mob.ai.move_control.kind = MoveControlKind::Flying(BABY_MOVE_CONTROL);
        mob.ai.look_control.kind = LookControlKind::Default;
        mob.ai.navigation = create_baby_navigation(mob);
        happy_ghast_mut(mob).server_still_timeout = 0;
        sync_stay_still_flag(mob);
        mob.ai.goal_selector = GoalSelector::default();
    }

    pub fn age_boundary_reached(mob: &mut Mob) {
        if mob.is_baby() {
            HappyGhast::baby_ghast_setup(mob);
        } else {
            HappyGhast::adult_ghast_setup(mob);
        }
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        output.push(("still_timeout".into(), Nbt::Int(happy_ghast(mob).server_still_timeout)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        let still_timeout = tag_value_input::get_numeric(input, "still_timeout").and_then(int_value).unwrap_or(0);
        happy_ghast_mut(mob).server_still_timeout = still_timeout;
        sync_stay_still_flag(mob);
    }
}

fn create_baby_navigation(mob: &Mob) -> PathNavigation {
    let follow_range_base = mob.living.attributes.base_value(Attribute::FollowRange);
    let mut navigation = PathNavigation::of_kind(PathNavigationKind::BabyFlying, follow_range_base);
    navigation.set_can_open_doors(false);
    navigation.set_can_float(true);
    navigation.set_required_path_length_detached(follow_range_base, 48.0);
    navigation
}

impl ServerLevel {
    pub fn is_happy_ghast(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::HappyGhast(_)))
    }

    pub fn happy_ghast_is_on_still_timeout(&self, id: EntityId) -> bool {
        is_on_still_timeout(self.mob(id).expect("a happy ghast"))
    }

    fn happy_ghast_set_server_still_timeout(&mut self, id: EntityId, server_still_timeout: i32) {
        if happy_ghast(self.mob(id).expect("a happy ghast")).server_still_timeout <= 0 && server_still_timeout > 0 {
            let packet = crate::server::level::server_entity::position_sync_of(self.entity(id), self.tracking_position(id));
            self.send_to_tracking_players(id, &packet);
        }
        let mob = self.mob_mut(id).expect("a happy ghast");
        happy_ghast_mut(mob).server_still_timeout = server_still_timeout;
        sync_stay_still_flag(mob);
    }

    pub fn happy_ghast_age_boundary_reached(&mut self, id: EntityId) {
        if !self.is_baby(id) {
            Brain::stop_all(self, id);
            self.get_brain_mut(id).clear_memories();
        }
    }

    pub fn happy_ghast_travel(&mut self, id: EntityId, input: Vec3) {
        let speed = self.attribute_value(id, Attribute::FlyingSpeed) as f32 * 5.0 / 3.0;
        self.travel_flying_with(id, input, speed, speed, speed);
    }

    pub fn happy_ghast_can_breathe_underwater(&self, id: EntityId) -> bool {
        self.is_baby(id)
    }

    pub fn happy_ghast_get_ambient_sound_interval(&self, id: EntityId, interval: i32) -> i32 {
        if self.is_vehicle(id) { interval * 6 } else { interval }
    }

    pub fn happy_ghast_get_breed_offspring(&mut self, _id: EntityId, _partner: EntityId) -> Option<Box<Mob>> {
        Some(Box::new(HappyGhast::create()))
    }

    pub fn happy_ghast_can_use_slot(&self, id: EntityId, slot: EquipmentSlot) -> bool {
        if slot != EquipmentSlot::Body { true } else { self.is_alive(id) && !self.is_baby(id) }
    }

    pub fn happy_ghast_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        if self.is_baby(id) {
            return self.animal_mob_interact(id, player, hand);
        }
        let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if !item_stack.is_empty() {
            let interaction_result = crate::world::item::interact_living_entity(self, &mut item_stack, player, id, hand);
            self.player_mut(player).update_item_in_hand(hand, &item_stack);
            if interaction_result.consumes_action() {
                return interaction_result;
            }
        }
        if self.is_wearing_body_armor(id) && !self.player(player).is_secondary_use_active() {
            self.start_riding(player, id, false, true);
            InteractionResult::SUCCESS
        } else {
            self.animal_mob_interact(id, player, hand)
        }
    }

    pub fn happy_ghast_add_passenger(&mut self, id: EntityId, passenger: EntityId) {
        if !self.is_vehicle(id) {
            self.entity_play_sound(id, sound_event("minecraft:entity.happy_ghast.harness_goggles_down"), 1.0, 1.0);
        }
        self.entity_add_passenger(id, passenger);
        if !self.happy_ghast_scan_player_above_ghast(id) {
            self.happy_ghast_set_server_still_timeout(id, 0);
        } else if happy_ghast(self.mob(id).expect("a happy ghast")).server_still_timeout > MAX_STILL_TIMEOUT {
            self.happy_ghast_set_server_still_timeout(id, MAX_STILL_TIMEOUT);
        }
    }

    pub fn happy_ghast_remove_passenger(&mut self, id: EntityId) {
        self.happy_ghast_set_server_still_timeout(id, MAX_STILL_TIMEOUT);
        if !self.is_vehicle(id) {
            self.mob_mut(id).expect("a happy ghast").clear_home();
            self.entity_play_sound(id, sound_event("minecraft:entity.happy_ghast.harness_goggles_up"), 1.0, 1.0);
        }
    }

    pub fn happy_ghast_can_add_passenger(&self, id: EntityId) -> bool {
        self.passengers(id).len() < MAX_PASSANGERS
    }

    pub fn happy_ghast_get_controlling_passenger(&self, id: EntityId) -> Option<EntityId> {
        if self.is_wearing_body_armor(id)
            && !self.happy_ghast_is_on_still_timeout(id)
            && let Some(player) = self.first_passenger(id).filter(|&passenger| self.is_player(passenger))
        {
            return Some(player);
        }
        self.mob_get_controlling_passenger(id)
    }

    pub fn happy_ghast_get_ridden_input(&self, id: EntityId, controller: EntityId) -> Vec3 {
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
        if c.jumping {
            up += 0.5;
        }
        Vec3::new(strafe as f64, up as f64, forward as f64).scale(3.9f32 as f64 * self.attribute_value(id, Attribute::FlyingSpeed))
    }

    pub fn happy_ghast_tick_ridden(&mut self, id: EntityId, controller: EntityId, ridden_input: Vec3) {
        self.living_entity_tick_ridden(id, controller, ridden_input);
        let c = self.entity(controller);
        let (rotation_x, rotation_y) = (c.x_rot * 0.5, c.y_rot);
        let e = self.entity_mut(id);
        let mut y_rot = e.y_rot;
        let diff = mth::wrap_degrees(rotation_y - y_rot);
        y_rot += diff * 0.08;
        e.set_rot(y_rot, rotation_x);
        e.y_rot_o = y_rot;
        e.y_head_rot = y_rot;
        self.living_mut(id).y_body_rot = y_rot;
    }

    pub fn happy_ghast_custom_server_ai_step(&mut self, id: EntityId) {
        if self.is_baby(id) {
            Brain::tick(self, id);
            if self.mob(id).is_none() {
                return;
            }
            happy_ghast_ai::update_activity(self.get_brain_mut(id));
        }
        self.happy_ghast_check_restriction(id);
        self.animal_custom_server_ai_step(id);
    }

    pub fn happy_ghast_tick(&mut self, id: EntityId) {
        self.mob_tick(id);
        self.then_tick(move |level| level.happy_ghast_tick_end(id));
    }

    fn happy_ghast_tick_end(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a happy ghast");
        let ghast = happy_ghast_mut(mob);
        if ghast.leash_holder_time > 0 {
            ghast.leash_holder_time -= 1;
        }
        let leash_holder = ghast.leash_holder_time > 0;
        mob.entity.data.set(DATA_IS_LEASH_HOLDER, DataValue::Boolean(leash_holder));
        let server_still_timeout = happy_ghast(mob).server_still_timeout;
        if server_still_timeout > 0 {
            let server_still_timeout = if mob.entity.tick_count > STILL_TIMEOUT_ON_LOAD_GRACE_PERIOD { server_still_timeout - 1 } else { server_still_timeout };
            self.happy_ghast_set_server_still_timeout(id, server_still_timeout);
        }
        if self.happy_ghast_scan_player_above_ghast(id) {
            self.happy_ghast_set_server_still_timeout(id, MAX_STILL_TIMEOUT);
        }
    }

    pub fn happy_ghast_ai_step(&mut self, id: EntityId) {
        let on_still_timeout = self.happy_ghast_is_on_still_timeout(id);
        self.entity_mut(id).requires_precise_position = on_still_timeout;
        self.ageable_mob_ai_step(id);
        if self.mob(id).is_some() {
            self.happy_ghast_continuous_heal(id);
        }
    }

    fn get_happy_ghast_restriction_radius(&self, id: EntityId) -> i32 {
        if !self.is_baby(id) && self.item_by_slot_ref(id, EquipmentSlot::Body).is_none() { LARGE_RESTRICTION_RADIUS } else { SMALL_RESTRICTION_RADIUS }
    }

    fn happy_ghast_check_restriction(&mut self, id: EntityId) {
        if !self.is_leashed(id) && !self.is_vehicle(id) {
            let radius = self.get_happy_ghast_restriction_radius(id);
            let block_position = self.entity(id).block_position();
            let mob = self.mob_mut(id).expect("a happy ghast");
            let close = mob.home_position.dist_sqr(block_position) < ((radius + RESTRICTION_RADIUS_BUFFER) as f64).powi(2);
            if !mob.has_home() || !close || radius != mob.home_radius {
                mob.set_home_to(block_position, radius);
            }
        }
    }

    fn happy_ghast_continuous_heal(&mut self, id: EntityId) {
        if self.is_alive(id) && self.living(id).death_time == 0 && self.max_health(id) != self.health(id) {
            let pos = self.entity(id).block_position();
            let is_fast_healing = self.is_in_clouds(id) || self.precipitation_at(pos) != Precipitation::None;
            if self.entity(id).tick_count % if is_fast_healing { FAST_HEALING_TICKS } else { SLOW_HEALING_TICKS } == 0 {
                self.heal(id, 1.0);
            }
        }
    }

    pub fn happy_ghast_on_elastic_leash_pull(&mut self, id: EntityId) {
        self.mob_mut(id).expect("a happy ghast").ai.move_control.set_wait();
    }

    pub fn happy_ghast_notify_leash_holder(&mut self, id: EntityId, entity: EntityId) {
        if self.support_quad_leash(entity) {
            happy_ghast_mut(self.mob_mut(id).expect("a happy ghast")).leash_holder_time = 5;
        }
    }

    fn happy_ghast_scan_player_above_ghast(&self, id: EntityId) -> bool {
        let happy_ghast_bb = self.entity(id).bounding_box();
        let ghast_detection_box = AABB::new(
            happy_ghast_bb.min_x - 1.0,
            happy_ghast_bb.max_y - 1.0e-5f32 as f64,
            happy_ghast_bb.min_z - 1.0,
            happy_ghast_bb.max_x + 1.0,
            happy_ghast_bb.max_y + happy_ghast_bb.y_size() / 2.0,
            happy_ghast_bb.max_z + 1.0,
        );
        self.players.values().any(|player| {
            let player_id = player.entity.id;
            if self.is_spectator(player_id) {
                return false;
            }
            let root_vehicle = self.root_vehicle(player_id);
            !self.is_happy_ghast(root_vehicle) && {
                let position = self.entity(root_vehicle).position();
                ghast_detection_box.contains(position.x, position.y, position.z)
            }
        })
    }

    pub fn happy_ghast_body_rotation_control_client_tick(&mut self, id: EntityId) {
        if self.is_vehicle(id) {
            let y_rot = self.entity(id).y_rot;
            self.entity_mut(id).y_head_rot = y_rot;
            self.living_mut(id).y_body_rot = y_rot;
        }
        self.body_rotation_control_tick(id);
    }

    pub fn happy_ghast_can_be_collided_with(&self, id: EntityId, other: Option<EntityId>) -> bool {
        if !self.is_baby(id) && self.is_alive(id) {
            if self.is_vehicle(id) && other.is_some_and(|other| self.is_happy_ghast(other)) { true } else { self.happy_ghast_is_on_still_timeout(id) }
        } else {
            false
        }
    }

    pub fn happy_ghast_is_flying_vehicle(&self, id: EntityId) -> bool {
        !self.is_baby(id)
    }

    pub fn happy_ghast_dismount_location_for_passenger(&self, id: EntityId) -> Vec3 {
        let e = self.entity(id);
        Vec3::new(e.x(), e.bounding_box().max_y, e.z())
    }

    pub fn happy_ghast_float_goal_can_use(&mut self, id: EntityId, goal: &mut FloatGoal) -> bool {
        !self.happy_ghast_is_on_still_timeout(id) && goal.can_use(self, id)
    }

    pub fn happy_ghast_look_control_tick(&mut self, id: EntityId) {
        if self.happy_ghast_is_on_still_timeout(id) {
            let e = self.entity_mut(id);
            let close_angle = mth::wrap_degrees90(e.y_rot);
            let y_rot = e.y_rot - close_angle;
            e.set_y_rot(y_rot);
            self.mob_mut(id).expect("a happy ghast").set_y_head_rot(y_rot);
            return;
        }
        let mob = self.mob_mut(id).expect("a happy ghast");
        if mob.ai.look_control.look_at_cooldown > 0 {
            mob.ai.look_control.look_at_cooldown -= 1;
            let xdd = mob.ai.look_control.wanted_x - mob.entity.x();
            let zdd = mob.ai.look_control.wanted_z - mob.entity.z();
            mob.entity.set_y_rot(-(mth::atan2(xdd, zdd) as f32) * mth::RAD_TO_DEG);
            mob.living.y_body_rot = mob.entity.y_rot;
            mob.entity.y_head_rot = mob.living.y_body_rot;
        } else {
            ghast::face_movement_direction(self, id);
        }
    }
}

impl ServerLevel {
    pub fn is_flying_vehicle(&self, id: EntityId) -> bool {
        self.is_happy_ghast(id) && self.happy_ghast_is_flying_vehicle(id)
    }
}
