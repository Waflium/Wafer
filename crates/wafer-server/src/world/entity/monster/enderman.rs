use std::borrow::Cow;

use wafer_core::random::RandomSource;
use wafer_core::{AABB, BlockPos, Vec3, mth};
use wafer_data::{BlockState, block_tags, blocks, damage_type_tags, fluid_tags, game_rule, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::server::level::events::PARTICLES_ENDERMAN_TELEPORT;
use crate::util::block_util;
use crate::util::time_util;
use crate::util::valueproviders::IntProvider;
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::goal::{Goal, reduced_tick_delay};
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::melee_attack_goal::MeleeAttackGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::goal::target::reset_universal_anger_target_goal::ResetUniversalAngerTargetGoal;
use crate::world::entity::ai::goal::target::target_goal::get_follow_distance;
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::attributes::{Attribute, AttributeModifier, ENDERMAN_ATTRIBUTES, Operation};
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::entity_reference::EntityReference;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::monster::monster;
use crate::world::entity::neutral_mob::NeutralMob;
use crate::world::entity::projectile::projectile::ProjectileKind;
use crate::world::entity::{Entity, EntityDimensions, EntityId, sound_event};
use crate::world::item::ItemStack;
use crate::world::item::alchemy::potion_contents::PotionContents;
use crate::world::item::component as dc;
use crate::world::item::enchantment::enchantment_helper;
use crate::world::item::enchantment::providers::vanilla_enchantment_providers;
use crate::world::level::BlockGetter;
use crate::world::level::block;
use crate::world::level::chunk::serialization::{state_from_nbt, state_to_nbt};
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};
use crate::world::phys::shapes::collision_context::CollisionContext;
use crate::world::level::pathfinder::path_type::PathType;
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::level::update_flags::UPDATE_ALL;

const SPEED_MODIFIER_ATTACKING_ID: &str = "minecraft:attacking";
const SPEED_MODIFIER_ATTACKING: AttributeModifier = AttributeModifier { id: Cow::Borrowed(SPEED_MODIFIER_ATTACKING_ID), amount: 0.15f32 as f64, operation: Operation::AddValue };
const MIN_DEAGGRESSION_TIME: i32 = 600;
pub const DATA_CARRY_STATE: usize = 16;
pub const DATA_CREEPY: usize = 17;
pub const DATA_STARED_AT: usize = 18;
const PERSISTENT_ANGER_TIME: IntProvider = time_util::range_of_seconds(20, 39);
pub const DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(0.6, 2.9).with_eye_height(2.55).with_passenger_attachments(&[Vec3::new(0.0, 2.80625f32 as f64, 0.0)]);

#[derive(Default)]
pub struct Enderman {
    target_change_time: i32,
    persistent_anger_end_time: i64,
    persistent_anger_target: Option<EntityReference>,
}

impl NeutralMob for Enderman {
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

pub fn enderman(mob: &Mob) -> &Enderman {
    match &mob.kind {
        MobKind::Enderman(enderman) => enderman,
        _ => unreachable!("not an enderman"),
    }
}

pub fn enderman_mut(mob: &mut Mob) -> &mut Enderman {
    match &mut mob.kind {
        MobKind::Enderman(enderman) => enderman,
        _ => unreachable!("not an enderman"),
    }
}

pub fn enderman_defaults() -> Vec<DataValue> {
    let mut data = mob_defaults();
    data.extend([DataValue::OptionalBlockState(None), DataValue::Boolean(false), DataValue::Boolean(false)]);
    data
}

pub fn get_carried_block(entity: &Entity) -> Option<BlockState> {
    match entity.data.get(DATA_CARRY_STATE) {
        DataValue::OptionalBlockState(state) => *state,
        _ => unreachable!("the carried block is an optional block state"),
    }
}

pub fn set_carried_block(entity: &mut Entity, carrying_block: Option<BlockState>) {
    entity.data.set(DATA_CARRY_STATE, DataValue::OptionalBlockState(carrying_block));
}

pub fn is_creepy(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_CREEPY), DataValue::Boolean(true))
}

