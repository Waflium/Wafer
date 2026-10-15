use wafer_core::random::RandomSource;
use wafer_core::{Vec3, mth};
use wafer_data::{fluid_tags, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::water_animal;
use crate::server::level::ServerLevel;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::control::move_control::{MoveControlKind, Operation, rotlerp};
use crate::world::entity::ai::goal::avoid_entity_goal::AvoidEntityGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::panic_goal::PanicGoal;
use crate::world::entity::ai::goal::random_swimming_goal::RandomSwimmingGoal;
use crate::world::entity::attributes::{ABSTRACT_FISH_ATTRIBUTES, Attribute};
use crate::world::entity::bucketable;
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::entity_selector;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{Entity, EntityDimensions, EntityId, MoverType, sound_event};
use crate::world::item::ItemStack;
use crate::world::{InteractionHand, InteractionResult};

pub const DATA_FROM_BUCKET: usize = 16;
pub const DEFAULT_FROM_BUCKET: bool = false;

pub fn abstract_fish_defaults() -> Vec<DataValue> {
    let mut values = mob_defaults();
    values.push(DataValue::Boolean(DEFAULT_FROM_BUCKET));
    values
}

pub struct AbstractFish;

impl AbstractFish {
    pub fn create_with(entity_type: &str, dimensions: EntityDimensions, defaults: Vec<DataValue>, kind: MobKind) -> Mob {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, entity_type), dimensions, defaults);
        let mut mob = Mob::new(entity, LivingEntity::new(&ABSTRACT_FISH_ATTRIBUTES), kind);
        water_animal::init(&mut mob);
        mob.ai.move_control.kind = MoveControlKind::Fish;
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::Panic(PanicGoal::new(1.25)));
        goal_selector.add_goal(
            2,
            Goal::AvoidEntity(AvoidEntityGoal::with_predicate_on_avoid_entity("minecraft:player", 8.0, 1.6, 1.4, |_, target, level| {
                entity_selector::no_spectators(level, target)
            })),
        );
        goal_selector.add_goal(4, Goal::FishSwim(FishSwimGoal::new()));
    }

    pub fn get_swim_sound() -> &'static str {
        "minecraft:entity.fish.swim"
    }

    pub fn get_pickup_sound() -> &'static str {
        "minecraft:item.bucket.fill_fish"
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        out.push(("FromBucket".into(), Nbt::Byte(mob.from_bucket() as i8)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, nbt: &Nbt) {
        let from_bucket = nbt.get("FromBucket").and_then(Nbt::as_i64).map_or(DEFAULT_FROM_BUCKET, |v| v != 0);
        mob.set_from_bucket(from_bucket);
    }
}

impl Mob {
    pub fn is_abstract_fish(&self) -> bool {
        matches!(self.kind, MobKind::Cod(_) | MobKind::Salmon(_) | MobKind::Pufferfish(_) | MobKind::TropicalFish(_) | MobKind::Tadpole(_))
    }

    pub fn from_bucket(&self) -> bool {
        match self.kind {
            MobKind::Axolotl(_) => crate::world::entity::animal::axolotl::axolotl::from_bucket(&self.entity),
            MobKind::Tadpole(_) => true,
            _ => matches!(self.entity.data.get(DATA_FROM_BUCKET), DataValue::Boolean(true)),
        }
    }

    pub fn set_from_bucket(&mut self, from_bucket: bool) {
        match self.kind {
            MobKind::Axolotl(_) => crate::world::entity::animal::axolotl::axolotl::set_from_bucket(&mut self.entity, from_bucket),
            MobKind::Tadpole(_) => {}
            _ => self.entity.data.set(DATA_FROM_BUCKET, DataValue::Boolean(from_bucket)),
        }
    }

    pub fn abstract_fish_remove_when_far_away(&self) -> bool {
        !self.from_bucket() && !self.entity.has_custom_name()
    }

    pub fn abstract_fish_get_max_spawn_cluster_size(&self) -> i32 {
        8
    }

    pub fn get_flop_sound(&self) -> &'static str {
        match self.kind {
            MobKind::Cod(_) => super::cod::Cod::get_flop_sound(),
            MobKind::Salmon(_) => super::salmon::Salmon::get_flop_sound(),
            MobKind::Pufferfish(_) => super::pufferfish::Pufferfish::get_flop_sound(),
            MobKind::TropicalFish(_) => super::tropical_fish::TropicalFish::get_flop_sound(),
            MobKind::Tadpole(_) => crate::world::entity::animal::frog::tadpole::get_flop_sound(),
            _ => unreachable!("not a fish"),
        }
    }

    pub fn abstract_fish_save_to_bucket_tag(&self, bucket: &mut ItemStack) {
        bucketable::save_default_data_to_bucket_tag(self, bucket);
    }

    pub fn abstract_fish_load_from_bucket_tag(&mut self, tag: &Nbt) {
        bucketable::load_default_data_from_bucket_tag(self, tag);
    }
}

