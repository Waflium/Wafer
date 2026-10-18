use std::borrow::Cow;
use std::cell::Cell;

use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::properties::AGE_7;
use wafer_data::{block_tags, blocks, game_rule, item_tags, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::ageable_mob::AgeableMobGroupData;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::control::move_control::{MoveControlKind, Operation};
use crate::world::entity::ai::goal::avoid_entity_goal::{AvoidClass, AvoidEntityGoal};
use crate::world::entity::ai::goal::breed_goal::BreedGoal;
use crate::world::entity::ai::goal::climb_on_top_of_powder_snow_goal::ClimbOnTopOfPowderSnowGoal;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::melee_attack_goal::MeleeAttackGoal;
use crate::world::entity::ai::goal::move_to_block_goal::MoveToBlockGoal;
use crate::world::entity::ai::goal::panic_goal::PanicGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::goal::tempt_goal::TemptGoal;
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::{Attribute, AttributeModifier, Operation as ModifierOperation, RABBIT_ATTRIBUTES};
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{Entity, EntityDimensions, EntityId, entity_event, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub const BABY_DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.24, 0.4).with_eye_height(0.39);
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.49, 0.6).with_eye_height(0.59);
pub const STROLL_SPEED_MOD: f64 = 0.6;
pub const BREED_SPEED_MOD: f64 = 0.8;
pub const FOLLOW_SPEED_MOD: f64 = 1.0;
pub const FLEE_SPEED_MOD: f64 = 2.2;
pub const ATTACK_SPEED_MOD: f64 = 1.4;
const JUMP_DELAY_TICKS: i32 = 10;
const PANIC_JUMP_DELAY_TICKS: i32 = 3;
const JUMP_DURATION_IN_TICKS: i32 = 15;
pub const DATA_TYPE_ID: usize = 18;
const DEFAULT_MORE_CARROT_TICKS: i32 = 0;
const KILLER_BUNNY: &str = "entity.minecraft.killer_bunny";
const EVIL_ATTACK_POWER_INCREMENT: f64 = 5.0;
const EVIL_ATTACK_POWER_MODIFIER: &str = "minecraft:evil";
const EVIL_ARMOR_VALUE: f64 = 8.0;
const MORE_CARROTS_DELAY: i32 = 40;

#[derive(Default)]
pub struct Rabbit {
    pub animal: Animal,
    pub jump_control: RabbitJumpControl,
    pub move_control: RabbitMoveControl,
    pub idle_animation_timeout: i32,
    pub jump_ticks: i32,
    pub jump_duration: i32,
    pub was_on_ground: bool,
    pub jump_delay_ticks: i32,
    pub more_carrot_ticks: i32,
}

pub fn rabbit(mob: &Mob) -> &Rabbit {
    match &mob.kind {
        MobKind::Rabbit(rabbit) => rabbit,
        _ => unreachable!("not a rabbit"),
    }
}

pub fn rabbit_mut(mob: &mut Mob) -> &mut Rabbit {
    match &mut mob.kind {
        MobKind::Rabbit(rabbit) => rabbit,
        _ => unreachable!("not a rabbit"),
    }
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::RABBIT_FOOD)
}

pub fn get_variant(entity: &Entity) -> Variant {
    match entity.data.get(DATA_TYPE_ID) {
        DataValue::Int(id) => Variant::by_id(*id),
        _ => Variant::DEFAULT,
    }
}

pub fn set_variant(mob: &mut Mob, variant: Variant) {
    if variant == Variant::Evil {
        mob.living.attributes.instance(Attribute::Armor).expect("rabbits have armor").set_base_value(EVIL_ARMOR_VALUE);
        mob.ai.goal_selector.add_goal(4, Goal::MeleeAttack(MeleeAttackGoal::new(ATTACK_SPEED_MOD, true)));
        mob.ai.target_selector.add_goal(1, Goal::HurtByTarget(HurtByTargetGoal::new(&[]).set_alert_others(&[])));
        mob.ai.target_selector.add_goal(2, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::Player, true)));
        mob.ai.target_selector.add_goal(2, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::Wolf, true)));
        mob.living.attributes.instance(Attribute::AttackDamage).expect("rabbits have attack damage").add_or_update_transient_modifier(AttributeModifier {
            id: Cow::Borrowed(EVIL_ATTACK_POWER_MODIFIER),
            amount: EVIL_ATTACK_POWER_INCREMENT,
            operation: ModifierOperation::AddValue,
        });
        if !mob.entity.has_custom_name() {
            mob.entity.set_custom_name(Some(Nbt::Compound(vec![("translate".into(), Nbt::String(KILLER_BUNNY.into()))])));
        }
    } else {
        mob.living.attributes.instance(Attribute::AttackDamage).expect("rabbits have attack damage").remove_modifier(EVIL_ATTACK_POWER_MODIFIER);
    }
    mob.entity.data.set(DATA_TYPE_ID, DataValue::Int(variant.id()));
}

