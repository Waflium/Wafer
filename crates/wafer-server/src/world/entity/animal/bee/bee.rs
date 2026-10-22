use std::collections::HashMap;

use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::properties::{AGE_3, AGE_7, DOUBLE_BLOCK_HALF, DoubleBlockHalf, WATERLOGGED};
use wafer_data::{BlockState, block_tags, blocks, damage_types, item_tags, mob_effects, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::util::time_util;
use crate::util::valueproviders::IntProvider;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::effect::mob_effect_instance::MobEffectInstance;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::control::flying_move_control::FlyingMoveControl;
use crate::world::entity::ai::control::look_control::LookControlKind;
use crate::world::entity::ai::control::move_control::MoveControlKind;
use crate::world::entity::ai::goal::breed_goal::BreedGoal;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::follow_parent_goal::FollowParentGoal;
use crate::world::entity::ai::goal::goal::{Goal, reduced_tick_delay};
use crate::world::entity::ai::goal::melee_attack_goal::MeleeAttackGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::goal::target::reset_universal_anger_target_goal::ResetUniversalAngerTargetGoal;
use crate::world::entity::ai::goal::tempt_goal::TemptGoal;
use crate::world::entity::ai::navigation::path_navigation::{PathNavigation, PathNavigationKind};
use crate::world::entity::ai::util::{air_and_water_random_pos, air_random_pos, hover_random_pos};
use crate::world::entity::ai::village::poi::poi_manager::Occupancy;
use crate::world::entity::ai::village::poi::poi_types::PoiType;
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::BEE_ATTRIBUTES;
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::entity_reference::EntityReference;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::neutral_mob::{self, NeutralMob};
use crate::world::entity::{Entity, EntityDimensions, EntityId, calculate_view_vector, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::block;
use crate::world::level::block::crop_block;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::block::entity::beehive_block_entity::BeehiveBlockEntity;
use crate::world::level::pathfinder::path::Path;
use crate::world::level::pathfinder::path_type::PathType;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::{InteractionHand, InteractionResult};

pub const DATA_FLAGS_ID: usize = 18;
pub const DATA_ANGER_END_TIME: usize = 19;
pub const TICKS_PER_FLAP: i32 = 2;
const FLAG_ROLL: i8 = 2;
const FLAG_HAS_STUNG: i8 = 4;
const FLAG_HAS_NECTAR: i8 = 8;
const STING_DEATH_COUNTDOWN: i32 = 1200;
const TICKS_BEFORE_GOING_TO_KNOWN_FLOWER: i32 = 600;
const TICKS_WITHOUT_NECTAR_BEFORE_GOING_HOME: i32 = 3600;
const MAX_CROPS_GROWABLE: i32 = 10;
const POISON_SECONDS_NORMAL: i32 = 10;
const POISON_SECONDS_HARD: i32 = 18;
const TOO_FAR_DISTANCE: i32 = 48;
const HIVE_CLOSE_ENOUGH_DISTANCE: i32 = 2;
const RESTRICTED_WANDER_DISTANCE_REDUCTION: i32 = 24;
const DEFAULT_WANDER_DISTANCE_REDUCTION: i32 = 16;
const PATHFIND_TO_HIVE_WHEN_CLOSER_THAN: i32 = 16;
const HIVE_SEARCH_DISTANCE: i32 = 20;
const PERSISTENT_ANGER_TIME: IntProvider = time_util::range_of_seconds(20, 39);
const COOLDOWN_BEFORE_LOCATING_NEW_HIVE: i32 = 200;
const COOLDOWN_BEFORE_LOCATING_NEW_FLOWER: i32 = 200;
const MIN_FIND_FLOWER_RETRY_COOLDOWN: i32 = 20;
const MAX_FIND_FLOWER_RETRY_COOLDOWN: i32 = 60;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.55, 0.5).with_eye_height(0.3);
const MOVE_CONTROL: FlyingMoveControl = FlyingMoveControl::new(20, true);

pub struct Bee {
    pub animal: Animal,
    persistent_anger_target: Option<EntityReference>,
    pub roll_amount: f32,
    pub roll_amount_o: f32,
    time_since_sting: i32,
    ticks_without_nectar_since_exiting_hive: i32,
    stay_out_of_hive_countdown: i32,
    num_crops_grown_since_pollination: i32,
    remaining_cooldown_before_locating_new_hive: i32,
    remaining_cooldown_before_locating_new_flower: i32,
    saved_flower_pos: Option<BlockPos>,
    hive_pos: Option<BlockPos>,
    pollinating: bool,
    blacklisted_targets: Vec<BlockPos>,
    under_water_ticks: i32,
}

impl Default for Bee {
    fn default() -> Bee {
        Bee {
            animal: Animal::default(),
            persistent_anger_target: None,
            roll_amount: 0.0,
            roll_amount_o: 0.0,
            time_since_sting: 0,
            ticks_without_nectar_since_exiting_hive: 0,
            stay_out_of_hive_countdown: 0,
            num_crops_grown_since_pollination: 0,
            remaining_cooldown_before_locating_new_hive: 0,
            remaining_cooldown_before_locating_new_flower: 0,
            saved_flower_pos: None,
            hive_pos: None,
            pollinating: false,
            blacklisted_targets: Vec::new(),
            under_water_ticks: 0,
        }
    }
}

impl NeutralMob for Bee {
    fn get_persistent_anger_end_time(&self, entity: &Entity) -> i64 {
        match entity.data.get(DATA_ANGER_END_TIME) {
            DataValue::Long(end_time) => *end_time,
            _ => neutral_mob::NO_ANGER_END_TIME,
        }
    }

    fn set_persistent_anger_end_time(&mut self, entity: &mut Entity, end_time: i64) {
        entity.data.set(DATA_ANGER_END_TIME, DataValue::Long(end_time));
    }

    fn get_persistent_anger_target(&self) -> Option<EntityReference> {
        self.persistent_anger_target
    }

    fn set_persistent_anger_target(&mut self, persistent_anger_target: Option<EntityReference>) {
        self.persistent_anger_target = persistent_anger_target;
    }
}

impl Bee {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        data.extend([DataValue::Byte(0), DataValue::Long(-1)]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:bee"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&BEE_ATTRIBUTES), MobKind::Bee(Bee::default()));
        Animal::init(&mut mob);
        let remaining_cooldown_before_locating_new_flower = mth::next_int(&mut mob.entity.random, MIN_FIND_FLOWER_RETRY_COOLDOWN, MAX_FIND_FLOWER_RETRY_COOLDOWN);
        bee_mut(&mut mob).remaining_cooldown_before_locating_new_flower = remaining_cooldown_before_locating_new_flower;
        mob.ai.move_control.kind = MoveControlKind::Flying(MOVE_CONTROL);
        mob.ai.look_control.kind = LookControlKind::Bee;
        mob.set_pathfinding_malus(PathType::Fire, -1.0);
        mob.set_pathfinding_malus(PathType::Water, -1.0);
        mob.set_pathfinding_malus(PathType::WaterBorder, 16.0);
        mob.set_pathfinding_malus(PathType::Cocoa, -1.0);
        mob.set_pathfinding_malus(PathType::Fence, -1.0);
        mob
    }

    pub fn create_navigation(follow_range_base: f64) -> PathNavigation {
        let mut flying_path_navigation = PathNavigation::of_kind(PathNavigationKind::Bee, follow_range_base);
        flying_path_navigation.path_finder.node_evaluator.node_evaluator_mut().can_open_doors = false;
        flying_path_navigation.set_can_float(false);
        flying_path_navigation.set_required_path_length_detached(follow_range_base, 48.0);
        flying_path_navigation
    }

    pub fn register_goals(ai: &mut MobAi, random: &mut impl RandomSource) {
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::BeeAttack(BeeAttackGoal { melee_attack_goal: MeleeAttackGoal::new(1.4f32 as f64, true) }));
        goal_selector.add_goal(1, Goal::BeeEnterHive(BeeEnterHiveGoal));
        goal_selector.add_goal(2, Goal::Breed(BreedGoal::new(1.0)));
        goal_selector.add_goal(3, Goal::Tempt(TemptGoal::new(1.25, |i| i.get_item().is(&item_tags::BEE_FOOD), false)));
        goal_selector.add_goal(3, Goal::ValidateHive(ValidateHiveGoal::new(random)));
        goal_selector.add_goal(3, Goal::ValidateFlower(ValidateFlowerGoal::new(random)));
        goal_selector.add_goal(4, Goal::BeePollinate(BeePollinateGoal::default()));
        goal_selector.add_goal(5, Goal::FollowParent(FollowParentGoal::new(1.25)));
        goal_selector.add_goal(5, Goal::BeeLocateHive(BeeLocateHiveGoal));
        goal_selector.add_goal(5, Goal::BeeGoToHive(BeeGoToHiveGoal::default()));
        goal_selector.add_goal(6, Goal::BeeGoToKnownFlower(BeeGoToKnownFlowerGoal::default()));
        goal_selector.add_goal(7, Goal::BeeGrowCrop(BeeGrowCropGoal));
        goal_selector.add_goal(8, Goal::BeeWander(BeeWanderGoal));
        goal_selector.add_goal(9, Goal::Float(float_goal));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::BeeHurtByOther(BeeHurtByOtherGoal { hurt_by_target_goal: HurtByTargetGoal::new(&[]).set_alert_others(&[]) }));
        target_selector.add_goal(2, Goal::BeeBecomeAngryTarget(BeeBecomeAngryTargetGoal::new()));
        target_selector.add_goal(3, Goal::ResetUniversalAngerTarget(ResetUniversalAngerTargetGoal::new(true)));
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        let bee = bee(mob);
        let pos = |pos: BlockPos| Nbt::IntArray(vec![pos.x, pos.y, pos.z]);
        if let Some(hive_pos) = bee.hive_pos {
            output.push(("hive_pos".into(), pos(hive_pos)));
        }
        if let Some(saved_flower_pos) = bee.saved_flower_pos {
            output.push(("flower_pos".into(), pos(saved_flower_pos)));
        }
        output.push(("HasNectar".into(), Nbt::Byte(has_nectar(&mob.entity) as i8)));
        output.push(("HasStung".into(), Nbt::Byte(has_stung(&mob.entity) as i8)));
        output.push(("TicksSincePollination".into(), Nbt::Int(bee.ticks_without_nectar_since_exiting_hive)));
        output.push(("CannotEnterHiveTicks".into(), Nbt::Int(bee.stay_out_of_hive_countdown)));
        output.push(("CropsGrownSincePollination".into(), Nbt::Int(bee.num_crops_grown_since_pollination)));
        bee.add_persistent_anger_save_data(&mob.entity, output);
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        let boolean = |key: &str| input.get(key).and_then(Nbt::as_i64).is_some_and(|v| v != 0);
        let int = |key: &str| input.get(key).and_then(crate::world::level::block::entity::int_value).unwrap_or(0);
        let pos = |key: &str| match input.get(key) {
            Some(Nbt::IntArray(v)) if v.len() == 3 => Some(BlockPos::new(v[0], v[1], v[2])),
            _ => None,
        };
        set_has_nectar(mob, boolean("HasNectar"));
        set_has_stung(&mut mob.entity, boolean("HasStung"));
        let bee = bee_mut(mob);
        bee.ticks_without_nectar_since_exiting_hive = int("TicksSincePollination");
        bee.stay_out_of_hive_countdown = int("CannotEnterHiveTicks");
        bee.num_crops_grown_since_pollination = int("CropsGrownSincePollination");
        bee.hive_pos = pos("hive_pos");
        bee.saved_flower_pos = pos("flower_pos");
    }

    pub fn get_saved_flower_pos(&self) -> Option<BlockPos> {
        self.saved_flower_pos
    }

    pub fn has_saved_flower_pos(&self) -> bool {
        self.saved_flower_pos.is_some()
    }

    pub fn set_saved_flower_pos(&mut self, saved_flower_pos: BlockPos) {
        self.saved_flower_pos = Some(saved_flower_pos);
    }

    fn is_tired_of_looking_for_nectar(&self) -> bool {
        self.ticks_without_nectar_since_exiting_hive > TICKS_WITHOUT_NECTAR_BEFORE_GOING_HOME
    }

    pub fn set_stay_out_of_hive_countdown(&mut self, ticks: i32) {
        self.stay_out_of_hive_countdown = ticks;
    }

    pub fn get_roll_amount(&self, a: f32) -> f32 {
        mth::lerp_f(a, self.roll_amount_o, self.roll_amount)
    }

    pub fn reset_ticks_without_nectar_since_exiting_hive(&mut self) {
        self.ticks_without_nectar_since_exiting_hive = 0;
    }

    pub fn has_hive(&self) -> bool {
        self.hive_pos.is_some()
    }

    pub fn get_hive_pos(&self) -> Option<BlockPos> {
        self.hive_pos
    }

    pub fn set_hive_pos(&mut self, hive_pos: BlockPos) {
        self.hive_pos = Some(hive_pos);
    }

    fn get_crops_grown_since_pollination(&self) -> i32 {
        self.num_crops_grown_since_pollination
    }

    fn reset_num_crops_grown_since_pollination(&mut self) {
        self.num_crops_grown_since_pollination = 0;
    }

    fn increment_num_crops_grown_since_pollination(&mut self) {
        self.num_crops_grown_since_pollination += 1;
    }
}

