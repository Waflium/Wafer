use std::borrow::Cow;

use wafer_core::random::RandomSource;
use wafer_core::{AABB, Vec3};
use wafer_data::{EquipmentSlot, items};
use wafer_protocol::nbt::Nbt;

use super::zombie::Zombie;
use crate::server::level::ServerLevel;
use crate::util::time_util;
use crate::util::valueproviders::IntProvider;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::spear_use_goal::SpearUseGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::goal::target::reset_universal_anger_target_goal::ResetUniversalAngerTargetGoal;
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::ai::goal::zombie_attack_goal::ZombieAttackGoal;
use crate::world::entity::attributes::{Attribute, AttributeModifier, Operation, ZOMBIFIED_PIGLIN_ATTRIBUTES};
use crate::world::entity::entity_reference::EntityReference;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::neutral_mob::NeutralMob;
use crate::world::entity::{Entity, EntityDimensions, EntityId, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::pathfinder::path_type::PathType;

pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.6, 1.95)
    .with_eye_height(1.79)
    .with_passenger_attachments(&[Vec3::new(0.0, 2.0f32 as f64, 0.0)])
    .with_vehicle_attachment(&[Vec3::new(0.0, 0.7f32 as f64, 0.0)]);
pub const BABY_DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(0.49, 0.98).with_eye_height(0.78).with_vehicle_attachment(&[Vec3::new(0.0, 0.1875f32 as f64, 0.0)]);
const SPEED_MODIFIER_ATTACKING_ID: &str = "minecraft:attacking";
const SPEED_MODIFIER_ATTACKING: AttributeModifier = AttributeModifier { id: Cow::Borrowed(SPEED_MODIFIER_ATTACKING_ID), amount: 0.05, operation: Operation::AddValue };
const FIRST_ANGER_SOUND_DELAY: IntProvider = time_util::range_of_seconds(0, 1);
const PERSISTENT_ANGER_TIME: IntProvider = time_util::range_of_seconds(20, 39);
const ALERT_RANGE_Y: f64 = 10.0;
const ALERT_INTERVAL: IntProvider = time_util::range_of_seconds(4, 6);

#[derive(Default)]
pub struct ZombifiedPiglin {
    pub zombie: Zombie,
    play_first_anger_sound_in: i32,
    persistent_anger_end_time: i64,
    persistent_anger_target: Option<EntityReference>,
    ticks_until_next_alert: i32,
}

impl NeutralMob for ZombifiedPiglin {
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

fn zombified_piglin(mob: &Mob) -> &ZombifiedPiglin {
    match &mob.kind {
        MobKind::ZombifiedPiglin(zombified_piglin) => zombified_piglin,
        _ => unreachable!("not a zombified piglin"),
    }
}

fn zombified_piglin_mut(mob: &mut Mob) -> &mut ZombifiedPiglin {
    match &mut mob.kind {
        MobKind::ZombifiedPiglin(zombified_piglin) => zombified_piglin,
        _ => unreachable!("not a zombified piglin"),
    }
}

fn is_angry_at_selector(mob: EntityId, target: EntityId, level: &ServerLevel) -> bool {
    level.is_angry_at(mob, target)
}

impl ZombifiedPiglin {
    pub fn create() -> Mob {
        let mob = Zombie::create_with("minecraft:zombified_piglin", DIMENSIONS, &ZOMBIFIED_PIGLIN_ATTRIBUTES, MobKind::ZombifiedPiglin(ZombifiedPiglin::default()));
        mob.set_pathfinding_malus(PathType::Lava, 8.0);
        mob
    }

    pub fn add_behaviour_goals(ai: &mut MobAi) {
        ai.goal_selector.add_goal(1, Goal::SpearUse(SpearUseGoal::new(1.0, 1.0, 10.0, 2.0)));
        ai.goal_selector.add_goal(2, Goal::ZombieAttack(ZombieAttackGoal::new(1.0, false)));
        ai.goal_selector.add_goal(7, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::new(1.0)));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::HurtByTarget(HurtByTargetGoal::new(&[]).set_alert_others(&[])));
        target_selector.add_goal(2, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::with(TargetType::Player, 10, true, false, Some(is_angry_at_selector))));
        target_selector.add_goal(3, Goal::ResetUniversalAngerTarget(ResetUniversalAngerTargetGoal::new(true)));
    }

    pub fn get_default_dimensions(mob: &Mob) -> EntityDimensions {
        if mob.is_baby() { BABY_DIMENSIONS } else { DIMENSIONS }
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.zombified_piglin.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.zombified_piglin.death"
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        zombified_piglin(mob).add_persistent_anger_save_data(&mob.entity, out);
    }
}


