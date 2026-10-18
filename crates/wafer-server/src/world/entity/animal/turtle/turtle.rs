use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::properties::EGGS;
use wafer_data::{block_tags, blocks, damage_types, fluid_tags, game_rule, item_tags, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::{ServerLevel, SoundSource};
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::control::move_control::{MoveControlKind, Operation, rotlerp};
use crate::world::entity::ai::goal::breed_goal::BreedGoal;
use crate::world::entity::ai::goal::goal::{Goal, reduced_tick_delay};
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::move_to_block_goal::MoveToBlockGoal;
use crate::world::entity::ai::goal::panic_goal::PanicGoal;
use crate::world::entity::ai::goal::random_stroll_goal::RandomStrollGoal;
use crate::world::entity::ai::goal::tempt_goal::TemptGoal;
use crate::world::entity::ai::navigation::path_navigation::PathNavigation;
use crate::world::entity::ai::util::default_random_pos;
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::{Attribute, TURTLE_ATTRIBUTES};
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, MoverType, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::block::turtle_egg_block;
use crate::world::level::{BlockGetter, LevelReader};
use crate::world::level::path_navigation_region::PathNavigationRegion;
use crate::world::level::pathfinder::path_type::PathType;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::update_flags::UPDATE_ALL;

pub const HAS_EGG: usize = 18;
pub const LAYING_EGG: usize = 19;
const BABY_SCALE: f32 = 0.3;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(1.2, 0.4).with_passenger_attachments(&[Vec3::new(0.0, 0.55625, -0.25)]);
const DEFAULT_HAS_EGG: bool = false;
const TURTLE_GROW: &str = "minecraft:gameplay/turtle_grow";

const BABY_PASSENGER_ATTACHMENTS: &[Vec3] = &[Vec3::new(0.0, 0.4f32 as f64, -0.25f32 as f64)];

pub fn baby_dimensions() -> EntityDimensions {
    EntityDimensions::scalable(1.2, 0.4).with_passenger_attachments(BABY_PASSENGER_ATTACHMENTS).scale(BABY_SCALE)
}

#[derive(Default)]
pub struct Turtle {
    pub animal: Animal,
    pub lay_egg_counter: i32,
    pub home_pos: BlockPos,
    pub travel_pos: Option<BlockPos>,
    pub going_home: bool,
}

pub fn turtle(mob: &Mob) -> &Turtle {
    match &mob.kind {
        MobKind::Turtle(turtle) => turtle,
        _ => unreachable!("not a turtle"),
    }
}

pub fn turtle_mut(mob: &mut Mob) -> &mut Turtle {
    match &mut mob.kind {
        MobKind::Turtle(turtle) => turtle,
        _ => unreachable!("not a turtle"),
    }
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::TURTLE_FOOD)
}

pub fn has_egg(entity: &Entity) -> bool {
    matches!(entity.data.get(HAS_EGG), DataValue::Boolean(true))
}

fn set_has_egg(entity: &mut Entity, on_off: bool) {
    entity.data.set(HAS_EGG, DataValue::Boolean(on_off));
}

pub fn is_laying_egg(entity: &Entity) -> bool {
    matches!(entity.data.get(LAYING_EGG), DataValue::Boolean(true))
}

fn set_laying_egg(mob: &mut Mob, on: bool) {
    turtle_mut(mob).lay_egg_counter = if on { 1 } else { 0 };
    mob.entity.data.set(LAYING_EGG, DataValue::Boolean(on));
}

pub fn set_home_pos(mob: &mut Mob, pos: BlockPos) {
    turtle_mut(mob).home_pos = pos;
}

pub fn get_home_pos(mob: &Mob) -> BlockPos {
    turtle(mob).home_pos
}

pub fn finalize_spawn(mob: &mut Mob) {
    let pos = mob.entity.block_position();
    set_home_pos(mob, pos);
}

pub fn check_turtle_spawn_rules(level: &mut (impl ServerLevelAccessor + ?Sized), pos: BlockPos) -> bool {
    pos.y < level.get_sea_level() + 4 && turtle_egg_block::on_sand(level, pos) && super::super::is_bright_enough_to_spawn(level, pos)
}

pub fn get_walk_target_value(mob: &Mob, pos: BlockPos, level: &mut (impl ServerLevelAccessor + ?Sized)) -> f32 {
    if !turtle(mob).going_home && level.fluid_state(pos).is(&fluid_tags::WATER) {
        10.0
    } else if turtle_egg_block::on_sand(level, pos) {
        10.0
    } else {
        level.get_pathfinding_cost_from_light_levels(pos)
    }
}