pub fn set_being_stared_at(entity: &mut Entity) {
    entity.data.set(DATA_STARED_AT, DataValue::Boolean(true));
}

impl Enderman {
    pub fn create() -> Mob {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:enderman"), DIMENSIONS, enderman_defaults());
        let mut mob = Mob::new(entity, LivingEntity::new(&ENDERMAN_ATTRIBUTES), MobKind::Enderman(Enderman::default()));
        mob.xp_reward = monster::XP_REWARD;
        mob.set_pathfinding_malus(PathType::Water, -1.0);
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::Float(float_goal));
        goal_selector.add_goal(1, Goal::EndermanFreezeWhenLookedAt(EndermanFreezeWhenLookedAt::default()));
        goal_selector.add_goal(2, Goal::MeleeAttack(MeleeAttackGoal::new(1.0, false)));
        goal_selector.add_goal(7, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::with_probability(1.0, 0.0)));
        goal_selector.add_goal(8, Goal::LookAtPlayer(LookAtPlayerGoal::new(8.0)));
        goal_selector.add_goal(8, Goal::RandomLookAround(RandomLookAroundGoal::default()));
        goal_selector.add_goal(10, Goal::EndermanLeaveBlock(EndermanLeaveBlockGoal));
        goal_selector.add_goal(11, Goal::EndermanTakeBlock(EndermanTakeBlockGoal));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::EndermanLookForPlayer(EndermanLookForPlayerGoal::new()));
        target_selector.add_goal(2, Goal::HurtByTarget(HurtByTargetGoal::new(&[])));
        target_selector.add_goal(3, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::with(TargetType::Endermite, 10, true, false, None)));
        target_selector.add_goal(4, Goal::ResetUniversalAngerTarget(ResetUniversalAngerTargetGoal::new(false)));
    }

    pub fn get_walk_target_value() -> f32 {
        0.0
    }

    pub fn get_ambient_sound(mob: &Mob) -> &'static str {
        if is_creepy(&mob.entity) { "minecraft:entity.enderman.scream" } else { "minecraft:entity.enderman.ambient" }
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.enderman.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.enderman.death"
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        if let Some(block_state) = get_carried_block(&mob.entity) {
            out.push(("carriedBlockState".into(), state_to_nbt(block_state)));
        }
        enderman(mob).add_persistent_anger_save_data(&mob.entity, out);
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        let carried_block_state = input.get("carriedBlockState").and_then(state_from_nbt).filter(|block_state| !block_state.is_air());
        set_carried_block(&mut mob.entity, carried_block_state);
    }
}

#[derive(Default)]
pub struct EndermanFreezeWhenLookedAt {
    target: Option<EntityId>,
}

impl EndermanFreezeWhenLookedAt {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.target = level.get_target(mob);
        match self.target.filter(|&target| level.is_player(target)) {
            Some(player_target) => {
                let dist = level.entity(player_target).distance_to_sqr(level.entity(mob));
                if dist > 256.0 { false } else { level.is_being_stared_by(mob, player_target) }
            }
            None => false,
        }
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.get_navigation(mob, |navigation, _| navigation.stop());
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let Some(target) = self.target.filter(|&target| level.entity_ref(target).is_some()) else { return };
        let t = level.entity(target);
        let (x, y, z) = (t.x(), t.eye_y(), t.z());
        level.set_look_at(mob, x, y, z);
    }
}

pub struct EndermanLeaveBlockGoal;