pub fn is_bee(mob: &Mob) -> bool {
    matches!(mob.kind, MobKind::Bee(_))
}

pub fn bee(mob: &Mob) -> &Bee {
    match &mob.kind {
        MobKind::Bee(bee) => bee,
        _ => unreachable!("not a bee"),
    }
}

pub fn bee_mut(mob: &mut Mob) -> &mut Bee {
    match &mut mob.kind {
        MobKind::Bee(bee) => bee,
        _ => unreachable!("not a bee"),
    }
}

pub fn is_pollinating(mob: &Mob) -> bool {
    bee(mob).pollinating
}

fn get_flag(entity: &Entity, flag: i8) -> bool {
    matches!(entity.data.get(DATA_FLAGS_ID), DataValue::Byte(flags) if flags & flag != 0)
}

fn set_flag(entity: &mut Entity, flag: i8, value: bool) {
    let flags = match entity.data.get(DATA_FLAGS_ID) {
        DataValue::Byte(flags) => *flags,
        _ => 0,
    };
    entity.data.set(DATA_FLAGS_ID, DataValue::Byte(if value { flags | flag } else { flags & !flag }));
}

pub fn has_nectar(entity: &Entity) -> bool {
    get_flag(entity, FLAG_HAS_NECTAR)
}

fn set_has_nectar(mob: &mut Mob, has_nectar: bool) {
    if has_nectar {
        bee_mut(mob).reset_ticks_without_nectar_since_exiting_hive();
    }
    set_flag(&mut mob.entity, FLAG_HAS_NECTAR, has_nectar);
}