impl Turtle {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        data.extend([DataValue::Boolean(DEFAULT_HAS_EGG), DataValue::Boolean(false)]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:turtle"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&TURTLE_ATTRIBUTES), MobKind::Turtle(Turtle::default()));
        Animal::init(&mut mob);
        mob.set_pathfinding_malus(PathType::Water, 0.0);
        mob.set_pathfinding_malus(PathType::DoorIronClosed, -1.0);
        mob.set_pathfinding_malus(PathType::DoorWoodClosed, -1.0);
        mob.set_pathfinding_malus(PathType::DoorOpen, -1.0);
        mob.ai.move_control.kind = MoveControlKind::Turtle;
        mob
    }

    pub fn create_navigation(follow_range_base: f64) -> PathNavigation {
        PathNavigation::of_kind(crate::world::entity::ai::navigation::path_navigation::PathNavigationKind::Turtle, follow_range_base)
    }

    pub fn register_goals(ai: &mut MobAi, is_baby: bool) {
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::TurtlePanic(TurtlePanicGoal::new(1.2)));
        goal_selector.add_goal(1, Goal::TurtleBreed(TurtleBreedGoal::new(1.0)));
        goal_selector.add_goal(1, Goal::TurtleLayEgg(TurtleLayEggGoal::new(1.0)));
        goal_selector.add_goal(2, Goal::Tempt(TemptGoal::new(1.1, is_food, false)));
        goal_selector.add_goal(3, Goal::TurtleGoToWater(TurtleGoToWaterGoal::new(1.0, is_baby)));
        goal_selector.add_goal(4, Goal::TurtleGoHome(TurtleGoHomeGoal::new(1.0)));
        goal_selector.add_goal(7, Goal::TurtleTravel(TurtleTravelGoal::new(1.0)));
        goal_selector.add_goal(8, Goal::LookAtPlayer(LookAtPlayerGoal::new(8.0)));
        goal_selector.add_goal(9, Goal::TurtleRandomStroll(TurtleRandomStrollGoal::new(1.0, 100)));
    }

    pub fn is_pushed_by_fluid() -> bool {
        false
    }

    pub fn get_ambient_sound_interval() -> i32 {
        200
    }

    pub fn get_ambient_sound(mob: &Mob) -> Option<&'static str> {
        (!mob.entity.is_in_water() && mob.entity.on_ground && !mob.is_baby()).then_some("minecraft:entity.turtle.ambient_land")
    }

    pub fn get_swim_sound() -> &'static str {
        "minecraft:entity.turtle.swim"
    }

    pub fn get_hurt_sound(mob: &Mob) -> &'static str {
        if mob.is_baby() { "minecraft:entity.turtle.hurt_baby" } else { "minecraft:entity.turtle.hurt" }
    }

    pub fn get_death_sound(mob: &Mob) -> &'static str {
        if mob.is_baby() { "minecraft:entity.turtle.death_baby" } else { "minecraft:entity.turtle.death" }
    }

    pub fn get_step_sound(mob: &Mob) -> &'static str {
        if mob.is_baby() { "minecraft:entity.turtle.shamble_baby" } else { "minecraft:entity.turtle.shamble" }
    }

    pub fn next_step(entity: &Entity) -> f32 {
        entity.move_dist + 0.15
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        let home_pos = turtle(mob).home_pos;
        let home_pos = Nbt::IntArray(vec![home_pos.x, home_pos.y, home_pos.z]);
        match output.iter_mut().find(|(key, _)| key == "home_pos") {
            Some((_, value)) => *value = home_pos,
            None => output.push(("home_pos".into(), home_pos)),
        }
        output.push(("has_egg".into(), Nbt::Byte(has_egg(&mob.entity) as i8)));
    }

    pub fn read_home_pos(mob: &mut Mob, input: &Nbt) {
        let home_pos = match input.get("home_pos") {
            Some(Nbt::IntArray(v)) if v.len() == 3 => BlockPos::new(v[0], v[1], v[2]),
            _ => mob.entity.block_position(),
        };
        set_home_pos(mob, home_pos);
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        let has_egg = input.get("has_egg").and_then(Nbt::as_i64).map_or(DEFAULT_HAS_EGG, |v| v != 0);
        set_has_egg(&mut mob.entity, has_egg);
    }
}

pub struct TurtlePanicGoal {
    pub panic_goal: PanicGoal,
}

