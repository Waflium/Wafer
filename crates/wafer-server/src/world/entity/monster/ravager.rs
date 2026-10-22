use wafer_core::random::RandomSource;
use wafer_core::{AABB, BlockPos, Vec3, mth};
use wafer_data::{damage_types, game_rule, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::melee_attack_goal::MeleeAttackGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::attributes::{Attribute, RAVAGER_ATTRIBUTES};
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::raid::raider::{RAIDER_TYPES, Raider};
use crate::world::entity::{Entity, EntityDimensions, EntityId, sound_event};
use crate::world::level::BlockGetter;
use crate::world::level::pathfinder::path_type::PathType;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

const BASE_MOVEMENT_SPEED: f64 = 0.3;
const ATTACK_MOVEMENT_SPEED: f64 = 0.35;
pub const ATTACK_DURATION: i32 = 10;
pub const STUN_DURATION: i32 = 40;
const DEFAULT_ATTACK_TICK: i32 = 0;
const DEFAULT_STUN_TICK: i32 = 0;
const DEFAULT_ROAR_TICK: i32 = 0;
const XP_REWARD: i32 = 20;
const EVENT_ATTACK: u8 = 4;
const EVENT_STUNNED: u8 = 39;
const EVENT_ROAR: u8 = 69;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(1.95, 2.2).with_passenger_attachments(&[Vec3::new(0.0, 2.2625, -0.0625)]);

#[derive(Default)]
pub struct Ravager {
    pub raider: Raider,
    attack_tick: i32,
    stunned_tick: i32,
    roar_tick: i32,
}

fn ravager(mob: &Mob) -> &Ravager {
    match &mob.kind {
        MobKind::Ravager(ravager) => ravager,
        _ => unreachable!("a ravager"),
    }
}

fn ravager_mut(mob: &mut Mob) -> &mut Ravager {
    match &mut mob.kind {
        MobKind::Ravager(ravager) => ravager,
        _ => unreachable!("a ravager"),
    }
}

fn not_a_baby(_mob: EntityId, target: EntityId, level: &ServerLevel) -> bool {
    !level.is_baby(target)
}

impl Ravager {
    pub fn create() -> Mob {
        let mut data = mob_defaults();
        data.push(DataValue::Boolean(false));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:ravager"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&RAVAGER_ATTRIBUTES), MobKind::Ravager(Ravager::default()));
        mob.xp_reward = XP_REWARD;
        mob.set_pathfinding_malus(PathType::Leaves, 0.0);
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        Raider::register_goals(ai);
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::Float(float_goal));
        goal_selector.add_goal(4, Goal::MeleeAttack(MeleeAttackGoal::new(1.0, true)));
        goal_selector.add_goal(5, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::new(0.4)));
        goal_selector.add_goal(6, Goal::LookAtPlayer(LookAtPlayerGoal::new(6.0)));
        goal_selector.add_goal(10, Goal::LookAtPlayer(LookAtPlayerGoal::of_type(TargetType::Mob, 8.0)));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(2, Goal::HurtByTarget(HurtByTargetGoal::new(RAIDER_TYPES).set_alert_others(&[])));
        target_selector.add_goal(3, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::Player, true)));
        target_selector.add_goal(4, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::with(TargetType::AbstractVillager, 10, true, false, Some(not_a_baby))));
        target_selector.add_goal(4, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::IronGolem, true)));
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        let ravager = ravager(mob);
        ravager.raider.add_additional_save_data(out);
        out.push(("AttackTick".into(), Nbt::Int(ravager.attack_tick)));
        out.push(("StunTick".into(), Nbt::Int(ravager.stunned_tick)));
        out.push(("RoarTick".into(), Nbt::Int(ravager.roar_tick)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        let ravager = ravager_mut(mob);
        ravager.raider.read_additional_save_data(input);
        let int = |key: &str, default: i32| input.get(key).and_then(Nbt::as_i64).map_or(default, |v| v as i32);
        ravager.attack_tick = int("AttackTick", DEFAULT_ATTACK_TICK);
        ravager.stunned_tick = int("StunTick", DEFAULT_STUN_TICK);
        ravager.roar_tick = int("RoarTick", DEFAULT_ROAR_TICK);
    }

    pub fn get_celebrate_sound() -> &'static str {
        "minecraft:entity.ravager.celebrate"
    }

    pub fn get_max_head_y_rot() -> i32 {
        45
    }

    pub fn is_immobile(mob: &Mob) -> bool {
        let ravager = ravager(mob);
        ravager.attack_tick > 0 || ravager.stunned_tick > 0 || ravager.roar_tick > 0
    }

    pub fn has_line_of_sight(mob: &Mob) -> bool {
        let ravager = ravager(mob);
        ravager.stunned_tick <= 0 && ravager.roar_tick <= 0
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.ravager.ambient"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.ravager.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.ravager.death"
    }

    pub fn get_step_sound() -> &'static str {
        "minecraft:entity.ravager.step"
    }

    pub fn can_be_leader() -> bool {
        false
    }

    pub fn check_spawn_obstruction(mob: &Mob, level: &mut (impl ServerLevelAccessor + ?Sized)) -> bool {
        !level.contains_any_liquid(&mob.entity.bounding_box())
    }

    pub fn get_attack_bounding_box(default_bb: AABB) -> AABB {
        default_bb.deflate(0.05, 0.0, 0.05)
    }
}

