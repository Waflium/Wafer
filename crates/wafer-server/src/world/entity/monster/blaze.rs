use wafer_core::Vec3;
use wafer_core::random::RandomSource;
use wafer_data::{registry, registry_id};

use crate::server::level::ServerLevel;
use crate::server::level::events::SOUND_BLAZE_FIREBALL;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::move_towards_restriction_goal::MoveTowardsRestrictionGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::attributes::{Attribute, BLAZE_ATTRIBUTES};
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::projectile::hurtingprojectile::small_fireball::SmallFireball;
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity};
use crate::world::level::pathfinder::path_type::PathType;

pub const DATA_FLAGS_ID: usize = 16;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.6, 1.8);
const XP_REWARD: i32 = 10;

pub struct Blaze {
    allowed_height_offset: f32,
    next_height_offset_change_tick: i32,
}

impl Default for Blaze {
    fn default() -> Blaze {
        Blaze { allowed_height_offset: 0.5, next_height_offset_change_tick: 0 }
    }
}

fn blaze_mut(mob: &mut Mob) -> &mut Blaze {
    match &mut mob.kind {
        MobKind::Blaze(blaze) => blaze,
        _ => unreachable!("not a blaze"),
    }
}

pub fn blaze_defaults() -> Vec<DataValue> {
    let mut data = mob_defaults();
    data.push(DataValue::Byte(0));
    data
}

pub fn is_charged(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_FLAGS_ID), DataValue::Byte(flags) if flags & 1 != 0)
}

fn set_charged(entity: &mut Entity, value: bool) {
    let DataValue::Byte(mut flags) = *entity.data.get(DATA_FLAGS_ID) else { unreachable!("the blaze flags are a byte") };
    if value {
        flags |= 1;
    } else {
        flags &= -2;
    }
    entity.data.set(DATA_FLAGS_ID, DataValue::Byte(flags));
}

impl Blaze {
    pub fn create() -> Mob {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:blaze"), DIMENSIONS, blaze_defaults());
        let mut mob = Mob::new(entity, LivingEntity::new(&BLAZE_ATTRIBUTES), MobKind::Blaze(Blaze::default()));
        mob.set_pathfinding_malus(PathType::Water, -1.0);
        mob.set_pathfinding_malus(PathType::Lava, 8.0);
        mob.set_pathfinding_malus(PathType::FireInNeighbor, 0.0);
        mob.set_pathfinding_malus(PathType::Fire, 0.0);
        mob.xp_reward = XP_REWARD;
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(4, Goal::BlazeAttack(BlazeAttackGoal::default()));
        goal_selector.add_goal(5, Goal::MoveTowardsRestriction(MoveTowardsRestrictionGoal::new(1.0)));
        goal_selector.add_goal(7, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::with_probability(1.0, 0.0)));
        goal_selector.add_goal(8, Goal::LookAtPlayer(LookAtPlayerGoal::new(8.0)));
        goal_selector.add_goal(8, Goal::RandomLookAround(RandomLookAroundGoal::default()));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::HurtByTarget(HurtByTargetGoal::new(&[]).set_alert_others(&[])));
        target_selector.add_goal(2, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::Player, true)));
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.blaze.ambient"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.blaze.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.blaze.death"
    }

    pub fn get_light_level_dependent_magic_value() -> f32 {
        1.0
    }

    pub fn is_on_fire(entity: &Entity) -> bool {
        is_charged(entity)
    }
}

impl ServerLevel {
    pub fn blaze_ai_step(&mut self, id: EntityId) {
        let e = self.entity_mut(id);
        if !e.on_ground && e.delta_movement.y < 0.0 {
            e.set_delta_movement(e.delta_movement.multiply(1.0, 0.6, 1.0));
        }
        self.monster_ai_step(id);
    }

