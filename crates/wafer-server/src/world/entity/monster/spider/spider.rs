use wafer_core::Vec3;
use wafer_core::random::RandomSource;
use wafer_data::{BlockState, blocks, mob_effects, registry, registry_id};

use crate::server::level::ServerLevel;
use crate::world::difficulty::Difficulty;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::avoid_entity_goal::AvoidEntityGoal;
use crate::world::entity::animal::armadillo::armadillo;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::leap_at_target_goal::LeapAtTargetGoal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::melee_attack_goal::MeleeAttackGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::attributes::{Attribute, AttributeSupplier, SPIDER_ATTRIBUTES};
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::monster::monster;
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity};

pub const DATA_FLAGS_ID: usize = 16;
pub const DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(1.4, 0.9).with_eye_height(0.65).with_passenger_attachments(&[Vec3::new(0.0, 0.765f32 as f64, 0.0)]);

#[derive(Default)]
pub struct Spider;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SpiderEffectsGroupData {
    pub effect: Option<wafer_data::MobEffect>,
}

impl SpiderEffectsGroupData {
    pub fn set_random_effect(&mut self, random: &mut impl RandomSource) {
        let selection = random.next_int_bounded(5);
        if selection <= 1 {
            self.effect = Some(mob_effects::SPEED);
        } else if selection <= 2 {
            self.effect = Some(mob_effects::STRENGTH);
        } else if selection <= 3 {
            self.effect = Some(mob_effects::REGENERATION);
        } else if selection <= 4 {
            self.effect = Some(mob_effects::INVISIBILITY);
        }
    }
}

pub struct SpiderAttackGoal {
    pub melee_attack_goal: MeleeAttackGoal,
}

impl SpiderAttackGoal {
    pub fn new() -> SpiderAttackGoal {
        SpiderAttackGoal { melee_attack_goal: MeleeAttackGoal::new(1.0, true) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.melee_attack_goal.can_use(level, mob) && !level.is_vehicle(mob)
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let br = level.get_entity_light_level_dependent_magic_value(mob);
        if br >= 0.5 && level.entity_mut(mob).random.next_int_bounded(100) == 0 {
            level.set_target(mob, None);
            false
        } else {
            self.melee_attack_goal.can_continue_to_use(level, mob)
        }
    }
}

pub struct SpiderTargetGoal {
    pub nearest_attackable_target_goal: NearestAttackableTargetGoal,
}

impl SpiderTargetGoal {
    pub fn new(target_type: TargetType) -> SpiderTargetGoal {
        SpiderTargetGoal { nearest_attackable_target_goal: NearestAttackableTargetGoal::new(target_type, true) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let br = level.get_entity_light_level_dependent_magic_value(mob);
        if br >= 0.5 { false } else { self.nearest_attackable_target_goal.can_use(level, mob) }
    }
}

pub fn spider_defaults() -> Vec<DataValue> {
    let mut data = mob_defaults();
    data.push(DataValue::Byte(0));
    data
}

pub fn is_climbing(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_FLAGS_ID), DataValue::Byte(flags) if flags & 1 != 0)
}

pub fn set_climbing(entity: &mut Entity, value: bool) {
    let DataValue::Byte(mut flags) = *entity.data.get(DATA_FLAGS_ID) else { return };
    if value {
        flags |= 1;
    } else {
        flags &= -2;
    }
    entity.data.set(DATA_FLAGS_ID, DataValue::Byte(flags));
}

impl Spider {
    pub fn create() -> Mob {
        Spider::create_with("minecraft:spider", DIMENSIONS, &SPIDER_ATTRIBUTES, MobKind::Spider(Spider))
    }