impl TurtlePanicGoal {
    pub fn new(speed_modifier: f64) -> TurtlePanicGoal {
        TurtlePanicGoal { panic_goal: PanicGoal::new(speed_modifier) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if !self.panic_goal.should_panic(level, mob) {
            return false;
        }
        match self.panic_goal.look_for_water(level, mob, 7) {
            Some(block_pos) => {
                self.panic_goal.pos_x = block_pos.x as f64;
                self.panic_goal.pos_y = block_pos.y as f64;
                self.panic_goal.pos_z = block_pos.z as f64;
                true
            }
            None => self.panic_goal.find_random_position(level, mob),
        }
    }
}

pub struct TurtleBreedGoal {
    pub breed_goal: BreedGoal,
}

impl TurtleBreedGoal {
    pub fn new(speed_modifier: f64) -> TurtleBreedGoal {
        TurtleBreedGoal { breed_goal: BreedGoal::new(speed_modifier) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.breed_goal.can_use(level, mob) && !has_egg(level.entity(mob))
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.breed_goal.tick_with(level, mob, |_, level, animal, partner| level.turtle_breed(animal, partner));
    }
}

pub struct TurtleGoHomeGoal {
    speed_modifier: f64,
    stuck: bool,
    close_to_home_try_ticks: i32,
}

const GO_HOME_GIVE_UP_TICKS: i32 = 600;

impl TurtleGoHomeGoal {
    pub fn new(speed_modifier: f64) -> TurtleGoHomeGoal {
        TurtleGoHomeGoal { speed_modifier, stuck: false, close_to_home_try_ticks: 0 }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let m = level.mob(mob).expect("a turtle");
        if m.is_baby() {
            false
        } else if has_egg(&m.entity) {
            true
        } else if level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(700)) != 0 {
            false
        } else {
            let m = level.mob(mob).expect("a turtle");
            !turtle(m).home_pos.closer_to_center_than(m.entity.position(), 64.0)
        }
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        turtle_mut(level.mob_mut(mob).expect("a turtle")).going_home = true;
        self.stuck = false;
        self.close_to_home_try_ticks = 0;
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        turtle_mut(level.mob_mut(mob).expect("a turtle")).going_home = false;
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let m = level.mob(mob).expect("a turtle");
        !turtle(m).home_pos.closer_to_center_than(m.entity.position(), 7.0) && !self.stuck && self.close_to_home_try_ticks <= reduced_tick_delay(GO_HOME_GIVE_UP_TICKS)
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let m = level.mob(mob).expect("a turtle");
        let home_pos = turtle(m).home_pos;
        let close_to_home = home_pos.closer_to_center_than(m.entity.position(), 16.0);
        if close_to_home {
            self.close_to_home_try_ticks += 1;
        }
        if level.mob(mob).expect("a turtle").ai.navigation.is_done() {
            let home_pos_vec = Vec3::at_bottom_center_of(home_pos);
            let mut next_pos = default_random_pos::get_pos_towards(level, mob, 16, 3, home_pos_vec, (std::f64::consts::PI / 10.0) as f32 as f64);
            if next_pos.is_none() {
                next_pos = default_random_pos::get_pos_towards(level, mob, 8, 7, home_pos_vec, std::f32::consts::FRAC_PI_2 as f64);
            }
            if let Some(pos) = next_pos
                && !close_to_home
                && !level.block_state(BlockPos::containing_vec(pos)).is_block(blocks::WATER)
            {
                next_pos = default_random_pos::get_pos_towards(level, mob, 16, 5, home_pos_vec, std::f32::consts::FRAC_PI_2 as f64);
            }
            let Some(next_pos) = next_pos else {
                self.stuck = true;
                return;
            };
            let speed_modifier = self.speed_modifier;
            level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, next_pos.x, next_pos.y, next_pos.z, speed_modifier));
        }
    }
}

pub struct TurtleGoToWaterGoal {
    pub move_to_block_goal: MoveToBlockGoal,
}

const GO_TO_WATER_GIVE_UP_TICKS: i32 = 1200;

