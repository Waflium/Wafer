use std::borrow::Cow;

use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::{EquipmentSlot, item_tags, items, mob_effects, registry, registry_id};

use crate::server::level::{ServerLevel, SoundSource};
use crate::util::special_dates;
use crate::world::difficulty::Difficulty;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::break_door_goal::BreakDoorGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::remove_block_goal::{RemoveBlockGoal, RemoveBlockSounds};
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::ai::goal::zombie_attack_goal::ZombieAttackGoal;
use crate::world::entity::animal::chicken::Chicken;
use crate::world::entity::attributes::{Attribute, AttributeModifier, AttributeSupplier, Operation, ZOMBIE_ATTRIBUTES};
use crate::world::entity::conversion_tracker::{Conversion, ConversionTracker};
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::entity_selector;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind, RANDOM_SPAWN_BONUS_ID};
use crate::world::entity::monster::monster;
use crate::world::damagesource::DamageSource;
use crate::world::entity::monster::zombie::drowned::Drowned;
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::spawn_placements;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, sound_event};
use crate::world::item::ItemStack;

const SPEED_MODIFIER_BABY_ID: &str = "minecraft:baby";
const SPEED_MODIFIER_BABY: AttributeModifier = AttributeModifier { id: Cow::Borrowed(SPEED_MODIFIER_BABY_ID), amount: 0.5, operation: Operation::AddMultipliedBase };
const REINFORCEMENT_CALLER_CHARGE_ID: &str = "minecraft:reinforcement_caller_charge";
const ZOMBIE_REINFORCEMENT_CALLEE_CHARGE: AttributeModifier =
    AttributeModifier { id: Cow::Borrowed("minecraft:reinforcement_callee_charge"), amount: -0.05f32 as f64, operation: Operation::AddValue };
const LEADER_ZOMBIE_BONUS_ID: &str = "minecraft:leader_zombie_bonus";
const ZOMBIE_RANDOM_SPAWN_BONUS_ID: &str = "minecraft:zombie_random_spawn_bonus";
pub const DATA_BABY_ID: usize = 16;
pub const DATA_DROWNED_CONVERSION_ID: usize = 18;
pub const ZOMBIE_LEADER_CHANCE: f32 = 0.05;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.6, 1.95)
    .with_eye_height(1.74)
    .with_passenger_attachments(&[Vec3::new(0.0, 2.0125f32 as f64, 0.0)])
    .with_vehicle_attachment(&[Vec3::new(0.0, 0.7f32 as f64, 0.0)]);
pub const BABY_DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(0.49, 0.98).with_eye_height(0.775).with_vehicle_attachment(&[Vec3::new(0.0, 0.1875f32 as f64, 0.0)]);
const BREAK_DOOR_CHANCE: f32 = 0.1;
const DEFAULT_BABY: bool = false;
const DEFAULT_CAN_BREAK_DOORS: bool = false;

fn door_breaking_predicate(difficulty: Difficulty) -> bool {
    difficulty == Difficulty::Hard
}

pub struct Zombie {
    break_door_goal: Option<BreakDoorGoal>,
    can_break_doors: bool,
    pub drowning_tracker: ConversionTracker,
}