pub fn has_stung(entity: &Entity) -> bool {
    get_flag(entity, FLAG_HAS_STUNG)
}

fn set_has_stung(entity: &mut Entity, has_stung: bool) {
    set_flag(entity, FLAG_HAS_STUNG, has_stung);
}

fn is_rolling(entity: &Entity) -> bool {
    get_flag(entity, FLAG_ROLL)
}

fn set_rolling(entity: &mut Entity, rolling: bool) {
    set_flag(entity, FLAG_ROLL, rolling);
}

pub fn drop_off_nectar(mob: &mut Mob) {
    set_has_nectar(mob, false);
    bee_mut(mob).reset_num_crops_grown_since_pollination();
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::BEE_FOOD)
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.bee.hurt"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.bee.death"
}

pub fn get_sound_volume() -> f32 {
    0.4
}

pub fn get_walk_target_value(pos: BlockPos, level: &mut (impl BlockGetter + ?Sized)) -> f32 {
    if level.block_state(pos).is_air() { 10.0 } else { 0.0 }
}

pub fn attracts_bees(state: BlockState) -> bool {
    if !state.is(&block_tags::BEE_ATTRACTIVE) {
        return false;
    }
    if state.has(WATERLOGGED) && state.get(WATERLOGGED) {
        return false;
    }
    if state.is_block(blocks::SUNFLOWER) { state.get(DOUBLE_BLOCK_HALF) == DoubleBlockHalf::Upper } else { true }
}

fn get_bee_interaction_effect(item_stack: &ItemStack) -> Option<MobEffectInstance> {
    let block = crate::world::item::block_item_behaviour(item_stack.get_item())?.block();
    if !block.is_instance_of("FlowerBlock") {
        return None;
    }
    if block.is_instance_of("EyeblossomBlock") {
        Some(crate::world::level::block::eyeblossom_block::get_bee_interaction_effect())
    } else if block.is_instance_of("WitherRoseBlock") {
        Some(crate::world::level::block::wither_rose_block::get_bee_interaction_effect())
    } else {
        None
    }
}

fn bee_of(level: &ServerLevel, id: EntityId) -> &Bee {
    bee(level.mob(id).expect("a bee"))
}

fn bee_of_mut(level: &mut ServerLevel, id: EntityId) -> &mut Bee {
    bee_mut(level.mob_mut(id).expect("a bee"))
}

fn closer_than(level: &ServerLevel, id: EntityId, target_pos: BlockPos, distance: i32) -> bool {
    target_pos.closer_than(level.entity(id).block_position(), distance as f64)
}

fn is_too_far_away(level: &ServerLevel, id: EntityId, target_pos: BlockPos) -> bool {
    !closer_than(level, id, target_pos, TOO_FAR_DISTANCE)
}

impl ServerLevel {
    pub fn is_bee(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(is_bee)
    }

    pub fn bee_get_walk_target_value(&self, pos: BlockPos) -> f32 {
        if crate::world::level::path_navigation_region::PathNavigationRegion::new(self).get_block_state(pos).is_air() { 10.0 } else { 0.0 }
    }

    pub fn bee_get_breed_offspring(&mut self, _id: EntityId, _partner: EntityId) -> Option<Box<Mob>> {
        Some(Box::new(Bee::create()))
    }

    pub fn bee_start_persistent_anger_timer(&mut self, id: EntityId) {
        let remaining_time = PERSISTENT_ANGER_TIME.sample(&mut self.entity_mut(id).random);
        self.set_time_to_remain_angry(id, remaining_time as i64);
    }

    pub fn bee_do_hurt_target(&mut self, id: EntityId, target: EntityId) -> bool {
        let damage_source = DamageSource::entity(damage_types::STING, id);
        let damage = self.attribute_value(id, crate::world::entity::attributes::Attribute::AttackDamage) as i32 as f32;
        let was_hurt = self.hurt_server(target, damage_source.clone(), damage);
        if was_hurt {
            self.do_post_attack_effects(target, &damage_source);
            if self.is_living(target) {
                let stinger_count = self.get_stinger_count(target);
                self.set_stinger_count(target, stinger_count + 1);
                let poison_time = match self.difficulty {
                    Difficulty::Normal => POISON_SECONDS_NORMAL,
                    Difficulty::Hard => POISON_SECONDS_HARD,
                    _ => 0,
                };
                if poison_time > 0 {
                    self.add_effect(target, MobEffectInstance::new(mob_effects::POISON, poison_time * 20, 0), Some(id));
                }
            }
            set_has_stung(self.entity_mut(id), true);
            self.stop_being_angry(id);
            self.entity_play_sound(id, sound_event("minecraft:entity.bee.sting"), 1.0, 1.0);
        }
        was_hurt
    }

