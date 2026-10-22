use wafer_core::random::RandomSource;
use wafer_core::{Axis, BlockPos, Direction, Vec3, mth};
use wafer_data::{game_rule, registry, registry_id};

use super::spellcaster_illager::{IllagerSpell, SpellcasterCastingSpellGoal, SpellcasterIllager, SpellcasterUseSpellGoal};
use crate::server::level::ServerLevel;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::avoid_entity_goal::AvoidEntityGoal;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::random_stroll_goal::RandomStrollGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::animal::sheep::sheep;
use crate::world::entity::attributes::EVOKER_ATTRIBUTES;
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::monster::vex::Vex;
use crate::world::entity::projectile::evoker_fangs::EvokerFangs;
use crate::world::entity::raid::raider::RAIDER_TYPES;
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity};
use crate::world::item::dye_color::DyeColor;
use crate::world::level::BlockGetter;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::storage::format_uuid;

const XP_REWARD: i32 = 10;
const BLUE: DyeColor = DyeColor::VALUES[11];
const RED: DyeColor = DyeColor::VALUES[14];
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.6, 1.95)
    .with_passenger_attachments(&[Vec3::new(0.0, 2.0, 0.0)])
    .with_vehicle_attachment(&[Vec3::new(0.0, 0.6f32 as f64, 0.0)]);

#[derive(Default)]
pub struct Evoker {
    pub spellcaster_illager: SpellcasterIllager,
    wololo_target: Option<EntityId>,
}

fn evoker(mob: &Mob) -> &Evoker {
    match &mob.kind {
        MobKind::Evoker(evoker) => evoker,
        _ => unreachable!("an evoker"),
    }
}

fn evoker_mut(mob: &mut Mob) -> &mut Evoker {
    match &mut mob.kind {
        MobKind::Evoker(evoker) => evoker,
        _ => unreachable!("an evoker"),
    }
}

impl Evoker {
    pub fn create() -> Mob {
        let mut data = mob_defaults();
        data.push(DataValue::Boolean(false));
        data.push(DataValue::Byte(0));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:evoker"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&EVOKER_ATTRIBUTES), MobKind::Evoker(Evoker::default()));
        mob.xp_reward = XP_REWARD;
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        crate::world::entity::raid::raider::Raider::register_goals(ai);
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::Float(float_goal));
        goal_selector.add_goal(1, Goal::EvokerCastingSpell(EvokerCastingSpellGoal { casting_spell_goal: SpellcasterCastingSpellGoal }));
        goal_selector.add_goal(2, Goal::AvoidEntity(AvoidEntityGoal::new("minecraft:player", 8.0, 0.6, 1.0)));
        goal_selector.add_goal(3, Goal::AvoidEntity(AvoidEntityGoal::new("minecraft:creaking", 8.0, 0.6, 1.0)));
        goal_selector.add_goal(4, Goal::EvokerSummonSpell(EvokerSummonSpellGoal::default()));
        goal_selector.add_goal(5, Goal::EvokerAttackSpell(EvokerAttackSpellGoal::default()));
        goal_selector.add_goal(6, Goal::EvokerWololoSpell(EvokerWololoSpellGoal::default()));
        goal_selector.add_goal(8, Goal::RandomStroll(RandomStrollGoal::new(0.6)));
        goal_selector.add_goal(9, Goal::LookAtPlayer(LookAtPlayerGoal::new(3.0).with_probability(1.0)));
        goal_selector.add_goal(10, Goal::LookAtPlayer(LookAtPlayerGoal::of_type(TargetType::Mob, 8.0)));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::HurtByTarget(HurtByTargetGoal::new(RAIDER_TYPES).set_alert_others(&[])));
        let mut player = NearestAttackableTargetGoal::new(TargetType::Player, true);
        player.target_goal.set_unseen_memory_ticks(300);
        target_selector.add_goal(2, Goal::NearestAttackableTarget(player));
        let mut villager = NearestAttackableTargetGoal::new(TargetType::AbstractVillager, false);
        villager.target_goal.set_unseen_memory_ticks(300);
        target_selector.add_goal(3, Goal::NearestAttackableTarget(villager));
        target_selector.add_goal(3, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::IronGolem, false)));
    }

    pub fn get_celebrate_sound() -> &'static str {
        "minecraft:entity.evoker.celebrate"
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.evoker.ambient"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.evoker.death"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.evoker.hurt"
    }

    pub fn get_casting_sound_event() -> &'static str {
        "minecraft:entity.evoker.cast_spell"
    }

    fn get_wololo_target(mob: &Mob) -> Option<EntityId> {
        evoker(mob).wololo_target
    }

    fn set_wololo_target(mob: &mut Mob, wololo_target: Option<EntityId>) {
        evoker_mut(mob).wololo_target = wololo_target;
    }
}

impl ServerLevel {
    pub fn evoker_considers_entity_as_ally(&self, id: EntityId, other: EntityId) -> bool {
        if other == id {
            return true;
        }
        if self.abstract_illager_considers_entity_as_ally(id, other) {
            return true;
        }
        if self.is_vex(other)
            && let Some(root_owner) = self.get_root_owner(other)
            && (root_owner == id || self.abstract_illager_considers_entity_as_ally(id, root_owner))
        {
            return true;
        }
        false
    }
}

