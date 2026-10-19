use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Direction, Vec3};
use wafer_data::{damage_type_tags, game_rule, registry, registry_id};

use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::climb_on_top_of_powder_snow_goal::ClimbOnTopOfPowderSnowGoal;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::goal::{Goal, reduced_tick_delay};
use crate::world::entity::ai::goal::melee_attack_goal::MeleeAttackGoal;
use crate::world::entity::ai::goal::random_stroll_goal::RandomStrollGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::attributes::SILVERFISH_ATTRIBUTES;
use crate::world::entity::data::mob_defaults;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::monster::monster;
use crate::world::entity::{Entity, EntityDimensions, EntityId};
use crate::world::level::BlockGetter;
use crate::world::level::block::infested_block;
use crate::world::level::path_navigation_region::PathNavigationRegion;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::update_flags::UPDATE_ALL;

pub const DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(0.4, 0.3).with_eye_height(0.13).with_passenger_attachments(&[Vec3::new(0.0, 0.2375f32 as f64, 0.0)]);

#[derive(Default)]
pub struct Silverfish {
    pub friends_goal: SilverfishWakeUpFriendsGoal,
}

fn friends_goal(mob: &Mob) -> &SilverfishWakeUpFriendsGoal {
    match &mob.kind {
        MobKind::Silverfish(silverfish) => &silverfish.friends_goal,
        _ => unreachable!("not a silverfish"),
    }
}

fn friends_goal_mut(mob: &mut Mob) -> &mut SilverfishWakeUpFriendsGoal {
    match &mut mob.kind {
        MobKind::Silverfish(silverfish) => &mut silverfish.friends_goal,
        _ => unreachable!("not a silverfish"),
    }
}

impl Silverfish {
    pub fn create() -> Mob {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:silverfish"), DIMENSIONS, mob_defaults());
        let mut mob = Mob::new(entity, LivingEntity::new(&SILVERFISH_ATTRIBUTES), MobKind::Silverfish(Silverfish::default()));
        mob.xp_reward = monster::XP_REWARD;
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(1, Goal::Float(float_goal));
        goal_selector.add_goal(1, Goal::ClimbOnTopOfPowderSnow(ClimbOnTopOfPowderSnowGoal));
        goal_selector.add_goal(3, Goal::SilverfishWakeUpFriends);
        goal_selector.add_goal(4, Goal::MeleeAttack(MeleeAttackGoal::new(1.0, false)));
        goal_selector.add_goal(5, Goal::SilverfishMergeWithStone(SilverfishMergeWithStoneGoal::new()));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::HurtByTarget(HurtByTargetGoal::new(&[]).set_alert_others(&[])));
        target_selector.add_goal(2, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::Player, true)));
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.silverfish.ambient"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.silverfish.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.silverfish.death"
    }

    pub fn get_step_sound() -> &'static str {
        "minecraft:entity.silverfish.step"
    }
}

pub fn get_walk_target_value(pos: BlockPos, level: &mut (impl ServerLevelAccessor + ?Sized)) -> f32 {
    if infested_block::is_compatible_host_block(level.block_state(pos.below())) { 10.0 } else { monster::get_walk_target_value(pos, level) }
}

impl ServerLevel {
    pub fn silverfish_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        if self.is_invulnerable_to(id, &source) {
            return false;
        }
        if source.causing_entity.is_some() || source.is(&damage_type_tags::ALWAYS_TRIGGERS_SILVERFISH) {
            friends_goal_mut(self.mob_mut(id).expect("a silverfish")).notify_hurt();
        }
        self.living_entity_hurt_server(id, source, damage)
    }

    pub fn silverfish_tick(&mut self, id: EntityId) {
        let y_rot = self.entity(id).y_rot;
        self.living_mut(id).y_body_rot = y_rot;
        self.mob_tick(id);
    }

    pub fn silverfish_set_y_body_rot(&mut self, id: EntityId, y_body_rot: f32) {
        self.entity_mut(id).set_y_rot(y_body_rot);
        self.living_entity_set_y_body_rot(id, y_body_rot);
    }

    pub fn silverfish_get_walk_target_value(&self, pos: BlockPos) -> f32 {
        if infested_block::is_compatible_host_block(PathNavigationRegion::new(self).get_block_state(pos.below())) {
            10.0
        } else {
            self.monster_get_walk_target_value(pos)
        }
    }
}