    pub fn bee_tick(&mut self, id: EntityId) {
        self.mob_tick(id);
        let Some(mob) = self.mob_mut(id) else { return };
        let crops_grown = bee(mob).get_crops_grown_since_pollination();
        if has_nectar(&mob.entity) && crops_grown < MAX_CROPS_GROWABLE && mob.entity.random.next_float() < 0.05 {
            let mut i = 0;
            while i < self.mob_mut(id).expect("a bee").entity.random.next_int_bounded(2) + 1 {
                self.random.next_double();
                self.random.next_double();
                i += 1;
            }
        }
        self.bee_update_roll_amount(id);
    }

    fn bee_pathfind_randomly_towards(&mut self, id: EntityId, target_pos: BlockPos) {
        let target_vec = Vec3::at_bottom_center_of(target_pos);
        let mut y_adjust = 0;
        let bee_pos = self.entity(id).block_position();
        let y_delta = target_vec.y as i32 - bee_pos.y;
        if y_delta > 2 {
            y_adjust = 4;
        } else if y_delta < -2 {
            y_adjust = -4;
        }
        let mut xz_dist = 6;
        let mut y_dist = 8;
        let dist = bee_pos.dist_manhattan(target_pos);
        if dist < 15 {
            xz_dist = dist / 2;
            y_dist = dist / 2;
        }
        let next_pos_towards = air_random_pos::get_pos_towards(self, id, xz_dist, y_dist, y_adjust, target_vec, (std::f64::consts::PI / 10.0) as f32 as f64);
        if let Some(next_pos_towards) = next_pos_towards {
            self.get_navigation(id, |navigation, level| {
                navigation.set_max_visited_nodes_multiplier(0.5);
                navigation.move_to(level, id, next_pos_towards.x, next_pos_towards.y, next_pos_towards.z, 1.0)
            });
        }
    }

    fn bee_drop_hive(&mut self, id: EntityId) {
        let bee = bee_of_mut(self, id);
        bee.hive_pos = None;
        bee.remaining_cooldown_before_locating_new_hive = COOLDOWN_BEFORE_LOCATING_NEW_HIVE;
    }

    fn bee_drop_flower(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a bee");
        let remaining_cooldown_before_locating_new_flower = mth::next_int(&mut mob.entity.random, MIN_FIND_FLOWER_RETRY_COOLDOWN, MAX_FIND_FLOWER_RETRY_COOLDOWN);
        let bee = bee_mut(mob);
        bee.saved_flower_pos = None;
        bee.remaining_cooldown_before_locating_new_flower = remaining_cooldown_before_locating_new_flower;
    }

    fn bee_wants_to_enter_hive(&mut self, id: EntityId) -> bool {
        let mob = self.mob(id).expect("a bee");
        let bee = bee(mob);
        if bee.stay_out_of_hive_countdown <= 0 && !bee.pollinating && !has_stung(&mob.entity) && self.get_target(id).is_none() {
            let position = mob.entity.position();
            let wants_to_enter_hive = has_nectar(&mob.entity)
                || bee.is_tired_of_looking_for_nectar()
                || self.environment_attributes().get_value(crate::world::attribute::environment_attributes::BEES_STAY_IN_HIVE, position);
            wants_to_enter_hive && !self.bee_is_hive_near_fire(id)
        } else {
            false
        }
    }

    fn bee_update_roll_amount(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a bee");
        let rolling = is_rolling(&mob.entity);
        let bee = bee_mut(mob);
        bee.roll_amount_o = bee.roll_amount;
        if rolling {
            bee.roll_amount = (bee.roll_amount + 0.2).min(1.0);
        } else {
            bee.roll_amount = (bee.roll_amount - 0.24).max(0.0);
        }
    }

    pub fn bee_custom_server_ai_step(&mut self, id: EntityId) {
        let has_stung = has_stung(self.entity(id));
        let in_water = self.entity(id).is_in_water();
        let bee = bee_of_mut(self, id);
        if in_water {
            bee.under_water_ticks += 1;
        } else {
            bee.under_water_ticks = 0;
        }
        if bee.under_water_ticks > 20 {
            self.hurt_server(id, DamageSource::new(damage_types::DROWN), 1.0);
        }
        if has_stung {
            let mob = self.mob_mut(id).expect("a bee");
            let bee = bee_mut(mob);
            bee.time_since_sting += 1;
            let time_since_sting = bee.time_since_sting;
            if time_since_sting % 5 == 0 && mob.entity.random.next_int_bounded((STING_DEATH_COUNTDOWN - time_since_sting).clamp(1, STING_DEATH_COUNTDOWN)) == 0 {
                let health = self.mob(id).expect("a bee").health;
                self.hurt_server(id, DamageSource::new(damage_types::GENERIC), health);
            }
        }
        let Some(mob) = self.mob_mut(id) else { return };
        if !has_nectar(&mob.entity) {
            bee_mut(mob).ticks_without_nectar_since_exiting_hive += 1;
        }
        self.update_persistent_anger(id, false);
    }

    fn bee_is_hive_near_fire(&mut self, id: EntityId) -> bool {
        self.bee_get_beehive_block_entity(id).is_some() && self.beehive_is_fire_nearby(bee_of(self, id).hive_pos.expect("a hive"))
    }

    fn bee_does_hive_have_space(&mut self, hive_pos: BlockPos) -> bool {
        match self.get_block_entity(hive_pos).map(|block_entity| &block_entity.kind) {
            Some(BlockEntityKind::Beehive(beehive_block_entity)) => !beehive_block_entity.is_full(),
            _ => false,
        }
    }

    pub fn bee_ai_step(&mut self, id: EntityId) {
        self.animal_ai_step(id);
        let Some(mob) = self.mob_mut(id) else { return };
        let bee = bee_mut(mob);
        if bee.stay_out_of_hive_countdown > 0 {
            bee.stay_out_of_hive_countdown -= 1;
        }
        if bee.remaining_cooldown_before_locating_new_hive > 0 {
            bee.remaining_cooldown_before_locating_new_hive -= 1;
        }
        if bee.remaining_cooldown_before_locating_new_flower > 0 {
            bee.remaining_cooldown_before_locating_new_flower -= 1;
        }
        let should_roll = self.is_angry(id)
            && !has_stung(self.entity(id))
            && self.get_target(id).is_some_and(|target| self.entity(target).distance_to_sqr(self.entity(id)) < 4.0);
        set_rolling(self.entity_mut(id), should_roll);
        if self.entity(id).tick_count % 20 == 0 && !self.bee_is_hive_valid(id) {
            bee_of_mut(self, id).hive_pos = None;
        }
    }

