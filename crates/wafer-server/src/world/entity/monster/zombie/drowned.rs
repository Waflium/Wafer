use wafer_core::random::RandomSource;
use wafer_core::shapes::is_face_full;
use wafer_core::{BlockPos, Direction, Vec3, mth};
use wafer_data::{EquipmentSlot, blocks, item_tags, items};

use super::zombie::{self, Zombie};
use crate::server::level::ServerLevel;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::control::move_control::{MoveControlKind, Operation, rotlerp};
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::move_to_block_goal::MoveToBlockGoal;
use crate::world::entity::ai::goal::random_stroll_goal::RandomStrollGoal;
use crate::world::entity::ai::goal::ranged_attack_goal::RangedAttackGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::goal::zombie_attack_goal::ZombieAttackGoal;
use crate::world::entity::ai::util::default_random_pos;
use crate::world::entity::attributes::{Attribute, DROWNED_ATTRIBUTES};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::projectile::arrow::thrown_trident::ThrownTrident;
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{EntityDimensions, EntityId, MoverType, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::path_navigation_region::PathNavigationRegion;
use crate::world::level::pathfinder::path_type::PathType;

pub const NAUTILUS_SHELL_CHANCE: f32 = 0.03;
pub const BABY_DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(0.49, 0.98).with_eye_height(0.775).with_vehicle_attachment(&[Vec3::new(0.0, 0.1875f32 as f64, 0.0)]);

pub struct Drowned {
    pub zombie: Zombie,
    searching_for_land: bool,
    ranged_attack_uncertainty: f32,
}

impl Default for Drowned {
    fn default() -> Drowned {
        Drowned { zombie: Zombie::default(), searching_for_land: false, ranged_attack_uncertainty: -1.0 }
    }
}

fn drowned(mob: &Mob) -> &Drowned {
    match &mob.kind {
        MobKind::Drowned(drowned) => drowned,
        _ => unreachable!("not a drowned"),
    }
}

fn drowned_mut(mob: &mut Mob) -> &mut Drowned {
    match &mut mob.kind {
        MobKind::Drowned(drowned) => drowned,
        _ => unreachable!("not a drowned"),
    }
}

fn ok_target(target: EntityId, level: &ServerLevel) -> bool {
    !level.is_bright_outside() || level.entity(target).is_in_water()
}

impl Drowned {
    pub fn create() -> Mob {
        let mut mob = Zombie::create_with("minecraft:drowned", zombie::DIMENSIONS, &DROWNED_ATTRIBUTES, MobKind::Drowned(Drowned::default()));
        mob.ai.move_control.kind = MoveControlKind::Drowned;
        mob.set_pathfinding_malus(PathType::Water, 0.0);
        mob
    }

    pub fn add_behaviour_goals(ai: &mut MobAi) {
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(1, Goal::DrownedGoToWater(DrownedGoToWaterGoal::new(1.0)));
        goal_selector.add_goal(2, Goal::DrownedTridentAttack(DrownedTridentAttackGoal::new(1.0, 40, 10.0)));
        goal_selector.add_goal(2, Goal::DrownedAttack(DrownedAttackGoal::new(1.0, false)));
        goal_selector.add_goal(5, Goal::DrownedGoToBeach(DrownedGoToBeachGoal::new(1.0)));
        goal_selector.add_goal(6, Goal::DrownedSwimUp(DrownedSwimUpGoal::new(1.0)));
        goal_selector.add_goal(7, Goal::RandomStroll(RandomStrollGoal::new(1.0)));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::HurtByTarget(HurtByTargetGoal::new(&["minecraft:drowned"]).set_alert_others(&["minecraft:zombified_piglin"])));
        target_selector.add_goal(2, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::with(TargetType::Player, 10, true, false, Some(|_mob, target, level| ok_target(target, level)))));
        target_selector.add_goal(3, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::AbstractVillager, false)));
        target_selector.add_goal(3, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::IronGolem, true)));
        target_selector.add_goal(3, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::with(TargetType::Axolotl, 10, true, false, None)));
        target_selector.add_goal(5, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::with(TargetType::Turtle, 10, true, false, Some(zombie::baby_on_land_selector))));
    }

    pub fn get_ambient_sound(mob: &Mob) -> &'static str {
        if mob.entity.is_in_water() { "minecraft:entity.drowned.ambient_water" } else { "minecraft:entity.drowned.ambient" }
    }

    pub fn get_hurt_sound(mob: &Mob) -> &'static str {
        if mob.entity.is_in_water() { "minecraft:entity.drowned.hurt_water" } else { "minecraft:entity.drowned.hurt" }
    }

    pub fn get_death_sound(mob: &Mob) -> &'static str {
        if mob.entity.is_in_water() { "minecraft:entity.drowned.death_water" } else { "minecraft:entity.drowned.death" }
    }

    pub fn get_step_sound(_mob: &Mob) -> &'static str {
        "minecraft:entity.drowned.step"
    }

    pub fn get_swim_sound() -> &'static str {
        "minecraft:entity.drowned.swim"
    }

    pub fn can_spawn_in_liquids(_mob: &Mob) -> bool {
        true
    }

    pub fn converts_in_water(_mob: &Mob) -> bool {
        false
    }

    pub fn is_pushed_by_fluid(mob: &Mob) -> bool {
        !mob.entity.is_swimming()
    }

    pub fn set_searching_for_land(mob: &mut Mob, searching_for_land: bool) {
        drowned_mut(mob).searching_for_land = searching_for_land;
    }

    pub fn is_searching_for_land(mob: &Mob) -> bool {
        drowned(mob).searching_for_land
    }

    pub fn set_ranged_attack_uncertainty(mob: &mut Mob, ranged_attack_uncertainty: f32) {
        drowned_mut(mob).ranged_attack_uncertainty = ranged_attack_uncertainty;
    }

    pub fn get_preferred_weapon_type() -> &'static wafer_data::Tag {
        &item_tags::DROWNED_PREFERRED_WEAPONS
    }
}