impl ServerLevel {
    pub fn abstract_fish_requires_custom_persistence(&self, id: EntityId) -> bool {
        self.mob_requires_custom_persistence(id) || self.mob(id).expect("a fish").from_bucket()
    }

    pub fn abstract_fish_travel_in_water(&mut self, id: EntityId, input: Vec3, _base_gravity: f64, _is_falling: bool, _old_y: f64) {
        self.entity_mut(id).move_relative(0.01, input);
        let delta = self.entity(id).delta_movement;
        self.move_entity(id, MoverType::SelfMove, delta);
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.scale(0.9));
        if self.get_target(id).is_none() {
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.add(0.0, -0.005, 0.0));
        }
    }

    pub fn abstract_fish_ai_step(&mut self, id: EntityId) {
        let e = self.entity(id);
        if !e.is_in_water() && e.on_ground && e.vertical_collision {
            let e = self.entity_mut(id);
            let xa = (e.random.next_float() * 2.0 - 1.0) * 0.05;
            let za = (e.random.next_float() * 2.0 - 1.0) * 0.05;
            e.set_delta_movement(e.delta_movement.add(xa as f64, 0.4f32 as f64, za as f64));
            self.set_on_ground(id, false);
            self.entity_mut(id).needs_sync = true;
            let flop_sound = self.mob(id).expect("a fish").get_flop_sound();
            self.make_sound(id, sound_event(flop_sound));
        }
        self.mob_ai_step(id);
    }

    pub fn abstract_fish_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        match bucketable::bucket_mob_pickup(self, player, hand, id) {
            Some(result) => result,
            None => self.mob_mob_interact(id, player, hand),
        }
    }

    pub fn can_random_swim(&self, id: EntityId) -> bool {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Cod(_) | MobKind::Salmon(_) | MobKind::TropicalFish(_)) => self.abstract_schooling_fish_can_random_swim(id),
            _ => self.abstract_fish_can_random_swim(),
        }
    }

    pub fn abstract_fish_can_random_swim(&self) -> bool {
        true
    }

    pub fn fish_move_control_tick(&mut self, id: EntityId) {
        if self.entity(id).is_eye_in_fluid(&fluid_tags::WATER) {
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.add(0.0, 0.005, 0.0));
        }
        let movement_speed = self.attribute_value(id, Attribute::MovementSpeed);
        let mob = self.mob_mut(id).expect("a fish");
        if mob.ai.move_control.operation == Operation::MoveTo && !mob.ai.navigation.is_done() {
            let control = &mob.ai.move_control;
            let target_speed = (control.speed_modifier * movement_speed) as f32;
            let (wanted_x, wanted_y, wanted_z) = (control.wanted_x, control.wanted_y, control.wanted_z);
            let speed = mth::lerp_f(0.125, mob.living.speed, target_speed);
            mob.set_speed(speed);
            let e = &mob.entity;
            let xd = wanted_x - e.x();
            let yd = wanted_y - e.y();
            let zd = wanted_z - e.z();
            if yd != 0.0 {
                let dd = (xd * xd + yd * yd + zd * zd).sqrt();
                let speed = mob.living.speed;
                let e = &mut mob.entity;
                e.set_delta_movement(e.delta_movement.add(0.0, speed as f64 * (yd / dd) * 0.1, 0.0));
            }
            if xd != 0.0 || zd != 0.0 {
                let y_rot_d = (mth::atan2(zd, xd) * mth::RAD_TO_DEG as f64) as f32 - 90.0;
                let y_rot = rotlerp(mob.entity.y_rot, y_rot_d, 90.0);                mob.entity.set_y_rot(y_rot);
                mob.living.y_body_rot = mob.entity.y_rot;
            }
        } else {
            mob.set_speed(0.0);
        }
    }
}

pub struct FishSwimGoal {
    pub random_swimming_goal: RandomSwimmingGoal,
}

impl FishSwimGoal {
    pub fn new() -> FishSwimGoal {
        FishSwimGoal { random_swimming_goal: RandomSwimmingGoal::new(1.0, 40) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        level.can_random_swim(mob) && self.random_swimming_goal.can_use(level, mob)
    }
}