pub fn get_random_rabbit_variant(level: &mut (impl ServerLevelAccessor + ?Sized), pos: BlockPos) -> Variant {
    let biome = level.get_biome(pos);
    let random_val = level.get_random().next_int_bounded(100);
    if biome.is("minecraft:spawns_white_rabbits") {
        if random_val < 80 { Variant::White } else { Variant::WhiteSplotched }
    } else if biome.is("minecraft:spawns_gold_rabbits") {
        Variant::Gold
    } else if random_val < 50 {
        Variant::Brown
    } else if random_val < 90 {
        Variant::Salt
    } else {
        Variant::Black
    }
}

pub fn check_rabbit_spawn_rules(level: &mut (impl ServerLevelAccessor + ?Sized), pos: BlockPos) -> bool {
    level.block_state(pos.below()).is(&block_tags::RABBITS_SPAWNABLE_ON) && super::super::is_bright_enough_to_spawn(level, pos)
}

pub fn finalize_spawn(level: &mut (impl ServerLevelAccessor + ?Sized), mob: &mut Mob, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
    let mut variant = get_random_rabbit_variant(level, mob.entity.block_position());
    let group_data = match group_data {
        Some(SpawnGroupData::Rabbit(rabbit_group_data)) => {
            variant = rabbit_group_data.variant;
            group_data
        }
        _ => Some(SpawnGroupData::Rabbit(RabbitGroupData::new(variant))),
    };
    set_variant(mob, variant);
    group_data
}

