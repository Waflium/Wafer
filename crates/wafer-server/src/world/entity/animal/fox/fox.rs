use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::properties::AGE_3;
use wafer_data::{BlockState, EquipmentSlot, blocks, fluid_tags, game_rule, item_tags, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::ageable_mob::{self, AgeableMobGroupData, BABY_START_AGE};
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::control::look_control::LookControlKind;
use crate::world::entity::ai::control::move_control::MoveControlKind;
use crate::world::entity::ai::goal::avoid_entity_goal::AvoidEntityGoal;
use crate::world::entity::ai::goal::breed_goal::BreedGoal;
use crate::world::entity::ai::goal::climb_on_top_of_powder_snow_goal::ClimbOnTopOfPowderSnowGoal;
use crate::world::entity::ai::goal::flee_sun_goal::FleeSunGoal;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::follow_parent_goal::FollowParentGoal;
use crate::world::entity::ai::goal::goal::{Goal, reduced_tick_delay};
use crate::world::entity::ai::goal::leap_at_target_goal::LeapAtTargetGoal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::melee_attack_goal::MeleeAttackGoal;
use crate::world::entity::ai::goal::move_to_block_goal::MoveToBlockGoal;
use crate::world::entity::ai::goal::panic_goal::PanicGoal;
use crate::world::entity::ai::goal::stroll_through_village_goal::StrollThroughVillageGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::FOX_ATTRIBUTES;
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::entity_reference::EntityReference;
use crate::world::entity::entity_selector;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::item::ItemEntity;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{self, Mob, MobKind};
use crate::world::entity::monster::zombie::zombie;
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, entity_event, sound_event};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::BlockGetter;
use crate::world::level::block::cave_vines;
use crate::world::level::path_navigation_region::PathNavigationRegion;
use crate::world::level::pathfinder::path_type::PathType;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::update_flags::UPDATE_CLIENTS;
use crate::world::level::storage::tag_value_input;

pub const DATA_TYPE_ID: usize = 18;
pub const DATA_FLAGS_ID: usize = 19;
pub const DATA_TRUSTED_ID_0: usize = 20;
pub const DATA_TRUSTED_ID_1: usize = 21;
const FLAG_SITTING: u8 = 1;
pub const FLAG_CROUCHING: u8 = 4;
pub const FLAG_INTERESTED: u8 = 8;
pub const FLAG_POUNCING: u8 = 16;
const FLAG_SLEEPING: u8 = 32;
const FLAG_FACEPLANTED: u8 = 64;
const FLAG_DEFENDING: u8 = 128;
const MIN_TICKS_BEFORE_EAT: i32 = 600;
const MAX_CROUCH_AMOUNT: f32 = 5.0;
const DEFAULT_SLEEPING: bool = false;
const DEFAULT_SITTING: bool = false;
const DEFAULT_CROUCHING: bool = false;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.6, 0.7).with_eye_height(0.4).with_passenger_attachments(&[Vec3::new(0.0, 0.6375, -0.25)]);
pub const BABY_DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(0.6 * 0.6, 0.7 * 0.6).with_eye_height(0.34375).with_passenger_attachments(&[Vec3::new(0.0, 0.375f32 as f64, 0.0)]);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Variant {
    Red,
    Snow,
}

impl Variant {
    pub const DEFAULT: Variant = Variant::Red;

    pub fn get_id(self) -> i32 {
        self as i32
    }

    pub fn get_serialized_name(self) -> &'static str {
        match self {
            Variant::Red => "red",
            Variant::Snow => "snow",
        }
    }

    pub fn by_id(id: i32) -> Variant {
        match id {
            1 => Variant::Snow,
            _ => Variant::Red,
        }
    }

    pub fn by_name(name: &str) -> Option<Variant> {
        match name {
            "red" => Some(Variant::Red),
            "snow" => Some(Variant::Snow),
            _ => None,
        }
    }

    pub fn by_biome(biome: crate::world::level::chunk::Biome) -> Variant {
        if biome.is("minecraft:spawns_snow_foxes") { Variant::Snow } else { Variant::Red }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FoxGroupData {
    pub ageable_mob_group_data: AgeableMobGroupData,
    pub variant: Variant,
}

impl FoxGroupData {
    pub fn new(variant: Variant) -> FoxGroupData {
        FoxGroupData { ageable_mob_group_data: AgeableMobGroupData::new(false), variant }
    }
}

#[derive(Default)]
pub struct Fox {
    pub animal: Animal,
    interested_angle: f32,
    interested_angle_o: f32,
    crouch_amount: f32,
    crouch_amount_o: f32,
    ticks_since_eaten: i32,
}

pub fn fox(mob: &Mob) -> &Fox {
    match &mob.kind {
        MobKind::Fox(fox) => fox,
        _ => unreachable!("not a fox"),
    }
}

pub fn fox_mut(mob: &mut Mob) -> &mut Fox {
    match &mut mob.kind {
        MobKind::Fox(fox) => fox,
        _ => unreachable!("not a fox"),
    }
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::FOX_FOOD)
}

fn is_consumable_food(item_stack: &ItemStack) -> bool {
    item_stack.has(dc::FOOD) && item_stack.has(dc::CONSUMABLE)
}

pub fn get_variant(entity: &Entity) -> Variant {
    match entity.data.get(DATA_TYPE_ID) {
        DataValue::Int(id) => Variant::by_id(*id),
        _ => Variant::DEFAULT,
    }
}

pub fn set_variant(entity: &mut Entity, variant: Variant) {
    entity.data.set(DATA_TYPE_ID, DataValue::Int(variant.get_id()));
}

fn get_flag(entity: &Entity, flag: u8) -> bool {
    matches!(entity.data.get(DATA_FLAGS_ID), DataValue::Byte(flags) if *flags as u8 & flag != 0)
}

fn set_flag(entity: &mut Entity, flag: u8, value: bool) {
    let flags = match entity.data.get(DATA_FLAGS_ID) {
        DataValue::Byte(flags) => *flags as u8,
        _ => 0,
    };
    entity.data.set(DATA_FLAGS_ID, DataValue::Byte((if value { flags | flag } else { flags & !flag }) as i8));
}

pub fn is_sitting(entity: &Entity) -> bool {
    get_flag(entity, FLAG_SITTING)
}

pub fn set_sitting(entity: &mut Entity, value: bool) {
    set_flag(entity, FLAG_SITTING, value);
}

pub fn is_faceplanted(entity: &Entity) -> bool {
    get_flag(entity, FLAG_FACEPLANTED)
}

fn set_faceplanted(entity: &mut Entity, faceplanted: bool) {
    set_flag(entity, FLAG_FACEPLANTED, faceplanted);
}

pub fn is_defending(entity: &Entity) -> bool {
    get_flag(entity, FLAG_DEFENDING)
}

fn set_defending(entity: &mut Entity, defending: bool) {
    set_flag(entity, FLAG_DEFENDING, defending);
}

pub fn is_sleeping(entity: &Entity) -> bool {
    get_flag(entity, FLAG_SLEEPING)
}

fn set_sleeping(entity: &mut Entity, sleeping: bool) {
    set_flag(entity, FLAG_SLEEPING, sleeping);
}

pub fn is_pouncing(entity: &Entity) -> bool {
    get_flag(entity, FLAG_POUNCING)
}