impl TurtleGoToWaterGoal {
    pub fn new(speed_modifier: f64, is_baby: bool) -> TurtleGoToWaterGoal {
        let mut move_to_block_goal = MoveToBlockGoal::new(if is_baby { 2.0 } else { speed_modifier }, 24, 1);
        move_to_block_goal.vertical_search_start = -1;
        TurtleGoToWaterGoal { move_to_block_goal }
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        !level.entity(mob).is_in_water() && self.move_to_block_goal.try_ticks <= GO_TO_WATER_GIVE_UP_TICKS && go_to_water_is_valid_target(level, self.move_to_block_goal.block_pos)
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let m = level.mob(mob).expect("a turtle");
        if m.is_baby() && !m.entity.is_in_water() {
            self.move_to_block_goal.can_use(level, mob, &go_to_water_is_valid_target)
        } else if !turtle(m).going_home && !m.entity.is_in_water() && !has_egg(&m.entity) {
            self.move_to_block_goal.can_use(level, mob, &go_to_water_is_valid_target)
        } else {
            false
        }
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.move_to_block_goal.tick_with(level, mob, 1.0, |goal| goal.try_ticks % 160 == 0);
    }
}

fn go_to_water_is_valid_target(level: &ServerLevel, pos: BlockPos) -> bool {
    PathNavigationRegion::new(level).get_block_state(pos).is_block(blocks::WATER)
}

pub struct TurtleLayEggGoal {
    pub move_to_block_goal: MoveToBlockGoal,
}

impl TurtleLayEggGoal {
    pub fn new(speed_modifier: f64) -> TurtleLayEggGoal {
        TurtleLayEggGoal { move_to_block_goal: MoveToBlockGoal::new(speed_modifier, 16, 1) }
    }

    fn home_is_close(level: &ServerLevel, mob: EntityId) -> bool {
        let m = level.mob(mob).expect("a turtle");
        turtle(m).home_pos.closer_to_center_than(m.entity.position(), 9.0)
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if has_egg(level.entity(mob)) && TurtleLayEggGoal::home_is_close(level, mob) { self.move_to_block_goal.can_use(level, mob, &lay_egg_is_valid_target) } else { false }
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.move_to_block_goal.can_continue_to_use(level, &lay_egg_is_valid_target) && has_egg(level.entity(mob)) && TurtleLayEggGoal::home_is_close(level, mob)
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId, lay_egg_delay: i32) {
        self.move_to_block_goal.tick(level, mob, 1.0);
        let turtle_pos = level.entity(mob).block_position();
        if !level.entity(mob).is_in_water() && self.move_to_block_goal.is_reached_target() {
            let lay_egg_counter = turtle(level.mob(mob).expect("a turtle")).lay_egg_counter;
            if lay_egg_counter < 1 {
                set_laying_egg(level.mob_mut(mob).expect("a turtle"), true);
            } else if lay_egg_counter > lay_egg_delay {
                let pitch = 0.9 + level.random.next_float() * 0.2;
                level.play_sound(None, turtle_pos, sound_event("minecraft:entity.turtle.lay_egg"), SoundSource::Blocks, 0.3, pitch);
                let egg_pos = self.move_to_block_goal.block_pos.above();
                let eggs = level.entity_mut(mob).random.next_int_bounded(4) + 1;
                let egg_state = blocks::TURTLE_EGG.default_state().set(EGGS, eggs);
                level.set_block(egg_pos, egg_state, UPDATE_ALL);
                let m = level.mob_mut(mob).expect("a turtle");
                set_has_egg(&mut m.entity, false);
                set_laying_egg(m, false);
                m.animal_mut().expect("a turtle").in_love = 600;
            }
            let m = level.mob_mut(mob).expect("a turtle");
            if is_laying_egg(&m.entity) {
                turtle_mut(m).lay_egg_counter += 1;
            }
        }
    }
}

fn lay_egg_is_valid_target(level: &ServerLevel, pos: BlockPos) -> bool {
    let region = PathNavigationRegion::new(level);
    if !region.get_block_state(pos.above()).is_air() { false } else { region.get_block_state(pos).is(&block_tags::SAND) }
}

pub struct TurtleRandomStrollGoal {
    pub random_stroll_goal: RandomStrollGoal,
}

