use wafer_core::Vec3;
use wafer_data::{Tag, damage_type_tags, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::util::time_util;
use crate::util::valueproviders::IntProvider;
use crate::world::entity::ageable_mob::AgeableMobGroupData;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::follow_parent_goal::FollowParentGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::melee_attack_goal::MeleeAttackGoal;
use crate::world::entity::ai::goal::panic_goal::PanicGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::random_stroll_goal::RandomStrollGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::goal::target::reset_universal_anger_target_goal::ResetUniversalAngerTargetGoal;
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::POLAR_BEAR_ATTRIBUTES;
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::entity_reference::EntityReference;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::neutral_mob::NeutralMob;
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{Entity, EntityDimensions, EntityId, sound_event};
use crate::world::level::server_level_accessor::ServerLevelAccessor;

pub const DATA_STANDING_ID: usize = 18;
const PERSISTENT_ANGER_TIME: IntProvider = time_util::range_of_seconds(20, 39);
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(1.4, 1.4);
pub const BABY_DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.7, 0.7).with_eye_height(0.34375).with_passenger_attachments(&[Vec3::new(0.0, 0.625, 0.0)]);

pub struct PolarBear {
    pub animal: Animal,
    warning_sound_ticks: i32,
    persistent_anger_end_time: i64,
    persistent_anger_target: Option<EntityReference>,
}

impl Default for PolarBear {
    fn default() -> PolarBear {
        PolarBear { animal: Animal::default(), warning_sound_ticks: 0, persistent_anger_end_time: crate::world::entity::neutral_mob::NO_ANGER_END_TIME, persistent_anger_target: None }
    }
}

impl NeutralMob for PolarBear {
    fn get_persistent_anger_end_time(&self, _entity: &Entity) -> i64 {
        self.persistent_anger_end_time
    }

    fn set_persistent_anger_end_time(&mut self, _entity: &mut Entity, end_time: i64) {
        self.persistent_anger_end_time = end_time;
    }

    fn get_persistent_anger_target(&self) -> Option<EntityReference> {
        self.persistent_anger_target
    }

    fn set_persistent_anger_target(&mut self, persistent_anger_target: Option<EntityReference>) {
        self.persistent_anger_target = persistent_anger_target;
    }
}

pub fn polar_bear(mob: &Mob) -> &PolarBear {
    match &mob.kind {
        MobKind::PolarBear(polar_bear) => polar_bear,
        _ => unreachable!("a polar bear"),
    }
}

pub fn polar_bear_mut(mob: &mut Mob) -> &mut PolarBear {
    match &mut mob.kind {
        MobKind::PolarBear(polar_bear) => polar_bear,
        _ => unreachable!("a polar bear"),
    }
}

pub fn set_standing(entity: &mut Entity, value: bool) {
    entity.data.set(DATA_STANDING_ID, DataValue::Boolean(value));
}

fn panic_causes(level: &ServerLevel, bear: EntityId) -> &'static Tag {
    if level.is_baby(bear) { &damage_type_tags::PANIC_CAUSES } else { &damage_type_tags::PANIC_ENVIRONMENTAL_CAUSES }
}

fn is_angry_at(mob: EntityId, target: EntityId, level: &ServerLevel) -> bool {
    level.is_angry_at(mob, target)
}

fn not_a_baby(mob: EntityId, _target: EntityId, level: &ServerLevel) -> bool {
    !level.is_baby(mob)
}

pub fn get_ambient_sound(mob: &Mob) -> &'static str {
    if mob.is_baby() { "minecraft:entity.polar_bear.ambient_baby" } else { "minecraft:entity.polar_bear.ambient" }
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.polar_bear.hurt"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.polar_bear.death"
}

pub fn get_step_sound() -> &'static str {
    "minecraft:entity.polar_bear.step"
}

pub fn get_water_slow_down() -> f32 {
    0.98
}

pub fn finalize_spawn(_level: &mut (impl ServerLevelAccessor + ?Sized), group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
    Some(group_data.unwrap_or(SpawnGroupData::AgeableMob(AgeableMobGroupData::with_chance(true, 1.0))))
}

impl PolarBear {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        data.push(DataValue::Boolean(false));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:polar_bear"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&POLAR_BEAR_ATTRIBUTES), MobKind::PolarBear(PolarBear::default()));
        Animal::init(&mut mob);
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::Float(float_goal));
        goal_selector.add_goal(1, Goal::PolarBearMeleeAttack(PolarBearMeleeAttackGoal { melee_attack_goal: MeleeAttackGoal::new(1.25, true) }));
        goal_selector.add_goal(1, Goal::Panic(PanicGoal::with_damage_types(2.0, panic_causes)));
        goal_selector.add_goal(4, Goal::FollowParent(FollowParentGoal::new(1.25)));
        goal_selector.add_goal(5, Goal::RandomStroll(RandomStrollGoal::new(1.0)));
        goal_selector.add_goal(6, Goal::LookAtPlayer(LookAtPlayerGoal::new(6.0)));
        goal_selector.add_goal(7, Goal::RandomLookAround(RandomLookAroundGoal::default()));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::PolarBearHurtByTarget(PolarBearHurtByTargetGoal { hurt_by_target_goal: HurtByTargetGoal::new(&[]) }));
        target_selector.add_goal(2, Goal::PolarBearAttackPlayers(PolarBearAttackPlayersGoal::new()));
        target_selector.add_goal(3, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::with(TargetType::Player, 10, true, false, Some(is_angry_at))));
        target_selector.add_goal(4, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::with(TargetType::Fox, 10, true, true, Some(not_a_baby))));
        target_selector.add_goal(5, Goal::ResetUniversalAngerTarget(ResetUniversalAngerTargetGoal::new(false)));
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        polar_bear(mob).add_persistent_anger_save_data(&mob.entity, out);
    }
}