    pub fn blaze_custom_server_ai_step(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a blaze");
        let blaze = blaze_mut(mob);
        blaze.next_height_offset_change_tick -= 1;
        if blaze.next_height_offset_change_tick <= 0 {
            blaze.next_height_offset_change_tick = 100;
            let allowed_height_offset = mob.entity.random.triangle(0.5, 6.891) as f32;
            blaze_mut(mob).allowed_height_offset = allowed_height_offset;
        }
        let allowed_height_offset = blaze_mut(self.mob_mut(id).expect("a blaze")).allowed_height_offset;
        if let Some(target) = self.get_target(id)
            && self.entity(target).eye_y() > self.entity(id).eye_y() + allowed_height_offset as f64
            && self.can_attack(id, target)
        {
            let e = self.entity_mut(id);
            let movement = e.delta_movement;
            e.set_delta_movement(e.delta_movement.add(0.0, (0.3f32 as f64 - movement.y) * 0.3f32 as f64, 0.0));
            e.needs_sync = true;
        }
    }
}

#[derive(Default)]
pub struct BlazeAttackGoal {
    attack_step: i32,
    attack_time: i32,
    last_seen: i32,
}

impl BlazeAttackGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        level.get_target(mob).is_some_and(|target| level.is_alive(target) && level.can_attack(mob, target))
    }

    pub fn start(&mut self) {
        self.attack_step = 0;
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        set_charged(level.entity_mut(mob), false);
        self.last_seen = 0;
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.attack_time -= 1;
        let Some(target) = level.get_target(mob) else { return };
        let has_line_of_sight = level.sensing_has_line_of_sight(mob, target);
        if has_line_of_sight {
            self.last_seen = 0;
        } else {
            self.last_seen += 1;
        }
        let distance = level.entity(mob).distance_to_sqr(level.entity(target));
        let follow_distance = level.attribute_value(mob, Attribute::FollowRange);
        if distance < 4.0 {
            if !has_line_of_sight {
                return;
            }
            if self.attack_time <= 0 {
                self.attack_time = 20;
                level.do_hurt_target(mob, target);
            }
            let t = level.entity(target);
            let (x, y, z) = (t.x(), t.y(), t.z());
            level.mob_mut(mob).expect("a blaze").ai.move_control.set_wanted_position(x, y, z, 1.0);
        } else if distance < follow_distance * follow_distance && has_line_of_sight {
            let (t, b) = (level.entity(target), level.entity(mob));
            let xd = t.x() - b.x();
            let yd = t.get_y(0.5) - b.get_y(0.5);
            let zd = t.z() - b.z();
            if self.attack_time <= 0 {
                self.attack_step += 1;
                if self.attack_step == 1 {
                    self.attack_time = 60;
                    set_charged(level.entity_mut(mob), true);
                } else if self.attack_step <= 4 {
                    self.attack_time = 6;
                } else {
                    self.attack_time = 100;
                    self.attack_step = 0;
                    set_charged(level.entity_mut(mob), false);
                }
                if self.attack_step > 1 {
                    let sqd = distance.sqrt().sqrt() * 0.5;
                    if !level.entity(mob).is_silent() {
                        let pos = level.entity(mob).block_position();
                        level.level_event(None, SOUND_BLAZE_FIREBALL, pos, 0);
                    }
                    let random = &mut level.entity_mut(mob).random;
                    let direction_x = random.triangle(xd, 2.297 * sqd);
                    let direction_z = random.triangle(zd, 2.297 * sqd);
                    let direction = Vec3::new(direction_x, yd, direction_z);
                    let mut entity = SmallFireball::from_mob(level, mob, direction.normalize());
                    let (x, z) = (entity.entity.x(), entity.entity.z());
                    entity.entity.set_pos(Vec3::new(x, level.entity(mob).get_y(0.5) + 0.5, z));
                    level.add_fresh_entity(LevelEntity::Projectile(Box::new(entity)));
                }
            }
            level.set_look_at_entity(mob, target, 10.0, 10.0);
        } else if self.last_seen < 5 {
            let t = level.entity(target);
            let (x, y, z) = (t.x(), t.y(), t.z());
            level.mob_mut(mob).expect("a blaze").ai.move_control.set_wanted_position(x, y, z, 1.0);
        }
    }
}
