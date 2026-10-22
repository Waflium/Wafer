use wafer_core::Vec3;
use wafer_core::random::RandomSource;
use wafer_data::{EquipmentSlot, item_tags, items, registry, registry_id};

use super::abstract_horse::{self, AbstractHorse, abstract_horse_defaults};
use crate::server::level::ServerLevel;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::tempt_goal::TemptGoal;
use crate::world::entity::attributes::{Attribute, ZOMBIE_HORSE_ATTRIBUTES};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::{InteractionHand, InteractionResult};

const SPEED_FACTOR: f32 = 42.16;
const BASE_JUMP_STRENGTH: f64 = 0.5;
const PER_RANDOM_JUMP_STRENGTH: f64 = 0.06666666666666667;
const BASE_SPEED: f64 = 9.0;
const PER_RANDOM_SPEED: f64 = 1.0;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(1.3964844, 1.6).with_eye_height(1.52).with_passenger_attachments(&[Vec3::new(0.0, 1.31875f32 as f64, 0.0)]);
const BABY_DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(1.3964844, 1.6).with_eye_height(1.52).with_passenger_attachments(&[Vec3::new(0.0, (1.6f32 - 0.25f32) as f64, 0.0)]);

#[derive(Default)]
pub struct ZombieHorse {
    pub abstract_horse: AbstractHorse,
}

pub fn baby_dimensions() -> EntityDimensions {
    BABY_DIMENSIONS.scale(abstract_horse::BABY_SCALE)
}

pub fn randomize_attributes(mob: &mut Mob, random: &mut (impl RandomSource + ?Sized)) {
    let attributes = &mut mob.living.attributes;
    let jump_strength = generate_zombie_horse_jump_strength(|| random.next_double());
    attributes.instance(Attribute::JumpStrength).expect("horses have a jump strength").set_base_value(jump_strength);
    let speed = generate_zombie_horse_speed(|| random.next_double());
    attributes.instance(Attribute::MovementSpeed).expect("horses have a movement speed").set_base_value(speed);
}

fn generate_zombie_horse_jump_strength(mut probability_provider: impl FnMut() -> f64) -> f64 {
    BASE_JUMP_STRENGTH + probability_provider() * PER_RANDOM_JUMP_STRENGTH + probability_provider() * PER_RANDOM_JUMP_STRENGTH + probability_provider() * PER_RANDOM_JUMP_STRENGTH
}

fn generate_zombie_horse_speed(mut probability_provider: impl FnMut() -> f64) -> f64 {
    (BASE_SPEED + probability_provider() * PER_RANDOM_SPEED + probability_provider() * PER_RANDOM_SPEED + probability_provider() * PER_RANDOM_SPEED) / SPEED_FACTOR as f64
}

pub fn get_ambient_sound() -> &'static str {
    "minecraft:entity.zombie_horse.ambient"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.zombie_horse.death"
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.zombie_horse.hurt"
}

pub fn get_angry_sound() -> &'static str {
    "minecraft:entity.zombie_horse.angry"
}

pub fn get_eating_sound() -> &'static str {
    "minecraft:entity.zombie_horse.eat"
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::ZOMBIE_HORSE_FOOD)
}

pub fn charge_speed_modifier() -> f32 {
    1.4
}

pub fn sun_protection_slot() -> EquipmentSlot {
    EquipmentSlot::Body
}

impl ZombieHorse {
    pub fn create() -> Mob {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:zombie_horse"), DIMENSIONS, abstract_horse_defaults());
        let mut mob = Mob::new(entity, LivingEntity::new(&ZOMBIE_HORSE_ATTRIBUTES), MobKind::ZombieHorse(ZombieHorse::default()));
        AbstractHorse::init(&mut mob);
        mob
    }

    pub fn add_behaviour_goals(ai: &mut MobAi) {
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::Float(float_goal));
        goal_selector.add_goal(3, Goal::Tempt(TemptGoal::new(1.25, is_food, false)));
    }
}

impl ServerLevel {
    pub fn is_zombie_horse(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::ZombieHorse(_)))
    }

    pub fn zombie_horse_is_mob_controlled(&self, id: EntityId) -> bool {
        self.first_passenger(id).is_some_and(|passenger| self.is_mob(passenger))
    }

    pub fn zombie_horse_get_breed_offspring(&mut self, _id: EntityId, _partner: EntityId) -> Option<Box<Mob>> {
        Some(Box::new(ZombieHorse::create()))
    }

    pub fn zombie_horse_finalize_spawn(&mut self, id: EntityId, difficulty: &DifficultyInstance, spawn_reason: EntitySpawnReason, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
        if spawn_reason == EntitySpawnReason::Natural
            && let Some(LevelEntity::Mob(mut zombie)) = ServerLevel::create_entity("minecraft:zombie")
        {
            let e = self.entity(id);
            let (x, y, z, y_rot) = (e.x(), e.y(), e.z(), e.y_rot);
            zombie.entity.snap_to(x, y, z, y_rot, 0.0);
            let zombie_id = zombie.entity.id;
            self.entities.insert(zombie_id, LevelEntity::Mob(zombie));
            self.finalize_spawn(zombie_id, difficulty, spawn_reason, None);
            self.living_set_item_slot(zombie_id, EquipmentSlot::Mainhand, ItemStack::new(items::IRON_SPEAR, 1));
            self.start_riding(zombie_id, id, false, false);
        }
        let (index, _, entity) = self.entities.shift_remove_full(&id).expect("the zombie horse to finalize");
        let LevelEntity::Mob(mut mob) = entity else { unreachable!("a mob") };
        let group_data = crate::world::entity::mob::finalize_spawn(self, &mut mob, difficulty, spawn_reason, group_data);
        self.entities.shift_insert(index, id, LevelEntity::Mob(mob));
        group_data
    }

    pub fn zombie_horse_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let should_open_inventory = !self.is_baby(id) && self.is_tamed(id) && self.player(player).is_secondary_use_active();
        if !self.is_vehicle(id) && !should_open_inventory {
            let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
            if !item_stack.is_empty() {
                if is_food(&item_stack) {
                    return self.fed_food(id, player, hand, &mut item_stack);
                }
                if !self.is_tamed(id) {
                    self.make_mad(id);
                    return InteractionResult::SUCCESS;
                }
            }
        }
        self.abstract_horse_mob_interact(id, player, hand)
    }
}
