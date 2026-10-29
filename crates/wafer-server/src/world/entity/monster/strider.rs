use std::borrow::Cow;
use std::sync::LazyLock;

use wafer_core::random::RandomSource;
use wafer_core::shapes::VoxelShape;
use wafer_core::{AABB, BlockPos, Direction, Vec3};
use wafer_data::{EquipmentSlot, Fluid, block_tags, blocks, fluid_tags, item_tags, items, registry, registry_id};

use crate::server::level::ServerLevel;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::ageable_mob::AgeableMobGroupData;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::breed_goal::BreedGoal;
use crate::world::entity::ai::goal::follow_parent_goal::FollowParentGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::move_to_block_goal::MoveToBlockGoal;
use crate::world::entity::ai::goal::panic_goal::PanicGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::random_stroll_goal::RandomStrollGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::TargetType;
use crate::world::entity::ai::goal::tempt_goal::TemptGoal;
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::{Attribute, AttributeModifier, Operation, STRIDER_ATTRIBUTES};
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::item_based_steering::ItemBasedSteering;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::monster::zombie::zombie::{self, ZombieGroupData};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::vehicle::dismount_helper;
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, get_collision_horizontal_escape_vector, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::path_navigation_region::PathNavigationRegion;
use crate::world::phys::shapes::collision_context::CollisionContext;
use crate::world::{InteractionHand, InteractionResult};

const SUFFOCATING_MODIFIER_ID: &str = "minecraft:suffocating";
const SUFFOCATING_MODIFIER: AttributeModifier = AttributeModifier { id: Cow::Borrowed(SUFFOCATING_MODIFIER_ID), amount: -0.34f32 as f64, operation: Operation::AddMultipliedBase };
const SUFFOCATE_STEERING_MODIFIER: f32 = 0.35;
const STEERING_MODIFIER: f32 = 0.55;
pub const DATA_BOOST_TIME: usize = 18;
pub const DATA_SUFFOCATING: usize = 19;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.9, 1.7);
pub const BABY_DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.45, 0.85).with_eye_height(0.4375).with_passenger_attachments(&[Vec3::new(0.0, 0.65625, 0.0)]);

#[derive(Default)]
pub struct Strider {
    pub animal: Animal,
    steering: ItemBasedSteering,
}

fn strider_mut(mob: &mut Mob) -> (&mut Entity, &mut Strider) {
    match mob {
        Mob { entity, kind: MobKind::Strider(strider), .. } => (entity, strider),
        _ => unreachable!("not a strider"),
    }
}

pub fn is_suffocating(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_SUFFOCATING), DataValue::Boolean(true))
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::STRIDER_FOOD)
}

pub fn get_liquid_collision_shape() -> &'static VoxelShape {
    static SHAPE: LazyLock<VoxelShape> = LazyLock::new(|| VoxelShape::from_boxes(&[AABB::new(0.0, 0.0, 0.0, 1.0, 0.5, 1.0)]));
    &SHAPE
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.strider.hurt"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.strider.death"
}

pub fn next_step(entity: &Entity) -> f32 {
    entity.move_dist + 0.6
}

impl Strider {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        data.extend([DataValue::Int(0), DataValue::Boolean(false)]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:strider"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&STRIDER_ATTRIBUTES), MobKind::Strider(Strider::default()));
        Animal::init(&mut mob);
        mob.set_pathfinding_malus(crate::world::level::pathfinder::path_type::PathType::Water, -1.0);
        mob.set_pathfinding_malus(crate::world::level::pathfinder::path_type::PathType::Lava, 0.0);
        mob.set_pathfinding_malus(crate::world::level::pathfinder::path_type::PathType::FireInNeighbor, 0.0);
        mob.set_pathfinding_malus(crate::world::level::pathfinder::path_type::PathType::Fire, 0.0);
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(1, Goal::Panic(PanicGoal::new(1.65)));
        goal_selector.add_goal(2, Goal::Breed(BreedGoal::new(1.0)));
        goal_selector.add_goal(3, Goal::Tempt(TemptGoal::new(1.4, |i| i.get_item().is(&item_tags::STRIDER_TEMPT_ITEMS), false)));
        goal_selector.add_goal(4, Goal::StriderGoToLava(StriderGoToLavaGoal::new(1.0)));
        goal_selector.add_goal(5, Goal::FollowParent(FollowParentGoal::new(1.0)));
        goal_selector.add_goal(7, Goal::RandomStroll(RandomStrollGoal::with_interval(1.0, 60)));
        goal_selector.add_goal(8, Goal::LookAtPlayer(LookAtPlayerGoal::new(8.0)));
        goal_selector.add_goal(8, Goal::RandomLookAround(RandomLookAroundGoal::default()));
        goal_selector.add_goal(9, Goal::LookAtPlayer(LookAtPlayerGoal::of_type(TargetType::Strider, 8.0)));
    }
}