impl ServerLevel {
    pub fn is_polar_bear(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::PolarBear(_)))
    }

    pub fn polar_bear_get_breed_offspring(&mut self, _id: EntityId, _partner: EntityId) -> Option<Box<Mob>> {
        Some(Box::new(PolarBear::create()))
    }

    pub fn polar_bear_start_persistent_anger_timer(&mut self, id: EntityId) {
        let remaining_time = PERSISTENT_ANGER_TIME.sample(&mut self.entity_mut(id).random);
        self.set_time_to_remain_angry(id, remaining_time as i64);
    }

    fn polar_bear_play_warning_sound(&mut self, id: EntityId) {
        if polar_bear(self.mob(id).expect("a polar bear")).warning_sound_ticks <= 0 {
            self.make_sound(id, sound_event("minecraft:entity.polar_bear.warning"));
            polar_bear_mut(self.mob_mut(id).expect("a polar bear")).warning_sound_ticks = 40;
        }
    }

    pub fn polar_bear_tick(&mut self, id: EntityId) {
        self.mob_tick(id);
        self.then_tick(move |level| level.polar_bear_tick_end(id));
    }

    fn polar_bear_tick_end(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a polar bear");
        let polar_bear = polar_bear_mut(mob);
        if polar_bear.warning_sound_ticks > 0 {
            polar_bear.warning_sound_ticks -= 1;
        }
        self.update_persistent_anger(id, true);
    }
}

pub struct PolarBearAttackPlayersGoal {
    pub nearest_attackable_target_goal: NearestAttackableTargetGoal,
}

impl PolarBearAttackPlayersGoal {
    fn new() -> PolarBearAttackPlayersGoal {
        let mut nearest_attackable_target_goal = NearestAttackableTargetGoal::with(TargetType::Player, 20, true, true, None);
        nearest_attackable_target_goal.target_goal.follow_distance_scale = 0.5;
        PolarBearAttackPlayersGoal { nearest_attackable_target_goal }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if level.is_baby(mob) {
            return false;
        }
        if self.nearest_attackable_target_goal.can_use(level, mob) {
            let bb = level.entity(mob).bounding_box().inflate(8.0, 4.0, 8.0);
            for bear in level.get_entities_of_class_same_kind(mob, &bb) {
                if level.is_baby(bear) {
                    return true;
                }
            }
        }
        false
    }
}

pub struct PolarBearHurtByTargetGoal {
    pub hurt_by_target_goal: HurtByTargetGoal,
}

impl PolarBearHurtByTargetGoal {
    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.hurt_by_target_goal.start(level, mob);
        if level.is_baby(mob) {
            self.hurt_by_target_goal.alert_others_with(level, mob, PolarBearHurtByTargetGoal::alert_other);
            self.hurt_by_target_goal.target_goal.stop(level, mob);
        }
    }

    fn alert_other(goal: &HurtByTargetGoal, level: &mut ServerLevel, mob: EntityId, other: EntityId, hurt_by_mob: EntityId) {
        if level.is_polar_bear(other) && !level.is_baby(other) {
            goal.alert_other(level, mob, other, hurt_by_mob);
        }
    }
}

pub struct PolarBearMeleeAttackGoal {
    pub melee_attack_goal: MeleeAttackGoal,
}

impl PolarBearMeleeAttackGoal {
    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.melee_attack_goal.tick_with(level, mob, PolarBearMeleeAttackGoal::check_and_perform_attack);
    }

    fn check_and_perform_attack(goal: &mut MeleeAttackGoal, level: &mut ServerLevel, mob: EntityId, target: EntityId) {
        if goal.can_perform_attack(level, mob, target) {
            goal.reset_attack_cooldown();
            level.do_hurt_target(mob, target);
            set_standing(level.entity_mut(mob), false);
        } else {
            let width = level.entity(target).dimensions.width + 3.0;
            let t = level.entity(target);
            if level.entity(mob).distance_to_sqr_xyz(t.x(), t.y(), t.z()) < (width * width) as f64 {
                if goal.is_time_to_attack() {
                    set_standing(level.entity_mut(mob), false);
                    goal.reset_attack_cooldown();
                }
                if goal.get_ticks_until_next_attack() <= 10 {
                    set_standing(level.entity_mut(mob), true);
                    level.polar_bear_play_warning_sound(mob);
                }
            } else {
                goal.reset_attack_cooldown();
                set_standing(level.entity_mut(mob), false);
            }
        }
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        set_standing(level.entity_mut(mob), false);
        self.melee_attack_goal.stop(level, mob);
    }
}