pub struct EvokerCastingSpellGoal {
    pub casting_spell_goal: SpellcasterCastingSpellGoal,
}

impl EvokerCastingSpellGoal {
    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let m = level.mob(mob).expect("an evoker");
        let (y_max, x_max) = (m.max_head_y_rot() as f32, m.get_max_head_x_rot() as f32);
        if let Some(target) = level.get_target(mob) {
            level.set_look_at_entity(mob, target, y_max, x_max);
        } else if let Some(wololo_target) = Evoker::get_wololo_target(m).filter(|&wololo_target| level.entity_ref(wololo_target).is_some()) {
            level.set_look_at_entity(mob, wololo_target, y_max, x_max);
        }
    }
}

#[derive(Default)]
pub struct EvokerAttackSpellGoal {
    pub use_spell_goal: SpellcasterUseSpellGoal,
}

impl EvokerAttackSpellGoal {
    const CASTING_TIME: i32 = 40;
    const CASTING_INTERVAL: i32 = 100;

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.use_spell_goal.start(level, mob, 20, Self::CASTING_TIME, Self::CASTING_INTERVAL, Some("minecraft:entity.evoker.prepare_attack"), IllagerSpell::Fangs);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if self.use_spell_goal.tick_warmup() {
            self.perform_spell_casting(level, mob);
            SpellcasterUseSpellGoal::play_casting_sound(level, mob);
        }
    }

    fn perform_spell_casting(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let Some(target) = level.get_target(mob) else { return };
        let (t, e) = (level.entity(target), level.entity(mob));
        let min_y = t.y().min(e.y());
        let max_y = t.y().max(e.y()) + 1.0;
        let angle_towards_target = mth::atan2(t.z() - e.z(), t.x() - e.x()) as f32;
        let (x, z) = (e.x(), e.z());
        let distance_to_sqr = e.distance_to_sqr_xyz(t.x(), t.y(), t.z());
        let pi = std::f64::consts::PI as f32;
        if distance_to_sqr < 9.0 {
            for i in 0..5 {
                let angle = angle_towards_target + i as f32 * pi * 0.4;
                self.create_spell_entity(level, mob, x + mth::cos(angle as f64) as f64 * 1.5, z + mth::sin(angle as f64) as f64 * 1.5, min_y, max_y, angle, 0);
            }
            for i in 0..8 {
                let angle = angle_towards_target + i as f32 * pi * 2.0 / 8.0 + (std::f64::consts::PI * 2.0 / 5.0) as f32;
                self.create_spell_entity(level, mob, x + mth::cos(angle as f64) as f64 * 2.5, z + mth::sin(angle as f64) as f64 * 2.5, min_y, max_y, angle, 3);
            }
        } else {
            for i in 0..16 {
                let reach = 1.25 * (i + 1) as f64;
                let spell_speed = i;
                self.create_spell_entity(
                    level,
                    mob,
                    x + mth::cos(angle_towards_target as f64) as f64 * reach,
                    z + mth::sin(angle_towards_target as f64) as f64 * reach,
                    min_y,
                    max_y,
                    angle_towards_target,
                    spell_speed,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn create_spell_entity(&mut self, level: &mut ServerLevel, mob: EntityId, x: f64, z: f64, min_y: f64, max_y: f64, angle: f32, delay_ticks: i32) {
        let mut pos = BlockPos::containing(x, max_y, z);
        let mut success = false;
        let mut top_offset = 0.0;
        loop {
            let below = pos.below();
            let below_state = level.block_state(below);
            if below_state.is_face_sturdy_full(Direction::Up) {
                if !level.block_state(pos).is_air() {
                    let block_state = level.block_state(pos);
                    let shape = block_state.get_collision_shape(pos);
                    if !shape.is_empty() {
                        top_offset = shape.max(Axis::Y);
                    }
                }
                success = true;
                break;
            }
            pos = pos.below();
            if pos.y < mth::floor(min_y) - 1 {
                break;
            }
        }
        if success {
            let owner = level.entity(mob).uuid;
            level.add_fresh_entity(LevelEntity::EvokerFangs(EvokerFangs::new(x, pos.y as f64 + top_offset, z, angle, delay_ticks, owner)));
            level.game_event(crate::world::level::gameevent::game_event::ENTITY_PLACE, Vec3::new(x, pos.y as f64 + top_offset, z), crate::world::level::gameevent::game_event::Context::of(Some(mob)));
        }
    }
}

#[derive(Default)]
pub struct EvokerSummonSpellGoal {
    pub use_spell_goal: SpellcasterUseSpellGoal,
}

impl EvokerSummonSpellGoal {
    const CASTING_TIME: i32 = 100;
    const CASTING_INTERVAL: i32 = 340;

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if !self.use_spell_goal.can_use(level, mob) {
            return false;
        }
        let vex_count_targeting = TargetingConditions::for_non_combat().range(16.0).ignore_line_of_sight().ignore_invisibility_testing();
        let bb = level.entity(mob).bounding_box().inflate_all(16.0);
        let nearby = level.get_living_entities(&bb, |level, entity| level.is_vex(entity));
        let vexes = nearby.into_iter().filter(|&vex| vex_count_targeting.test(level, Some(mob), vex)).count() as i32;
        level.entity_mut(mob).random.next_int_bounded(8) + 1 > vexes
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.use_spell_goal.start(level, mob, 20, Self::CASTING_TIME, Self::CASTING_INTERVAL, Some("minecraft:entity.evoker.prepare_summon"), IllagerSpell::SummonVex);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if self.use_spell_goal.tick_warmup() {
            self.perform_spell_casting(level, mob);
            SpellcasterUseSpellGoal::play_casting_sound(level, mob);
        }
    }

    fn perform_spell_casting(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let evoker_team = level.team_of(mob).map(|team| team.name().to_string());
        for _ in 0..3 {
            let random = &mut level.entity_mut(mob).random;
            let dx = -2 + random.next_int_bounded(5);
            let dz = -2 + random.next_int_bounded(5);
            let pos = level.entity(mob).block_position().offset(dx, 1, dz);
            let mut vex = Vex::create();
            vex.entity.snap_to(pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5, 0.0, 0.0);
            let difficulty = level.current_difficulty_at(pos);
            let (vex, _) = level.finalize_spawn_detached(LevelEntity::Mob(Box::new(vex)), &difficulty, EntitySpawnReason::MobSummoned, None);
            let LevelEntity::Mob(mut vex) = vex else { unreachable!("a mob") };
            Vex::set_owner(&mut vex, level.entity(mob).uuid);
            Vex::set_bound_origin(&mut vex, Some(pos));
            let life = 20 * (30 + level.entity_mut(mob).random.next_int_bounded(90));
            Vex::set_limited_life_ticks(&mut vex, life);
            if let Some(evoker_team) = &evoker_team {
                let name = format_uuid(vex.entity.uuid);
                level.scoreboard.borrow_mut().add_player_to_team(&name, evoker_team);
            }
            level.add_fresh_entity_with_passengers(LevelEntity::Mob(vex));
            level.game_event_at_with(crate::world::level::gameevent::game_event::ENTITY_PLACE, pos, crate::world::level::gameevent::game_event::Context::of(Some(mob)));
        }
    }
}

#[derive(Default)]
pub struct EvokerWololoSpellGoal {
    pub use_spell_goal: SpellcasterUseSpellGoal,
}

fn is_blue_sheep(target: EntityId, level: &ServerLevel) -> bool {
    sheep::get_color(level.entity(target)) == BLUE
}

impl EvokerWololoSpellGoal {
    const CAST_WARMUP_TIME: i32 = 40;
    const CASTING_TIME: i32 = 60;
    const CASTING_INTERVAL: i32 = 140;

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if level.get_target(mob).is_some() {
            return false;
        }
        let m = level.mob(mob).expect("an evoker");
        if evoker(m).spellcaster_illager.is_casting_spell() {
            return false;
        }
        if m.entity.tick_count < self.use_spell_goal.next_attack_tick_count {
            return false;
        }
        if !level.game_rules.get_bool(game_rule::MOB_GRIEFING) {
            return false;
        }
        let wololo_targeting = TargetingConditions::for_non_combat().range(16.0).selector(&is_blue_sheep);
        let bb = level.entity(mob).bounding_box().inflate(16.0, 4.0, 16.0);
        let nearby = level.get_living_entities(&bb, |level, entity| level.is_sheep(entity));
        let entities: Vec<EntityId> = nearby.into_iter().filter(|&sheep| wololo_targeting.test(level, Some(mob), sheep)).collect();
        if entities.is_empty() {
            return false;
        }
        let index = level.entity_mut(mob).random.next_int_bounded(entities.len() as i32) as usize;
        Evoker::set_wololo_target(level.mob_mut(mob).expect("an evoker"), Some(entities[index]));
        true
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        Evoker::get_wololo_target(level.mob(mob).expect("an evoker")).is_some() && self.use_spell_goal.attack_warmup_delay > 0
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.use_spell_goal.start(level, mob, Self::CAST_WARMUP_TIME, Self::CASTING_TIME, Self::CASTING_INTERVAL, Some("minecraft:entity.evoker.prepare_wololo"), IllagerSpell::Wololo);
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        Evoker::set_wololo_target(level.mob_mut(mob).expect("an evoker"), None);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if self.use_spell_goal.tick_warmup() {
            self.perform_spell_casting(level, mob);
            SpellcasterUseSpellGoal::play_casting_sound(level, mob);
        }
    }

    fn perform_spell_casting(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if let Some(wololo_target) = Evoker::get_wololo_target(level.mob(mob).expect("an evoker"))
            && level.entity_ref(wololo_target).is_some()
            && level.is_alive(wololo_target)
        {
            sheep::set_color(level.entity_mut(wololo_target), RED);
        }
    }
}