impl EndermanLeaveBlockGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if get_carried_block(level.entity(mob)).is_none() {
            false
        } else if !level.game_rules.get_bool(game_rule::MOB_GRIEFING) {
            false
        } else {
            level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(2000)) == 0
        }
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let e = level.entity_mut(mob);
        let (x, y, z) = (e.x(), e.y(), e.z());
        let random = &mut e.random;
        let xt = mth::floor(x - 1.0 + random.next_double() * 2.0);
        let yt = mth::floor(y + random.next_double() * 2.0);
        let zt = mth::floor(z - 1.0 + random.next_double() * 2.0);
        let pos = BlockPos::new(xt, yt, zt);
        let target_state = level.block_state(pos);
        let below = pos.below();
        let below_state = level.block_state(below);
        if let Some(carried) = get_carried_block(level.entity(mob)) {
            let carried = level.update_from_neighbour_shapes(carried, pos);
            if self.can_place_block(level, mob, pos, carried, target_state, below_state) {
                level.set_block(pos, carried, UPDATE_ALL);
                level.game_event_at_with(crate::world::level::gameevent::game_event::BLOCK_PLACE, pos, crate::world::level::gameevent::game_event::Context::of_entity_and_state(Some(mob), Some(carried)));
                set_carried_block(level.entity_mut(mob), None);
            }
        }
    }

    fn can_place_block(&self, level: &mut ServerLevel, mob: EntityId, pos: BlockPos, carried: BlockState, target_state: BlockState, below_state: BlockState) -> bool {
        target_state.is_air()
            && !below_state.is_air()
            && !below_state.is_block(blocks::BEDROCK)
            && level.is_collision_shape_full_block_at(pos.below(), below_state)
            && block::of(carried).can_survive(carried, level, pos)
            && level.get_entities(Some(mob), &AABB::unit_cube_from_lower_corner(Vec3::at_lower_corner_of(pos))).is_empty()
    }
}

pub struct EndermanLookForPlayerGoal {
    pub nearest_attackable_target_goal: NearestAttackableTargetGoal,
    pending_target: Option<EntityId>,
    aggro_time: i32,
    teleport_time: i32,
}

const CONTINUE_AGGRO_TARGET_CONDITIONS: TargetingConditions<'static> = TargetingConditions::for_combat().ignore_line_of_sight();

fn is_anger_inducing(level: &mut ServerLevel, enderman: EntityId, target: EntityId) -> bool {
    level.entity_ref(target).is_some() && (level.is_being_stared_by(enderman, target) || level.is_angry_at(enderman, target)) && !level.has_indirect_passenger(enderman, target)
}

impl EndermanLookForPlayerGoal {
    pub fn new() -> EndermanLookForPlayerGoal {
        EndermanLookForPlayerGoal {
            nearest_attackable_target_goal: NearestAttackableTargetGoal::with(TargetType::Player, 10, false, false, None),
            pending_target: None,
            aggro_time: 0,
            teleport_time: 0,
        }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let players: Vec<EntityId> = level.players.keys().copied().collect();
        let anger_inducing: Vec<EntityId> = players.into_iter().filter(|&player| is_anger_inducing(level, mob, player)).collect();
        let is_anger_inducing = |target: EntityId, _: &ServerLevel| anger_inducing.contains(&target);
        let start_aggro_target_conditions = TargetingConditions::for_combat().range(get_follow_distance(level, mob)).selector(&is_anger_inducing);
        let e = level.entity(mob);
        let (x, y, z) = (e.x(), e.y(), e.z());
        self.pending_target = level.get_nearest_player(&start_aggro_target_conditions, Some(mob), x, y, z);
        self.pending_target.is_some()
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.aggro_time = reduced_tick_delay(5);
        self.teleport_time = 0;
        set_being_stared_at(level.entity_mut(mob));
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.pending_target = None;
        self.nearest_attackable_target_goal.target_goal.stop(level, mob);
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if let Some(pending_target) = self.pending_target {
            if !is_anger_inducing(level, mob, pending_target) {
                return false;
            }
            level.look_at(mob, pending_target, 10.0, 10.0);
            return true;
        }
        if let Some(target) = self.nearest_attackable_target_goal.target.filter(|&target| level.entity_ref(target).is_some()) {
            if level.has_indirect_passenger(mob, target) {
                return false;
            }
            if CONTINUE_AGGRO_TARGET_CONDITIONS.test(level, Some(mob), target) {
                return true;
            }
        }
        self.nearest_attackable_target_goal.target_goal.can_continue_to_use(level, mob)
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if level.get_target(mob).is_none() {
            self.nearest_attackable_target_goal.set_target(None);
        }
        if let Some(pending_target) = self.pending_target {
            self.aggro_time -= 1;
            if self.aggro_time <= 0 {
                self.nearest_attackable_target_goal.target = Some(pending_target);
                self.pending_target = None;
                self.nearest_attackable_target_goal.start(level, mob);
            }
        } else if let Some(target) = self.nearest_attackable_target_goal.target.filter(|&target| level.entity_ref(target).is_some())
            && !level.is_passenger(mob)
        {
            if level.is_being_stared_by(mob, target) {
                if level.entity(target).distance_to_sqr(level.entity(mob)) < 16.0 {
                    level.enderman_teleport(mob);
                }
                self.teleport_time = 0;
            } else if level.entity(target).distance_to_sqr(level.entity(mob)) > 256.0
                && {
                    let teleport_time = self.teleport_time;
                    self.teleport_time += 1;
                    teleport_time >= reduced_tick_delay(30)
                }
                && level.enderman_teleport_towards(mob, target)
            {
                self.teleport_time = 0;
            }
        }
    }
}

