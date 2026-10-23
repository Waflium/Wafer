use wafer_core::Vec3;
use wafer_core::random::RandomSource;
use wafer_data::{block_tags, blocks, item_tags, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::ageable_mob::{self, AgeableMobGroupData};
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::avoid_entity_goal::AvoidEntityGoal;
use crate::world::entity::ai::goal::breed_goal::BreedGoal;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::leap_at_target_goal::LeapAtTargetGoal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::ocelot_attack_goal::OcelotAttackGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::goal::tempt_goal::TemptGoal;
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::OCELOT_ATTRIBUTES;
use crate::world::entity::data::{DataValue, Pose, ageable_mob_defaults};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{self, Mob, MobKind};
use crate::world::entity::monster::zombie::zombie;
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{Entity, EntityDimensions, EntityId, entity_event};
use crate::world::item::ItemStack;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::{InteractionHand, InteractionResult};
use crate::world::level::storage::tag_value_input;

pub const CROUCH_SPEED_MOD: f64 = 0.6;
pub const SPRINT_SPEED_MOD: f64 = 1.33;
pub const DATA_TRUSTING: usize = 18;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.6, 0.7).with_passenger_attachments(&[Vec3::new(0.0, 0.6375f32 as f64, 0.0)]);
pub const BABY_DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(0.3, 0.35).with_eye_height(0.34375).with_passenger_attachments(&[Vec3::new(0.0, 0.3125f32 as f64, 0.0)]);
const DEFAULT_TRUSTING: bool = false;

#[derive(Default)]
pub struct Ocelot {
    pub animal: Animal,
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::OCELOT_FOOD)
}

pub fn is_trusting(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_TRUSTING), DataValue::Boolean(true))
}

pub fn set_trusting(mob: &mut Mob, trusting: bool) {
    mob.entity.data.set(DATA_TRUSTING, DataValue::Boolean(trusting));
    reassess_trusting_goals(mob);
}

pub fn remove_when_far_away(mob: &Mob) -> bool {
    !is_trusting(&mob.entity) && mob.entity.tick_count > 2400
}

pub fn reassess_trusting_goals(mob: &mut Mob) {
    let goal_selector = &mut mob.ai.goal_selector;
    let ocelot_avoid_players_goal = match goal_selector.remove_goal(|goal| matches!(goal, Goal::OcelotAvoidEntity(_))) {
        Some(wrapped_goal) => {
            let running = wrapped_goal.is_running();
            let Goal::OcelotAvoidEntity(mut ocelot_avoid_players_goal) = wrapped_goal.into_goal() else { unreachable!("an ocelot avoid entity goal") };
            if running {
                ocelot_avoid_players_goal.avoid_entity_goal.stop();
            }
            ocelot_avoid_players_goal
        }
        None => OcelotAvoidEntityGoal::new("minecraft:player", 16.0, 0.8, 1.33),
    };
    if !is_trusting(&mob.entity) {
        mob.ai.goal_selector.add_goal(4, Goal::OcelotAvoidEntity(ocelot_avoid_players_goal));
    }
}

pub fn check_spawn_obstruction(mob: &Mob, level: &mut (impl ServerLevelAccessor + ?Sized)) -> bool {
    let bounding_box = mob.entity.bounding_box();
    if level.is_unobstructed(&bounding_box) && !level.contains_any_liquid(&bounding_box) {
        let pos = mob.entity.block_position();
        if pos.y < level.get_sea_level() {
            return false;
        }
        let state = level.block_state(pos.below());
        if state.is_block(blocks::GRASS_BLOCK) || state.is(&block_tags::LEAVES) {
            return true;
        }
    }
    false
}

pub fn finalize_spawn(
    level: &mut (impl ServerLevelAccessor + ?Sized),
    mob: &mut Mob,
    difficulty: &DifficultyInstance,
    spawn_reason: EntitySpawnReason,
    group_data: Option<SpawnGroupData>,
) -> Option<SpawnGroupData> {
    let group_data = group_data.unwrap_or(SpawnGroupData::AgeableMob(AgeableMobGroupData::with_chance(true, 1.0)));
    let group_data = ageable_mob::finalize_spawn(level, mob, Some(group_data));
    mob::mob_finalize_spawn(level, mob, difficulty, spawn_reason, group_data)
}