impl Default for Zombie {
    fn default() -> Zombie {
        Zombie {
            break_door_goal: Some(BreakDoorGoal::new(door_breaking_predicate)),
            can_break_doors: DEFAULT_CAN_BREAK_DOORS,
            drowning_tracker: ConversionTracker::new(DATA_DROWNED_CONVERSION_ID, "InWaterTime", 600, "DrownedConversionTime", 300),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ZombieGroupData {
    pub is_baby: bool,
    pub can_spawn_jockey: bool,
}

pub struct ZombieAttackTurtleEggGoal {
    pub remove_block_goal: RemoveBlockGoal,
}

impl ZombieAttackTurtleEggGoal {
    pub fn new(speed_modifier: f64, vertical_search_range: i32) -> ZombieAttackTurtleEggGoal {
        ZombieAttackTurtleEggGoal { remove_block_goal: RemoveBlockGoal::new(wafer_data::blocks::TURTLE_EGG, speed_modifier, vertical_search_range) }
    }

    pub fn accepted_distance(&self) -> f64 {
        1.14
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let accepted_distance = self.accepted_distance();
        self.remove_block_goal.tick(level, mob, accepted_distance, &ZombieAttackTurtleEggSounds);
    }
}

struct ZombieAttackTurtleEggSounds;

impl RemoveBlockSounds for ZombieAttackTurtleEggSounds {
    fn play_destroy_progress_sound(&self, level: &mut ServerLevel, mob: EntityId, pos: BlockPos) {
        let pitch = 0.9 + level.entity_mut(mob).random.next_float() * 0.2;
        level.play_sound(None, pos, sound_event("minecraft:entity.zombie.destroy_egg"), SoundSource::Hostile, 0.5, pitch);
    }

    fn play_break_sound(&self, level: &mut ServerLevel, pos: BlockPos) {
        let pitch = 0.9 + level.random.next_float() * 0.2;
        level.play_sound(None, pos, sound_event("minecraft:entity.turtle.egg_break"), SoundSource::Blocks, 0.7, pitch);
    }
}

pub fn zombie(mob: &Mob) -> &Zombie {
    match &mob.kind {
        MobKind::Zombie(zombie) => zombie,
        MobKind::Husk(husk) => &husk.zombie,
        MobKind::Drowned(drowned) => &drowned.zombie,
        _ => unreachable!("not a zombie"),
    }
}

pub fn zombie_mut(mob: &mut Mob) -> &mut Zombie {
    match &mut mob.kind {
        MobKind::Zombie(zombie) => zombie,
        MobKind::Husk(husk) => &mut husk.zombie,
        MobKind::Drowned(drowned) => &mut drowned.zombie,
        _ => unreachable!("not a zombie"),
    }
}

fn drowning_tracker(kind: &mut MobKind) -> &mut ConversionTracker {
    match kind {
        MobKind::Zombie(zombie) => &mut zombie.drowning_tracker,
        MobKind::Husk(husk) => &mut husk.zombie.drowning_tracker,
        MobKind::Drowned(drowned) => &mut drowned.zombie.drowning_tracker,
        _ => unreachable!("not a zombie"),
    }
}

pub fn zombie_defaults() -> Vec<DataValue> {
    let mut data = mob_defaults();
    data.extend([DataValue::Boolean(DEFAULT_BABY), DataValue::Int(0), DataValue::Boolean(false)]);
    data
}

pub fn is_baby(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_BABY_ID), DataValue::Boolean(true))
}

pub fn get_spawn_as_baby_odds(random: &mut impl RandomSource) -> bool {
    random.next_float() < 0.05
}

pub fn baby_on_land_selector(_mob: EntityId, target: EntityId, level: &ServerLevel) -> bool {
    level.is_baby(target) && !level.entity(target).is_in_water()
}

impl Zombie {
    pub fn create() -> Mob {
        Zombie::create_with("minecraft:zombie", DIMENSIONS, &ZOMBIE_ATTRIBUTES, MobKind::Zombie(Zombie::default()))
    }

    pub fn create_with(entity_type: &str, dimensions: EntityDimensions, attributes: &'static AttributeSupplier, kind: MobKind) -> Mob {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, entity_type), dimensions, zombie_defaults());
        let mut mob = Mob::new(entity, LivingEntity::new(attributes), kind);
        mob.xp_reward = monster::XP_REWARD;
        mob
    }

    pub fn register_goals(ai: &mut MobAi, kind: &MobKind) {
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(4, Goal::ZombieAttackTurtleEgg(ZombieAttackTurtleEggGoal::new(1.0, 3)));
        goal_selector.add_goal(8, Goal::LookAtPlayer(LookAtPlayerGoal::new(8.0)));
        goal_selector.add_goal(8, Goal::RandomLookAround(RandomLookAroundGoal::default()));
        match kind {
            MobKind::Drowned(_) => Drowned::add_behaviour_goals(ai),
            _ => Zombie::add_behaviour_goals(ai),
        }
    }