pub struct EndermanTakeBlockGoal;

impl EndermanTakeBlockGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if get_carried_block(level.entity(mob)).is_some() {
            false
        } else if !level.game_rules.get_bool(game_rule::MOB_GRIEFING) {
            false
        } else {
            level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(20)) == 0
        }
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let e = level.entity_mut(mob);
        let (x, y, z) = (e.x(), e.y(), e.z());
        let block_position = e.block_position();
        let random = &mut e.random;
        let xt = mth::floor(x - 2.0 + random.next_double() * 4.0);
        let yt = mth::floor(y + random.next_double() * 3.0);
        let zt = mth::floor(z - 2.0 + random.next_double() * 4.0);
        let pos = BlockPos::new(xt, yt, zt);
        let block_state = level.block_state(pos);
        let from = Vec3::new(block_position.x as f64 + 0.5, yt as f64 + 0.5, block_position.z as f64 + 0.5);
        let to = Vec3::new(xt as f64 + 0.5, yt as f64 + 0.5, zt as f64 + 0.5);
        let collision_context = CollisionContext::of(level, mob);
        let result = level.clip(&ClipContext { from, to, block: ClipBlock::Outline, fluid: ClipFluid::None, collision_context });
        let reachable = result.hit.block_pos == pos;
        if block_state.is(&block_tags::ENDERMAN_HOLDABLE) && reachable {
            level.remove_block(pos, false);
            level.game_event_at_with(crate::world::level::gameevent::game_event::BLOCK_DESTROY, pos, crate::world::level::gameevent::game_event::Context::of_entity_and_state(Some(mob), Some(block_state)));
            set_carried_block(level.entity_mut(mob), Some(block_state.block().default_state()));
        }
    }
}

impl ServerLevel {
    pub fn enderman_set_target(&mut self, id: EntityId, target: Option<EntityId>) {
        self.mob_set_target(id, target);
        let tick_count = self.entity(id).tick_count;
        let mob = self.mob_mut(id).expect("an enderman");
        match target {
            None => {
                enderman_mut(mob).target_change_time = 0;
                mob.entity.data.set(DATA_CREEPY, DataValue::Boolean(false));
                mob.entity.data.set(DATA_STARED_AT, DataValue::Boolean(false));
                let movement_speed = mob.living.attributes.instance(Attribute::MovementSpeed).expect("endermen have a movement speed");
                movement_speed.remove_modifier(SPEED_MODIFIER_ATTACKING_ID);
            }
            Some(_) => {
                enderman_mut(mob).target_change_time = tick_count;
                mob.entity.data.set(DATA_CREEPY, DataValue::Boolean(true));
                let movement_speed = mob.living.attributes.instance(Attribute::MovementSpeed).expect("endermen have a movement speed");
                if !movement_speed.has_modifier(SPEED_MODIFIER_ATTACKING_ID) {
                    movement_speed.add_transient_modifier(SPEED_MODIFIER_ATTACKING);
                }
            }
        }
    }

