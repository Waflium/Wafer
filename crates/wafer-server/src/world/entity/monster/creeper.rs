use wafer_core::mth;
use wafer_core::random::RandomSource;
use wafer_data::{item_tags, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::avoid_entity_goal::AvoidEntityGoal;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::melee_attack_goal::MeleeAttackGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::swell_goal::SwellGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::area_effect_cloud::AreaEffectCloud;
use crate::world::entity::attributes::CREEPER_ATTRIBUTES;
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::living_entity::{LivingEntity, get_comfortable_fall_distance};
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::monster::monster;
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, RemovalReason, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::server_explosion::ExplosionInteraction;
use crate::world::{InteractionHand, InteractionResult};

pub const DATA_SWELL_DIR: usize = 16;
pub const DATA_IS_POWERED: usize = 17;
pub const DATA_IS_IGNITED: usize = 18;
const DEFAULT_IGNITED: bool = false;
const DEFAULT_POWERED: bool = false;
const DEFAULT_MAX_SWELL: i16 = 30;
const DEFAULT_EXPLOSION_RADIUS: i8 = 3;
const CHARGED_CREEPER: &str = "minecraft:charged_creeper/root";
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.6, 1.7);

pub struct Creeper {
    old_swell: i32,
    pub swell: i32,
    pub max_swell: i32,
    pub explosion_radius: i32,
    dropped_skulls: bool,
}

impl Default for Creeper {
    fn default() -> Creeper {
        Creeper { old_swell: 0, swell: 0, max_swell: DEFAULT_MAX_SWELL as i32, explosion_radius: DEFAULT_EXPLOSION_RADIUS as i32, dropped_skulls: false }
    }
}

pub fn creeper(mob: &Mob) -> &Creeper {
    match &mob.kind {
        MobKind::Creeper(creeper) => creeper,
        _ => unreachable!("not a creeper"),
    }
}

pub fn creeper_mut(mob: &mut Mob) -> &mut Creeper {
    match &mut mob.kind {
        MobKind::Creeper(creeper) => creeper,
        _ => unreachable!("not a creeper"),
    }
}

pub fn creeper_defaults() -> Vec<DataValue> {
    let mut data = mob_defaults();
    data.extend([DataValue::Int(-1), DataValue::Boolean(DEFAULT_POWERED), DataValue::Boolean(DEFAULT_IGNITED)]);
    data
}

pub fn is_powered(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_IS_POWERED), DataValue::Boolean(true))
}

pub fn get_swell_dir(entity: &Entity) -> i32 {
    match entity.data.get(DATA_SWELL_DIR) {
        DataValue::Int(dir) => *dir,
        _ => unreachable!("the swell dir is an int"),
    }
}

pub fn set_swell_dir(entity: &mut Entity, dir: i32) {
    entity.data.set(DATA_SWELL_DIR, DataValue::Int(dir));
}

pub fn is_ignited(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_IS_IGNITED), DataValue::Boolean(true))
}

pub fn ignite(entity: &mut Entity) {
    entity.data.set(DATA_IS_IGNITED, DataValue::Boolean(true));
}