pub fn set_is_pouncing(entity: &mut Entity, pouncing: bool) {
    set_flag(entity, FLAG_POUNCING, pouncing);
}

pub fn set_is_crouching(entity: &mut Entity, is_crouching: bool) {
    set_flag(entity, FLAG_CROUCHING, is_crouching);
}

pub fn is_crouching(entity: &Entity) -> bool {
    get_flag(entity, FLAG_CROUCHING)
}

pub fn set_is_interested(entity: &mut Entity, value: bool) {
    set_flag(entity, FLAG_INTERESTED, value);
}

pub fn is_interested(entity: &Entity) -> bool {
    get_flag(entity, FLAG_INTERESTED)
}

fn wake_up(entity: &mut Entity) {
    set_sleeping(entity, false);
}

fn clear_states(entity: &mut Entity) {
    set_is_interested(entity, false);
    set_is_crouching(entity, false);
    set_sitting(entity, false);
    set_sleeping(entity, false);
    set_defending(entity, false);
    set_faceplanted(entity, false);
}

pub fn can_move(entity: &Entity) -> bool {
    !is_sleeping(entity) && !is_sitting(entity) && !is_faceplanted(entity)
}

pub fn get_trusted_entities(entity: &Entity) -> impl Iterator<Item = EntityReference> + '_ {
    [DATA_TRUSTED_ID_0, DATA_TRUSTED_ID_1].into_iter().filter_map(|id| match entity.data.get(id) {
        DataValue::OptionalLivingEntityReference(Some(uuid)) => Some(EntityReference::of(*uuid)),
        _ => None,
    })
}

fn add_trusted_entity(entity: &mut Entity, reference: EntityReference) {
    let value = DataValue::OptionalLivingEntityReference(Some(reference.get_uuid()));
    if matches!(entity.data.get(DATA_TRUSTED_ID_0), DataValue::OptionalLivingEntityReference(Some(_))) {
        entity.data.set(DATA_TRUSTED_ID_1, value);
    } else {
        entity.data.set(DATA_TRUSTED_ID_0, value);
    }
}

fn clear_trusted(entity: &mut Entity) {
    entity.data.set(DATA_TRUSTED_ID_0, DataValue::OptionalLivingEntityReference(None));
    entity.data.set(DATA_TRUSTED_ID_1, DataValue::OptionalLivingEntityReference(None));
}

fn is_stalkable_prey(level: &ServerLevel, entity: EntityId) -> bool {
    TargetType::Chicken.is_instance(level, entity) || TargetType::Rabbit.is_instance(level, entity)
}

fn is_trusted_target(level: &ServerLevel, entity: EntityId) -> bool {
    level.is_living(entity) && {
        let living = level.living(entity);
        living.last_hurt_mob.is_some() && living.last_hurt_mob_timestamp < level.entity(entity).tick_count + 600
    }
}

fn is_land_target(_mob: EntityId, target: EntityId, level: &ServerLevel) -> bool {
    is_stalkable_prey(level, target)
}

fn is_fish_target(_mob: EntityId, target: EntityId, level: &ServerLevel) -> bool {
    level.mob(target).is_some_and(|mob| mob.abstract_schooling_fish().is_some())
}

fn is_defend_trusted_target(mob: EntityId, target: EntityId, level: &ServerLevel) -> bool {
    is_trusted_target(level, target) && !level.trusts(mob, target)
}

fn avoid_players(mob: EntityId, entity: EntityId, level: &ServerLevel) -> bool {
    let avoid = !level.entity(entity).is_discrete() && entity_selector::no_creative_or_spectator(level, entity);
    avoid && !level.trusts(mob, entity) && !is_defending(level.entity(mob))
}

fn avoid_wolves(mob: EntityId, entity: EntityId, level: &ServerLevel) -> bool {
    !level.is_tame(entity) && !is_defending(level.entity(mob))
}

fn avoid_polar_bears(mob: EntityId, _entity: EntityId, level: &ServerLevel) -> bool {
    !is_defending(level.entity(mob))
}

fn is_target_goal(goal: &Goal, target_type: TargetType) -> bool {
    matches!(goal, Goal::NearestAttackableTarget(goal) if goal.target_type() == target_type)
}

fn add_target_goal(mob: &mut Mob, prio: i32, target_type: TargetType) {
    if mob.ai.target_selector.get_available_goals().iter().any(|goal| is_target_goal(goal.get_goal(), target_type)) {
        return;
    }
    let goal = match target_type {
        TargetType::Animal => NearestAttackableTargetGoal::with(TargetType::Animal, 10, false, false, Some(is_land_target)),
        TargetType::Turtle => NearestAttackableTargetGoal::with(TargetType::Turtle, 10, false, false, Some(zombie::baby_on_land_selector)),
        _ => NearestAttackableTargetGoal::with(TargetType::AbstractFish, 20, false, false, Some(is_fish_target)),
    };
    mob.ai.target_selector.add_goal(prio, Goal::NearestAttackableTarget(goal));
}

fn set_target_goals(mob: &mut Mob) {
    if get_variant(&mob.entity) == Variant::Red {
        add_target_goal(mob, 4, TargetType::Animal);
        add_target_goal(mob, 4, TargetType::Turtle);
        add_target_goal(mob, 6, TargetType::AbstractFish);
    } else {
        add_target_goal(mob, 4, TargetType::AbstractFish);
        add_target_goal(mob, 6, TargetType::Animal);
        add_target_goal(mob, 6, TargetType::Turtle);
    }
}

fn populate_default_equipment_slots(mob: &mut Mob, random: &mut dyn RandomSource) {
    if random.next_float() < 0.2 {
        let odds = random.next_float();
        let held_in_mouth = if odds < 0.05 {
            items::EMERALD
        } else if odds < 0.2 {
            items::EGG
        } else if odds < 0.4 {
            if random.next_boolean() { items::RABBIT_FOOT } else { items::RABBIT_HIDE }
        } else if odds < 0.6 {
            items::WHEAT
        } else if odds < 0.8 {
            items::LEATHER
        } else {
            items::FEATHER
        };
        mob.equipment[EquipmentSlot::Mainhand as usize] = Some(ItemStack::new(held_in_mouth, 1));
    }
}

pub fn finalize_spawn(
    level: &mut (impl ServerLevelAccessor + ?Sized),
    mob: &mut Mob,
    difficulty: &DifficultyInstance,
    spawn_reason: EntitySpawnReason,
    group_data: Option<SpawnGroupData>,
    is_server_level: bool,
) -> Option<SpawnGroupData> {
    let biome = level.get_biome(mob.entity.block_position());
    let mut variant = Variant::by_biome(biome);
    let mut is_baby = false;
    let group_data = match group_data {
        Some(SpawnGroupData::Fox(fox_group_data)) => {
            variant = fox_group_data.variant;
            if fox_group_data.ageable_mob_group_data.get_group_size() >= 2 {
                is_baby = true;
            }
            Some(SpawnGroupData::Fox(fox_group_data))
        }
        _ => Some(SpawnGroupData::Fox(FoxGroupData::new(variant))),
    };
    set_variant(&mut mob.entity, variant);
    if is_baby && mob.set_age(BABY_START_AGE) {
        mob.refresh_dimensions();
    }
    if is_server_level {
        set_target_goals(mob);
    }
    populate_default_equipment_slots(mob, level.get_random());
    let group_data = ageable_mob::finalize_spawn(level, mob, group_data);
    mob::mob_finalize_spawn(level, mob, difficulty, spawn_reason, group_data)
}