    pub fn enderman_start_persistent_anger_timer(&mut self, id: EntityId) {
        let remaining_time = PERSISTENT_ANGER_TIME.sample(&mut self.entity_mut(id).random);
        self.set_time_to_remain_angry(id, remaining_time as i64);
    }

    pub fn is_being_stared_by(&mut self, id: EntityId, player: EntityId) -> bool {
        if !self.player_not_wearing_disguise_item(player) {
            return false;
        }
        let eye_y = self.entity(id).eye_y();
        self.is_looking_at_me(id, player, 0.025, true, false, &[eye_y])
    }

    pub fn enderman_ai_step(&mut self, id: EntityId) {
        self.living_mut(id).jumping = false;
        self.update_persistent_anger(id, true);
        self.monster_ai_step(id);
    }

    pub fn enderman_custom_server_ai_step(&mut self, id: EntityId) {
        let tick_count = self.entity(id).tick_count;
        let target_change_time = enderman(self.mob(id).expect("an enderman")).target_change_time;
        if self.is_bright_outside() && tick_count >= target_change_time + MIN_DEAGGRESSION_TIME {
            let br = self.get_entity_light_level_dependent_magic_value(id);
            let block_position = self.entity(id).block_position();
            if br > 0.5 && self.can_see_sky(block_position) && self.entity_mut(id).random.next_float() * 30.0 < (br - 0.4) * 2.0 {
                self.set_target(id, None);
                self.enderman_teleport(id);
            }
        }
    }

    pub fn enderman_teleport(&mut self, id: EntityId) -> bool {
        if !self.is_alive(id) {
            return false;
        }
        let e = self.entity_mut(id);
        let (x, y, z) = (e.x(), e.y(), e.z());
        let random = &mut e.random;
        let xx = x + (random.next_double() - 0.5) * 64.0;
        let yy = y + (random.next_int_bounded(64) - 32) as f64;
        let zz = z + (random.next_double() - 0.5) * 64.0;
        self.enderman_teleport_xyz(id, xx, yy, zz)
    }

    pub fn enderman_teleport_towards(&mut self, id: EntityId, entity: EntityId) -> bool {
        let (e, other) = (self.entity(id), self.entity(entity));
        let dir = Vec3::new(e.x() - other.x(), e.y() + e.dimensions.height as f64 * 0.5 - other.eye_y(), e.z() - other.z()).normalize();
        let e = self.entity_mut(id);
        let (x, y, z) = (e.x(), e.y(), e.z());
        let random = &mut e.random;
        let xx = x + (random.next_double() - 0.5) * 8.0 - dir.x * 16.0;
        let yy = y + (random.next_int_bounded(16) - 8) as f64 - dir.y * 16.0;
        let zz = z + (random.next_double() - 0.5) * 8.0 - dir.z * 16.0;
        self.enderman_teleport_xyz(id, xx, yy, zz)
    }

    pub fn enderman_teleport_xyz(&mut self, id: EntityId, x: f64, y: f64, z: f64) -> bool {
        if self.is_passenger(id) {
            return false;
        }
        let old_pos = self.entity(id).position();
        let result = self.random_teleport(id, x, y, z, true, &|state| state.is(&block_tags::ENDERMAN_DOES_NOT_TELEPORT_TO));
        if result {
            self.game_event(crate::world::level::gameevent::game_event::TELEPORT, old_pos, crate::world::level::gameevent::game_event::Context::of(Some(id)));
            if !self.entity(id).is_silent() {
                let old_block_pos = BlockPos::containing_vec(old_pos);
                let packed_diff = block_util::clamped_pack_difference_in_position(old_block_pos, self.entity(id).block_position(), 127, 127, 127);
                self.level_event(None, PARTICLES_ENDERMAN_TELEPORT, old_block_pos, packed_diff);
                let (xo, source) = (self.entity(id).xo, self.sound_source(id));
                let sound = sound_event("minecraft:entity.enderman.teleport");
                self.play_sound_at(None, xo.x, xo.y, xo.z, sound, source, 1.0, 1.0);
                self.entity_play_sound(id, sound, 1.0, 1.0);
            }
        }
        result
    }