    fn bee_get_beehive_block_entity(&mut self, id: EntityId) -> Option<&mut BeehiveBlockEntity> {
        let hive_pos = bee_of(self, id).hive_pos?;
        if is_too_far_away(self, id, hive_pos) {
            return None;
        }
        match &mut self.get_block_entity(hive_pos)?.kind {
            BlockEntityKind::Beehive(beehive_block_entity) => Some(beehive_block_entity),
            _ => None,
        }
    }

    fn bee_is_hive_valid(&mut self, id: EntityId) -> bool {
        self.bee_get_beehive_block_entity(id).is_some()
    }

    pub fn bee_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let mut held_item = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if is_food(&held_item)
            && let Some(effect) = get_bee_interaction_effect(&held_item)
        {
            self.use_player_item(player, hand, &mut held_item);
            self.add_effect(id, effect, None);
            return InteractionResult::SUCCESS;
        }
        self.animal_mob_interact(id, player, hand)
    }

    pub fn bee_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        if self.is_invulnerable_to(id, &source) {
            return false;
        }
        bee_of_mut(self, id).pollinating = false;
        self.living_entity_hurt_server(id, source, damage)
    }

    pub fn bee_jump_in_liquid(&mut self, id: EntityId) {
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.add(0.0, 0.01, 0.0));
    }

    pub fn bee_is_flapping(&self, id: EntityId) -> bool {
        let e = self.entity(id);
        !e.on_ground && e.tick_count % TICKS_PER_FLAP == 0
    }
}

fn is_angry(level: &ServerLevel, id: EntityId) -> bool {
    level.is_angry(id)
}

fn is_angry_at(mob: EntityId, target: EntityId, level: &ServerLevel) -> bool {
    level.is_angry_at(mob, target)
}

pub struct BeeAttackGoal {
    pub melee_attack_goal: MeleeAttackGoal,
}

impl BeeAttackGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.melee_attack_goal.can_use(level, mob) && is_angry(level, mob) && !has_stung(level.entity(mob))
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.melee_attack_goal.can_continue_to_use(level, mob) && is_angry(level, mob) && !has_stung(level.entity(mob))
    }
}

pub struct BeeBecomeAngryTargetGoal {
    pub nearest_attackable_target_goal: NearestAttackableTargetGoal,
}

impl BeeBecomeAngryTargetGoal {
    fn new() -> BeeBecomeAngryTargetGoal {
        BeeBecomeAngryTargetGoal { nearest_attackable_target_goal: NearestAttackableTargetGoal::with(TargetType::Player, 10, true, false, Some(is_angry_at)) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        bee_can_target(level, mob) && self.nearest_attackable_target_goal.can_use(level, mob)
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let bee_can_target = bee_can_target(level, mob);
        if bee_can_target && level.get_target(mob).is_some() {
            return self.nearest_attackable_target_goal.target_goal.can_continue_to_use(level, mob);
        }
        self.nearest_attackable_target_goal.target_goal.target_mob = None;
        false
    }
}

fn bee_can_target(level: &ServerLevel, mob: EntityId) -> bool {
    is_angry(level, mob) && !has_stung(level.entity(mob))
}

pub struct BeeEnterHiveGoal;

impl BeeEnterHiveGoal {
    fn can_bee_use(&self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let Some(hive_pos) = bee_of(level, mob).hive_pos else { return false };
        if level.bee_wants_to_enter_hive(mob) && hive_pos.closer_to_center_than(level.entity(mob).position(), HIVE_CLOSE_ENOUGH_DISTANCE as f64) {
            if let Some(beehive_block_entity) = level.bee_get_beehive_block_entity(mob) {
                if !beehive_block_entity.is_full() {
                    return true;
                }
                bee_of_mut(level, mob).hive_pos = None;
            }
        }
        false
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.can_bee_use(level, mob) && !is_angry(level, mob)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if level.bee_get_beehive_block_entity(mob).is_some() {
            let hive_pos = bee_of(level, mob).hive_pos.expect("a hive");
            level.beehive_add_occupant(hive_pos, mob);
        }
    }
}

#[derive(Default)]
pub struct BeeGoToHiveGoal {
    pub travelling_ticks: i32,
    last_path: Option<Path>,
    ticks_stuck: i32,
}

impl BeeGoToHiveGoal {
    fn can_bee_use(&self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let Some(hive_pos) = bee_of(level, mob).hive_pos else { return false };
        !is_too_far_away(level, mob, hive_pos)
            && !level.mob(mob).expect("a bee").has_home()
            && level.bee_wants_to_enter_hive(mob)
            && !self.has_reached_target(level, mob, hive_pos)
            && level.block_state(hive_pos).is(&block_tags::BEEHIVES)
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.can_bee_use(level, mob) && !is_angry(level, mob)
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.can_bee_use(level, mob) && !is_angry(level, mob)
    }

    pub fn start(&mut self) {
        self.travelling_ticks = 0;
        self.ticks_stuck = 0;
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.travelling_ticks = 0;
        self.ticks_stuck = 0;
        let navigation = &mut level.mob_mut(mob).expect("a bee").ai.navigation;
        navigation.stop();
        navigation.reset_max_visited_nodes_multiplier();
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let Some(hive_pos) = bee_of(level, mob).hive_pos else { return };
        self.travelling_ticks += 1;
        if self.travelling_ticks > reduced_tick_delay(2400) {
            self.drop_and_blacklist_hive(level, mob);
        } else if !level.mob(mob).expect("a bee").ai.navigation.is_in_progress() {
            if !closer_than(level, mob, hive_pos, PATHFIND_TO_HIVE_WHEN_CLOSER_THAN) {
                if is_too_far_away(level, mob, hive_pos) {
                    level.bee_drop_hive(mob);
                } else {
                    level.bee_pathfind_randomly_towards(mob, hive_pos);
                }
            } else {
                let can_reach_all_the_way_to_target = self.pathfind_directly_towards(level, mob, hive_pos);
                if !can_reach_all_the_way_to_target {
                    self.drop_and_blacklist_hive(level, mob);
                } else if self.last_path.is_some() && level.mob(mob).expect("a bee").ai.navigation.get_path().expect("a path").same_as(self.last_path.as_ref()) {
                    self.ticks_stuck += 1;
                    if self.ticks_stuck > 60 {
                        level.bee_drop_hive(mob);
                        self.ticks_stuck = 0;
                    }
                } else {
                    self.last_path = level.mob(mob).expect("a bee").ai.navigation.get_path().cloned();
                }
            }
        }
    }