    pub fn create_with(entity_type: &str, dimensions: EntityDimensions, attributes: &'static AttributeSupplier, kind: MobKind) -> Mob {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, entity_type), dimensions, spider_defaults());
        let mut mob = Mob::new(entity, LivingEntity::new(attributes), kind);
        mob.xp_reward = monster::XP_REWARD;
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(1, Goal::Float(float_goal));
        goal_selector.add_goal(
            2,
            Goal::AvoidEntity(AvoidEntityGoal::with_predicate_on_avoid_entity("minecraft:armadillo", 6.0, 1.0, 1.2, |entity, level| !armadillo::is_scared(level.entity(entity)))),
        );
        goal_selector.add_goal(3, Goal::LeapAtTarget(LeapAtTargetGoal::new(0.4)));
        goal_selector.add_goal(4, Goal::SpiderAttack(SpiderAttackGoal::new()));
        goal_selector.add_goal(5, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::new(0.8)));
        goal_selector.add_goal(6, Goal::LookAtPlayer(LookAtPlayerGoal::new(8.0)));
        goal_selector.add_goal(6, Goal::RandomLookAround(RandomLookAroundGoal::default()));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::HurtByTarget(HurtByTargetGoal::new(&[])));
        target_selector.add_goal(2, Goal::SpiderTarget(SpiderTargetGoal::new(TargetType::Player)));
        target_selector.add_goal(3, Goal::SpiderTarget(SpiderTargetGoal::new(TargetType::IronGolem)));
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.spider.ambient"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.spider.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.spider.death"
    }

    pub fn get_step_sound() -> &'static str {
        "minecraft:entity.spider.step"
    }

    pub fn on_climbable(mob: &Mob) -> bool {
        is_climbing(&mob.entity)
    }
}

impl ServerLevel {
    pub fn spider_tick(&mut self, id: EntityId) {
        self.mob_tick(id);
        if let Some(mob) = self.mob_mut(id) {
            let horizontal_collision = mob.entity.horizontal_collision;
            set_climbing(&mut mob.entity, horizontal_collision);
        }
    }

    pub fn spider_make_stuck_in_block(&mut self, id: EntityId, state: BlockState, speed_multiplier: Vec3) {
        if !state.is_block(blocks::COBWEB) {
            self.entity_make_stuck_in_block(id, state, speed_multiplier);
        }
    }

    pub fn spider_can_be_affected(&self, id: EntityId, new_effect: &MobEffectInstance) -> bool {
        if new_effect.effect == mob_effects::POISON { false } else { self.living_entity_can_be_affected(id, new_effect) }
    }

    pub fn spider_finalize_spawn(&mut self, id: EntityId, difficulty: &DifficultyInstance, spawn_reason: EntitySpawnReason, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
        let mut group_data = self.mob_finalize_spawn(id, difficulty, spawn_reason, group_data);
        if self.random.next_int_bounded(100) == 0
            && let Some(LevelEntity::Mob(mut skeleton)) = ServerLevel::create_entity("minecraft:skeleton")
        {
            let e = self.entity(id);
            let (x, y, z, y_rot) = (e.x(), e.y(), e.z(), e.y_rot);
            skeleton.entity.snap_to(x, y, z, y_rot, 0.0);
            let skeleton_id = skeleton.entity.id;
            self.entities.insert(skeleton_id, LevelEntity::Mob(skeleton));
            self.finalize_spawn(skeleton_id, difficulty, spawn_reason, None);
            self.start_riding(skeleton_id, id, false, false);
        }
        if group_data.is_none() {
            let mut spider_effects_group_data = SpiderEffectsGroupData::default();
            if self.difficulty == Difficulty::Hard && self.random.next_float() < 0.1 * difficulty.get_special_multiplier() {
                spider_effects_group_data.set_random_effect(&mut self.random);
            }
            group_data = Some(SpawnGroupData::SpiderEffects(spider_effects_group_data));
        }
        if let Some(SpawnGroupData::SpiderEffects(spider_effects_group_data)) = group_data
            && let Some(effect) = spider_effects_group_data.effect
        {
            self.add_effect(id, MobEffectInstance::new(effect, -1, 0), None);
        }
        group_data
    }

    pub fn spider_get_vehicle_attachment_point(&self, id: EntityId, vehicle: EntityId) -> Vec3 {
        if self.entity(vehicle).dimensions.width <= self.entity(id).dimensions.width {
            Vec3::new(0.0, 0.3125 * self.attribute_value(id, Attribute::Scale) as f32 as f64, 0.0)
        } else {
            self.entity_get_vehicle_attachment_point(id, vehicle)
        }
    }
}