impl Fox {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        data.extend([
            DataValue::Int(Variant::DEFAULT.get_id()),
            DataValue::Byte(0),
            DataValue::OptionalLivingEntityReference(None),
            DataValue::OptionalLivingEntityReference(None),
        ]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:fox"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&FOX_ATTRIBUTES), MobKind::Fox(Fox::default()));
        Animal::init(&mut mob);
        mob.ai.look_control.kind = LookControlKind::Fox;
        mob.ai.move_control.kind = MoveControlKind::Fox;
        mob.set_pathfinding_malus(PathType::DamagingInNeighbor, 0.0);
        mob.set_pathfinding_malus(PathType::Damaging, 0.0);
        mob.can_pick_up_loot = true;
        mob.ai.navigation.required_path_length = 32.0;
        mob
    }

    pub fn register_goals(ai: &mut MobAi, random: &mut impl RandomSource, follow_distance: f64) {
        let float_goal = FoxFloatGoal { float_goal: FloatGoal::new(&mut ai.navigation) };
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::FoxFloat(float_goal));
        goal_selector.add_goal(0, Goal::ClimbOnTopOfPowderSnow(ClimbOnTopOfPowderSnowGoal));
        goal_selector.add_goal(1, Goal::Faceplant(FaceplantGoal { countdown: 0 }));
        goal_selector.add_goal(2, Goal::FoxPanic(FoxPanicGoal { panic_goal: PanicGoal::new(2.2) }));
        goal_selector.add_goal(3, Goal::FoxBreed(FoxBreedGoal { breed_goal: BreedGoal::new(1.0) }));
        goal_selector.add_goal(4, Goal::AvoidEntity(AvoidEntityGoal::with_predicate_on_avoid_entity("minecraft:player", 16.0, 1.6, 1.4, avoid_players)));
        goal_selector.add_goal(4, Goal::AvoidEntity(AvoidEntityGoal::with_predicate_on_avoid_entity("minecraft:wolf", 8.0, 1.6, 1.4, avoid_wolves)));
        goal_selector.add_goal(4, Goal::AvoidEntity(AvoidEntityGoal::with_predicate_on_avoid_entity("minecraft:polar_bear", 8.0, 1.6, 1.4, avoid_polar_bears)));
        goal_selector.add_goal(5, Goal::StalkPrey(StalkPreyGoal));
        goal_selector.add_goal(6, Goal::FoxPounce(FoxPounceGoal));
        goal_selector.add_goal(6, Goal::SeekShelter(SeekShelterGoal { flee_sun_goal: FleeSunGoal::new(1.25), interval: reduced_tick_delay(100) }));
        goal_selector.add_goal(7, Goal::FoxMeleeAttack(FoxMeleeAttackGoal { melee_attack_goal: MeleeAttackGoal::new(1.2f32 as f64, true) }));
        goal_selector.add_goal(7, Goal::Sleep(SleepGoal { countdown: random.next_int_bounded(SleepGoal::wait_time_before_sleep()) }));
        goal_selector.add_goal(8, Goal::FoxFollowParent(FoxFollowParentGoal { follow_parent_goal: FollowParentGoal::new(1.25) }));
        goal_selector.add_goal(9, Goal::FoxStrollThroughVillage(FoxStrollThroughVillageGoal { stroll_through_village_goal: StrollThroughVillageGoal::new(200) }));
        goal_selector.add_goal(10, Goal::FoxEatBerries(FoxEatBerriesGoal { move_to_block_goal: MoveToBlockGoal::new(1.2f32 as f64, 12, 1), ticks_waited: 0 }));
        goal_selector.add_goal(10, Goal::LeapAtTarget(LeapAtTargetGoal::new(0.4)));
        goal_selector.add_goal(11, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::new(1.0)));
        goal_selector.add_goal(11, Goal::FoxSearchForItems(FoxSearchForItemsGoal));
        goal_selector.add_goal(12, Goal::FoxLookAtPlayer(FoxLookAtPlayerGoal { look_at_player_goal: LookAtPlayerGoal::new(24.0) }));
        goal_selector.add_goal(13, Goal::PerchAndSearch(PerchAndSearchGoal { rel_x: 0.0, rel_z: 0.0, look_time: 0, looks_remaining: 0 }));
        ai.target_selector.add_goal(3, Goal::DefendTrustedTarget(DefendTrustedTargetGoal::new(follow_distance)));
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.fox.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.fox.death"
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        let trusted = get_trusted_entities(&mob.entity).map(|reference| crate::world::level::storage::uuid_ints(reference.get_uuid())).collect();
        output.push(("Trusted".into(), Nbt::List(trusted)));
        output.push(("Sleeping".into(), Nbt::Byte(is_sleeping(&mob.entity) as i8)));
        output.push(("Type".into(), Nbt::String(get_variant(&mob.entity).get_serialized_name().into())));
        output.push(("Sitting".into(), Nbt::Byte(is_sitting(&mob.entity) as i8)));
        output.push(("Crouching".into(), Nbt::Byte(is_crouching(&mob.entity) as i8)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        let boolean = |key: &str, default: bool| tag_value_input::get_i64(input, key).map_or(default, |v| v != 0);
        clear_trusted(&mut mob.entity);
        let trusted: Vec<EntityReference> = tag_value_input::read(input, "Trusted", |tag| crate::util::codec::list_partial(tag, crate::world::entity::entity_save::decode_uuid))
            .unwrap_or_default()
            .into_iter()
            .map(EntityReference::of)
            .collect();
        for reference in trusted {
            add_trusted_entity(&mut mob.entity, reference);
        }
        set_sleeping(&mut mob.entity, boolean("Sleeping", DEFAULT_SLEEPING));
        let variant = tag_value_input::read_or_error(input, "Type", |tag| crate::util::codec::enum_by_name(tag, Variant::by_name)).unwrap_or(Variant::DEFAULT);
        set_variant(&mut mob.entity, variant);
        set_sitting(&mut mob.entity, boolean("Sitting", DEFAULT_SITTING));
        set_is_crouching(&mut mob.entity, boolean("Crouching", DEFAULT_CROUCHING));
        set_target_goals(mob);
    }

    pub fn is_fully_crouched(&self) -> bool {
        self.crouch_amount == MAX_CROUCH_AMOUNT
    }

    pub fn get_head_roll_angle(&self, a: f32) -> f32 {
        mth::lerp_f(a, self.interested_angle_o, self.interested_angle) * 0.11 * std::f32::consts::PI
    }

    pub fn get_crouch_amount(&self, a: f32) -> f32 {
        mth::lerp_f(a, self.crouch_amount_o, self.crouch_amount)
    }

    pub fn ticks_since_eaten(&self) -> i32 {
        self.ticks_since_eaten
    }
}