    fn pathfind_directly_towards(&mut self, level: &mut ServerLevel, mob: EntityId, target_pos: BlockPos) -> bool {
        let close_enough = if closer_than(level, mob, target_pos, 3) { 1 } else { 2 };
        level.get_navigation(mob, |navigation, level| {
            navigation.set_max_visited_nodes_multiplier(10.0);
            let path = navigation.create_path_to(level, mob, target_pos.x as f64, target_pos.y as f64, target_pos.z as f64, close_enough);
            navigation.move_to_path(level, mob, path, 1.0);
            navigation.get_path().is_some_and(Path::can_reach)
        })
    }

    fn drop_and_blacklist_hive(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if let Some(hive_pos) = bee_of(level, mob).hive_pos {
            let blacklisted_targets = &mut bee_of_mut(level, mob).blacklisted_targets;
            blacklisted_targets.push(hive_pos);
            while blacklisted_targets.len() > 3 {
                blacklisted_targets.remove(0);
            }
        }
        level.bee_drop_hive(mob);
    }

    fn has_reached_target(&self, level: &ServerLevel, mob: EntityId, target_pos: BlockPos) -> bool {
        if closer_than(level, mob, target_pos, 2) {
            return true;
        }
        let path = level.mob(mob).expect("a bee").ai.navigation.get_path();
        path.is_some_and(|path| path.get_target() == target_pos && path.can_reach() && path.is_done())
    }
}

#[derive(Default)]
pub struct BeeGoToKnownFlowerGoal {
    pub travelling_ticks: i32,
}

impl BeeGoToKnownFlowerGoal {
    fn can_bee_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        let bee = bee_of(level, mob);
        let Some(saved_flower_pos) = bee.saved_flower_pos else { return false };
        !level.mob(mob).expect("a bee").has_home() && self.wants_to_go_to_known_flower(bee) && !closer_than(level, mob, saved_flower_pos, 2)
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.can_bee_use(level, mob) && !is_angry(level, mob)
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.can_bee_use(level, mob) && !is_angry(level, mob)
    }

    pub fn start(&mut self) {
        self.travelling_ticks = 0;
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.travelling_ticks = 0;
        let navigation = &mut level.mob_mut(mob).expect("a bee").ai.navigation;
        navigation.stop();
        navigation.reset_max_visited_nodes_multiplier();
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let Some(saved_flower_pos) = bee_of(level, mob).saved_flower_pos else { return };
        self.travelling_ticks += 1;
        if self.travelling_ticks > reduced_tick_delay(2400) {
            level.bee_drop_flower(mob);
        } else if !level.mob(mob).expect("a bee").ai.navigation.is_in_progress() {
            if is_too_far_away(level, mob, saved_flower_pos) {
                level.bee_drop_flower(mob);
            } else {
                level.bee_pathfind_randomly_towards(mob, saved_flower_pos);
            }
        }
    }

    fn wants_to_go_to_known_flower(&self, bee: &Bee) -> bool {
        bee.ticks_without_nectar_since_exiting_hive > TICKS_BEFORE_GOING_TO_KNOWN_FLOWER
    }
}

pub struct BeeGrowCropGoal;

impl BeeGrowCropGoal {
    fn can_bee_use(&self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if bee_of(level, mob).get_crops_grown_since_pollination() >= MAX_CROPS_GROWABLE {
            return false;
        }
        if level.entity_mut(mob).random.next_float() < 0.3 {
            return false;
        }
        has_nectar(level.entity(mob)) && level.bee_is_hive_valid(mob)
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.can_bee_use(level, mob) && !is_angry(level, mob)
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.can_bee_use(level, mob) && !is_angry(level, mob)
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(30)) != 0 {
            return;
        }
        for i in 1..=2 {
            let below_pos = level.entity(mob).block_position().offset(0, -i, 0);
            let below_state = level.block_state(below_pos);
            let below_block = below_state.block();
            let mut grow_state = None;
            if !below_state.is(&block_tags::BEE_GROWABLES) {
                continue;
            }
            if let Some(crop_block_below) = crop_block::as_crop(below_block) {
                if !crop_block::is_max_age(crop_block_below, below_state) {
                    grow_state = Some(crop_block_below.state_for_age(below_block, crop_block::age(crop_block_below, below_state) + 1));
                }
            } else if below_block.is_instance_of("StemBlock") {
                let age = below_state.get(AGE_7);
                if age < 7 {
                    grow_state = Some(below_state.set(AGE_7, age + 1));
                }
            } else if below_state.is_block(blocks::SWEET_BERRY_BUSH) {
                let age = below_state.get(AGE_3);
                if age < 3 {
                    grow_state = Some(below_state.set(AGE_3, age + 1));
                }
            } else if below_state.is_block(blocks::CAVE_VINES) || below_state.is_block(blocks::CAVE_VINES_PLANT) {
                let bonemealable_block = block::of(below_state);
                if bonemealable_block.is_valid_bonemeal_target(level, below_pos, below_state) {
                    bonemealable_block.perform_bonemeal(level, below_pos, below_state);
                    grow_state = Some(level.block_state(below_pos));
                }
            }
            if let Some(grow_state) = grow_state {
                level.level_event(None, crate::server::level::events::PARTICLES_BEE_GROWTH, below_pos, 15);
                level.set_block(below_pos, grow_state, UPDATE_ALL);
                bee_of_mut(level, mob).increment_num_crops_grown_since_pollination();
            }
        }
    }
}

pub struct BeeHurtByOtherGoal {
    pub hurt_by_target_goal: HurtByTargetGoal,
}

impl BeeHurtByOtherGoal {
    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        is_angry(level, mob) && self.hurt_by_target_goal.target_goal.can_continue_to_use(level, mob)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.hurt_by_target_goal.start_with(level, mob, BeeHurtByOtherGoal::alert_other);
    }

    fn alert_other(_goal: &HurtByTargetGoal, level: &mut ServerLevel, mob: EntityId, other: EntityId, hurt_by_mob: EntityId) {
        if level.is_bee(other) && level.has_line_of_sight(mob, hurt_by_mob) {
            level.set_target(other, Some(hurt_by_mob));
        }
    }
}