impl ServerLevel {
    pub fn is_drowned(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Drowned(_)))
    }

    pub fn drowned_finalize_spawn(&mut self, id: EntityId, difficulty: &DifficultyInstance, spawn_reason: EntitySpawnReason, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
        let group_data = self.zombie_finalize_spawn(id, difficulty, spawn_reason, group_data);
        if self.item_by_slot_ref(id, EquipmentSlot::Offhand).is_none() && self.random.next_float() < NAUTILUS_SHELL_CHANCE {
            self.living_set_item_slot(id, EquipmentSlot::Offhand, ItemStack::new(items::NAUTILUS_SHELL, 1));
            self.set_guaranteed_drop(id, EquipmentSlot::Offhand);
        }
        group_data
    }

    pub fn drowned_populate_default_equipment_slots(&mut self, id: EntityId, _difficulty: &DifficultyInstance) {
        if self.random.next_float() as f64 > 0.9 {
            let rand = self.random.next_int_bounded(16);
            let item = if rand < 10 { items::TRIDENT } else { items::FISHING_ROD };
            self.living_set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::new(item, 1));
        }
    }

    pub fn drowned_can_replace_current_item(&self, id: EntityId, new_item_stack: &ItemStack, current_item_stack: &ItemStack, slot: EquipmentSlot) -> bool {
        if current_item_stack.is(items::NAUTILUS_SHELL) { false } else { self.mob_can_replace_current_item(id, new_item_stack, current_item_stack, slot) }
    }

    pub fn drowned_wants_to_pick_up(&self, id: EntityId, item_stack: &ItemStack) -> bool {
        if item_stack.get_item().is(&item_tags::SPEARS) { false } else { self.zombie_wants_to_pick_up(id, item_stack) }
    }

    pub fn ok_target(&self, target: Option<EntityId>) -> bool {
        target.is_some_and(|target| ok_target(target, self))
    }

    pub fn wants_to_swim(&self, id: EntityId) -> bool {
        if Drowned::is_searching_for_land(self.mob(id).expect("a drowned")) {
            return true;
        }
        self.get_target(id).is_some_and(|target| self.entity(target).is_in_water())
    }

    pub fn drowned_travel_in_water(&mut self, id: EntityId, input: Vec3, base_gravity: f64, is_falling: bool, old_y: f64) {
        if self.entity(id).is_under_water() && self.wants_to_swim(id) {
            self.entity_mut(id).move_relative(0.01, input);
            let delta = self.entity(id).delta_movement;
            self.move_entity(id, MoverType::SelfMove, delta);
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.scale(0.9));
        } else {
            self.living_entity_travel_in_water(id, input, base_gravity, is_falling, old_y);
        }
    }

    pub fn drowned_update_swimming(&mut self, id: EntityId) {
        let swimming = self.is_effective_ai(id) && self.entity(id).is_under_water() && self.wants_to_swim(id);
        self.entity_mut(id).set_swimming(swimming);
    }

    fn close_to_next_pos(&self, id: EntityId) -> bool {
        if let Some(path) = self.mob(id).expect("a drowned").ai.navigation.get_path() {
            let pos = path.get_target();
            let sqr_dist_to_next_pos = self.entity(id).distance_to_sqr_xyz(pos.x as f64, pos.y as f64, pos.z as f64);
            if sqr_dist_to_next_pos < 4.0 {
                return true;
            }
        }
        false
    }

    pub fn drowned_perform_ranged_attack(&mut self, id: EntityId, target: EntityId, _power: f32) {
        let main_hand_item = self.item_by_slot(id, EquipmentSlot::Mainhand).unwrap_or_else(ItemStack::empty);
        let trident_item_stack = if main_hand_item.is(items::TRIDENT) { main_hand_item } else { ItemStack::new(items::TRIDENT, 1) };
        let trident = ThrownTrident::from_owner(self, id, &trident_item_stack);
        let t = self.entity(target);
        let e = self.entity(id);
        let xd = t.x() - e.x();
        let yd = t.y() + t.dimensions.height as f64 * 0.3333333333333333 - trident.entity.y();
        let zd = t.z() - e.z();
        let distance_to_target = (xd * xd + zd * zd).sqrt();
        let uncertainty = self.ranged_attack_uncertainty(id);
        self.spawn_projectile_using_shoot(trident, &trident_item_stack, xd, yd + distance_to_target * 0.2f32 as f64, zd, 1.6, uncertainty);
        let pitch = 1.0 / (self.entity_mut(id).random.next_float() * 0.4 + 0.8);
        self.entity_play_sound(id, sound_event("minecraft:entity.drowned.shoot"), 1.0, pitch);
    }

    pub fn drowned_ranged_attack_uncertainty(&self, id: EntityId) -> f32 {
        let ranged_attack_uncertainty = drowned(self.mob(id).expect("a drowned")).ranged_attack_uncertainty;
        if ranged_attack_uncertainty < 0.0 { self.ranged_attack_mob_ranged_attack_uncertainty() } else { ranged_attack_uncertainty }
    }

    pub fn drowned_ride_tick(&mut self, id: EntityId) {
        self.entity_ride_tick(id);
        if self.entity_ref(id).is_none() {
            return;
        }
        if let Some(vehicle) = self.controlled_vehicle(id).filter(|&vehicle| self.is_mob(vehicle)) {
            let y_body_rot = self.living(vehicle).y_body_rot;
            self.living_mut(id).y_body_rot = y_body_rot;
        }
    }

    pub fn drowned_move_control_tick(&mut self, id: EntityId) {
        let target = self.get_target(id);
        if self.wants_to_swim(id) && self.entity(id).is_in_water() {
            let mob = self.mob(id).expect("a drowned");
            if target.is_some_and(|target| self.entity(target).y() > mob.entity.y()) || Drowned::is_searching_for_land(mob) {
                self.entity_mut(id).add_delta_movement(Vec3::new(0.0, 0.002, 0.0));
            }
            let speed_attribute = self.attribute_value(id, Attribute::MovementSpeed);
            let mob = self.mob_mut(id).expect("a drowned");
            if mob.ai.move_control.operation != Operation::MoveTo || mob.ai.navigation.is_done() {
                mob.set_speed(0.0);
                return;
            }
            let control = &mob.ai.move_control;
            let e = &mob.entity;
            let xd = control.wanted_x - e.x();
            let mut yd = control.wanted_y - e.y();
            let zd = control.wanted_z - e.z();
            let dd = (xd * xd + yd * yd + zd * zd).sqrt();
            yd /= dd;
            let y_rot_d = (mth::atan2(zd, xd) * 180.0f32 as f64 / std::f32::consts::PI as f64) as f32 - 90.0;
            let y_rot = rotlerp(e.y_rot, y_rot_d, 90.0);
            mob.entity.set_y_rot(y_rot);
            mob.living.y_body_rot = mob.entity.y_rot;
            let target_speed = (control.speed_modifier * speed_attribute) as f32;
            let new_speed = mth::lerp_f(0.125, mob.living.speed, target_speed);
            mob.set_speed(new_speed);
            let movement = Vec3::new(new_speed as f64 * xd * 0.005, new_speed as f64 * yd * 0.1, new_speed as f64 * zd * 0.005);
            mob.entity.add_delta_movement(movement);
        } else {
            if !self.entity(id).on_ground {
                self.entity_mut(id).add_delta_movement(Vec3::new(0.0, -0.008, 0.0));
            }
            self.move_control_tick(id);
        }
    }
}