impl ServerLevel {
    pub fn is_strider(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::Strider(_)))
    }

    pub fn strider_set_suffocating(&mut self, id: EntityId, flag: bool) {
        let mob = self.mob_mut(id).expect("a strider");
        mob.entity.data.set(DATA_SUFFOCATING, DataValue::Boolean(flag));
        if let Some(attribute) = mob.living.attributes.instance(Attribute::MovementSpeed) {
            if flag {
                attribute.add_or_update_transient_modifier(SUFFOCATING_MODIFIER);
            } else {
                attribute.remove_modifier(SUFFOCATING_MODIFIER_ID);
            }
        }
    }

    pub fn strider_get_controlling_passenger(&self, id: EntityId) -> Option<EntityId> {
        if self.is_saddled(id)
            && let Some(player) = self.first_passenger(id).filter(|&passenger| self.is_player(passenger))
            && self.is_holding(player, items::WARPED_FUNGUS_ON_A_STICK)
        {
            return Some(player);
        }
        self.mob_get_controlling_passenger(id)
    }

    pub fn strider_dismount_location_for_passenger(&mut self, id: EntityId, passenger: EntityId) -> Vec3 {
        let width = self.entity(id).dimensions.width as f64;
        let passenger_width = self.entity(passenger).dimensions.width as f64;
        let y_rot = self.entity(passenger).y_rot;
        let directions = [
            get_collision_horizontal_escape_vector(width, passenger_width, y_rot),
            get_collision_horizontal_escape_vector(width, passenger_width, y_rot - 22.5),
            get_collision_horizontal_escape_vector(width, passenger_width, y_rot + 22.5),
            get_collision_horizontal_escape_vector(width, passenger_width, y_rot - 45.0),
            get_collision_horizontal_escape_vector(width, passenger_width, y_rot + 45.0),
        ];
        let mut target_block_positions: Vec<BlockPos> = Vec::new();
        let e = self.entity(id);
        let collider_top = e.bounding_box().max_y;
        let collider_bottom = e.bounding_box().min_y - 0.5;
        for direction in directions {
            let e = self.entity(id);
            let mut block_pos = BlockPos::containing(e.x() + direction.x, collider_top, e.z() + direction.z);
            let mut y = collider_top;
            while y > collider_bottom {
                if !target_block_positions.contains(&block_pos) {
                    target_block_positions.push(block_pos);
                }
                block_pos = block_pos.relative(Direction::Down);
                y -= 1.0;
            }
        }
        for target_block_pos in target_block_positions {
            if !self.fluid_state(target_block_pos).is(&fluid_tags::LAVA) {
                let block_floor_height = dismount_helper::get_block_floor_height(self, target_block_pos);
                if dismount_helper::is_block_floor_valid(block_floor_height) {
                    let location = Vec3::up_from_bottom_center_of(target_block_pos, block_floor_height);
                    for &dismount_pose in self.dismount_poses(passenger) {
                        let pose_collision_box = self.local_bounds_for_pose(passenger, dismount_pose);
                        if dismount_helper::can_dismount_to(self, passenger, &pose_collision_box.move_by(location.x, location.y, location.z)) {
                            self.set_entity_pose(passenger, dismount_pose);
                            return location;
                        }
                    }
                }
            }
        }
        let e = self.entity(id);
        Vec3::new(e.x(), e.bounding_box().max_y, e.z())
    }

    pub fn strider_tick_ridden(&mut self, id: EntityId, controller: EntityId, ridden_input: Vec3) {
        let (y_rot, x_rot) = (self.entity(controller).y_rot, self.entity(controller).x_rot);
        let mob = self.mob_mut(id).expect("a strider");
        mob.entity.set_rot(y_rot, x_rot * 0.5);
        let y_rot = mob.entity.y_rot;
        mob.entity.y_rot_o = y_rot;
        mob.living.y_body_rot = y_rot;
        mob.entity.y_head_rot = y_rot;
        let (entity, strider) = strider_mut(mob);
        strider.steering.tick_boost(entity, DATA_BOOST_TIME);
        self.living_entity_tick_ridden(id, controller, ridden_input);
    }

    pub fn strider_get_ridden_input(&self, _id: EntityId, _controller: EntityId, _self_input: Vec3) -> Vec3 {
        Vec3::new(0.0, 0.0, 1.0)
    }

    pub fn strider_get_ridden_speed(&self, id: EntityId, _controller: EntityId) -> f32 {
        let mob = self.mob(id).expect("a strider");
        let MobKind::Strider(strider) = &mob.kind else { unreachable!("not a strider") };
        let boost_factor = strider.steering.boost_factor(&mob.entity, DATA_BOOST_TIME);
        let modifier = if is_suffocating(&mob.entity) { SUFFOCATE_STEERING_MODIFIER } else { STEERING_MODIFIER };
        (self.attribute_value(id, Attribute::MovementSpeed) * modifier as f64 * boost_factor as f64) as f32
    }

    pub fn strider_play_step_sound(&mut self, id: EntityId) {
        let sound = if self.entity(id).is_in_lava() { "minecraft:entity.strider.step_lava" } else { "minecraft:entity.strider.step" };
        self.entity_play_sound(id, sound_event(sound), 1.0, 1.0);
    }

    pub fn strider_boost(&mut self, id: EntityId) -> bool {
        let (entity, strider) = strider_mut(self.mob_mut(id).expect("a strider"));
        strider.steering.boost(entity, DATA_BOOST_TIME)
    }

    pub fn strider_can_use_slot(&self, id: EntityId, slot: EquipmentSlot) -> bool {
        if slot != EquipmentSlot::Saddle { true } else { self.is_alive(id) && !self.is_baby(id) }
    }

    pub fn strider_check_fall_damage(&mut self, id: EntityId) -> bool {
        if self.entity(id).is_in_lava() {
            self.reset_fall_distance(id);
            return true;
        }
        false
    }

    pub fn strider_tick(&mut self, id: EntityId) {
        if self.strider_is_being_tempted(id) && self.entity_mut(id).random.next_int_bounded(140) == 0 {
            self.make_sound(id, sound_event("minecraft:entity.strider.happy"));
        } else if self.is_panicking(id) && self.entity_mut(id).random.next_int_bounded(60) == 0 {
            self.make_sound(id, sound_event("minecraft:entity.strider.retreat"));
        }
        if !self.mob(id).expect("a strider").is_no_ai() {
            let state_inside = self.block_state(self.entity(id).block_position());
            let on_pos = self.on_pos_legacy(id);
            let state_on = self.block_state(on_pos);
            let in_warm_blocks = state_inside.is(&block_tags::STRIDER_WARM_BLOCKS) || state_on.is(&block_tags::STRIDER_WARM_BLOCKS) || self.entity(id).fluid_height(&fluid_tags::LAVA) > 0.0;
            let on_warm_strider = self.vehicle(id).is_some_and(|vehicle| self.is_strider(vehicle) && !is_suffocating(self.entity(vehicle)));
            self.strider_set_suffocating(id, !in_warm_blocks && !on_warm_strider);
        }
        self.mob_tick(id);
        self.then_tick(move |level| level.strider_float(id));
    }

    fn strider_is_being_tempted(&self, id: EntityId) -> bool {
        self.mob(id).expect("a strider").ai.goal_selector.get_available_goals().iter().any(|wrapped_goal| matches!(wrapped_goal.get_goal(), Goal::Tempt(goal) if goal.is_running()))
    }

    fn strider_float(&mut self, id: EntityId) {
        if self.entity(id).is_in_lava() {
            let context = CollisionContext::of(self, id);
            let block_pos = self.entity(id).block_position();
            if context.is_above(get_liquid_collision_shape(), block_pos, true) && !self.fluid_state(block_pos.above()).is(&fluid_tags::LAVA) {
                self.set_on_ground(id, true);
            } else {
                let e = self.entity_mut(id);
                e.set_delta_movement(e.delta_movement.scale(0.5).add(0.0, 0.05, 0.0));
            }
        }
    }

    pub fn strider_get_ambient_sound(&self, id: EntityId) -> Option<&'static str> {
        (!self.is_panicking(id) && !self.strider_is_being_tempted(id)).then_some("minecraft:entity.strider.ambient")
    }

    pub fn strider_can_add_passenger(&self, id: EntityId) -> bool {
        !self.is_vehicle(id) && !self.entity(id).is_eye_in_fluid(&fluid_tags::LAVA)
    }

    pub fn strider_get_walk_target_value(&self, id: EntityId, pos: BlockPos) -> f32 {
        if PathNavigationRegion::new(self).get_block_state(pos).fluid_state().is(&fluid_tags::LAVA) {
            10.0
        } else if self.entity(id).is_in_lava() {
            f32::NEG_INFINITY
        } else {
            0.0
        }
    }

    pub fn strider_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let has_food = is_food(&self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty));
        if !has_food && self.is_saddled(id) && !self.is_vehicle(id) && !self.player(player).is_secondary_use_active() {
            self.start_riding(player, id, false, true);
            return InteractionResult::SUCCESS;
        }
        let interaction_result = self.animal_mob_interact(id, player, hand);
        if !interaction_result.consumes_action() {
            let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
            if !self.is_equippable_in_slot(id, &item_stack, EquipmentSlot::Saddle) {
                return InteractionResult::Pass;
            }
            let result = crate::world::item::interact_living_entity(self, &mut item_stack, player, id, hand);
            self.player_mut(player).update_item_in_hand(hand, &item_stack);
            return result;
        }
        if has_food && !self.entity(id).is_silent() {
            let e = self.entity(id);
            let (x, y, z) = (e.x(), e.y(), e.z());
            let random = &mut self.entity_mut(id).random;
            let pitch = 1.0 + (random.next_float() - random.next_float()) * 0.2;
            let source = self.sound_source(id);
            self.play_sound_at(None, x, y, z, sound_event("minecraft:entity.strider.eat"), source, 1.0, pitch);
        }
        interaction_result
    }

    pub fn strider_finalize_spawn(&mut self, id: EntityId, difficulty: &DifficultyInstance, spawn_reason: EntitySpawnReason, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
        if self.is_baby(id) {
            return self.default_finalize_spawn(id, difficulty, spawn_reason, group_data);
        }
        let group_data = if self.random.next_int_bounded(30) == 0 && self.is_spawning_monsters() {
            let jockey = crate::world::entity::monster::zombie::zombified_piglin::ZombifiedPiglin::create();
            let is_baby = zombie::get_spawn_as_baby_odds(&mut self.random);
            let (jockey, data) = self.strider_spawn_jockey(id, difficulty, jockey, Some(SpawnGroupData::Zombie(ZombieGroupData { is_baby, can_spawn_jockey: false })));
            self.living_set_item_slot(jockey, EquipmentSlot::Mainhand, ItemStack::new(items::WARPED_FUNGUS_ON_A_STICK, 1));
            self.living_set_item_slot(id, EquipmentSlot::Saddle, ItemStack::new(items::SADDLE, 1));
            self.set_guaranteed_drop(id, EquipmentSlot::Saddle);
            data
        } else if self.random.next_int_bounded(10) == 0 {
            let mut jockey = Strider::create();
            jockey.set_age(-24000);
            self.strider_spawn_jockey(id, difficulty, jockey, None).1
        } else {
            Some(SpawnGroupData::AgeableMob(AgeableMobGroupData::with_chance(true, 0.5)))
        };
        self.default_finalize_spawn(id, difficulty, spawn_reason, group_data)
    }

    fn strider_spawn_jockey(&mut self, id: EntityId, difficulty: &DifficultyInstance, mut jockey: Mob, jockey_group_data: Option<SpawnGroupData>) -> (EntityId, Option<SpawnGroupData>) {
        let e = self.entity(id);
        let (x, y, z, y_rot) = (e.x(), e.y(), e.z(), e.y_rot);
        jockey.entity.snap_to(x, y, z, y_rot, 0.0);
        let jockey_id = jockey.entity.id;
        self.entities.insert(jockey_id, LevelEntity::Mob(Box::new(jockey)));
        self.finalize_spawn(jockey_id, difficulty, EntitySpawnReason::Jockey, jockey_group_data);
        self.start_riding(jockey_id, id, true, false);
        (jockey_id, Some(SpawnGroupData::AgeableMob(AgeableMobGroupData::with_chance(true, 0.0))))
    }

    pub fn strider_get_breed_offspring(&mut self, _id: EntityId, _partner: EntityId) -> Option<Box<Mob>> {
        Some(Box::new(Strider::create()))
    }
}

pub struct StriderGoToLavaGoal {
    pub move_to_block_goal: MoveToBlockGoal,
}

impl StriderGoToLavaGoal {
    pub fn new(speed_modifier: f64) -> StriderGoToLavaGoal {
        StriderGoToLavaGoal { move_to_block_goal: MoveToBlockGoal::new(speed_modifier, 8, 2) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        !level.entity(mob).is_in_lava() && self.move_to_block_goal.can_use(level, mob, &is_valid_target)
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        !level.entity(mob).is_in_lava() && is_valid_target(level, self.move_to_block_goal.block_pos)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.move_to_block_goal.start(level, mob);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let move_to_target = self.move_to_block_goal.block_pos;
        self.move_to_block_goal.tick_towards(level, mob, move_to_target, 1.0, |goal| goal.try_ticks % 20 == 0);
    }
}

fn is_valid_target(level: &ServerLevel, pos: BlockPos) -> bool {
    let region = PathNavigationRegion::new(level);
    region.get_block_state(pos).is_block(blocks::LAVA) && region.get_block_state(pos.above()).is_pathfindable_land()
}

pub fn can_stand_on_fluid(fluid: Fluid) -> bool {
    fluid.is(&fluid_tags::LAVA)
}