impl Creeper {
    pub fn create() -> Mob {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:creeper"), DIMENSIONS, creeper_defaults());
        let mut mob = Mob::new(entity, LivingEntity::new(&CREEPER_ATTRIBUTES), MobKind::Creeper(Creeper::default()));
        mob.xp_reward = monster::XP_REWARD;
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(1, Goal::Float(float_goal));
        goal_selector.add_goal(2, Goal::Swell(SwellGoal::default()));
        goal_selector.add_goal(3, Goal::AvoidEntity(AvoidEntityGoal::new("minecraft:ocelot", 6.0, 1.0, 1.2)));
        goal_selector.add_goal(3, Goal::AvoidEntity(AvoidEntityGoal::new("minecraft:cat", 6.0, 1.0, 1.2)));
        goal_selector.add_goal(4, Goal::MeleeAttack(MeleeAttackGoal::new(1.0, false)));
        goal_selector.add_goal(5, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::new(0.8)));
        goal_selector.add_goal(6, Goal::LookAtPlayer(LookAtPlayerGoal::new(8.0)));
        goal_selector.add_goal(6, Goal::RandomLookAround(RandomLookAroundGoal::default()));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::Player, true)));
        target_selector.add_goal(2, Goal::HurtByTarget(HurtByTargetGoal::new(&[])));
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.creeper.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.creeper.death"
    }

    pub fn get_swelling(mob: &Mob, a: f32) -> f32 {
        let creeper = creeper(mob);
        mth::lerp_f(a, creeper.old_swell as f32, creeper.swell as f32) / (creeper.max_swell - 2) as f32
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        let creeper = creeper(mob);
        out.push(("powered".into(), Nbt::Byte(is_powered(&mob.entity) as i8)));
        out.push(("Fuse".into(), Nbt::Short(creeper.max_swell as i16)));
        out.push(("ExplosionRadius".into(), Nbt::Byte(creeper.explosion_radius as i8)));
        out.push(("ignited".into(), Nbt::Byte(is_ignited(&mob.entity) as i8)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        let boolean = |key: &str| input.get(key).and_then(Nbt::as_i64).is_some_and(|v| v as i8 != 0);
        mob.entity.data.set(DATA_IS_POWERED, DataValue::Boolean(boolean("powered")));
        let max_swell = input.get("Fuse").and_then(Nbt::as_i64).map_or(DEFAULT_MAX_SWELL, |v| v as i16);
        let explosion_radius = input.get("ExplosionRadius").and_then(Nbt::as_i64).map_or(DEFAULT_EXPLOSION_RADIUS, |v| v as i8);
        let creeper = creeper_mut(mob);
        creeper.max_swell = max_swell as i32;
        creeper.explosion_radius = explosion_radius as i32;
        if boolean("ignited") {
            ignite(&mut mob.entity);
        }
    }
}

impl ServerLevel {
    pub fn creeper_get_max_fall_distance(&self, id: EntityId) -> i32 {
        if self.get_target(id).is_none() { get_comfortable_fall_distance(0.0) } else { get_comfortable_fall_distance(self.health(id) - 1.0) }
    }

    pub fn creeper_cause_fall_damage(&mut self, id: EntityId, fall_distance: f64, damage_modifier: f32, damage_source: DamageSource) -> bool {
        let damaged = self.living_entity_cause_fall_damage(id, fall_distance, damage_modifier, damage_source);
        let creeper = creeper_mut(self.mob_mut(id).expect("a creeper"));
        creeper.swell += (fall_distance * 1.5) as i32;
        if creeper.swell > creeper.max_swell - 5 {
            creeper.swell = creeper.max_swell - 5;
        }
        damaged
    }

    pub fn creeper_tick(&mut self, id: EntityId) {
        if self.is_alive(id) {
            let mob = self.mob_mut(id).expect("a creeper");
            let creeper = creeper_mut(mob);
            creeper.old_swell = creeper.swell;
            if is_ignited(&mob.entity) {
                set_swell_dir(&mut mob.entity, 1);
            }
            let swell_dir = get_swell_dir(&mob.entity);
            if swell_dir > 0 && creeper_mut(mob).swell == 0 {
                self.entity_play_sound(id, sound_event("minecraft:entity.creeper.primed"), 1.0, 0.5);
                self.entity_game_event(id, crate::world::level::gameevent::game_event::PRIME_FUSE);
            }
            let creeper = creeper_mut(self.mob_mut(id).expect("a creeper"));
            creeper.swell += swell_dir;
            if creeper.swell < 0 {
                creeper.swell = 0;
            }
            if creeper.swell >= creeper.max_swell {
                creeper.swell = creeper.max_swell;
                self.explode_creeper(id);
            }
        }
        self.mob_tick(id);
    }

    pub fn creeper_set_target(&mut self, id: EntityId, target: Option<EntityId>) {
        if !target.is_some_and(|target| registry::ENTITY_TYPE[self.entity(target).entity_type as usize] == "minecraft:goat") {
            self.mob_set_target(id, target);
        }
    }

    pub fn creeper_killed_entity(&mut self, id: EntityId, entity: EntityId, source: &DamageSource) -> bool {
        let mob = self.mob(id).expect("a creeper");
        if self.should_drop_loot(id) && is_powered(&mob.entity) && !creeper(mob).dropped_skulls {
            self.drop_from_loot_table_into(entity, source, false, CHARGED_CREEPER, &mut |level, item_stack| {
                level.spawn_at_location(entity, item_stack, 0.0);
                creeper_mut(level.mob_mut(id).expect("a creeper")).dropped_skulls = true;
            });
        }
        self.entity_killed_entity(id, entity, source)
    }

    pub fn creeper_do_hurt_target(&mut self, _id: EntityId, _target: EntityId) -> bool {
        true
    }

    pub fn creeper_thunder_hit(&mut self, id: EntityId, lightning_bolt: EntityId) {
        self.entity_thunder_hit(id, lightning_bolt);
        if let Some(mob) = self.mob_mut(id) {
            mob.entity.data.set(DATA_IS_POWERED, DataValue::Boolean(true));
        }
    }

    pub fn creeper_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if item_stack.get_item().is(&item_tags::CREEPER_IGNITERS) {
            let sound = if item_stack.is(items::FIRE_CHARGE) { "minecraft:item.firecharge.use" } else { "minecraft:item.flintandsteel.use" };
            let pitch = self.entity_mut(id).random.next_float() * 0.4 + 0.8;
            let pos = self.entity(id).position();
            let source = self.sound_source(id);
            self.play_sound_at(Some(player), pos.x, pos.y, pos.z, sound_event(sound), source, 1.0, pitch);
            ignite(self.entity_mut(id));
            if !item_stack.is_damageable_item() {
                item_stack.shrink(1);
                self.player_mut(player).update_item_in_hand(hand, &item_stack);
            } else {
                self.hurt_and_break(player, hand.as_equipment_slot(), 1);
            }
            return InteractionResult::SUCCESS;
        }
        self.mob_mob_interact(id, player, hand)
    }

    fn explode_creeper(&mut self, id: EntityId) {
        let mob = self.mob(id).expect("a creeper");
        let explosion_multiplier = if is_powered(&mob.entity) { 2.0 } else { 1.0 };
        let radius = creeper(mob).explosion_radius as f32 * explosion_multiplier;
        let position = mob.entity.position();
        self.living_mut(id).dead = true;
        self.explode(Some(id), Some(id), None, position, radius, false, ExplosionInteraction::Mob);
        self.spawn_lingering_cloud(id);
        self.trigger_on_death_mob_effects(id, RemovalReason::Killed);
        self.remove(id, RemovalReason::Discarded);
    }

    fn spawn_lingering_cloud(&mut self, id: EntityId) {
        let active_effects = self.living(id).active_effects.clone();
        if !active_effects.is_empty() {
            let position = self.entity(id).position();
            let mut cloud = AreaEffectCloud::new(position.x, position.y, position.z);
            cloud.set_radius(2.5);
            cloud.radius_on_use = -0.5;
            cloud.wait_time = 10;
            cloud.duration = 300;
            cloud.set_potion_duration_scale(0.25);
            cloud.radius_per_tick = -cloud.get_radius() / cloud.duration as f32;
            for mob_effect in &active_effects {
                cloud.add_effect(mob_effect);
            }
            self.add_fresh_entity(LevelEntity::AreaEffectCloud(Box::new(cloud)));
        }
    }
}