pub struct DrownedAttackGoal {
    pub zombie_attack_goal: ZombieAttackGoal,
}

impl DrownedAttackGoal {
    pub fn new(speed_modifier: f64, track_target: bool) -> DrownedAttackGoal {
        DrownedAttackGoal { zombie_attack_goal: ZombieAttackGoal::new(speed_modifier, track_target) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.zombie_attack_goal.melee_attack_goal.can_use(level, mob) && level.ok_target(level.get_target(mob))
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.zombie_attack_goal.melee_attack_goal.can_continue_to_use(level, mob) && level.ok_target(level.get_target(mob))
    }
}

pub struct DrownedGoToBeachGoal {
    pub move_to_block_goal: MoveToBlockGoal,
}

impl DrownedGoToBeachGoal {
    pub fn new(speed_modifier: f64) -> DrownedGoToBeachGoal {
        DrownedGoToBeachGoal { move_to_block_goal: MoveToBlockGoal::new(speed_modifier, 8, 2) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.move_to_block_goal.can_use(level, mob, &is_valid_beach_target)
            && !level.is_bright_outside()
            && level.entity(mob).is_in_water()
            && level.entity(mob).y() >= (level.sea_level() - 3) as f64
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel) -> bool {
        self.move_to_block_goal.can_continue_to_use(level, &is_valid_beach_target)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        Drowned::set_searching_for_land(level.mob_mut(mob).expect("a drowned"), false);
        self.move_to_block_goal.start(level, mob);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.move_to_block_goal.tick(level, mob, 1.0);
    }
}

fn is_valid_beach_target(level: &ServerLevel, pos: BlockPos) -> bool {
    let region = PathNavigationRegion::new(level);
    let above = pos.above();
    if region.get_block_state(above).is_air() && region.get_block_state(above.above()).is_air() {
        is_face_full(&region.get_block_state(pos).collision_voxel_shape(), Direction::Up)
    } else {
        false
    }
}

pub struct DrownedGoToWaterGoal {
    wanted_x: f64,
    wanted_y: f64,
    wanted_z: f64,
    speed_modifier: f64,
}

impl DrownedGoToWaterGoal {
    pub fn new(speed_modifier: f64) -> DrownedGoToWaterGoal {
        DrownedGoToWaterGoal { wanted_x: 0.0, wanted_y: 0.0, wanted_z: 0.0, speed_modifier }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if !level.is_bright_outside() {
            return false;
        }
        if level.entity(mob).is_in_water() {
            return false;
        }
        let Some(pos) = self.get_water_pos(level, mob) else { return false };
        self.wanted_x = pos.x;
        self.wanted_y = pos.y;
        self.wanted_z = pos.z;
        true
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        !level.mob(mob).expect("a mob").ai.navigation.is_done()
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let (x, y, z, speed_modifier) = (self.wanted_x, self.wanted_y, self.wanted_z, self.speed_modifier);
        level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, x, y, z, speed_modifier));
    }