pub fn is_path_clear(level: &mut ServerLevel, fox: EntityId, target: EntityId) -> bool {
    let (f, t) = (level.entity(fox), level.entity(target));
    let (fx, fy, fz) = (f.x(), f.y(), f.z());
    let zdiff = t.z() - fz;
    let xdiff = t.x() - fx;
    let slope = zdiff / xdiff;
    for i in 0..6 {
        let z = if slope == 0.0 { 0.0 } else { zdiff * (i as f32 / 6.0) as f64 };
        let x = if slope == 0.0 { xdiff * (i as f32 / 6.0) as f64 } else { z / slope };
        for j in 1..4 {
            if !level.block_state(BlockPos::containing(fx + x, fy + j as f64, fz + z)).can_be_replaced() {
                return false;
            }
        }
    }
    true
}

impl ServerLevel {
    pub fn is_fox(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::Fox(_)))
    }

    pub fn trusts(&self, id: EntityId, entity: EntityId) -> bool {
        get_trusted_entities(self.entity(id)).any(|trusted| trusted.matches(self, entity))
    }

    fn fox_add_trusted_entity(&mut self, id: EntityId, entity: EntityId) {
        let reference = EntityReference::of(self.entity(entity).uuid);
        add_trusted_entity(self.entity_mut(id), reference);
    }

    pub fn fox_ai_step(&mut self, id: EntityId) {
        if self.is_alive(id) && self.is_effective_ai(id) {
            let fox = fox_mut(self.mob_mut(id).expect("a fox"));
            fox.ticks_since_eaten += 1;
            let ticks_since_eaten = fox.ticks_since_eaten;
            let item_in_mouth = self.item_by_slot(id, EquipmentSlot::Mainhand).unwrap_or_else(ItemStack::empty);
            if self.fox_can_eat(id, &item_in_mouth) {
                if ticks_since_eaten > MIN_TICKS_BEFORE_EAT {
                    let remaining_food = self.finish_using_item(id, item_in_mouth);
                    if !remaining_food.is_empty() {
                        self.living_set_item_slot(id, EquipmentSlot::Mainhand, remaining_food);
                    } else {
                        self.set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::empty());
                    }
                    fox_mut(self.mob_mut(id).expect("a fox")).ticks_since_eaten = 0;
                } else if ticks_since_eaten > 560 && self.entity_mut(id).random.next_float() < 0.1 {
                    self.fox_play_eating_sound(id);
                    self.broadcast_entity_event(id, entity_event::FOX_EAT);
                }
            }
            if self.get_target(id).is_none_or(|target| !self.is_alive(target)) {
                let e = self.entity_mut(id);
                set_is_crouching(e, false);
                set_is_interested(e, false);
            }
        }
        if self.is_sleeping(id) || self.is_immobile(id) {
            let living = self.living_mut(id);
            living.jumping = false;
            living.xxa = 0.0;
            living.zza = 0.0;
        }
        self.animal_ai_step(id);
        if !self.is_fox(id) {
            return;
        }
        if is_defending(self.entity(id)) && self.entity_mut(id).random.next_float() < 0.05 {
            self.entity_play_sound(id, sound_event("minecraft:entity.fox.aggro"), 1.0, 1.0);
        }
    }

    fn fox_can_eat(&self, id: EntityId, item_in_mouth: &ItemStack) -> bool {
        is_consumable_food(item_in_mouth) && self.get_target(id).is_none() && self.entity(id).on_ground && !self.is_sleeping(id)
    }

    pub fn fox_play_eating_sound(&mut self, id: EntityId) {
        self.entity_play_sound(id, sound_event("minecraft:entity.fox.eat"), 1.0, 1.0);
    }

    pub fn fox_get_breed_offspring(&mut self, id: EntityId, partner: EntityId) -> Option<Box<Mob>> {
        let mut baby = Box::new(Fox::create());
        let variant = if self.entity_mut(id).random.next_boolean() { get_variant(self.entity(id)) } else { get_variant(self.entity(partner)) };
        set_variant(&mut baby.entity, variant);
        Some(baby)
    }

    pub fn fox_finalize_spawn(&mut self, id: EntityId, difficulty: &DifficultyInstance, spawn_reason: EntitySpawnReason, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
        let (index, _, entity) = self.entities.shift_remove_full(&id).expect("the fox to finalize");
        let LevelEntity::Mob(mut mob) = entity else { unreachable!("a fox") };
        let group_data = finalize_spawn(self, &mut mob, difficulty, spawn_reason, group_data, true);
        self.entities.shift_insert(index, id, LevelEntity::Mob(mob));
        group_data
    }

    pub fn fox_can_hold_item(&self, id: EntityId, item_stack: &ItemStack) -> bool {
        let held_item_stack = self.item_by_slot_ref(id, EquipmentSlot::Mainhand);
        let ticks_since_eaten = fox(self.mob(id).expect("a fox")).ticks_since_eaten;
        held_item_stack.is_none_or(ItemStack::is_empty) || ticks_since_eaten > 0 && is_consumable_food(item_stack) && !held_item_stack.is_some_and(is_consumable_food)
    }

    fn spit_out_item(&mut self, id: EntityId, item_stack: ItemStack) {
        if item_stack.is_empty() {
            return;
        }
        let e = self.entity(id);
        let look_angle = e.look_angle();
        let pos = Vec3::new(e.x() + look_angle.x, e.y() + 1.0, e.z() + look_angle.z);
        let thrower = e.uuid;
        let mut thrown_item = ItemEntity::new(pos, item_stack);
        thrown_item.pickup_delay = 40;
        thrown_item.thrower = Some(thrower);
        self.entity_play_sound(id, sound_event("minecraft:entity.fox.spit"), 1.0, 1.0);
        self.add_fresh_entity(LevelEntity::Item(thrown_item));
    }

    fn fox_drop_item_stack(&mut self, id: EntityId, item_stack: ItemStack) {
        let position = self.entity(id).position();
        let item_entity = ItemEntity::new(position, item_stack);
        self.add_fresh_entity(LevelEntity::Item(item_entity));
    }

    pub fn fox_pick_up_item(&mut self, id: EntityId, entity: EntityId) {
        let Some(LevelEntity::Item(item)) = self.find_entity(entity) else { return };
        let mut item_stack = item.item.clone();
        if !self.fox_can_hold_item(id, &item_stack) {
            return;
        }
        let count = item_stack.count;
        if count > 1 {
            let dropped = item_stack.split(count - 1);
            self.fox_drop_item_stack(id, dropped);
        }
        let held = self.item_by_slot(id, EquipmentSlot::Mainhand).unwrap_or_else(ItemStack::empty);
        self.spit_out_item(id, held);
        self.on_item_pickup(id, entity);
        let in_mouth = item_stack.split(1);
        self.living_set_item_slot(id, EquipmentSlot::Mainhand, in_mouth);
        self.set_guaranteed_drop(id, EquipmentSlot::Mainhand);
        if let Some(LevelEntity::Item(item)) = self.find_entity_mut(entity) {
            item.item.count = item_stack.count;
        }
        self.take(id, entity, item_stack.count);
        self.discard(entity);
        fox_mut(self.mob_mut(id).expect("a fox")).ticks_since_eaten = 0;
    }

    pub fn fox_tick(&mut self, id: EntityId) {
        self.mob_tick(id);
        if !self.is_fox(id) {
            return;
        }
        if self.is_effective_ai(id) {
            let in_water = self.entity(id).is_in_water();
            if in_water || self.get_target(id).is_some() || self.is_thundering() {
                wake_up(self.entity_mut(id));
            }
            if in_water || self.is_sleeping(id) {
                set_sitting(self.entity_mut(id), false);
            }
            if is_faceplanted(self.entity(id)) && self.random.next_float() < 0.2 {
                let pos = self.entity(id).block_position();
                let state = self.block_state(pos);
                self.level_event(None, 2001, pos, state.id() as i32);
            }
        }
        let (interested, crouching) = (is_interested(self.entity(id)), is_crouching(self.entity(id)));
        let fox = fox_mut(self.mob_mut(id).expect("a fox"));
        fox.interested_angle_o = fox.interested_angle;
        if interested {
            fox.interested_angle += (1.0 - fox.interested_angle) * 0.4;
        } else {
            fox.interested_angle += (0.0 - fox.interested_angle) * 0.4;
        }
        fox.crouch_amount_o = fox.crouch_amount;
        if crouching {
            fox.crouch_amount += 0.2;
            if fox.crouch_amount > MAX_CROUCH_AMOUNT {
                fox.crouch_amount = MAX_CROUCH_AMOUNT;
            }
        } else {
            fox.crouch_amount = 0.0;
        }
    }

    pub fn fox_on_offspring_spawned_from_egg(&mut self, spawner: EntityId, offspring: EntityId) {
        self.fox_add_trusted_entity(offspring, spawner);
    }

    pub fn fox_set_target(&mut self, id: EntityId, target: Option<EntityId>) {
        if is_defending(self.entity(id)) && target.is_none() {
            set_defending(self.entity_mut(id), false);
        }
        self.mob_set_target(id, target);
    }

    pub fn fox_play_ambient_sound(&mut self, id: EntityId) {
        let ambient = self.get_ambient_sound(id);
        if ambient == Some("minecraft:entity.fox.screech") {
            let pitch = self.voice_pitch(id);
            self.entity_play_sound(id, sound_event("minecraft:entity.fox.screech"), 2.0, pitch);
        } else {
            self.mob_play_ambient_sound(id);
        }
    }

    pub fn fox_get_ambient_sound(&mut self, id: EntityId) -> Option<&'static str> {
        if self.is_sleeping(id) {
            return Some("minecraft:entity.fox.sleep");
        }
        if !self.is_bright_outside() && self.entity_mut(id).random.next_float() < 0.1 {
            let area = self.entity(id).bounding_box().inflate(16.0, 16.0, 16.0);
            let nearby_entities = self.get_entities(None, &area).into_iter().any(|entity| self.is_player(entity));
            if !nearby_entities {
                return Some("minecraft:entity.fox.screech");
            }
        }
        Some("minecraft:entity.fox.ambient")
    }

    pub fn fox_drop_all_death_loot(&mut self, id: EntityId, source: &DamageSource) {
        if let Some(item_stack) = self.item_by_slot(id, EquipmentSlot::Mainhand) {
            self.spawn_at_location(id, item_stack, 0.0);
            self.living_set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::empty());
        }
        self.living_entity_drop_all_death_loot(id, source);
    }

    pub fn fox_move_control_tick(&mut self, id: EntityId) {
        if can_move(self.entity(id)) {
            self.move_control_tick(id);
        }
    }

    pub fn fox_look_control_tick(&mut self, id: EntityId) {
        if !self.is_sleeping(id) {
            self.look_control_look_control_tick(id);
        }
    }

    pub fn fox_look_control_reset_x_rot_on_tick(&self, id: EntityId) -> bool {
        let e = self.entity(id);
        !is_pouncing(e) && !is_crouching(e) && !is_interested(e) && !is_faceplanted(e)
    }

    fn fox_has_shelter(&mut self, id: EntityId) -> bool {
        let e = self.entity(id);
        let fox_pos = BlockPos::containing(e.x(), e.bounding_box().max_y, e.z());
        !self.can_see_sky(fox_pos) && self.get_walk_target_value(id, fox_pos) >= 0.0
    }

    fn fox_alertable(&mut self, id: EntityId) -> bool {
        let selector = move |target: EntityId, level: &ServerLevel| level.fox_alertable_entities_selector(id, target);
        let alertable_targeting = TargetingConditions::for_combat().range(12.0).ignore_line_of_sight().selector(&selector);
        let area = self.entity(id).bounding_box().inflate(12.0, 6.0, 12.0);
        let nearby: Vec<EntityId> = self.get_entities_matching(None, &area, |level, entity| level.is_living(entity));
        nearby.into_iter().any(|entity| alertable_targeting.test(self, Some(id), entity))
    }

    fn fox_alertable_entities_selector(&self, id: EntityId, target: EntityId) -> bool {
        if self.is_fox(target) {
            false
        } else if is_stalkable_prey(self, target) || self.is_monster(target) {
            true
        } else if self.is_tamable_animal(target) {
            !self.is_tame(target)
        } else if self.is_player(target) && (self.is_spectator(target) || self.player(target).game_mode.game_type.is_creative()) {
            false
        } else if self.trusts(id, target) {
            false
        } else {
            !self.is_sleeping(target) && !self.entity(target).is_discrete()
        }
    }
}