impl TurtleRandomStrollGoal {
    pub fn new(speed_modifier: f64, interval: i32) -> TurtleRandomStrollGoal {
        TurtleRandomStrollGoal { random_stroll_goal: RandomStrollGoal::with_interval(speed_modifier, interval) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let m = level.mob(mob).expect("a turtle");
        if !m.entity.is_in_water() && !turtle(m).going_home && !has_egg(&m.entity) { self.random_stroll_goal.can_use(level, mob, RandomStrollGoal::get_position) } else { false }
    }
}

pub struct TurtleTravelGoal {
    speed_modifier: f64,
    stuck: bool,
}

impl TurtleTravelGoal {
    pub fn new(speed_modifier: f64) -> TurtleTravelGoal {
        TurtleTravelGoal { speed_modifier, stuck: false }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let m = level.mob(mob).expect("a turtle");
        !turtle(m).going_home && !has_egg(&m.entity) && m.entity.is_in_water()
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let sea_level = level.get_sea_level();
        let m = level.mob_mut(mob).expect("a turtle");
        let random = &mut m.entity.random;
        let xt = random.next_int_bounded(1025) - 512;
        let mut yt = random.next_int_bounded(9) - 4;
        let zt = random.next_int_bounded(1025) - 512;
        if yt as f64 + m.entity.y() > (sea_level - 1) as f64 {
            yt = 0;
        }
        let travel_pos = BlockPos::containing(xt as f64 + m.entity.x(), yt as f64 + m.entity.y(), zt as f64 + m.entity.z());
        turtle_mut(m).travel_pos = Some(travel_pos);
        self.stuck = false;
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let Some(travel_pos) = turtle(level.mob(mob).expect("a turtle")).travel_pos else {
            self.stuck = true;
            return;
        };
        if level.mob(mob).expect("a turtle").ai.navigation.is_done() {
            let target_pos = Vec3::at_bottom_center_of(travel_pos);
            let mut next_pos = default_random_pos::get_pos_towards(level, mob, 16, 3, target_pos, (std::f64::consts::PI / 10.0) as f32 as f64);
            if next_pos.is_none() {
                next_pos = default_random_pos::get_pos_towards(level, mob, 8, 7, target_pos, std::f32::consts::FRAC_PI_2 as f64);
            }
            if let Some(pos) = next_pos {
                let xc = mth::floor(pos.x);
                let zc = mth::floor(pos.z);
                if !level.has_chunks_at(xc - 34, zc - 34, xc + 34, zc + 34) {
                    next_pos = None;
                }
            }
            let Some(next_pos) = next_pos else {
                self.stuck = true;
                return;
            };
            let speed_modifier = self.speed_modifier;
            level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, next_pos.x, next_pos.y, next_pos.z, speed_modifier));
        }
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let m = level.mob(mob).expect("a turtle");
        !m.ai.navigation.is_done() && !self.stuck && !turtle(m).going_home && !level.is_in_love(mob) && !has_egg(&m.entity)
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        turtle_mut(level.mob_mut(mob).expect("a turtle")).travel_pos = None;
    }
}