impl Rabbit {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        data.push(DataValue::Int(Variant::DEFAULT.id()));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:rabbit"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&RABBIT_ATTRIBUTES), MobKind::Rabbit(Rabbit::default()));
        Animal::init(&mut mob);
        let idle_animation_timeout = mob.entity.random.next_int_bounded(40) + 180;
        rabbit_mut(&mut mob).idle_animation_timeout = idle_animation_timeout;
        mob.ai.move_control.kind = MoveControlKind::Rabbit;
        Rabbit::set_speed_modifier(&mut mob, 0.0);
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(1, Goal::Float(float_goal));
        goal_selector.add_goal(1, Goal::ClimbOnTopOfPowderSnow(ClimbOnTopOfPowderSnowGoal));
        goal_selector.add_goal(1, Goal::RabbitPanic(RabbitPanicGoal::new(FLEE_SPEED_MOD)));
        goal_selector.add_goal(2, Goal::Breed(BreedGoal::new(BREED_SPEED_MOD)));
        goal_selector.add_goal(3, Goal::Tempt(TemptGoal::new(FOLLOW_SPEED_MOD, is_food, false)));
        goal_selector.add_goal(4, Goal::RabbitAvoidEntity(RabbitAvoidEntityGoal::new(AvoidClass::EntityType("minecraft:player"), 8.0, FLEE_SPEED_MOD, FLEE_SPEED_MOD)));
        goal_selector.add_goal(4, Goal::RabbitAvoidEntity(RabbitAvoidEntityGoal::new(AvoidClass::EntityType("minecraft:wolf"), 10.0, FLEE_SPEED_MOD, FLEE_SPEED_MOD)));
        goal_selector.add_goal(4, Goal::RabbitAvoidEntity(RabbitAvoidEntityGoal::new(AvoidClass::Monster, 4.0, FLEE_SPEED_MOD, FLEE_SPEED_MOD)));
        goal_selector.add_goal(5, Goal::RaidGarden(RaidGardenGoal::new()));
        goal_selector.add_goal(6, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::new(STROLL_SPEED_MOD)));
        goal_selector.add_goal(11, Goal::LookAtPlayer(LookAtPlayerGoal::new(10.0)));
    }

    fn set_speed_modifier(mob: &mut Mob, speed: f64) {
        mob.ai.navigation.set_speed_modifier(speed);
        let control = &mob.ai.move_control;
        let (x, y, z) = (control.wanted_x, control.wanted_y, control.wanted_z);
        Rabbit::move_control_set_wanted_position(mob, x, y, z, speed);
    }

    pub fn move_control_set_wanted_position(mob: &mut Mob, x: f64, y: f64, z: f64, mut speed_modifier: f64) {
        if mob.entity.is_in_water() {
            speed_modifier = 1.5;
        }
        mob.ai.move_control.set_wanted_position(x, y, z, speed_modifier);
        if speed_modifier > 0.0 {
            rabbit_mut(mob).move_control.next_jump_speed = speed_modifier;
        }
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.rabbit.ambient"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.rabbit.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.rabbit.death"
    }

    fn get_jump_sound() -> &'static str {
        "minecraft:entity.rabbit.jump"
    }

    pub fn get_sound_source(mob: &Mob) -> SoundSource {
        if get_variant(&mob.entity) == Variant::Evil { SoundSource::Hostile } else { SoundSource::Neutral }
    }

    pub fn can_spawn_sprint_particle() -> bool {
        false
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        output.push(("RabbitType".into(), Nbt::Int(get_variant(&mob.entity).id())));
        output.push(("MoreCarrotTicks".into(), Nbt::Int(rabbit(mob).more_carrot_ticks)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        let variant = input.get("RabbitType").and_then(crate::world::level::block::entity::int_value).map_or(Variant::DEFAULT, Variant::by_id);
        set_variant(mob, variant);
        rabbit_mut(mob).more_carrot_ticks = input.get("MoreCarrotTicks").and_then(crate::world::level::block::entity::int_value).unwrap_or(DEFAULT_MORE_CARROT_TICKS);
    }
}

#[derive(Default)]
pub struct RabbitJumpControl {
    can_jump: bool,
}

impl RabbitJumpControl {
    pub fn can_jump(&self) -> bool {
        self.can_jump
    }

    pub fn set_can_jump(&mut self, can_jump: bool) {
        self.can_jump = can_jump;
    }
}

#[derive(Default)]
pub struct RabbitMoveControl {
    pub next_jump_speed: f64,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RabbitGroupData {
    pub ageable_mob_group_data: AgeableMobGroupData,
    pub variant: Variant,
}

impl RabbitGroupData {
    pub fn new(variant: Variant) -> RabbitGroupData {
        RabbitGroupData { ageable_mob_group_data: AgeableMobGroupData::with_chance(true, 1.0), variant }
    }
}

pub struct RabbitAvoidEntityGoal {
    pub avoid_entity_goal: AvoidEntityGoal,
}

impl RabbitAvoidEntityGoal {
    pub fn new(avoid_class: AvoidClass, max_dist: f32, walk_speed_modifier: f64, sprint_speed_modifier: f64) -> RabbitAvoidEntityGoal {
        RabbitAvoidEntityGoal { avoid_entity_goal: AvoidEntityGoal::of_class(avoid_class, max_dist, walk_speed_modifier, sprint_speed_modifier) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        get_variant(level.entity(mob)) != Variant::Evil && self.avoid_entity_goal.can_use(level, mob)
    }
}

pub struct RabbitPanicGoal {
    pub panic_goal: PanicGoal,
}

impl RabbitPanicGoal {
    pub fn new(speed_modifier: f64) -> RabbitPanicGoal {
        RabbitPanicGoal { panic_goal: PanicGoal::new(speed_modifier) }
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let speed_modifier = self.panic_goal.speed_modifier;
        level.rabbit_set_speed_modifier(mob, speed_modifier);
    }
}

pub struct RaidGardenGoal {
    pub move_to_block_goal: MoveToBlockGoal,
    wants_to_raid: bool,
    can_raid: bool,
}

impl RaidGardenGoal {
    pub fn new() -> RaidGardenGoal {
        RaidGardenGoal { move_to_block_goal: MoveToBlockGoal::new(0.7f32 as f64, 16, 1), wants_to_raid: false, can_raid: false }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if self.move_to_block_goal.next_start_tick <= 0 {
            if !level.game_rules.get_bool(game_rule::MOB_GRIEFING) {
                return false;
            }
            self.can_raid = false;
            self.wants_to_raid = level.rabbit_wants_more_food(mob);
        }
        let can_raid = Cell::new(self.can_raid);
        let wants_to_raid = self.wants_to_raid;
        let is_valid_target = |level: &ServerLevel, pos: BlockPos| is_valid_target(level, pos, wants_to_raid, &can_raid);
        let result = self.move_to_block_goal.can_use(level, mob, &is_valid_target);
        self.can_raid = can_raid.get();
        result
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel) -> bool {
        if !self.can_raid {
            return false;
        }
        let can_raid = Cell::new(self.can_raid);
        let wants_to_raid = self.wants_to_raid;
        let is_valid_target = |level: &ServerLevel, pos: BlockPos| is_valid_target(level, pos, wants_to_raid, &can_raid);
        let result = self.move_to_block_goal.can_continue_to_use(level, &is_valid_target);
        self.can_raid = can_raid.get();
        result
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.move_to_block_goal.tick(level, mob, 1.0);
        let block_pos = self.move_to_block_goal.block_pos;
        let max_head_x_rot = level.mob(mob).expect("a rabbit").get_max_head_x_rot() as f32;
        level.mob_mut(mob).expect("a rabbit").ai.look_control.set_look_at(block_pos.x as f64 + 0.5, (block_pos.y + 1) as f64, block_pos.z as f64 + 0.5, 10.0, max_head_x_rot);
        if self.move_to_block_goal.is_reached_target() {
            let crops_pos = block_pos.above();
            let block_state = level.block_state(crops_pos);
            if self.can_raid && block_state.is_block(blocks::CARROTS) {
                let carrot_age = block_state.get(AGE_7);
                if carrot_age == 0 {
                    level.set_block(crops_pos, blocks::AIR.default_state(), UPDATE_CLIENTS);
                    level.destroy_block(crops_pos, true, Some(mob), 512);
                } else {
                    level.set_block(crops_pos, block_state.set(AGE_7, carrot_age - 1), UPDATE_CLIENTS);
                    level.level_event(None, crate::server::level::events::PARTICLES_AND_SOUND_DESTROY_BLOCK, crops_pos, block_state.id() as i32);
                }
                rabbit_mut(level.mob_mut(mob).expect("a rabbit")).more_carrot_ticks = MORE_CARROTS_DELAY;
            }
            self.can_raid = false;
            self.move_to_block_goal.next_start_tick = 10;
        }
    }
}

fn is_valid_target(level: &ServerLevel, pos: BlockPos, wants_to_raid: bool, can_raid: &Cell<bool>) -> bool {
    let region = crate::world::level::path_navigation_region::PathNavigationRegion::new(level);
    let state = region.get_block_state(pos);
    if state.is(&block_tags::SUPPORTS_CROPS) && wants_to_raid && !can_raid.get() {
        let state = region.get_block_state(pos.above());
        if state.is_block(blocks::CARROTS) && state.get(AGE_7) >= 7 {
            can_raid.set(true);
            return true;
        }
    }
    false
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Variant {
    Brown,
    White,
    Black,
    WhiteSplotched,
    Gold,
    Salt,
    Evil,
}

impl Variant {
    pub const DEFAULT: Variant = Variant::Brown;
    pub const VALUES: [Variant; 7] = [Variant::Brown, Variant::White, Variant::Black, Variant::WhiteSplotched, Variant::Gold, Variant::Salt, Variant::Evil];

    pub fn id(self) -> i32 {
        match self {
            Variant::Brown => 0,
            Variant::White => 1,
            Variant::Black => 2,
            Variant::WhiteSplotched => 3,
            Variant::Gold => 4,
            Variant::Salt => 5,
            Variant::Evil => 99,
        }
    }

    pub fn get_serialized_name(self) -> &'static str {
        match self {
            Variant::Brown => "brown",
            Variant::White => "white",
            Variant::Black => "black",
            Variant::WhiteSplotched => "white_splotched",
            Variant::Gold => "gold",
            Variant::Salt => "salt",
            Variant::Evil => "evil",
        }
    }

    pub fn by_id(id: i32) -> Variant {
        Variant::VALUES.into_iter().find(|variant| variant.id() == id).unwrap_or(Variant::DEFAULT)
    }

    pub fn by_name(name: &str) -> Option<Variant> {
        Variant::VALUES.into_iter().find(|variant| variant.get_serialized_name() == name)
    }
}

impl ServerLevel {
    pub fn is_rabbit(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::Rabbit(_)))
    }

    pub fn rabbit_get_jump_power(&mut self, id: EntityId) -> f32 {
        let mob = self.mob(id).expect("a rabbit");
        let e = &mob.entity;
        let mut base_jump_power = 0.3f32;
        if mob.ai.move_control.speed_modifier <= 0.6 {
            base_jump_power = 0.2;
        }
        if let Some(path) = mob.ai.navigation.get_path()
            && !path.is_done()
        {
            let current_pos = path.get_next_entity_pos(e);
            if current_pos.y > e.y() + 0.5 {
                base_jump_power = 0.5;
            }
        }
        if e.horizontal_collision || mob.living.jumping && mob.ai.move_control.wanted_y > e.y() + 0.5 {
            base_jump_power = 0.5;
        }
        self.get_jump_power_with_multiplier(id, base_jump_power / 0.42)
    }

    pub fn rabbit_jump_from_ground(&mut self, id: EntityId) {
        self.living_entity_jump_from_ground(id);
        let mob = self.mob_mut(id).expect("a rabbit");
        let speed_modifier = mob.ai.move_control.speed_modifier;
        if speed_modifier > 0.0 {
            let current = mob.entity.delta_movement.horizontal_distance_sqr();
            if current < 0.01 {
                let baby = mob.is_baby();
                mob.entity.move_relative(0.1, Vec3::new(0.0, if baby { 0.5 } else { 1.5 }, 1.0));
            }
        }
        self.broadcast_entity_event(id, entity_event::JUMP);
    }

    pub fn rabbit_set_speed_modifier(&mut self, id: EntityId, speed: f64) {
        Rabbit::set_speed_modifier(self.mob_mut(id).expect("a rabbit"), speed);
    }

    pub fn rabbit_set_jumping(&mut self, id: EntityId, jump: bool) {
        self.living_mut(id).jumping = jump;
        if jump {
            let volume = self.mob(id).expect("a rabbit").get_sound_volume();
            let random = &mut self.entity_mut(id).random;
            let pitch = ((random.next_float() - random.next_float()) * 0.2 + 1.0) * 0.8;
            self.entity_play_sound(id, sound_event(Rabbit::get_jump_sound()), volume, pitch);
        }
    }

    pub fn rabbit_start_jumping(&mut self, id: EntityId) {
        self.rabbit_set_jumping(id, true);
        let rabbit = rabbit_mut(self.mob_mut(id).expect("a rabbit"));
        rabbit.jump_duration = JUMP_DURATION_IN_TICKS;
        rabbit.jump_ticks = 0;
    }

    pub fn rabbit_custom_server_ai_step(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a rabbit");
        let state = rabbit_mut(mob);
        if state.jump_delay_ticks > 0 {
            state.jump_delay_ticks -= 1;
        }
        if state.more_carrot_ticks > 0 {
            let decrease = mob.entity.random.next_int_bounded(3);
            let state = rabbit_mut(mob);
            state.more_carrot_ticks -= decrease;
            if state.more_carrot_ticks < 0 {
                state.more_carrot_ticks = 0;
            }
        }
        if self.entity(id).on_ground {
            if !rabbit(self.mob(id).expect("a rabbit")).was_on_ground {
                self.rabbit_set_jumping(id, false);
                self.rabbit_check_landing_delay(id);
            }
            let mob = self.mob(id).expect("a rabbit");
            if get_variant(&mob.entity) == Variant::Evil && rabbit(mob).jump_delay_ticks == 0 {
                if let Some(target) = self.get_target(id)
                    && self.entity(id).distance_to_sqr(self.entity(target)) < 16.0
                {
                    let t = self.entity(target);
                    let (tx, ty, tz) = (t.x(), t.y(), t.z());
                    self.rabbit_face_point(id, tx, tz);
                    let speed_modifier = self.mob(id).expect("a rabbit").ai.move_control.speed_modifier;
                    self.set_wanted_position(id, tx, ty, tz, speed_modifier);
                    self.rabbit_start_jumping(id);
                    rabbit_mut(self.mob_mut(id).expect("a rabbit")).was_on_ground = true;
                }
            }
            let mob = self.mob(id).expect("a rabbit");
            if !mob.ai.jump_control.jump {
                if mob.ai.move_control.has_wanted() && rabbit(mob).jump_delay_ticks == 0 {
                    let control = &mob.ai.move_control;
                    let mut pos = Vec3::new(control.wanted_x, control.wanted_y, control.wanted_z);
                    if let Some(path) = mob.ai.navigation.get_path()
                        && !path.is_done()
                    {
                        pos = path.get_next_entity_pos(&mob.entity);
                    }
                    self.rabbit_face_point(id, pos.x, pos.z);
                    self.rabbit_start_jumping(id);
                }
            } else if !rabbit(mob).jump_control.can_jump() {
                self.rabbit_enable_jump_control(id);
            }
        }
        let on_ground = self.entity(id).on_ground;
        rabbit_mut(self.mob_mut(id).expect("a rabbit")).was_on_ground = on_ground;
    }

    fn rabbit_face_point(&mut self, id: EntityId, face_x: f64, face_z: f64) {
        let e = self.entity_mut(id);
        let y_rot = (mth::atan2(face_z - e.z(), face_x - e.x()) * mth::RAD_TO_DEG as f64) as f32 - 90.0;
        e.set_y_rot(y_rot);
    }

    fn rabbit_enable_jump_control(&mut self, id: EntityId) {
        rabbit_mut(self.mob_mut(id).expect("a rabbit")).jump_control.set_can_jump(true);
    }

    fn rabbit_disable_jump_control(&mut self, id: EntityId) {
        rabbit_mut(self.mob_mut(id).expect("a rabbit")).jump_control.set_can_jump(false);
    }

    fn rabbit_set_landing_delay(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a rabbit");
        let speed_modifier = mob.ai.move_control.speed_modifier;
        rabbit_mut(mob).jump_delay_ticks = if speed_modifier < FLEE_SPEED_MOD { JUMP_DELAY_TICKS } else { PANIC_JUMP_DELAY_TICKS };
    }

    fn rabbit_check_landing_delay(&mut self, id: EntityId) {
        self.rabbit_set_landing_delay(id);
        self.rabbit_disable_jump_control(id);
    }

    pub fn rabbit_ai_step(&mut self, id: EntityId) {
        self.animal_ai_step(id);
        let Some(mob) = self.mob_mut(id) else { return };
        let state = rabbit_mut(mob);
        if state.jump_ticks != state.jump_duration {
            state.jump_ticks += 1;
        } else if state.jump_duration != 0 {
            state.jump_ticks = 0;
            state.jump_duration = 0;
            self.rabbit_set_jumping(id, false);
        }
    }

    pub fn rabbit_jump_control_tick(&mut self, id: EntityId) {
        if self.mob(id).expect("a rabbit").ai.jump_control.jump {
            self.rabbit_start_jumping(id);
            self.mob_mut(id).expect("a rabbit").ai.jump_control.jump = false;
        }
    }

    pub fn rabbit_move_control_tick(&mut self, id: EntityId) {
        let mob = self.mob(id).expect("a rabbit");
        if mob.entity.on_ground && !mob.living.jumping && !mob.ai.jump_control.jump {
            self.rabbit_set_speed_modifier(id, 0.0);
        } else if mob.ai.move_control.has_wanted() || mob.ai.move_control.operation == Operation::Jumping {
            let next_jump_speed = rabbit(mob).move_control.next_jump_speed;
            self.rabbit_set_speed_modifier(id, next_jump_speed);
        }
        self.move_control_tick(id);
    }

    pub fn rabbit_play_attack_sound(&mut self, id: EntityId) {
        if get_variant(self.entity(id)) == Variant::Evil {
            let random = &mut self.entity_mut(id).random;
            let pitch = (random.next_float() - random.next_float()) * 0.2 + 1.0;
            self.entity_play_sound(id, sound_event("minecraft:entity.rabbit.attack"), 1.0, pitch);
        }
    }

    pub fn rabbit_get_breed_offspring(&mut self, id: EntityId, partner: EntityId) -> Option<Box<Mob>> {
        let mut offspring = Rabbit::create();
        let pos = self.entity(id).block_position();
        let mut variant = get_random_rabbit_variant(self, pos);
        if self.entity_mut(id).random.next_int_bounded(20) != 0 {
            variant = if self.is_rabbit(partner) && self.entity_mut(id).random.next_boolean() { get_variant(self.entity(partner)) } else { get_variant(self.entity(id)) };
        }
        set_variant(&mut offspring, variant);
        Some(Box::new(offspring))
    }

    fn rabbit_wants_more_food(&self, id: EntityId) -> bool {
        rabbit(self.mob(id).expect("a rabbit")).more_carrot_ticks <= 0
    }
}