    pub fn enderman_can_randomly_teleport_to(&mut self, x: f64, y: f64, z: f64) -> bool {
        let pos_below = BlockPos::containing(x, y, z).below();
        let state_below = self.block_state(pos_below);
        !state_below.fluid_state().is(&fluid_tags::WATER)
    }

    fn enderman_repeatedly_try_to_teleport(&mut self, id: EntityId) {
        if !self.is_passenger(id) {
            for _ in 0..64 {
                if self.enderman_teleport(id) {
                    return;
                }
            }
        }
    }

    pub fn enderman_drop_custom_death_loot(&mut self, id: EntityId, source: &DamageSource, killed_by_player: bool) {
        self.mob_drop_custom_death_loot(id, source, killed_by_player);
        let Some(carrying_block) = get_carried_block(self.entity(id)) else { return };
        let mut fake_tool = ItemStack::new(items::DIAMOND_AXE, 1);
        let difficulty = self.current_difficulty_at(self.entity(id).block_position());
        enchantment_helper::enchant_item_from_provider(&mut fake_tool, vanilla_enchantment_providers::ENDERMAN_LOOT_DROP, &difficulty, &mut self.entity_mut(id).random);
        let position = self.entity(id).position();
        let mut params = LootParams::new(self);
        params.origin = Some(position);
        params.tool = Some(&fake_tool);
        params.this_entity = Some(id);
        let drops = block::of(carrying_block).get_drops(carrying_block, params);
        for item_stack in drops {
            self.spawn_at_location(id, item_stack, 0.0);
        }
    }

    pub fn enderman_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        if self.is_invulnerable_to(id, &source) {
            return false;
        }
        let thrown_potion = source
            .direct_entity
            .filter(|&entity| self.as_projectile(entity).is_some_and(|projectile| matches!(projectile.kind, ProjectileKind::ThrownSplashPotion(_) | ProjectileKind::ThrownLingeringPotion(_))));
        if source.is(&damage_type_tags::IS_PROJECTILE) && !self.is_passenger(id) {
            self.enderman_repeatedly_try_to_teleport(id);
            return false;
        }
        if let Some(thrown_potion) = thrown_potion {
            let hurt_with_clean_water = self.hurt_with_clean_water(id, source, thrown_potion, damage);
            self.enderman_repeatedly_try_to_teleport(id);
            return hurt_with_clean_water;
        }
        let causing_entity = source.causing_entity;
        let result = self.living_entity_hurt_server(id, source, damage);
        if !causing_entity.is_some_and(|entity| self.is_living(entity)) && self.entity_mut(id).random.next_int_bounded(10) != 0 {
            self.enderman_teleport(id);
        }
        result
    }

    fn hurt_with_clean_water(&mut self, id: EntityId, source: DamageSource, thrown_potion: EntityId, damage: f32) -> bool {
        let potion_item_stack = self.projectile(thrown_potion).get_item();
        let empty = Nbt::Compound(Vec::new());
        let potion_contents = PotionContents::new(potion_item_stack.get(dc::POTION_CONTENTS).unwrap_or(&empty));
        if potion_contents.is_tag("minecraft:hurts_water_sensitive_entities") { self.living_entity_hurt_server(id, source, damage) } else { false }
    }

    pub fn enderman_projectile_receives_side_effects_on_hit(&self, was_hurt: bool) -> bool {
        was_hurt
    }

    pub fn enderman_requires_custom_persistence(&self, id: EntityId) -> bool {
        self.mob_requires_custom_persistence(id) || get_carried_block(self.entity(id)).is_some()
    }
}