pub struct SilverfishMergeWithStoneGoal {
    pub random_stroll_goal: RandomStrollGoal,
    selected_direction: Option<Direction>,
    do_merge: bool,
}

impl SilverfishMergeWithStoneGoal {
    fn new() -> SilverfishMergeWithStoneGoal {
        SilverfishMergeWithStoneGoal { random_stroll_goal: RandomStrollGoal::with_interval(1.0, 10), selected_direction: None, do_merge: false }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if level.get_target(mob).is_some() {
            return false;
        }
        if !level.mob(mob).expect("a silverfish").ai.navigation.is_done() {
            return false;
        }
        if level.game_rules.get_bool(game_rule::MOB_GRIEFING) && level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(10)) == 0 {
            let selected_direction = Direction::get_random(&mut level.entity_mut(mob).random);
            self.selected_direction = Some(selected_direction);
            let e = level.entity(mob);
            let pos = BlockPos::containing(e.x(), e.y() + 0.5, e.z()).relative(selected_direction);
            let block_state = level.block_state(pos);
            if infested_block::is_compatible_host_block(block_state) {
                self.do_merge = true;
                return true;
            }
        }
        self.do_merge = false;
        self.random_stroll_goal.can_use(level, mob, RandomStrollGoal::get_position)
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if self.do_merge { false } else { self.random_stroll_goal.can_continue_to_use(level, mob) }
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if !self.do_merge {
            self.random_stroll_goal.start(level, mob);
        } else {
            let e = level.entity(mob);
            let pos = BlockPos::containing(e.x(), e.y() + 0.5, e.z()).relative(self.selected_direction.expect("a selected direction"));
            let block_state = level.block_state(pos);
            if infested_block::is_compatible_host_block(block_state) {
                level.set_block(pos, infested_block::infested_state_by_host(block_state), UPDATE_ALL);
                level.spawn_anim(mob);
                level.discard(mob);
            }
        }
    }
}

#[derive(Default)]
pub struct SilverfishWakeUpFriendsGoal {
    look_for_friends: i32,
}

impl SilverfishWakeUpFriendsGoal {
    pub fn notify_hurt(&mut self) {
        if self.look_for_friends == 0 {
            self.look_for_friends = Goal::SilverfishWakeUpFriends.adjusted_tick_delay(20);
        }
    }

    pub fn can_use(level: &ServerLevel, silverfish: EntityId) -> bool {
        friends_goal(level.mob(silverfish).expect("a silverfish")).look_for_friends > 0
    }

    pub fn tick(level: &mut ServerLevel, silverfish: EntityId) {
        let friends_goal = friends_goal_mut(level.mob_mut(silverfish).expect("a silverfish"));
        friends_goal.look_for_friends -= 1;
        if friends_goal.look_for_friends > 0 {
            return;
        }
        let base_pos = level.entity(silverfish).block_position();
        let mut y_off = 0;
        while y_off <= 5 && y_off >= -5 {
            let mut x_off = 0;
            while x_off <= 10 && x_off >= -10 {
                let mut z_off = 0;
                while z_off <= 10 && z_off >= -10 {
                    let test_pos = base_pos.offset(x_off, y_off, z_off);
                    let block_state = level.block_state(test_pos);
                    if infested_block::is_infested_block(block_state) {
                        if level.game_rules.get_bool(game_rule::MOB_GRIEFING) {
                            level.destroy_block(test_pos, true, Some(silverfish), 512);
                        } else {
                            let host_state = infested_block::host_state_by_infested(level.block_state(test_pos));
                            level.set_block(test_pos, host_state, UPDATE_ALL);
                        }
                        if level.entity_mut(silverfish).random.next_boolean() {
                            return;
                        }
                    }
                    z_off = if z_off <= 0 { 1 } else { 0 } - z_off;
                }
                x_off = if x_off <= 0 { 1 } else { 0 } - x_off;
            }
            y_off = if y_off <= 0 { 1 } else { 0 } - y_off;
        }
    }
}