pub struct DefendTrustedTargetGoal {
    pub nearest_attackable_target_goal: NearestAttackableTargetGoal,
    target_conditions_range: f64,
    trusted_last_hurt_by: Option<EntityId>,
    trusted_last_hurt: Option<EntityId>,
    timestamp: i32,
}

impl DefendTrustedTargetGoal {
    fn new(follow_distance: f64) -> DefendTrustedTargetGoal {
        DefendTrustedTargetGoal {
            nearest_attackable_target_goal: NearestAttackableTargetGoal::with(TargetType::LivingEntity, 10, false, false, Some(is_defend_trusted_target)),
            target_conditions_range: follow_distance,
            trusted_last_hurt_by: None,
            trusted_last_hurt: None,
            timestamp: 0,
        }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let random_interval = self.nearest_attackable_target_goal.random_interval;
        if random_interval > 0 && level.entity_mut(mob).random.next_int_bounded(random_interval) != 0 {
            return false;
        }
        let trusted: Vec<EntityReference> = get_trusted_entities(level.entity(mob)).collect();
        for trusted_reference in trusted {
            if let Some(trusted_entity) = EntityReference::get_living_entity(Some(trusted_reference), level) {
                self.trusted_last_hurt = Some(trusted_entity);
                self.trusted_last_hurt_by = level.get_last_hurt_by_mob(trusted_entity);
                let timestamp = level.living(trusted_entity).last_hurt_by_mob_timestamp;
                let selector = move |target: EntityId, level: &ServerLevel| is_defend_trusted_target(mob, target, level);
                let target_conditions = TargetingConditions::for_combat().range(self.target_conditions_range).selector(&selector);
                return timestamp != self.timestamp && self.nearest_attackable_target_goal.target_goal.can_attack(level, mob, self.trusted_last_hurt_by, &target_conditions);
            }
        }
        false
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.nearest_attackable_target_goal.set_target(self.trusted_last_hurt_by);
        self.nearest_attackable_target_goal.target = self.trusted_last_hurt_by;
        if let Some(trusted_last_hurt) = self.trusted_last_hurt {
            self.timestamp = level.living(trusted_last_hurt).last_hurt_by_mob_timestamp;
        }
        level.entity_play_sound(mob, sound_event("minecraft:entity.fox.aggro"), 1.0, 1.0);
        set_defending(level.entity_mut(mob), true);
        wake_up(level.entity_mut(mob));
        self.nearest_attackable_target_goal.start(level, mob);
    }
}