    fn get_water_pos(&self, level: &mut ServerLevel, mob: EntityId) -> Option<Vec3> {
        let pos = level.entity(mob).block_position();
        for _ in 0..10 {
            let random = &mut level.entity_mut(mob).random;
            let x = random.next_int_bounded(20) - 10;
            let y = 2 - random.next_int_bounded(8);
            let z = random.next_int_bounded(20) - 10;
            let random_pos = pos.offset(x, y, z);
            if level.block_state(random_pos).is_block(blocks::WATER) {
                return Some(Vec3::at_bottom_center_of(random_pos));
            }
        }
        None
    }
}

pub struct DrownedSwimUpGoal {
    speed_modifier: f64,
    stuck: bool,
}

impl DrownedSwimUpGoal {
    pub fn new(speed_modifier: f64) -> DrownedSwimUpGoal {
        DrownedSwimUpGoal { speed_modifier, stuck: false }
    }

    pub fn can_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        let sea_level = level.sea_level();
        !level.is_bright_outside() && level.entity(mob).is_in_water() && level.entity(mob).y() < (sea_level - 2) as f64
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        self.can_use(level, mob) && !self.stuck
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let sea_level = level.sea_level();
        let e = level.entity(mob);
        if e.y() < (sea_level - 1) as f64 && (level.mob(mob).expect("a drowned").ai.navigation.is_done() || level.close_to_next_pos(mob)) {
            let towards = Vec3::new(e.x(), (sea_level - 1) as f64, e.z());
            let Some(next_pos) = default_random_pos::get_pos_towards(level, mob, 4, 8, towards, std::f32::consts::FRAC_PI_2 as f64) else {
                self.stuck = true;
                return;
            };
            let speed_modifier = self.speed_modifier;
            level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, next_pos.x, next_pos.y, next_pos.z, speed_modifier));
        }
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        Drowned::set_searching_for_land(level.mob_mut(mob).expect("a drowned"), true);
        self.stuck = false;
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        Drowned::set_searching_for_land(level.mob_mut(mob).expect("a drowned"), false);
    }
}

pub struct DrownedTridentAttackGoal {
    pub ranged_attack_goal: RangedAttackGoal,
}

impl DrownedTridentAttackGoal {
    pub fn new(speed_modifier: f64, attack_interval: i32, attack_radius: f32) -> DrownedTridentAttackGoal {
        DrownedTridentAttackGoal { ranged_attack_goal: RangedAttackGoal::new(speed_modifier, attack_interval, attack_radius) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.ranged_attack_goal.can_use(level, mob) && level.item_by_slot_ref(mob, EquipmentSlot::Mainhand).is_some_and(|item| item.is(items::TRIDENT))
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.mob_mut(mob).expect("a drowned").set_aggressive(true);
        level.start_using_item(mob, crate::world::InteractionHand::MainHand);
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.ranged_attack_goal.stop();
        level.stop_using_item(mob);
        level.mob_mut(mob).expect("a drowned").set_aggressive(false);
    }
}
