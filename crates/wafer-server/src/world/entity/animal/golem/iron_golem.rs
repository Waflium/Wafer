use wafer_core::random::RandomSource;
use wafer_core::shapes::is_face_full;
use wafer_core::{BlockPos, Direction};
use wafer_data::{damage_types, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::abstract_golem;
use crate::server::level::ServerLevel;
use crate::util::time_util;
use crate::util::valueproviders::IntProvider;
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::golem_random_stroll_in_village_goal::GolemRandomStrollInVillageGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::melee_attack_goal::MeleeAttackGoal;
use crate::world::entity::ai::goal::move_back_to_village_goal::MoveBackToVillageGoal;
use crate::world::entity::ai::goal::move_towards_target_goal::MoveTowardsTargetGoal;
use crate::world::entity::ai::goal::offer_flower_goal::OfferFlowerGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::target::defend_village_target_goal::DefendVillageTargetGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::goal::target::reset_universal_anger_target_goal::ResetUniversalAngerTargetGoal;
use crate::world::entity::attributes::{Attribute, IRON_GOLEM_ATTRIBUTES};
use crate::world::entity::crackiness::{self, Level};
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::entity_event;
use crate::world::entity::entity_reference::EntityReference;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::neutral_mob::NeutralMob;
use crate::world::entity::{Entity, EntityDimensions, EntityId, entity_types, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::natural_spawner;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::phys::shapes::collision_context::CollisionContext;
use crate::world::{InteractionHand, InteractionResult};

pub const DATA_FLAGS_ID: usize = 16;
const IRON_INGOT_HEAL_AMOUNT: f32 = 25.0;
const DEFAULT_PLAYER_CREATED: bool = false;
const PERSISTENT_ANGER_TIME: IntProvider = time_util::range_of_seconds(20, 39);
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(1.4, 2.7);

#[derive(Default)]
pub struct IronGolem {
    attack_animation_tick: i32,
    offer_flower_tick: i32,
    persistent_anger_end_time: i64,
    persistent_anger_target: Option<EntityReference>,
}

impl NeutralMob for IronGolem {
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

pub fn iron_golem(mob: &Mob) -> &IronGolem {
    match &mob.kind {
        MobKind::IronGolem(iron_golem) => iron_golem,
        _ => unreachable!("not an iron golem"),
    }
}

pub fn iron_golem_mut(mob: &mut Mob) -> &mut IronGolem {
    match &mut mob.kind {
        MobKind::IronGolem(iron_golem) => iron_golem,
        _ => unreachable!("not an iron golem"),
    }
}

pub fn iron_golem_defaults() -> Vec<DataValue> {
    let mut data = mob_defaults();
    data.push(DataValue::Byte(0));
    data
}

fn flags(entity: &Entity) -> i8 {
    match entity.data.get(DATA_FLAGS_ID) {
        DataValue::Byte(flags) => *flags,
        _ => unreachable!("the iron golem flags are a byte"),
    }
}

pub fn is_player_created(entity: &Entity) -> bool {
    flags(entity) & 1 != 0
}

pub fn set_player_created(entity: &mut Entity, value: bool) {
    let current = flags(entity);
    entity.data.set(DATA_FLAGS_ID, DataValue::Byte(if value { current | 1 } else { current & -2 }));
}

fn is_creeper(level: &ServerLevel, entity: EntityId) -> bool {
    level.entity(entity).entity_type == registry_id(registry::ENTITY_TYPE, "minecraft:creeper")
}

fn is_enemy_but_not_creeper(_mob: EntityId, target: EntityId, level: &ServerLevel) -> bool {
    level.is_enemy(target) && !is_creeper(level, target)
}

impl IronGolem {
    pub fn create() -> Mob {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:iron_golem"), DIMENSIONS, iron_golem_defaults());
        Mob::new(entity, LivingEntity::new(&IRON_GOLEM_ATTRIBUTES), MobKind::IronGolem(IronGolem::default()))
    }

    pub fn register_goals(ai: &mut MobAi) {
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(1, Goal::MeleeAttack(MeleeAttackGoal::new(1.0, true)));
        goal_selector.add_goal(2, Goal::MoveTowardsTarget(MoveTowardsTargetGoal::new(0.9, 32.0)));
        goal_selector.add_goal(2, Goal::MoveBackToVillage(MoveBackToVillageGoal::new(0.6, false)));
        goal_selector.add_goal(4, Goal::GolemRandomStrollInVillage(GolemRandomStrollInVillageGoal::new(0.6)));
        goal_selector.add_goal(5, Goal::OfferFlower(OfferFlowerGoal::default()));
        goal_selector.add_goal(7, Goal::LookAtPlayer(LookAtPlayerGoal::new(6.0)));
        goal_selector.add_goal(8, Goal::RandomLookAround(RandomLookAroundGoal::default()));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::DefendVillageTarget(DefendVillageTargetGoal::new()));
        target_selector.add_goal(2, Goal::HurtByTarget(HurtByTargetGoal::new(&[])));
        target_selector.add_goal(
            3,
            Goal::NearestAttackableTarget(NearestAttackableTargetGoal::with(TargetType::Player, 10, true, false, Some(|mob, target, level| level.is_angry_at(mob, target)))),
        );
        target_selector.add_goal(3, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::with(TargetType::Mob, 5, false, false, Some(is_enemy_but_not_creeper))));
        target_selector.add_goal(4, Goal::ResetUniversalAngerTarget(ResetUniversalAngerTargetGoal::new(false)));
    }

    pub fn get_ambient_sound_interval() -> i32 {
        abstract_golem::get_ambient_sound_interval()
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.iron_golem.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.iron_golem.death"
    }

    pub fn get_crackiness(mob: &Mob) -> Level {
        crackiness::GOLEM.by_fraction(mob.health / mob.living.attributes.value(Attribute::MaxHealth) as f32)
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        out.push(("PlayerCreated".into(), Nbt::Byte(is_player_created(&mob.entity) as i8)));
        iron_golem(mob).add_persistent_anger_save_data(&mob.entity, out);
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        let player_created = input.get("PlayerCreated").and_then(Nbt::as_i64).map_or(DEFAULT_PLAYER_CREATED, |v| v as i8 != 0);
        set_player_created(&mut mob.entity, player_created);
    }

    pub fn check_spawn_obstruction(mob: &Mob, level: &mut (impl ServerLevelAccessor + ?Sized)) -> bool {
        let pos = mob.entity.block_position();
        let below_pos = pos.below();
        let below = level.block_state(below_pos);
        let collision_context = CollisionContext::of_detached(&mob.entity);
        let (shape, offset) = collision_context.get_collision_shape(below, below_pos);
        if !is_face_full(&shape.moved(offset[0], offset[1], offset[2]), Direction::Up) {
            return false;
        }
        let entity_type = &entity_types::IRON_GOLEM;
        for i in 1..3 {
            let above_pos = pos.offset(0, i, 0);
            let above = level.block_state(above_pos);
            if !natural_spawner::is_valid_empty_spawn_block(level, above_pos, above, above.fluid_state(), entity_type) {
                return false;
            }
        }
        let state = level.block_state(pos);
        natural_spawner::is_valid_empty_spawn_block(level, pos, state, wafer_data::FluidState::EMPTY, entity_type) && level.is_unobstructed(&mob.entity.bounding_box())
    }
}

impl ServerLevel {
    pub fn iron_golem_decrease_air_supply(&self, current_supply: i32) -> i32 {
        current_supply
    }

    pub fn iron_golem_do_push(&mut self, id: EntityId, entity: EntityId) {
        if self.is_enemy(entity) && !is_creeper(self, entity) && self.entity_mut(id).random.next_int_bounded(20) == 0 {
            self.set_target(id, Some(entity));
        }
        self.living_entity_do_push(id, entity);
    }

    pub fn iron_golem_ai_step(&mut self, id: EntityId) {
        self.mob_ai_step(id);
        let Some(mob) = self.mob_mut(id) else { return };
        let iron_golem = iron_golem_mut(mob);
        if iron_golem.attack_animation_tick > 0 {
            iron_golem.attack_animation_tick -= 1;
        }
        if iron_golem.offer_flower_tick > 0 {
            iron_golem.offer_flower_tick -= 1;
        }
        self.update_persistent_anger(id, true);
    }

    pub fn iron_golem_can_spawn_sprint_particle(&mut self, id: EntityId) -> bool {
        let e = self.entity_mut(id);
        e.delta_movement.horizontal_distance_sqr() > 2.5000003e-7f32 as f64 && e.random.next_int_bounded(5) == 0
    }

    pub fn iron_golem_can_attack(&self, id: EntityId, target: EntityId) -> bool {
        if is_player_created(self.entity(id)) && self.is_player(target) {
            false
        } else if is_creeper(self, target) {
            false
        } else {
            self.mob_can_attack(id, target)
        }
    }

    pub fn iron_golem_start_persistent_anger_timer(&mut self, id: EntityId) {
        let remaining_time = PERSISTENT_ANGER_TIME.sample(&mut self.entity_mut(id).random);
        self.set_time_to_remain_angry(id, remaining_time as i64);
    }

    fn iron_golem_get_attack_damage(&self, id: EntityId) -> f32 {
        self.attribute_value(id, Attribute::AttackDamage) as f32
    }

    pub fn iron_golem_do_hurt_target(&mut self, id: EntityId, target: EntityId) -> bool {
        iron_golem_mut(self.mob_mut(id).expect("an iron golem")).attack_animation_tick = 10;
        self.broadcast_entity_event(id, entity_event::START_ATTACKING);
        let attack_damage = self.iron_golem_get_attack_damage(id);
        let damage = if attack_damage as i32 > 0 { attack_damage / 2.0 + self.entity_mut(id).random.next_int_bounded(attack_damage as i32) as f32 } else { attack_damage };
        let damage_source = DamageSource { causing_entity: Some(id), direct_entity: Some(id), ..DamageSource::new(damage_types::MOB_ATTACK) };
        let hurt = self.hurt_server(target, damage_source, damage);
        if hurt {
            let knockback_resistance = if self.is_living(target) { self.attribute_value(target, Attribute::KnockbackResistance) } else { 0.0 };
            let scale = (1.0 - knockback_resistance).max(0.0);
            let e = self.entity_mut(target);
            let movement = e.delta_movement.add(0.0, 0.4f32 as f64 * scale, 0.0);
            e.set_delta_movement(movement);
            self.do_post_attack_effects(target, &damage_source);
        }
        self.entity_play_sound(id, sound_event("minecraft:entity.iron_golem.attack"), 1.0, 1.0);
        hurt
    }

    pub fn iron_golem_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        let previous_crackiness = IronGolem::get_crackiness(self.mob(id).expect("an iron golem"));
        let was_hurt = self.living_entity_hurt_server(id, source, damage);
        if was_hurt && self.mob(id).is_some_and(|mob| IronGolem::get_crackiness(mob) != previous_crackiness) {
            self.entity_play_sound(id, sound_event("minecraft:entity.iron_golem.damage"), 1.0, 1.0);
        }
        was_hurt
    }

    pub fn offer_flower(&mut self, id: EntityId, offer: bool) {
        let iron_golem = iron_golem_mut(self.mob_mut(id).expect("an iron golem"));
        if offer {
            iron_golem.offer_flower_tick = 400;
            self.broadcast_entity_event(id, entity_event::OFFER_FLOWER);
        } else {
            iron_golem.offer_flower_tick = 0;
            self.broadcast_entity_event(id, entity_event::STOP_OFFER_FLOWER);
        }
    }

    pub fn iron_golem_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if !item_stack.is(items::IRON_INGOT) {
            return InteractionResult::Pass;
        }
        let health_before = self.health(id);
        self.heal(id, IRON_INGOT_HEAL_AMOUNT);
        if self.health(id) == health_before {
            return InteractionResult::Pass;
        }
        let random = &mut self.entity_mut(id).random;
        let pitch = 1.0 + (random.next_float() - random.next_float()) * 0.2;
        self.entity_play_sound(id, sound_event("minecraft:entity.iron_golem.repair"), 1.0, pitch);
        item_stack.consume(1, self, Some(player));
        self.player_mut(player).update_item_in_hand(hand, &item_stack);
        InteractionResult::SUCCESS
    }

    pub fn iron_golem_play_step_sound(&mut self, id: EntityId, _pos: BlockPos) {
        self.entity_play_sound(id, sound_event("minecraft:entity.iron_golem.step"), 1.0, 1.0);
    }
}