impl ServerLevel {
    pub fn is_turtle(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::Turtle(_)))
    }

    pub fn turtle_can_fall_in_love(&self, id: EntityId) -> bool {
        self.animal_can_fall_in_love(id) && !has_egg(self.entity(id))
    }

    pub fn turtle_get_walk_target_value(&self, id: EntityId, pos: BlockPos) -> f32 {
        let region = PathNavigationRegion::new(self);
        if !turtle(self.mob(id).expect("a turtle")).going_home && region.get_block_state(pos).fluid_state().is(&fluid_tags::WATER) {
            10.0
        } else if region.get_block_state(pos.below()).is(&block_tags::SAND) {
            10.0
        } else {
            self.get_pathfinding_cost_from_light_levels(pos)
        }
    }

    pub fn turtle_ai_step(&mut self, id: EntityId) {
        self.animal_ai_step(id);
        let Some(mob) = self.mob(id) else { return };
        let lay_egg_counter = turtle(mob).lay_egg_counter;
        if self.is_alive(id) && is_laying_egg(&mob.entity) && lay_egg_counter >= 1 && lay_egg_counter % 5 == 0 {
            let pos = mob.entity.block_position();
            if turtle_egg_block::on_sand(self, pos) {
                let below = self.block_state(pos.below());
                self.level_event(None, crate::server::level::events::PARTICLES_AND_SOUND_DESTROY_BLOCK, pos, below.id() as i32);
            }
        }
    }

    pub fn turtle_age_boundary_reached(&mut self, id: EntityId) {
        if !self.mob(id).expect("a turtle").is_baby() && self.game_rules.get_bool(game_rule::MOB_DROPS) {
            self.drop_from_gift_loot_table(id, TURTLE_GROW, |level, item_stack| {
                level.spawn_at_location(id, item_stack, 0.0);
            });
        }
    }

    pub fn turtle_travel_in_water(&mut self, id: EntityId, input: Vec3) {
        self.entity_mut(id).move_relative(0.1, input);
        let delta = self.entity(id).delta_movement;
        self.move_entity(id, MoverType::SelfMove, delta);
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.scale(0.9));
        let target = self.get_target(id);
        let mob = self.mob(id).expect("a turtle");
        let state = turtle(mob);
        if target.is_none() && (!state.going_home || !state.home_pos.closer_to_center_than(mob.entity.position(), 20.0)) {
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.add(0.0, -0.005, 0.0));
        }
    }

    pub fn turtle_thunder_hit(&mut self, id: EntityId, _lightning_bolt: EntityId) {
        self.hurt_server(id, DamageSource::new(damage_types::LIGHTNING_BOLT), f32::MAX);
    }

    pub fn turtle_get_breed_offspring(&mut self, _id: EntityId, _partner: EntityId) -> Option<Box<Mob>> {
        Some(Box::new(Turtle::create()))
    }

    fn turtle_breed(&mut self, mob: EntityId, partner: EntityId) {
        if let Some(cause) = self.get_love_cause(mob).or_else(|| self.get_love_cause(partner)) {
            self.award_stat(cause, crate::stats::stats::ANIMALS_BRED, 1);
            crate::advancements::triggers::criteria_triggers::BRED_ANIMALS.trigger(self, cause, mob, partner, None);
        }
        set_has_egg(self.entity_mut(mob), true);
        self.set_age(mob, 6000);
        self.set_age(partner, 6000);
        self.reset_love(mob);
        self.reset_love(partner);
        if self.game_rules.get_bool(game_rule::MOB_DROPS) {
            let e = self.entity_mut(mob);
            let value = e.random.next_int_bounded(7) + 1;
            let pos = e.position();
            let orb = self.new_experience_orb(pos, Vec3::ZERO, value);
            self.add_fresh_entity(LevelEntity::ExperienceOrb(orb));
        }
    }

    pub fn turtle_move_control_tick(&mut self, id: EntityId) {
        self.turtle_move_control_update_speed(id);
        let speed_attribute = self.attribute_value(id, Attribute::MovementSpeed);
        let mob = self.mob_mut(id).expect("a turtle");
        if mob.ai.move_control.operation == Operation::MoveTo && !mob.ai.navigation.is_done() {
            let control = &mob.ai.move_control;
            let e = &mob.entity;
            let xd = control.wanted_x - e.x();
            let mut yd = control.wanted_y - e.y();
            let zd = control.wanted_z - e.z();
            let dd = (xd * xd + yd * yd + zd * zd).sqrt();
            if dd < 1.0e-5f32 as f64 {
                mob.set_speed(0.0);
            } else {
                yd /= dd;
                let y_rot_d = (mth::atan2(zd, xd) * mth::RAD_TO_DEG as f64) as f32 - 90.0;
                let y_rot = rotlerp(mob.entity.y_rot, y_rot_d, 90.0);
                mob.entity.set_y_rot(y_rot);
                mob.living.y_body_rot = mob.entity.y_rot;
                let target_speed = (mob.ai.move_control.speed_modifier * speed_attribute) as f32;
                let speed = mth::lerp_f(0.125, mob.living.speed, target_speed);
                mob.set_speed(speed);
                let speed = mob.living.speed;
                let e = &mut mob.entity;
                e.set_delta_movement(e.delta_movement.add(0.0, speed as f64 * yd * 0.1, 0.0));
            }
        } else {
            mob.set_speed(0.0);
        }
    }

    fn turtle_move_control_update_speed(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a turtle");
        if mob.entity.is_in_water() {
            let e = &mut mob.entity;
            e.set_delta_movement(e.delta_movement.add(0.0, 0.005, 0.0));
            if !get_home_pos(mob).closer_to_center_than(mob.entity.position(), 16.0) {
                let speed = (mob.living.speed / 2.0).max(0.08);
                mob.set_speed(speed);
            }
            if mob.is_baby() {
                let speed = (mob.living.speed / 3.0).max(0.06);
                mob.set_speed(speed);
            }
        } else if mob.entity.on_ground {
            let speed = (mob.living.speed / 2.0).max(0.06);
            mob.set_speed(speed);
        }
    }
}

impl PathNavigation {
    pub fn turtle_path_navigation_is_stable_destination(&self, level: &ServerLevel, mob: EntityId, pos: BlockPos) -> bool {
        let region = PathNavigationRegion::new(level);
        if level.mob(mob).is_some_and(|m| matches!(&m.kind, MobKind::Turtle(turtle) if turtle.travel_pos.is_some())) {
            region.get_block_state(pos).is_block(blocks::WATER)
        } else {
            !region.get_block_state(pos.below()).is_air()
        }
    }
}