pub struct FaceplantGoal {
    countdown: i32,
}

impl FaceplantGoal {
    pub fn can_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        is_faceplanted(level.entity(mob))
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        self.can_use(level, mob) && self.countdown > 0
    }

    pub fn start(&mut self) {
        self.countdown = reduced_tick_delay(40);
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        set_faceplanted(level.entity_mut(mob), false);
    }

    pub fn tick(&mut self) {
        self.countdown -= 1;
    }
}

pub struct FoxBreedGoal {
    pub breed_goal: BreedGoal,
}

impl FoxBreedGoal {
    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        clear_states(level.entity_mut(mob));
        if let Some(partner) = self.breed_goal.partner {
            clear_states(level.entity_mut(partner));
        }
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.breed_goal.tick_with(level, mob, FoxBreedGoal::breed);
    }

    fn breed(_goal: &BreedGoal, level: &mut ServerLevel, animal: EntityId, partner: EntityId) {
        let Some(mut offspring) = level.get_breed_offspring(animal, partner) else { return };
        let animal_love_cause = level.get_love_cause(animal);
        let partner_love_cause = level.get_love_cause(partner);
        let mut love_cause = animal_love_cause;
        if let Some(animal_love_cause) = animal_love_cause {
            add_trusted_entity(&mut offspring.entity, EntityReference::of(level.entity(animal_love_cause).uuid));
        } else {
            love_cause = partner_love_cause;
        }
        if let Some(partner_love_cause) = partner_love_cause.filter(|&cause| animal_love_cause != Some(cause)) {
            add_trusted_entity(&mut offspring.entity, EntityReference::of(level.entity(partner_love_cause).uuid));
        }
        let offspring_id = offspring.entity.id;
        level.entities.insert(offspring_id, LevelEntity::Mob(offspring));
        if let Some(love_cause) = love_cause {
            level.award_stat(love_cause, crate::stats::stats::ANIMALS_BRED, 1);
            crate::advancements::triggers::criteria_triggers::BRED_ANIMALS.trigger(level, love_cause, animal, partner, Some(offspring_id));
        }
        let Some(LevelEntity::Mob(mut offspring)) = level.entities.shift_remove(&offspring_id) else { unreachable!("the offspring") };
        level.set_age(animal, 6000);
        level.set_age(partner, 6000);
        level.reset_love(animal);
        level.reset_love(partner);
        offspring.set_baby(true);
        let e = level.entity(animal);
        offspring.entity.snap_to(e.x(), e.y(), e.z(), 0.0, 0.0);
        level.add_fresh_entity_with_passengers(LevelEntity::Mob(offspring));
        level.broadcast_entity_event(animal, entity_event::IN_LOVE_HEARTS);
        if level.game_rules.get_bool(game_rule::MOB_DROPS) {
            let e = level.entity_mut(animal);
            let value = e.random.next_int_bounded(7) + 1;
            let pos = e.position();
            let orb = level.new_experience_orb(pos, Vec3::ZERO, value);
            level.add_fresh_entity(LevelEntity::ExperienceOrb(orb));
        }
    }
}

pub struct FoxEatBerriesGoal {
    pub move_to_block_goal: MoveToBlockGoal,
    ticks_waited: i32,
}

impl FoxEatBerriesGoal {
    const WAIT_TICKS: i32 = 40;

    fn accepted_distance() -> f64 {
        2.0
    }

    fn should_recalculate_path(move_to_block_goal: &MoveToBlockGoal) -> bool {
        move_to_block_goal.try_ticks % 100 == 0
    }

    fn is_valid_target(level: &ServerLevel, pos: BlockPos) -> bool {
        let block_state = PathNavigationRegion::new(level).get_block_state(pos);
        block_state.is_block(blocks::SWEET_BERRY_BUSH) && block_state.get(AGE_3) >= 2 || cave_vines::has_glow_berries(block_state)
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if self.move_to_block_goal.is_reached_target() {
            if self.ticks_waited >= Self::WAIT_TICKS {
                self.on_reached_target(level, mob);
            } else {
                self.ticks_waited += 1;
            }
        } else if !self.move_to_block_goal.is_reached_target() && level.entity_mut(mob).random.next_float() < 0.05 {
            level.entity_play_sound(mob, sound_event("minecraft:entity.fox.sniff"), 1.0, 1.0);
        }
        self.move_to_block_goal.tick_with(level, mob, Self::accepted_distance(), Self::should_recalculate_path);
    }

    fn on_reached_target(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if level.game_rules.get_bool(game_rule::MOB_GRIEFING) {
            let state = level.block_state(self.move_to_block_goal.block_pos);
            if state.is_block(blocks::SWEET_BERRY_BUSH) {
                self.pick_sweet_berries(level, mob, state);
            } else if cave_vines::has_glow_berries(state) {
                self.pick_glow_berry(level, mob, state);
            }
        }
    }

    fn pick_glow_berry(&self, level: &mut ServerLevel, mob: EntityId, state: BlockState) {
        cave_vines::use_vine(Some(mob), state, level, self.move_to_block_goal.block_pos);
    }

    fn pick_sweet_berries(&self, level: &mut ServerLevel, mob: EntityId, state: BlockState) {
        let block_pos = self.move_to_block_goal.block_pos;
        let age = state.get(AGE_3);
        let mut count = 1 + level.random.next_int_bounded(2) + if age == 3 { 1 } else { 0 };
        let held_item = level.item_by_slot_ref(mob, EquipmentSlot::Mainhand);
        if held_item.is_none() {
            level.living_set_item_slot(mob, EquipmentSlot::Mainhand, ItemStack::new(items::SWEET_BERRIES, 1));
            count -= 1;
        }
        if count > 0 {
            level.pop_resource(block_pos, ItemStack::new(items::SWEET_BERRIES, count));
        }
        level.entity_play_sound(mob, sound_event("minecraft:block.sweet_berry_bush.pick_berries"), 1.0, 1.0);
        level.set_block(block_pos, state.set(AGE_3, 1), UPDATE_CLIENTS);
        level.game_event_at_with(crate::world::level::gameevent::game_event::BLOCK_CHANGE, block_pos, crate::world::level::gameevent::game_event::Context::of(Some(mob)));
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        !level.is_sleeping(mob) && self.move_to_block_goal.can_use(level, mob, &Self::is_valid_target)
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel) -> bool {
        self.move_to_block_goal.can_continue_to_use(level, &Self::is_valid_target)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.ticks_waited = 0;
        set_sitting(level.entity_mut(mob), false);
        self.move_to_block_goal.start(level, mob);
    }
}

pub struct FoxFloatGoal {
    pub float_goal: FloatGoal,
}

impl FoxFloatGoal {
    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        clear_states(level.entity_mut(mob));
    }

    pub fn can_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        let e = level.entity(mob);
        e.fluid_height(&fluid_tags::ENTITY_FLOATABLE) > 0.25 || e.is_in_lava()
    }
}