    fn add_behaviour_goals(ai: &mut MobAi) {
        ai.goal_selector.add_goal(3, Goal::ZombieAttack(ZombieAttackGoal::new(1.0, false)));
        ai.goal_selector.add_goal(7, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::new(1.0)));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::HurtByTarget(HurtByTargetGoal::new(&[]).set_alert_others(&["minecraft:zombified_piglin"])));
        target_selector.add_goal(2, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::Player, true)));
        target_selector.add_goal(3, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::AbstractVillager, false)));
        target_selector.add_goal(3, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::IronGolem, true)));
        target_selector.add_goal(5, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::with(TargetType::Turtle, 10, true, false, Some(baby_on_land_selector))));
    }

    pub fn is_under_water_converting(mob: &Mob) -> bool {
        zombie(mob).drowning_tracker.is_converting(&mob.entity)
    }

    pub fn can_break_doors(mob: &Mob) -> bool {
        zombie(mob).can_break_doors
    }

    pub fn set_can_break_doors(mob: &mut Mob, can_break_doors: bool) {
        if mob.ai.navigation.can_navigate_ground() {
            if zombie(mob).can_break_doors != can_break_doors {
                zombie_mut(mob).can_break_doors = can_break_doors;
                mob.ai.navigation.set_can_open_doors(can_break_doors);
                if can_break_doors {
                    let break_door_goal = zombie_mut(mob).break_door_goal.take().expect("the break door goal");
                    mob.ai.goal_selector.add_goal(1, Goal::BreakDoor(break_door_goal));
                } else {
                    Zombie::remove_break_door_goal(mob);
                }
            }
        } else if zombie(mob).can_break_doors {
            Zombie::remove_break_door_goal(mob);
            zombie_mut(mob).can_break_doors = false;
        }
    }

    fn remove_break_door_goal(mob: &mut Mob) {
        if let Some(wrapped_goal) = mob.ai.goal_selector.remove_goal(|goal| matches!(goal, Goal::BreakDoor(_)))
            && let Goal::BreakDoor(break_door_goal) = wrapped_goal.into_goal()
        {
            zombie_mut(mob).break_door_goal = Some(break_door_goal);
        }
    }

    pub fn set_baby(mob: &mut Mob, baby: bool) {
        mob.entity.data.set(DATA_BABY_ID, DataValue::Boolean(baby));
        let speed = mob.living.attributes.instance(Attribute::MovementSpeed).expect("zombies have movement speed");
        speed.remove_modifier(SPEED_MODIFIER_BABY_ID);
        if baby {
            speed.add_transient_modifier(SPEED_MODIFIER_BABY);
        }
        mob.refresh_dimensions();
    }

    pub fn converts_in_water(mob: &Mob) -> bool {
        match mob.kind {
            MobKind::Drowned(_) => Drowned::converts_in_water(mob),
            _ => true,
        }
    }

    pub fn is_sun_sensitive(mob: &Mob) -> bool {
        !matches!(mob.kind, MobKind::Husk(_))
    }

    pub fn get_ambient_sound(mob: &Mob) -> &'static str {
        if matches!(mob.kind, MobKind::Husk(_)) { "minecraft:entity.husk.ambient" } else { "minecraft:entity.zombie.ambient" }
    }

    pub fn get_hurt_sound(mob: &Mob) -> &'static str {
        if matches!(mob.kind, MobKind::Husk(_)) { "minecraft:entity.husk.hurt" } else { "minecraft:entity.zombie.hurt" }
    }

    pub fn get_death_sound(mob: &Mob) -> &'static str {
        if matches!(mob.kind, MobKind::Husk(_)) { "minecraft:entity.husk.death" } else { "minecraft:entity.zombie.death" }
    }

    pub fn get_step_sound(mob: &Mob) -> &'static str {
        if matches!(mob.kind, MobKind::Husk(_)) { "minecraft:entity.husk.step" } else { "minecraft:entity.zombie.step" }
    }

    pub fn get_conversion_sound(mob: &Mob) -> i32 {
        if matches!(mob.kind, MobKind::Husk(_)) { 1041 } else { 1040 }
    }

    pub fn converts_to_when_drowning(mob: &Mob) -> &'static str {
        if matches!(mob.kind, MobKind::Husk(_)) { "minecraft:zombie" } else { "minecraft:drowned" }
    }

    pub fn can_spawn_in_liquids(mob: &Mob) -> bool {
        match mob.kind {
            MobKind::Drowned(_) => Drowned::can_spawn_in_liquids(mob),
            _ => false,
        }
    }

    pub fn add_additional_save_data(mob: &Mob, is_afflicted: bool, out: &mut Vec<(String, wafer_protocol::nbt::Nbt)>) {
        use wafer_protocol::nbt::Nbt;
        out.push(("IsBaby".into(), Nbt::Byte(mob.is_baby() as i8)));
        out.push(("CanBreakDoors".into(), Nbt::Byte(Zombie::can_break_doors(mob) as i8)));
        zombie(mob).drowning_tracker.add_additional_save_data(&mob.entity, is_afflicted, out);
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &wafer_protocol::nbt::Nbt) {
        let boolean = |key: &str| input.get(key).and_then(wafer_protocol::nbt::Nbt::as_i64).is_some_and(|v| v != 0);
        Zombie::set_baby(mob, boolean("IsBaby"));
        Zombie::set_can_break_doors(mob, boolean("CanBreakDoors"));
        let Mob { entity, kind, .. } = mob;
        drowning_tracker(kind).read_additional_save_data(entity, input);
    }
}

