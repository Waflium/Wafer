use wafer_core::AABB;
use wafer_core::random::RandomSource;
use wafer_data::registry;

use super::target_goal::TargetGoal;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::goal::goal::reduced_tick_delay;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;

const DEFAULT_RANDOM_INTERVAL: i32 = 10;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TargetType {
    Player,
    AbstractVillager,
    IronGolem,
    Turtle,
    Axolotl,
    Endermite,
    Raider,
    Animal,
    AbstractSkeleton,
    Mob,
    Rabbit,
    Chicken,
    AbstractFish,
    LivingEntity,
    Wolf,
    AbstractPiglin,
    Fox,
    Zombie,
    AbstractIllager,
    Guardian,
}

impl TargetType {
    pub fn is_instance(self, level: &ServerLevel, entity: EntityId) -> bool {
        match self {
            TargetType::Animal => level.mob(entity).is_some_and(|mob| mob.animal().is_some()),
            TargetType::AbstractSkeleton => level.mob(entity).is_some_and(|mob| mob.is_abstract_skeleton()),
            TargetType::Player => level.is_player(entity),
            TargetType::AbstractVillager => is_type(level, entity, &["minecraft:villager", "minecraft:wandering_trader"]),
            TargetType::IronGolem => is_type(level, entity, &["minecraft:iron_golem"]),
            TargetType::Turtle => is_type(level, entity, &["minecraft:turtle"]),
            TargetType::Axolotl => is_type(level, entity, &["minecraft:axolotl"]),
            TargetType::Endermite => is_type(level, entity, &["minecraft:endermite"]),
            TargetType::Raider => level.mob(entity).is_some_and(|mob| mob.raider().is_some()),
            TargetType::Mob => level.is_mob(entity),
            TargetType::Rabbit => is_type(level, entity, &["minecraft:rabbit"]),
            TargetType::Chicken => is_type(level, entity, &["minecraft:chicken"]),
            TargetType::AbstractFish => level.mob(entity).is_some_and(|mob| mob.is_abstract_fish()),
            TargetType::LivingEntity => level.is_living(entity),
            TargetType::Wolf => is_type(level, entity, &["minecraft:wolf"]),
            TargetType::AbstractPiglin => is_type(level, entity, &["minecraft:piglin", "minecraft:piglin_brute"]),
            TargetType::Fox => is_type(level, entity, &["minecraft:fox"]),
            TargetType::Zombie => level.mob(entity).is_some_and(|mob| mob.is_zombie()),
            TargetType::AbstractIllager => level.mob(entity).is_some_and(|mob| mob.is_abstract_illager()),
            TargetType::Guardian => level.is_guardian(entity),
        }
    }
}

fn is_type(level: &ServerLevel, entity: EntityId, types: &[&str]) -> bool {
    level.is_living(entity) && level.entity_ref(entity).is_some_and(|e| types.contains(&registry::ENTITY_TYPE[e.entity_type as usize]))
}

pub type Selector = fn(EntityId, EntityId, &ServerLevel) -> bool;

pub struct NearestAttackableTargetGoal {
    pub target_goal: TargetGoal,
    target_type: TargetType,
    pub random_interval: i32,
    pub target: Option<EntityId>,
    pub selector: Option<Selector>,
}

impl NearestAttackableTargetGoal {
    pub fn new(target_type: TargetType, must_see: bool) -> NearestAttackableTargetGoal {
        NearestAttackableTargetGoal::with(target_type, DEFAULT_RANDOM_INTERVAL, must_see, false, None)
    }

    pub fn with(target_type: TargetType, random_interval: i32, must_see: bool, must_reach: bool, selector: Option<Selector>) -> NearestAttackableTargetGoal {
        NearestAttackableTargetGoal { target_goal: TargetGoal::new(must_see, must_reach), target_type, random_interval: reduced_tick_delay(random_interval), target: None, selector }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if self.random_interval > 0 && level.entity_mut(mob).random.next_int_bounded(self.random_interval) != 0 {
            return false;
        }
        self.find_target(level, mob);
        self.target.is_some()
    }

    fn get_target_search_area(&self, level: &ServerLevel, mob: EntityId, follow_distance: f64) -> AABB {
        level.entity(mob).bounding_box().inflate(follow_distance, follow_distance, follow_distance)
    }

    pub fn find_target(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let follow_distance = self.target_goal.get_follow_distance(level, mob);
        let selector = self.selector.map(|selector| move |target: EntityId, level: &ServerLevel| selector(mob, target, level));
        let mut target_conditions = TargetingConditions::for_combat().range(follow_distance);
        if let Some(selector) = &selector {
            target_conditions = target_conditions.selector(selector);
        }
        let e = level.entity(mob);
        let (x, y, z) = (e.x(), e.eye_y(), e.z());
        if self.target_type != TargetType::Player {
            let area = self.get_target_search_area(level, mob, follow_distance);
            let target_type = self.target_type;
            let entities: Vec<EntityId> = level.get_entities(None, &area).into_iter().filter(|&entity| target_type.is_instance(level, entity)).collect();
            self.target = level.get_nearest_entity(&entities, &target_conditions, Some(mob), x, y, z);
        } else {
            self.target = level.get_nearest_player(&target_conditions, Some(mob), x, y, z);
        }
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.set_target(mob, self.target);
        self.target_goal.start();
    }

    pub fn set_target(&mut self, target: Option<EntityId>) {
        self.target = target;
    }

    pub fn target_type(&self) -> TargetType {
        self.target_type
    }
}
