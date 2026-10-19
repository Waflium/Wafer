use wafer_core::Vec3;
use wafer_data::{registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::climb_on_top_of_powder_snow_goal::ClimbOnTopOfPowderSnowGoal;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::melee_attack_goal::MeleeAttackGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::attributes::ENDERMITE_ATTRIBUTES;
use crate::world::entity::data::mob_defaults;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{Entity, EntityDimensions, EntityId};
use crate::world::level::block::entity::int_value;

const MAX_LIFE: i32 = 2400;
const DEFAULT_LIFE: i32 = 0;
pub const DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(0.4, 0.3).with_eye_height(0.13).with_passenger_attachments(&[Vec3::new(0.0, 0.2375f32 as f64, 0.0)]);

#[derive(Default)]
pub struct Endermite {
    life: i32,
}

fn endermite(mob: &Mob) -> &Endermite {
    match &mob.kind {
        MobKind::Endermite(endermite) => endermite,
        _ => unreachable!("not an endermite"),
    }
}

fn endermite_mut(mob: &mut Mob) -> &mut Endermite {
    match &mut mob.kind {
        MobKind::Endermite(endermite) => endermite,
        _ => unreachable!("not an endermite"),
    }
}

impl Endermite {
    pub fn create() -> Mob {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:endermite"), DIMENSIONS, mob_defaults());
        let mut mob = Mob::new(entity, LivingEntity::new(&ENDERMITE_ATTRIBUTES), MobKind::Endermite(Endermite { life: DEFAULT_LIFE }));
        mob.xp_reward = 3;
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(1, Goal::Float(float_goal));
        goal_selector.add_goal(1, Goal::ClimbOnTopOfPowderSnow(ClimbOnTopOfPowderSnowGoal));
        goal_selector.add_goal(2, Goal::MeleeAttack(MeleeAttackGoal::new(1.0, false)));
        goal_selector.add_goal(3, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::new(1.0)));
        goal_selector.add_goal(7, Goal::LookAtPlayer(LookAtPlayerGoal::new(8.0)));
        goal_selector.add_goal(8, Goal::RandomLookAround(RandomLookAroundGoal::default()));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::HurtByTarget(HurtByTargetGoal::new(&[]).set_alert_others(&[])));
        target_selector.add_goal(2, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::Player, true)));
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.endermite.ambient"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.endermite.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.endermite.death"
    }

    pub fn get_step_sound() -> &'static str {
        "minecraft:entity.endermite.step"
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        endermite_mut(mob).life = input.get("Lifetime").and_then(int_value).unwrap_or(DEFAULT_LIFE);
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        out.push(("Lifetime".into(), Nbt::Int(endermite(mob).life)));
    }
}

impl ServerLevel {
    pub fn endermite_tick(&mut self, id: EntityId) {
        let y_rot = self.entity(id).y_rot;
        self.living_mut(id).y_body_rot = y_rot;
        self.mob_tick(id);
    }

    pub fn endermite_set_y_body_rot(&mut self, id: EntityId, y_body_rot: f32) {
        self.entity_mut(id).set_y_rot(y_body_rot);
        self.living_entity_set_y_body_rot(id, y_body_rot);
    }

    pub fn endermite_ai_step(&mut self, id: EntityId) {
        self.monster_ai_step(id);
        let Some(mob) = self.mob_mut(id) else { return };
        let persistence_required = mob.is_persistence_required();
        let endermite = endermite_mut(mob);
        if !persistence_required {
            endermite.life += 1;
        }
        if endermite.life >= MAX_LIFE {
            self.discard(id);
        }
    }
}