pub struct FoxFollowParentGoal {
    pub follow_parent_goal: FollowParentGoal,
}

impl FoxFollowParentGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        !is_defending(level.entity(mob)) && self.follow_parent_goal.can_use(level, mob)
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        !is_defending(level.entity(mob)) && self.follow_parent_goal.can_continue_to_use(level, mob)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        clear_states(level.entity_mut(mob));
        self.follow_parent_goal.start();
    }
}

pub struct FoxLookAtPlayerGoal {
    pub look_at_player_goal: LookAtPlayerGoal,
}

impl FoxLookAtPlayerGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.look_at_player_goal.can_use(level, mob) && !is_faceplanted(level.entity(mob)) && !is_interested(level.entity(mob))
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.look_at_player_goal.can_continue_to_use(level, mob) && !is_faceplanted(level.entity(mob)) && !is_interested(level.entity(mob))
    }
}

pub struct FoxMeleeAttackGoal {
    pub melee_attack_goal: MeleeAttackGoal,
}

impl FoxMeleeAttackGoal {
    fn check_and_perform_attack(melee_attack_goal: &mut MeleeAttackGoal, level: &mut ServerLevel, mob: EntityId, target: EntityId) {
        if melee_attack_goal.can_perform_attack(level, mob, target) {
            melee_attack_goal.reset_attack_cooldown();
            level.do_hurt_target(mob, target);
            level.entity_play_sound(mob, sound_event("minecraft:entity.fox.bite"), 1.0, 1.0);
        }
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.melee_attack_goal.tick_with(level, mob, FoxMeleeAttackGoal::check_and_perform_attack);
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        set_is_interested(level.entity_mut(mob), false);
        self.melee_attack_goal.start(level, mob);
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let e = level.entity(mob);
        !is_sitting(e) && !is_sleeping(e) && !is_crouching(e) && !is_faceplanted(e) && self.melee_attack_goal.can_use(level, mob)
    }
}

pub struct FoxPanicGoal {
    pub panic_goal: PanicGoal,
}

impl FoxPanicGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        !is_defending(level.entity(mob)) && self.panic_goal.can_use(level, mob)
    }
}

pub struct FoxPounceGoal;

impl FoxPounceGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if !fox(level.mob(mob).expect("a fox")).is_fully_crouched() {
            return false;
        }
        let Some(target) = level.get_target(mob).filter(|&target| level.is_alive(target)) else { return false };
        let t = level.entity(target);
        if t.get_motion_direction() != t.get_direction() {
            return false;
        }
        let has_clear_path = is_path_clear(level, mob, target);
        if !has_clear_path {
            level.get_navigation(mob, |navigation, level| navigation.create_path_to_entity(level, mob, target, 0));
            let e = level.entity_mut(mob);
            set_is_crouching(e, false);
            set_is_interested(e, false);
        }
        has_clear_path
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if level.get_target(mob).is_some_and(|target| level.is_alive(target)) {
            let e = level.entity(mob);
            let yd = e.delta_movement.y;
            (!(yd * yd < 0.05f32 as f64) || !(e.x_rot.abs() < 15.0) || !e.on_ground) && !is_faceplanted(e)
        } else {
            false
        }
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.living_mut(mob).jumping = true;
        let e = level.entity_mut(mob);
        set_is_pouncing(e, true);
        set_is_interested(e, false);
        if let Some(target) = level.get_target(mob) {
            level.set_look_at_entity(mob, target, 60.0, 30.0);
            let (t, f) = (level.entity(target), level.entity(mob));
            let uv = Vec3::new(t.x() - f.x(), t.y() - f.y(), t.z() - f.z()).normalize();
            let movement = f.delta_movement.add(uv.x * 0.8, 0.9, uv.z * 0.8);
            level.entity_mut(mob).set_delta_movement(movement);
        }
        level.mob_mut(mob).expect("a fox").ai.navigation.stop();
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let m = level.mob_mut(mob).expect("a fox");
        set_is_crouching(&mut m.entity, false);
        let fox = fox_mut(m);
        fox.crouch_amount = 0.0;
        fox.crouch_amount_o = 0.0;
        set_is_interested(&mut m.entity, false);
        set_is_pouncing(&mut m.entity, false);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let target = level.get_target(mob);
        if let Some(target) = target {
            level.set_look_at_entity(mob, target, 60.0, 30.0);
        }
        if !is_faceplanted(level.entity(mob)) {
            let jumping = level.living(mob).jumping;
            let e = level.entity_mut(mob);
            let movement = e.delta_movement;
            if movement.y * movement.y < 0.03f32 as f64 && e.x_rot != 0.0 {
                let x_rot = mth::rot_lerp(0.2, e.x_rot, 0.0);
                e.set_x_rot(x_rot);
            } else {
                let direction = movement.horizontal_distance();
                let upwards_bias = if jumping && movement.y > 0.0 { 6.5f32 } else { 1.0 };
                let biased_y = movement.y * upwards_bias as f64;
                let len = (direction * direction + biased_y * biased_y).sqrt();
                if len > 1.0E-5f32 as f64 {
                    let signum = if -biased_y > 0.0 { 1.0 } else if -biased_y < 0.0 { -1.0 } else { -biased_y };
                    let rotation = signum * (direction / len).acos() * mth::RAD_TO_DEG as f64;
                    e.set_x_rot(rotation as f32);
                }
            }
        }
        if let Some(target) = target.filter(|&target| level.entity(mob).distance_to(level.entity(target)) <= 2.0) {
            level.do_hurt_target(mob, target);
        } else {
            let e = level.entity(mob);
            if e.x_rot > 0.0 && e.on_ground && e.delta_movement.y as f32 != 0.0 && level.block_state(e.block_position()).is_block(blocks::SNOW) {
                level.entity_mut(mob).set_x_rot(60.0);
                level.set_target(mob, None);
                set_faceplanted(level.entity_mut(mob), true);
            }
        }
    }
}

pub struct FoxSearchForItemsGoal;

impl FoxSearchForItemsGoal {
    fn items(level: &ServerLevel, mob: EntityId) -> Vec<EntityId> {
        let area = level.entity(mob).bounding_box().inflate(8.0, 8.0, 8.0);
        level.items_in(&area, None).into_iter().filter(|&item| matches!(level.find_entity(item), Some(LevelEntity::Item(item)) if !item.has_pickup_delay())).collect()
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if level.item_by_slot_ref(mob, EquipmentSlot::Mainhand).is_some() {
            return false;
        }
        if level.get_target(mob).is_some() || level.get_last_hurt_by_mob(mob).is_some() {
            return false;
        }
        if !can_move(level.entity(mob)) {
            return false;
        }
        if level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(10)) != 0 {
            return false;
        }
        let items = FoxSearchForItemsGoal::items(level, mob);
        !items.is_empty() && level.item_by_slot_ref(mob, EquipmentSlot::Mainhand).is_none()
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let items = FoxSearchForItemsGoal::items(level, mob);
        if level.item_by_slot_ref(mob, EquipmentSlot::Mainhand).is_none()
            && let Some(&item) = items.first()
        {
            level.get_navigation(mob, |navigation, level| navigation.move_to_entity(level, mob, item, 1.2f32 as f64));
        }
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let items = FoxSearchForItemsGoal::items(level, mob);
        if let Some(&item) = items.first() {
            level.get_navigation(mob, |navigation, level| navigation.move_to_entity(level, mob, item, 1.2f32 as f64));
        }
    }
}