impl ServerLevel {
    pub fn zombified_piglin_custom_server_ai_step(&mut self, id: EntityId) {
        let is_angry = self.is_angry(id);
        let mob = self.mob_mut(id).expect("a zombified piglin");
        let is_baby = mob.is_baby();
        let speed = mob.living.attributes.instance(Attribute::MovementSpeed).expect("zombified piglins have a movement speed");
        if is_angry {
            if !is_baby && !speed.has_modifier(SPEED_MODIFIER_ATTACKING_ID) {
                speed.add_transient_modifier(SPEED_MODIFIER_ATTACKING);
            }
            self.maybe_play_first_anger_sound(id);
        } else if speed.has_modifier(SPEED_MODIFIER_ATTACKING_ID) {
            speed.remove_modifier(SPEED_MODIFIER_ATTACKING_ID);
        }
        self.update_persistent_anger(id, true);
        if self.get_target(id).is_some() {
            self.maybe_alert_others(id);
        }
    }

    fn maybe_play_first_anger_sound(&mut self, id: EntityId) {
        let zombified_piglin = zombified_piglin_mut(self.mob_mut(id).expect("a zombified piglin"));
        if zombified_piglin.play_first_anger_sound_in > 0 {
            zombified_piglin.play_first_anger_sound_in -= 1;
            if zombified_piglin.play_first_anger_sound_in == 0 {
                self.play_anger_sound(id);
            }
        }
    }

    fn maybe_alert_others(&mut self, id: EntityId) {
        let zombified_piglin = zombified_piglin_mut(self.mob_mut(id).expect("a zombified piglin"));
        if zombified_piglin.ticks_until_next_alert > 0 {
            zombified_piglin.ticks_until_next_alert -= 1;
        } else {
            let target = self.get_target(id).expect("a target");
            if self.sensing_has_line_of_sight(id, target) {
                self.alert_others(id);
            }
            let mob = self.mob_mut(id).expect("a zombified piglin");
            let ticks_until_next_alert = ALERT_INTERVAL.sample(&mut mob.entity.random);
            zombified_piglin_mut(mob).ticks_until_next_alert = ticks_until_next_alert;
        }
    }

    fn alert_others(&mut self, id: EntityId) {
        let within = self.attribute_value(id, Attribute::FollowRange);
        let search_aabb = AABB::unit_cube_from_lower_corner(self.entity(id).position()).inflate(within, ALERT_RANGE_Y, within);
        let others: Vec<EntityId> = self
            .get_entities(None, &search_aabb)
            .into_iter()
            .filter(|&other| matches!(self.mob(other).map(|mob| &mob.kind), Some(MobKind::ZombifiedPiglin(_))))
            .collect();
        for other in others {
            if other == id || self.get_target(other).is_some() {
                continue;
            }
            let target = self.get_target(id);
            if target.is_some_and(|target| self.is_allied_to(other, target)) {
                continue;
            }
            self.set_target(other, target);
        }
    }

    fn play_anger_sound(&mut self, id: EntityId) {
        let volume = self.mob(id).expect("a zombified piglin").get_sound_volume() * 2.0;
        let pitch = self.voice_pitch(id) * 1.8;
        self.entity_play_sound(id, sound_event("minecraft:entity.zombified_piglin.angry"), volume, pitch);
    }

    pub fn zombified_piglin_set_target(&mut self, id: EntityId, target: Option<EntityId>) {
        if self.get_target(id).is_none() && target.is_some() {
            let mob = self.mob_mut(id).expect("a zombified piglin");
            let play_first_anger_sound_in = FIRST_ANGER_SOUND_DELAY.sample(&mut mob.entity.random);
            let ticks_until_next_alert = ALERT_INTERVAL.sample(&mut mob.entity.random);
            let zombified_piglin = zombified_piglin_mut(mob);
            zombified_piglin.play_first_anger_sound_in = play_first_anger_sound_in;
            zombified_piglin.ticks_until_next_alert = ticks_until_next_alert;
        }
        self.mob_set_target(id, target);
    }

    pub fn zombified_piglin_start_persistent_anger_timer(&mut self, id: EntityId) {
        let remaining_time = PERSISTENT_ANGER_TIME.sample(&mut self.entity_mut(id).random);
        self.set_time_to_remain_angry(id, remaining_time as i64);
    }

    pub fn zombified_piglin_get_ambient_sound(&self, id: EntityId) -> &'static str {
        if self.is_angry(id) { "minecraft:entity.zombified_piglin.angry" } else { "minecraft:entity.zombified_piglin.ambient" }
    }

    pub fn zombified_piglin_populate_default_equipment_slots(&mut self, id: EntityId, random: &mut impl RandomSource) {
        let item = if random.next_int_bounded(20) == 0 { items::GOLDEN_SPEAR } else { items::GOLDEN_SWORD };
        self.living_set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::new(item, 1));
    }

    pub fn zombified_piglin_randomize_reinforcements_chance(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a zombified piglin");
        mob.living.attributes.instance(Attribute::SpawnReinforcements).expect("zombies have a reinforcement chance").set_base_value(0.0);
    }

    pub fn zombified_piglin_is_preventing_player_rest(&self, id: EntityId, player: EntityId) -> bool {
        self.is_angry_at(id, player)
    }

    pub fn zombified_piglin_wants_to_pick_up(&self, id: EntityId, item_stack: &ItemStack) -> bool {
        self.zombie_can_hold_item(id, item_stack)
    }
}