pub struct BeeLocateHiveGoal;

impl BeeLocateHiveGoal {
    fn can_bee_use(&self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let bee = bee_of(level, mob);
        bee.remaining_cooldown_before_locating_new_hive == 0 && !bee.has_hive() && level.bee_wants_to_enter_hive(mob)
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.can_bee_use(level, mob) && !is_angry(level, mob)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        bee_of_mut(level, mob).remaining_cooldown_before_locating_new_hive = COOLDOWN_BEFORE_LOCATING_NEW_HIVE;
        let hives_with_space = self.find_nearby_hives_with_space(level, mob);
        if hives_with_space.is_empty() {
            return;
        }
        let bee = bee_of_mut(level, mob);
        for &pos_to_check in &hives_with_space {
            if !bee.blacklisted_targets.contains(&pos_to_check) {
                bee.hive_pos = Some(pos_to_check);
                return;
            }
        }
        bee.blacklisted_targets.clear();
        bee.hive_pos = Some(hives_with_space[0]);
    }

    fn find_nearby_hives_with_space(&self, level: &mut ServerLevel, mob: EntityId) -> Vec<BlockPos> {
        let bee_pos = level.entity(mob).block_position();
        let nearby_hives = level.chunks.poi_manager.get_in_range(&|poi_type| matches!(poi_type, PoiType::Beehive | PoiType::BeeNest), bee_pos, HIVE_SEARCH_DISTANCE, Occupancy::Any);
        let mut hives: Vec<BlockPos> = nearby_hives.into_iter().map(|record| record.pos).filter(|&pos| level.bee_does_hive_have_space(pos)).collect();
        hives.sort_by(|a, b| a.dist_sqr(bee_pos).total_cmp(&b.dist_sqr(bee_pos)));
        hives
    }
}

pub struct BeePollinateGoal {
    successful_pollinating_ticks: i32,
    last_sound_played_tick: i32,
    hover_pos: Option<Vec3>,
    pollinating_ticks: i32,
    unreachable_flower_cache: HashMap<i64, i64>,
}

impl Default for BeePollinateGoal {
    fn default() -> BeePollinateGoal {
        BeePollinateGoal { successful_pollinating_ticks: 0, last_sound_played_tick: 0, hover_pos: None, pollinating_ticks: 0, unreachable_flower_cache: HashMap::new() }
    }
}

const MIN_POLLINATION_TICKS: i32 = 400;
const ARRIVAL_THRESHOLD: f64 = 0.1;
const POSITION_CHANGE_CHANCE: i32 = 25;
const SPEED_MODIFIER: f32 = 0.35;
const HOVER_HEIGHT_WITHIN_FLOWER: f32 = 0.6;
const HOVER_POS_OFFSET: f32 = 0.33333334;
const FLOWER_SEARCH_RADIUS: i32 = 5;
const MAX_POLLINATING_TICKS: i32 = 600;

impl BeePollinateGoal {
    fn can_bee_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let bee = bee_of(level, mob);
        if bee.remaining_cooldown_before_locating_new_flower > 0 || has_nectar(level.entity(mob)) || level.is_raining() {
            return false;
        }
        match self.find_nearby_flower(level, mob) {
            Some(nearby_pos) => {
                bee_of_mut(level, mob).saved_flower_pos = Some(nearby_pos);
                level.get_navigation(mob, |navigation, level| {
                    navigation.move_to(level, mob, nearby_pos.x as f64 + 0.5, nearby_pos.y as f64 + 0.5, nearby_pos.z as f64 + 0.5, 1.2f32 as f64)
                });
                true
            }
            None => {
                let mob_ref = level.mob_mut(mob).expect("a bee");
                let cooldown = mth::next_int(&mut mob_ref.entity.random, MIN_FIND_FLOWER_RETRY_COOLDOWN, MAX_FIND_FLOWER_RETRY_COOLDOWN);
                bee_mut(mob_ref).remaining_cooldown_before_locating_new_flower = cooldown;
                false
            }
        }
    }

    fn can_bee_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let bee = bee_of(level, mob);
        if !bee.pollinating || !bee.has_saved_flower_pos() || level.is_raining() {
            return false;
        }
        if self.has_pollinated_long_enough() { level.entity_mut(mob).random.next_float() < 0.2 } else { true }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.can_bee_use(level, mob) && !is_angry(level, mob)
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.can_bee_continue_to_use(level, mob) && !is_angry(level, mob)
    }

    fn has_pollinated_long_enough(&self) -> bool {
        self.successful_pollinating_ticks > MIN_POLLINATION_TICKS
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.successful_pollinating_ticks = 0;
        self.pollinating_ticks = 0;
        self.last_sound_played_tick = 0;
        let bee = bee_of_mut(level, mob);
        bee.pollinating = true;
        bee.reset_ticks_without_nectar_since_exiting_hive();
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let mob_ref = level.mob_mut(mob).expect("a bee");
        if self.has_pollinated_long_enough() {
            set_has_nectar(mob_ref, true);
        }
        bee_mut(mob_ref).pollinating = false;
        mob_ref.ai.navigation.stop();
        bee_mut(mob_ref).remaining_cooldown_before_locating_new_flower = COOLDOWN_BEFORE_LOCATING_NEW_FLOWER;
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let Some(saved_flower_pos) = bee_of(level, mob).saved_flower_pos else { return };
        self.pollinating_ticks += 1;
        if self.pollinating_ticks > MAX_POLLINATING_TICKS {
            level.bee_drop_flower(mob);
            let bee = bee_of_mut(level, mob);
            bee.pollinating = false;
            bee.remaining_cooldown_before_locating_new_flower = COOLDOWN_BEFORE_LOCATING_NEW_FLOWER;
            return;
        }
        let flower_pos = Vec3::at_bottom_center_of(saved_flower_pos).add(0.0, HOVER_HEIGHT_WITHIN_FLOWER as f64, 0.0);
        if flower_pos.distance_to(level.entity(mob).position()) > 1.0 {
            self.hover_pos = Some(flower_pos);
            self.set_wanted_pos(level, mob);
            return;
        }
        if self.hover_pos.is_none() {
            self.hover_pos = Some(flower_pos);
        }
        let arrived_at_hover_pos = level.entity(mob).position().distance_to(self.hover_pos.expect("a hover position")) <= ARRIVAL_THRESHOLD;
        let mut should_set_wanted_pos = true;
        if !arrived_at_hover_pos && self.pollinating_ticks > MAX_POLLINATING_TICKS {
            level.bee_drop_flower(mob);
            return;
        }
        if arrived_at_hover_pos {
            let should_change_hover_positions = level.entity_mut(mob).random.next_int_bounded(POSITION_CHANGE_CHANCE) == 0;
            if should_change_hover_positions {
                let x = flower_pos.x + self.get_offset(level, mob) as f64;
                let z = flower_pos.z + self.get_offset(level, mob) as f64;
                self.hover_pos = Some(Vec3::new(x, flower_pos.y, z));
                level.mob_mut(mob).expect("a bee").ai.navigation.stop();
            } else {
                should_set_wanted_pos = false;
            }
            level.set_look_at(mob, flower_pos.x, flower_pos.y, flower_pos.z);
        }
        if should_set_wanted_pos {
            self.set_wanted_pos(level, mob);
        }
        self.successful_pollinating_ticks += 1;
        if level.entity_mut(mob).random.next_float() < 0.05 && self.successful_pollinating_ticks > self.last_sound_played_tick + 60 {
            self.last_sound_played_tick = self.successful_pollinating_ticks;
            level.entity_play_sound(mob, sound_event("minecraft:entity.bee.pollinate"), 1.0, 1.0);
        }
    }

    fn set_wanted_pos(&self, level: &mut ServerLevel, mob: EntityId) {
        let hover_pos = self.hover_pos.expect("a hover position");
        level.set_wanted_position(mob, hover_pos.x, hover_pos.y, hover_pos.z, SPEED_MODIFIER as f64);
    }

    fn get_offset(&self, level: &mut ServerLevel, mob: EntityId) -> f32 {
        (level.entity_mut(mob).random.next_float() * 2.0 - 1.0) * HOVER_POS_OFFSET
    }

    fn find_nearby_flower(&mut self, level: &mut ServerLevel, mob: EntityId) -> Option<BlockPos> {
        let mut temp_cache = HashMap::new();
        let origin = level.entity(mob).block_position();
        let mut result = None;
        for pos in BlockPos::within_box_by_manhattan_distance(origin, FLOWER_SEARCH_RADIUS, FLOWER_SEARCH_RADIUS, FLOWER_SEARCH_RADIUS) {
            if !attracts_bees(level.block_state(pos)) {
                continue;
            }
            let unreachable_until_time = self.unreachable_flower_cache.get(&pos.as_long()).copied().unwrap_or(i64::MIN);
            if level.game_time < unreachable_until_time {
                temp_cache.insert(pos.as_long(), unreachable_until_time);
                continue;
            }
            let path = level.get_navigation(mob, |navigation, level| navigation.create_path(level, mob, pos, 1));
            if path.is_some_and(|path| path.can_reach()) {
                result = Some(pos);
                break;
            }
            temp_cache.insert(pos.as_long(), level.game_time + 600);
        }
        self.unreachable_flower_cache = temp_cache;
        result
    }
}