impl Ocelot {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        data.push(DataValue::Boolean(DEFAULT_TRUSTING));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:ocelot"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&OCELOT_ATTRIBUTES), MobKind::Ocelot(Ocelot::default()));
        Animal::init(&mut mob);
        reassess_trusting_goals(&mut mob);
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let tempt_goal = OcelotTemptGoal::new(CROUCH_SPEED_MOD, is_food, true);
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(1, Goal::Float(float_goal));
        goal_selector.add_goal(3, Goal::OcelotTempt(tempt_goal));
        goal_selector.add_goal(7, Goal::LeapAtTarget(LeapAtTargetGoal::new(0.3)));
        goal_selector.add_goal(8, Goal::OcelotAttack(OcelotAttackGoal::default()));
        goal_selector.add_goal(9, Goal::Breed(BreedGoal::new(0.8)));
        goal_selector.add_goal(10, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::with_probability(0.8, 1.0000001E-5)));
        goal_selector.add_goal(11, Goal::LookAtPlayer(LookAtPlayerGoal::new(10.0)));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::Chicken, false)));
        target_selector.add_goal(1, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::with(TargetType::Turtle, 10, false, false, Some(zombie::baby_on_land_selector))));
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.ocelot.ambient"
    }

    pub fn get_ambient_sound_interval() -> i32 {
        900
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.ocelot.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.ocelot.death"
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        output.push(("Trusting".into(), Nbt::Byte(is_trusting(&mob.entity) as i8)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        let trusting = tag_value_input::get_i64(input, "Trusting").map_or(DEFAULT_TRUSTING, |v| v != 0);
        set_trusting(mob, trusting);
    }
}

pub struct OcelotAvoidEntityGoal {
    pub avoid_entity_goal: AvoidEntityGoal,
}

impl OcelotAvoidEntityGoal {
    pub fn new(avoid_class: &'static str, max_dist: f32, walk_speed_modifier: f64, sprint_speed_modifier: f64) -> OcelotAvoidEntityGoal {
        OcelotAvoidEntityGoal { avoid_entity_goal: AvoidEntityGoal::new(avoid_class, max_dist, walk_speed_modifier, sprint_speed_modifier) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        !is_trusting(level.entity(mob)) && self.avoid_entity_goal.can_use(level, mob)
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        !is_trusting(level.entity(mob)) && self.avoid_entity_goal.can_continue_to_use(level, mob)
    }
}

pub struct OcelotTemptGoal {
    pub tempt_goal: TemptGoal,
}

impl OcelotTemptGoal {
    pub fn new(speed_modifier: f64, items: fn(&ItemStack) -> bool, can_scare: bool) -> OcelotTemptGoal {
        OcelotTemptGoal { tempt_goal: TemptGoal::new(speed_modifier, items, can_scare) }
    }

    fn can_scare(&self, level: &ServerLevel, mob: EntityId) -> bool {
        self.tempt_goal.can_scare() && !is_trusting(level.entity(mob))
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let can_scare = self.can_scare(level, mob);
        self.tempt_goal.is_not_scared(level, mob, can_scare) && self.tempt_goal.can_use(level, mob)
    }
}

impl ServerLevel {
    pub fn is_ocelot(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::Ocelot(_)))
    }

    pub fn ocelot_custom_server_ai_step(&mut self, id: EntityId) {
        let move_control = &self.mob(id).expect("an ocelot").ai.move_control;
        let (pose, sprinting) = if move_control.has_wanted() {
            let speed = move_control.speed_modifier;
            if speed == CROUCH_SPEED_MOD {
                (Pose::Crouching, false)
            } else if speed == SPRINT_SPEED_MOD {
                (Pose::Standing, true)
            } else {
                (Pose::Standing, false)
            }
        } else {
            (Pose::Standing, false)
        };
        self.entity_mut(id).set_pose(pose);
        self.set_sprinting(id, sprinting);
    }

    pub fn ocelot_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        let tempt_goal_running = self.mob(id).expect("an ocelot").ai.goal_selector.get_available_goals().iter().any(|goal| matches!(goal.get_goal(), Goal::OcelotTempt(goal) if goal.tempt_goal.is_running()));
        if tempt_goal_running && !is_trusting(self.entity(id)) && is_food(&item_stack) && self.entity(player).distance_to_sqr(self.entity(id)) < 9.0 {
            self.use_player_item(player, hand, &mut item_stack);
            if self.entity_mut(id).random.next_int_bounded(3) == 0 {
                set_trusting(self.mob_mut(id).expect("an ocelot"), true);
                self.spawn_trusting_particles(id);
                self.broadcast_entity_event(id, entity_event::TRUSTING_SUCCEEDED);
            } else {
                self.spawn_trusting_particles(id);
                self.broadcast_entity_event(id, entity_event::TRUSTING_FAILED);
            }
            InteractionResult::SUCCESS
        } else {
            self.animal_mob_interact(id, player, hand)
        }
    }

    fn spawn_trusting_particles(&mut self, id: EntityId) {
        let e = self.entity_mut(id);
        for _ in 0..7 {
            e.random.next_gaussian();
            e.random.next_gaussian();
            e.random.next_gaussian();
            e.random_x(1.0);
            e.random_y();
            e.random_z(1.0);
        }
    }

    pub fn ocelot_is_stepping_carefully(&self, id: EntityId) -> bool {
        self.entity(id).is_crouching() || self.entity_is_stepping_carefully(id)
    }
}