pub struct FoxStrollThroughVillageGoal {
    pub stroll_through_village_goal: StrollThroughVillageGoal,
}

impl FoxStrollThroughVillageGoal {
    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        clear_states(level.entity_mut(mob));
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.stroll_through_village_goal.can_use(level, mob) && FoxStrollThroughVillageGoal::can_fox_move(level, mob)
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        self.stroll_through_village_goal.can_continue_to_use(level, mob) && FoxStrollThroughVillageGoal::can_fox_move(level, mob)
    }

    fn can_fox_move(level: &ServerLevel, mob: EntityId) -> bool {
        let e = level.entity(mob);
        !is_sleeping(e) && !is_sitting(e) && !is_defending(e) && level.get_target(mob).is_none()
    }
}

pub struct PerchAndSearchGoal {
    rel_x: f64,
    rel_z: f64,
    look_time: i32,
    looks_remaining: i32,
}

impl PerchAndSearchGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        level.get_last_hurt_by_mob(mob).is_none()
            && level.entity_mut(mob).random.next_float() < 0.02
            && !level.is_sleeping(mob)
            && level.get_target(mob).is_none()
            && level.mob(mob).expect("a fox").ai.navigation.is_done()
            && !level.fox_alertable(mob)
            && !is_pouncing(level.entity(mob))
            && !is_crouching(level.entity(mob))
    }

    pub fn can_continue_to_use(&self) -> bool {
        self.looks_remaining > 0
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.reset_look(level, mob);
        self.looks_remaining = 2 + level.entity_mut(mob).random.next_int_bounded(3);
        set_sitting(level.entity_mut(mob), true);
        level.mob_mut(mob).expect("a fox").ai.navigation.stop();
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        set_sitting(level.entity_mut(mob), false);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.look_time -= 1;
        if self.look_time <= 0 {
            self.looks_remaining -= 1;
            self.reset_look(level, mob);
        }
        let m = level.mob_mut(mob).expect("a fox");
        let (x, y, z) = (m.entity.x() + self.rel_x, m.entity.eye_y(), m.entity.z() + self.rel_z);
        let (max_head_y_rot, max_head_x_rot) = (m.max_head_y_rot() as f32, m.get_max_head_x_rot() as f32);
        m.ai.look_control.set_look_at(x, y, z, max_head_y_rot, max_head_x_rot);
    }

    fn reset_look(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let random = &mut level.entity_mut(mob).random;
        let rnd = (std::f64::consts::PI * 2.0) * random.next_double();
        self.rel_x = rnd.cos();
        self.rel_z = rnd.sin();
        self.look_time = reduced_tick_delay(80 + random.next_int_bounded(20));
    }
}

pub struct SeekShelterGoal {
    pub flee_sun_goal: FleeSunGoal,
    interval: i32,
}

impl SeekShelterGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if level.is_sleeping(mob) || level.get_target(mob).is_some() {
            return false;
        }
        if level.is_thundering() && level.can_see_sky(level.entity(mob).block_position()) {
            self.flee_sun_goal.set_wanted_pos(level, mob)
        } else if self.interval > 0 {
            self.interval -= 1;
            false
        } else {
            self.interval = 100;
            let pos = level.entity(mob).block_position();
            level.is_bright_outside() && level.can_see_sky(pos) && !level.is_village(pos) && self.flee_sun_goal.set_wanted_pos(level, mob)
        }
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        clear_states(level.entity_mut(mob));
        self.flee_sun_goal.start(level, mob);
    }
}

pub struct SleepGoal {
    countdown: i32,
}

impl SleepGoal {
    fn wait_time_before_sleep() -> i32 {
        reduced_tick_delay(140)
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let living = level.living(mob);
        if living.xxa == 0.0 && living.yya == 0.0 && living.zza == 0.0 { self.can_sleep(level, mob) || level.is_sleeping(mob) } else { false }
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.can_sleep(level, mob)
    }

    fn can_sleep(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if self.countdown > 0 {
            self.countdown -= 1;
            false
        } else {
            level.is_bright_outside() && level.fox_has_shelter(mob) && !level.fox_alertable(mob) && !level.entity(mob).is_in_powder_snow
        }
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.countdown = level.entity_mut(mob).random.next_int_bounded(SleepGoal::wait_time_before_sleep());
        clear_states(level.entity_mut(mob));
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let e = level.entity_mut(mob);
        set_sitting(e, false);
        set_is_crouching(e, false);
        set_is_interested(e, false);
        level.living_mut(mob).jumping = false;
        set_sleeping(level.entity_mut(mob), true);
        let m = level.mob_mut(mob).expect("a fox");
        m.ai.navigation.stop();
        let (x, y, z) = (m.entity.x(), m.entity.y(), m.entity.z());
        m.ai.move_control.set_wanted_position(x, y, z, 0.0);
    }

    pub fn countdown(&self) -> i32 {
        self.countdown
    }

    pub fn set_countdown(&mut self, countdown: i32) {
        self.countdown = countdown;
    }
}

pub struct StalkPreyGoal;

impl StalkPreyGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if level.is_sleeping(mob) {
            return false;
        }
        let Some(target) = level.get_target(mob) else { return false };
        let e = level.entity(mob);
        level.is_alive(target)
            && is_stalkable_prey(level, target)
            && e.distance_to_sqr(level.entity(target)) > 36.0
            && !is_crouching(e)
            && !is_interested(e)
            && !level.living(mob).jumping
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let e = level.entity_mut(mob);
        set_sitting(e, false);
        set_faceplanted(e, false);
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        match level.get_target(mob).filter(|&target| is_path_clear(level, mob, target)) {
            Some(target) => {
                let e = level.entity_mut(mob);
                set_is_interested(e, true);
                set_is_crouching(e, true);
                let m = level.mob_mut(mob).expect("a fox");
                m.ai.navigation.stop();
                let (max_head_y_rot, max_head_x_rot) = (m.max_head_y_rot() as f32, m.get_max_head_x_rot() as f32);
                level.set_look_at_entity(mob, target, max_head_y_rot, max_head_x_rot);
            }
            None => {
                let e = level.entity_mut(mob);
                set_is_interested(e, false);
                set_is_crouching(e, false);
            }
        }
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let Some(target) = level.get_target(mob) else { return };
        let m = level.mob(mob).expect("a fox");
        let (max_head_y_rot, max_head_x_rot) = (m.max_head_y_rot() as f32, m.get_max_head_x_rot() as f32);
        level.set_look_at_entity(mob, target, max_head_y_rot, max_head_x_rot);
        if level.entity(mob).distance_to_sqr(level.entity(target)) <= 36.0 {
            let e = level.entity_mut(mob);
            set_is_interested(e, true);
            set_is_crouching(e, true);
            level.mob_mut(mob).expect("a fox").ai.navigation.stop();
        } else {
            level.get_navigation(mob, |navigation, level| navigation.move_to_entity(level, mob, target, 1.5));
        }
    }
}