pub struct BeeWanderGoal;

impl BeeWanderGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        level.mob(mob).expect("a bee").ai.navigation.is_done() && level.entity_mut(mob).random.next_int_bounded(10) == 0
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        level.mob(mob).expect("a bee").ai.navigation.is_in_progress()
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if let Some(target_pos) = self.find_pos(level, mob) {
            level.get_navigation(mob, |navigation, level| {
                let path = navigation.create_path(level, mob, BlockPos::containing_vec(target_pos), 1);
                navigation.move_to_path(level, mob, path, 1.0)
            });
        }
    }

    fn find_pos(&self, level: &mut ServerLevel, mob: EntityId) -> Option<Vec3> {
        let wander_threshold = self.get_wander_threshold(level, mob);
        let hive_pos = bee_of(level, mob).hive_pos;
        let wander_direction = if level.bee_is_hive_valid(mob) && !closer_than(level, mob, hive_pos.expect("a hive"), wander_threshold) {
            let hive_pos_vec = Vec3::at_center_of(hive_pos.expect("a hive"));
            hive_pos_vec.subtract(level.entity(mob).position()).normalize()
        } else {
            let e = level.entity(mob);
            calculate_view_vector(e.x_rot_o, level.living(mob).y_head_rot_o)
        };
        let max_xz_radians = std::f32::consts::FRAC_PI_2;
        hover_random_pos::get_pos(level, mob, 8, 7, wander_direction.x, wander_direction.z, max_xz_radians, 3, 1)
            .or_else(|| air_and_water_random_pos::get_pos(level, mob, 8, 4, -2, wander_direction.x, wander_direction.z, max_xz_radians as f64))
    }

    fn get_wander_threshold(&self, level: &ServerLevel, mob: EntityId) -> i32 {
        let bee = bee_of(level, mob);
        let distance_reduction = if !bee.has_hive() && !bee.has_saved_flower_pos() { DEFAULT_WANDER_DISTANCE_REDUCTION } else { RESTRICTED_WANDER_DISTANCE_REDUCTION };
        TOO_FAR_DISTANCE - distance_reduction
    }
}

pub struct ValidateFlowerGoal {
    validate_flower_cooldown: i32,
    last_validate_tick: i64,
}

impl ValidateFlowerGoal {
    fn new(random: &mut impl RandomSource) -> ValidateFlowerGoal {
        ValidateFlowerGoal { validate_flower_cooldown: mth::next_int(random, 20, 40), last_validate_tick: -1 }
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if let Some(saved_flower_pos) = bee_of(level, mob).saved_flower_pos
            && level.is_loaded(saved_flower_pos)
            && !attracts_bees(level.block_state(saved_flower_pos))
        {
            level.bee_drop_flower(mob);
        }
        self.last_validate_tick = level.game_time;
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        level.game_time > self.last_validate_tick + self.validate_flower_cooldown as i64 && !is_angry(level, mob)
    }
}

pub struct ValidateHiveGoal {
    validate_hive_cooldown: i32,
    last_validate_tick: i64,
}

impl ValidateHiveGoal {
    fn new(random: &mut impl RandomSource) -> ValidateHiveGoal {
        ValidateHiveGoal { validate_hive_cooldown: mth::next_int(random, 20, 40), last_validate_tick: -1 }
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if let Some(hive_pos) = bee_of(level, mob).hive_pos
            && level.is_loaded(hive_pos)
            && !level.bee_is_hive_valid(mob)
        {
            level.bee_drop_hive(mob);
        }
        self.last_validate_tick = level.game_time;
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        level.game_time > self.last_validate_tick + self.validate_hive_cooldown as i64 && !is_angry(level, mob)
    }
}