impl ServerLevel {
    pub fn zombie_tick(&mut self, id: EntityId) {
        self.mob_tick(id);
        if self.mob(id).is_some_and(Zombie::converts_in_water) {
            let mob = self.mob(id).expect("a zombie");
            let conversion = Conversion {
                converts_to: Zombie::converts_to_when_drowning(mob),
                conversion_sound: Zombie::get_conversion_sound(mob),
                is_afflicted: mob.entity.is_eye_in_fluid(&wafer_data::fluid_tags::WATER),
                post_conversion_step: Some(|level, converted| {
                    let pos = level.entity(converted).block_position();
                    let difficulty_modifier = level.current_difficulty_at(pos).get_special_multiplier();
                    level.handle_attributes(converted, difficulty_modifier, EntitySpawnReason::Conversion);
                }),
            };
            self.conversion_tracker_tick(id, drowning_tracker, conversion);
        }
    }

    pub fn zombie_can_hold_item(&self, id: EntityId, item_stack: &ItemStack) -> bool {
        if item_stack.get_item().is(&item_tags::EGGS) && self.is_baby(id) && self.is_passenger(id) { false } else { self.mob_can_hold_item(id, item_stack) }
    }

    pub fn zombie_wants_to_pick_up(&self, id: EntityId, item_stack: &ItemStack) -> bool {
        if item_stack.is(items::GLOW_INK_SAC) { false } else { self.mob_wants_to_pick_up(id, item_stack) }
    }

    pub fn zombie_do_hurt_target(&mut self, id: EntityId, target: EntityId) -> bool {
        let result = self.mob_do_hurt_target(id, target);
        if result {
            let pos = self.entity(id).block_position();
            let difficulty = self.current_difficulty_at(pos).get_effective_difficulty();
            if self.item_by_slot_ref(id, EquipmentSlot::Mainhand).is_none()
                && self.is_on_fire(id)
                && self.entity_mut(id).random.next_float() < difficulty * 0.3
            {
                self.ignite_for_seconds(target, (2 * difficulty as i32) as f32);
            }
        }
        result
    }