impl ServerLevel {
    pub fn is_ravager(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Ravager(_)))
    }

    pub fn ravager_ai_step(&mut self, id: EntityId) {
        self.raider_ai_step(id);
        if !self.is_alive(id) {
            return;
        }
        let is_immobile = self.is_immobile(id);
        let has_target = self.get_target(id).is_some();
        let speed = self.mob_mut(id).expect("a ravager").living.attributes.instance(Attribute::MovementSpeed).expect("ravagers have a movement speed");
        if is_immobile {
            speed.set_base_value(0.0);
        } else {
            let max_speed = if has_target { ATTACK_MOVEMENT_SPEED } else { BASE_MOVEMENT_SPEED };
            let base_value = speed.base_value();
            speed.set_base_value(mth::lerp(0.1, base_value, max_speed));
        }
        if self.entity(id).horizontal_collision && self.game_rules.get_bool(game_rule::MOB_GRIEFING) {
            let mut destroyed_block = false;
            let bb = self.entity(id).bounding_box().inflate_all(0.2);
            let from = BlockPos::new(mth::floor(bb.min_x), mth::floor(bb.min_y), mth::floor(bb.min_z));
            let to = BlockPos::new(mth::floor(bb.max_x), mth::floor(bb.max_y), mth::floor(bb.max_z));
            for pos in BlockPos::between_closed(from, to) {
                let state = self.block_state(pos);
                if state.block().is_instance_of("LeavesBlock") {
                    destroyed_block = self.destroy_block(pos, true, Some(id), 512) || destroyed_block;
                }
            }
            if !destroyed_block && self.entity(id).on_ground {
                self.jump_from_ground(id);
            }
        }
        let ravager = ravager_mut(self.mob_mut(id).expect("a ravager"));
        if ravager.roar_tick > 0 {
            ravager.roar_tick -= 1;
            if ravager.roar_tick == 10 {
                self.roar(id);
            }
        }
        let ravager = ravager_mut(self.mob_mut(id).expect("a ravager"));
        if ravager.attack_tick > 0 {
            ravager.attack_tick -= 1;
        }
        if ravager.stunned_tick > 0 {
            ravager.stunned_tick -= 1;
            self.stun_effect(id);
            let ravager = ravager_mut(self.mob_mut(id).expect("a ravager"));
            if ravager.stunned_tick == 0 {
                self.entity_play_sound(id, sound_event("minecraft:entity.ravager.roar"), 1.0, 1.0);
                ravager_mut(self.mob_mut(id).expect("a ravager")).roar_tick = 20;
            }
        }
    }

    fn stun_effect(&mut self, id: EntityId) {
        let random = &mut self.entity_mut(id).random;
        if random.next_int_bounded(6) == 0 {
            random.next_double();
            random.next_double();
        }
    }

    pub fn ravager_blocked_by_item(&mut self, id: EntityId, defender: EntityId) {
        if ravager(self.mob(id).expect("a ravager")).roar_tick == 0 {
            if self.entity_mut(id).random.next_double() < 0.5 {
                ravager_mut(self.mob_mut(id).expect("a ravager")).stunned_tick = STUN_DURATION;
                self.entity_play_sound(id, sound_event("minecraft:entity.ravager.stunned"), 1.0, 1.0);
                self.broadcast_entity_event(id, EVENT_STUNNED);
                self.push_entity(defender, id);
            } else {
                self.strong_knockback(id, defender);
            }
            self.entity_mut(defender).sync_velocity = true;
        }
    }

    fn roar(&mut self, id: EntityId) {
        if !self.is_alive(id) {
            return;
        }
        let griefing = self.game_rules.get_bool(game_rule::MOB_GRIEFING);
        let bb = self.entity(id).bounding_box().inflate_all(4.0);
        let entities = self.get_living_entities(&bb, |level, entity| {
            !level.is_ravager(entity) && level.is_alive(entity) && (griefing || wafer_data::registry::ENTITY_TYPE[level.entity(entity).entity_type as usize] != "minecraft:armor_stand")
        });
        for entity in entities {
            if !self.is_abstract_illager(entity) {
                self.hurt_server(entity, DamageSource::entity(damage_types::MOB_ATTACK, id), 6.0);
            }
            if !self.is_player(entity) {
                self.strong_knockback(id, entity);
            }
        }
        self.broadcast_entity_event(id, EVENT_ROAR);
    }

    fn strong_knockback(&mut self, id: EntityId, entity: EntityId) {
        let (e, r) = (self.entity(entity), self.entity(id));
        let xd = e.x() - r.x();
        let zd = e.z() - r.z();
        let dd = (xd * xd + zd * zd).max(0.001);
        self.entity_push(entity, xd / dd * 4.0, 0.2, zd / dd * 4.0);
    }

    pub fn ravager_do_hurt_target(&mut self, id: EntityId, target: EntityId) -> bool {
        ravager_mut(self.mob_mut(id).expect("a ravager")).attack_tick = ATTACK_DURATION;
        self.broadcast_entity_event(id, EVENT_ATTACK);
        self.entity_play_sound(id, sound_event("minecraft:entity.ravager.attack"), 1.0, 1.0);
        self.mob_do_hurt_target(id, target)
    }
}