    pub fn husk_do_hurt_target(&mut self, id: EntityId, target: EntityId) -> bool {
        let result = self.zombie_do_hurt_target(id, target);
        if result && self.item_by_slot_ref(id, EquipmentSlot::Mainhand).is_none() && self.is_living(target) {
            let pos = self.entity(id).block_position();
            let difficulty = self.current_difficulty_at(pos).get_effective_difficulty();
            self.add_effect(target, MobEffectInstance::new(mob_effects::HUNGER, 140 * difficulty as i32, 0), Some(id));
        }
        result
    }

    pub fn zombie_get_base_experience_reward(&mut self, id: EntityId) -> i32 {
        let mob = self.mob_mut(id).expect("a zombie");
        if mob.is_baby() {
            mob.xp_reward = (mob.xp_reward as f64 * 2.5) as i32;
        }
        self.mob_get_base_experience_reward(id)
    }

    pub fn zombie_populate_default_equipment_slots(&mut self, id: EntityId, difficulty: &DifficultyInstance) {
        self.mob_populate_default_equipment_slots(id, difficulty);
        let chance = if self.difficulty == Difficulty::Hard { 0.05 } else { 0.01 };
        if self.random.next_float() < chance {
            let rand = self.random.next_int_bounded(6);
            let item = match rand {
                0 => items::IRON_SWORD,
                1 => items::IRON_SPEAR,
                _ => items::IRON_SHOVEL,
            };
            self.living_set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::new(item, 1));
        }
    }

    pub fn zombie_finalize_spawn(&mut self, id: EntityId, difficulty: &DifficultyInstance, spawn_reason: EntitySpawnReason, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
        let mut group_data = self.mob_finalize_spawn(id, difficulty, spawn_reason, group_data);
        let difficulty_modifier = difficulty.get_special_multiplier();
        if spawn_reason != EntitySpawnReason::Conversion {
            let can_pick_up_loot = self.random.next_float() < 0.55 * difficulty_modifier;
            self.mob_mut(id).expect("a zombie").can_pick_up_loot = can_pick_up_loot;
        }
        if group_data.is_none() {
            let is_baby = get_spawn_as_baby_odds(&mut self.random);
            group_data = Some(SpawnGroupData::Zombie(ZombieGroupData { is_baby, can_spawn_jockey: true }));
        }
        let zombie_data = match group_data {
            Some(SpawnGroupData::Zombie(zombie_data)) => Some(zombie_data),
            Some(SpawnGroupData::Husk(husk_data)) => Some(husk_data.zombie_group_data),
            _ => None,
        };
        if let Some(zombie_data) = zombie_data {
            if zombie_data.is_baby {
                Zombie::set_baby(self.mob_mut(id).expect("a zombie"), true);
                if zombie_data.can_spawn_jockey {
                    self.spawn_chicken_jockey(id, difficulty);
                }
            }
            let can_break_doors = self.random.next_float() < difficulty_modifier * BREAK_DOOR_CHANCE;
            Zombie::set_can_break_doors(self.mob_mut(id).expect("a zombie"), can_break_doors);
            if spawn_reason != EntitySpawnReason::Conversion {
                self.populate_default_equipment_slots(id, difficulty);
                self.populate_default_equipment_enchantments(id, difficulty);
            }
        }
        if self.item_by_slot_ref(id, EquipmentSlot::Head).is_none() && special_dates::is_halloween() && self.random.next_float() < 0.25 {
            let item = if self.random.next_float() < 0.1 { items::JACK_O_LANTERN } else { items::CARVED_PUMPKIN };
            self.living_set_item_slot(id, EquipmentSlot::Head, ItemStack::new(item, 1));
            self.mob_mut(id).expect("a zombie").set_drop_chance(EquipmentSlot::Head, 0.0);
        }
        self.handle_attributes(id, difficulty_modifier, spawn_reason);
        group_data
    }

    fn spawn_chicken_jockey(&mut self, id: EntityId, difficulty: &DifficultyInstance) {
        if (self.random.next_float() as f64) < 0.05 {
            let area = self.entity(id).bounding_box().inflate(5.0, 3.0, 5.0);
            let chickens: Vec<EntityId> = self
                .get_entities(None, &area)
                .into_iter()
                .filter(|&entity| matches!(self.mob(entity).map(|mob| &mob.kind), Some(MobKind::Chicken(_))) && entity_selector::entity_not_being_ridden(self, entity))
                .collect();
            if let Some(&chicken) = chickens.first() {
                self.set_chicken_jockey(chicken, true);
                self.start_riding(id, chicken, false, false);
            }
        } else if (self.random.next_float() as f64) < 0.05 {
            let mut chicken = Chicken::create();
            let e = self.entity(id);
            let (x, y, z, y_rot) = (e.x(), e.y(), e.z(), e.y_rot);
            chicken.entity.snap_to(x, y, z, y_rot, 0.0);
            let chicken_id = chicken.entity.id;
            self.entities.insert(chicken_id, LevelEntity::Mob(Box::new(chicken)));
            self.finalize_spawn(chicken_id, difficulty, EntitySpawnReason::Jockey, None);
            self.set_chicken_jockey(chicken_id, true);
            self.start_riding(id, chicken_id, false, false);
            let chicken = self.entities.shift_remove(&chicken_id).expect("the jockey chicken");
            self.add_fresh_entity(chicken);
        }
    }

    pub fn zombie_on_offspring_spawned_from_egg(&mut self, id: EntityId, offspring: EntityId) {
        let pos = self.entity(offspring).block_position();
        let difficulty_modifier = self.current_difficulty_at(pos).get_special_multiplier();
        let can_pick_up_loot = self.entity_mut(id).random.next_float() < 0.55 * difficulty_modifier;
        self.mob_mut(offspring).expect("a zombie").can_pick_up_loot = can_pick_up_loot;
    }

    pub fn handle_attributes(&mut self, id: EntityId, difficulty_modifier: f32, spawn_reason: EntitySpawnReason) {
        self.randomize_reinforcements_chance(id);
        let mob = self.mob_mut(id).expect("a zombie");
        let random = &mut mob.entity.random;
        let amount = random.next_double() * 0.05f32 as f64;
        add_or_replace_permanent_modifier(mob, Attribute::KnockbackResistance, RANDOM_SPAWN_BONUS_ID, amount, Operation::AddValue);
        let follow_range_modifier = mob.entity.random.next_double() * 1.5 * difficulty_modifier as f64;
        if follow_range_modifier > 1.0 {
            add_or_replace_permanent_modifier(mob, Attribute::FollowRange, ZOMBIE_RANDOM_SPAWN_BONUS_ID, follow_range_modifier, Operation::AddMultipliedTotal);
        }
        if mob.entity.random.next_float() < difficulty_modifier * ZOMBIE_LEADER_CHANCE {
            let amount = mob.entity.random.next_double() * 0.25 + 0.5;
            add_or_replace_permanent_modifier(mob, Attribute::SpawnReinforcements, LEADER_ZOMBIE_BONUS_ID, amount, Operation::AddValue);
            let amount = mob.entity.random.next_double() * 3.0 + 1.0;
            add_or_replace_permanent_modifier(mob, Attribute::MaxHealth, LEADER_ZOMBIE_BONUS_ID, amount, Operation::AddMultipliedTotal);
            if !matches!(spawn_reason, EntitySpawnReason::Conversion | EntitySpawnReason::Load | EntitySpawnReason::DimensionTravel) {
                let max_health = mob.living.attributes.value(Attribute::MaxHealth) as f32;
                mob.set_health(max_health);
            }
            Zombie::set_can_break_doors(mob, true);
        }
    }

    pub fn zombie_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        let source_entity = source.causing_entity;
        if !self.living_entity_hurt_server(id, source, damage) {
            return false;
        }
        let mut target = self.get_target(id);
        if target.is_none() && source_entity.is_some_and(|entity| self.is_living(entity)) {
            target = source_entity;
        }
        let Some(target) = target else { return true };
        if self.difficulty != Difficulty::Hard {
            return true;
        }
        let chance = self.attribute_value(id, Attribute::SpawnReinforcements);
        if !((self.mob_mut(id).expect("a zombie").entity.random.next_float() as f64) < chance && self.is_spawning_monsters()) {
            return true;
        }
        let position = self.entity(id).position();
        let (x, y, z) = (mth::floor(position.x), mth::floor(position.y), mth::floor(position.z));
        let entity_type = self.mob(id).expect("a zombie").get_type();
        let Some(LevelEntity::Mob(mut reinforcement)) = ServerLevel::create_entity(entity_type.get_key()) else { return true };
        for _ in 0..50 {
            let random = &mut self.mob_mut(id).expect("a zombie").entity.random;
            let xt = x + mth::next_int(random, 7, 40) * mth::next_int(random, -1, 1);
            let yt = y + mth::next_int(random, 7, 40) * mth::next_int(random, -1, 1);
            let zt = z + mth::next_int(random, 7, 40) * mth::next_int(random, -1, 1);
            let spawn_pos = BlockPos::new(xt, yt, zt);
            if spawn_placements::is_spawn_position_ok(entity_type, self, spawn_pos)
                && spawn_placements::check_spawn_rules(entity_type, self, EntitySpawnReason::Reinforcement, spawn_pos)
            {
                reinforcement.entity.set_pos(Vec3::new(xt as f64, yt as f64, zt as f64));
                let bounding_box = reinforcement.entity.bounding_box();
                let candidate = LevelEntity::Mob(reinforcement);
                if !self.has_nearby_alive_player(xt as f64, yt as f64, zt as f64, 7.0)
                    && self.is_unobstructed_by_entities(None, &bounding_box)
                    && self.no_collision_for_new(&candidate, &bounding_box)
                    && (can_spawn_in_liquids(&candidate) || !ServerLevel::contains_any_liquid(self, &bounding_box))
                {
                    let difficulty = self.current_difficulty_at(candidate.entity().block_position());
                    let (candidate, _) = self.finalize_spawn_detached(candidate, &difficulty, EntitySpawnReason::Reinforcement, None);
                    let reinforcement_id = candidate.entity().id;
                    self.add_fresh_entity_with_passengers(candidate);
                    self.set_target(reinforcement_id, Some(target));
                    let attribute = self.mob_mut(id).expect("a zombie").living.attributes.instance(Attribute::SpawnReinforcements).expect("zombies have a reinforcement chance");
                    let existing_amount = attribute.modifier(REINFORCEMENT_CALLER_CHARGE_ID).map_or(0.0, |modifier| modifier.amount);
                    attribute.remove_modifier(REINFORCEMENT_CALLER_CHARGE_ID);
                    attribute.add_permanent_modifier(AttributeModifier { id: Cow::Borrowed(REINFORCEMENT_CALLER_CHARGE_ID), amount: existing_amount - 0.05, operation: Operation::AddValue });
                    if let Some(mob) = self.mob_mut(reinforcement_id)
                        && let Some(instance) = mob.living.attributes.instance(Attribute::SpawnReinforcements)
                    {
                        instance.add_permanent_modifier(ZOMBIE_REINFORCEMENT_CALLEE_CHARGE);
                    }
                    break;
                }
                let LevelEntity::Mob(candidate) = candidate else { unreachable!("a mob") };
                reinforcement = candidate;
            }
        }
        true
    }

    fn randomize_reinforcements_chance(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a zombie");
        let base_value = mob.entity.random.next_double() * 0.1f32 as f64;
        mob.living.attributes.instance(Attribute::SpawnReinforcements).expect("zombies have a reinforcement chance").set_base_value(base_value);
    }
}

fn can_spawn_in_liquids(_entity: &LevelEntity) -> bool {
    false
}

fn add_or_replace_permanent_modifier(mob: &mut Mob, attribute: Attribute, id: &'static str, amount: f64, operation: Operation) {
    let instance = mob.living.attributes.instance(attribute).expect("a zombie attribute");
    instance.add_or_replace_permanent_modifier(AttributeModifier { id: Cow::Borrowed(id), amount, operation });
}
