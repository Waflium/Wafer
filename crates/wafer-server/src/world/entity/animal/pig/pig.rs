use std::collections::HashMap;
use std::sync::OnceLock;

use wafer_core::random::RandomSource;
use wafer_data::{EquipmentSlot, item_tags, items, registry, registry_id};
use serde_json::Value;

use crate::server::level::ServerLevel;
use crate::world::difficulty::Difficulty;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::breed_goal::BreedGoal;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::follow_parent_goal::FollowParentGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::panic_goal::PanicGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::tempt_goal::TemptGoal;
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::{Attribute, PIG_ATTRIBUTES};
use crate::world::entity::item_based_steering::ItemBasedSteering;
use wafer_core::Vec3;
use crate::world::entity::conversion_params::ConversionParams;
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::variant::variant_utils::{self, SpawnPrioritySelectors};
use crate::world::entity::{Entity, EntityDimensions, EntityId};
use crate::world::item::ItemStack;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::{InteractionHand, InteractionResult};

pub const DATA_BOOST_TIME: usize = 18;
pub const DATA_VARIANT_ID: usize = 19;
pub const DATA_SOUND_VARIANT_ID: usize = 20;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.9, 0.9).with_passenger_attachments(&[wafer_core::Vec3::new(0.0, 0.86875f32 as f64, 0.0)]);
pub const BABY_DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.45, 0.45).with_eye_height(0.40625).with_passenger_attachments(&[wafer_core::Vec3::new(0.0, 0.5, 0.0)]);
pub const VARIANT_REGISTRY: &str = "minecraft:pig_variant";
pub const SOUND_VARIANT_REGISTRY: &str = "minecraft:pig_sound_variant";

#[derive(Default)]
pub struct Pig {
    pub animal: Animal,
    steering: ItemBasedSteering,
}

fn pig(mob: &Mob) -> &Pig {
    match &mob.kind {
        MobKind::Pig(pig) => pig,
        _ => unreachable!("not a pig"),
    }
}

pub struct PigSoundSet {
    pub ambient_sound: String,
    pub hurt_sound: String,
    pub death_sound: String,
    pub step_sound: String,
    pub eat_sound: String,
}

pub struct PigSoundVariant {
    pub adult_sounds: PigSoundSet,
    pub baby_sounds: PigSoundSet,
}

fn spawn_selectors() -> &'static SpawnPrioritySelectors {
    static SELECTORS: OnceLock<SpawnPrioritySelectors> = OnceLock::new();
    SELECTORS.get_or_init(|| SpawnPrioritySelectors::load(VARIANT_REGISTRY, wafer_data::PIG_VARIANTS))
}

pub fn sound_variant(id: i32) -> &'static PigSoundVariant {
    static SOUND_VARIANTS: OnceLock<Vec<PigSoundVariant>> = OnceLock::new();
    let all = SOUND_VARIANTS.get_or_init(|| {
        let json: HashMap<&str, Value> = wafer_data::PIG_SOUND_VARIANTS.iter().map(|&(id, json)| (id, serde_json::from_str(json).expect("vanilla JSON"))).collect();
        let set = |v: &Value| PigSoundSet {
            ambient_sound: v["ambient_sound"].as_str().unwrap_or_default().to_string(),
            hurt_sound: v["hurt_sound"].as_str().unwrap_or_default().to_string(),
            death_sound: v["death_sound"].as_str().unwrap_or_default().to_string(),
            step_sound: v["step_sound"].as_str().unwrap_or_default().to_string(),
            eat_sound: v["eat_sound"].as_str().unwrap_or_default().to_string(),
        };
        wafer_data::SyncedRegistry::get(SOUND_VARIANT_REGISTRY)
            .entries
            .iter()
            .map(|name| PigSoundVariant { adult_sounds: set(&json[name]["adult_sounds"]), baby_sounds: set(&json[name]["baby_sounds"]) })
            .collect()
    });
    &all[id as usize]
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::PIG_FOOD)
}

pub fn get_variant(entity: &Entity) -> i32 {
    match entity.data.get(DATA_VARIANT_ID) {
        DataValue::PigVariant(variant) => *variant,
        _ => 0,
    }
}

pub fn get_sound_variant(entity: &Entity) -> i32 {
    match entity.data.get(DATA_SOUND_VARIANT_ID) {
        DataValue::PigSoundVariant(variant) => *variant,
        _ => 0,
    }
}

pub fn set_variant(entity: &mut Entity, variant: i32) {
    entity.data.set(DATA_VARIANT_ID, DataValue::PigVariant(variant));
}

pub fn set_sound_variant(entity: &mut Entity, sound_variant: i32) {
    entity.data.set(DATA_SOUND_VARIANT_ID, DataValue::PigSoundVariant(sound_variant));
}

impl Pig {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        let temperate = variant_utils::read_variant(VARIANT_REGISTRY, "minecraft:temperate").unwrap_or(0);
        let classic = variant_utils::read_variant(SOUND_VARIANT_REGISTRY, "minecraft:classic").unwrap_or(0);
        data.extend([DataValue::Int(0), DataValue::PigVariant(temperate), DataValue::PigSoundVariant(classic)]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:pig"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&PIG_ATTRIBUTES), MobKind::Pig(Pig::default()));
        Animal::init(&mut mob);
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::Float(float_goal));
        goal_selector.add_goal(1, Goal::Panic(PanicGoal::new(1.25)));
        goal_selector.add_goal(3, Goal::Breed(BreedGoal::new(1.0)));
        goal_selector.add_goal(4, Goal::Tempt(TemptGoal::new(1.2, |i| i.is(items::CARROT_ON_A_STICK), false)));
        goal_selector.add_goal(4, Goal::Tempt(TemptGoal::new(1.2, |i| i.get_item().is(&item_tags::PIG_FOOD), false)));
        goal_selector.add_goal(5, Goal::FollowParent(FollowParentGoal::new(1.1)));
        goal_selector.add_goal(6, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::new(1.0)));
        goal_selector.add_goal(7, Goal::LookAtPlayer(LookAtPlayerGoal::new(6.0)));
        goal_selector.add_goal(8, Goal::RandomLookAround(RandomLookAroundGoal::default()));
    }

    pub fn get_sound_set(mob: &Mob) -> &'static PigSoundSet {
        let sound_variant = sound_variant(get_sound_variant(&mob.entity));
        if mob.is_baby() { &sound_variant.baby_sounds } else { &sound_variant.adult_sounds }
    }
}

impl ServerLevel {
    pub fn is_pig(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::Pig(_)))
    }

    pub fn pig_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let has_food = is_food(&self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty));
        if !has_food && self.is_saddled(id) && !self.is_vehicle(id) && !self.player(player).is_secondary_use_active() {
            self.start_riding(player, id, false, true);
            return InteractionResult::SUCCESS;
        }
        let interaction_result = self.animal_mob_interact(id, player, hand);
        if !interaction_result.consumes_action() {
            let item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
            if !self.is_equippable_in_slot(id, &item_stack, EquipmentSlot::Saddle) {
                return InteractionResult::Pass;
            }
            let mut item_stack = item_stack;
            let result = crate::world::item::interact_living_entity(self, &mut item_stack, player, id, hand);
            self.player_mut(player).update_item_in_hand(hand, &item_stack);
            return result;
        }
        interaction_result
    }

    pub fn pig_get_controlling_passenger(&self, id: EntityId) -> Option<EntityId> {
        if self.is_saddled(id)
            && let Some(player) = self.first_passenger(id).filter(|&passenger| self.is_player(passenger))
            && self.is_holding(player, items::CARROT_ON_A_STICK)
        {
            return Some(player);
        }
        self.mob_get_controlling_passenger(id)
    }

    pub fn pig_tick_ridden(&mut self, id: EntityId, controller: EntityId, ridden_input: Vec3) {
        self.living_entity_tick_ridden(id, controller, ridden_input);
        let (y_rot, x_rot) = (self.entity(controller).y_rot, self.entity(controller).x_rot);
        let mob = self.mob_mut(id).expect("a pig");
        mob.entity.set_rot(y_rot, x_rot * 0.5);
        let y_rot = mob.entity.y_rot;
        mob.entity.y_rot_o = y_rot;
        mob.entity.y_head_rot = y_rot;
        mob.living.y_body_rot = y_rot;
        let Mob { entity, kind: MobKind::Pig(pig), .. } = mob else { unreachable!("not a pig") };
        pig.steering.tick_boost(entity, DATA_BOOST_TIME);
    }

    pub fn pig_get_ridden_input(&self, _id: EntityId, _controller: EntityId, _self_input: Vec3) -> Vec3 {
        Vec3::new(0.0, 0.0, 1.0)
    }

    pub fn pig_get_ridden_speed(&self, id: EntityId, _controller: EntityId) -> f32 {
        let mob = self.mob(id).expect("a pig");
        let boost_factor = pig(mob).steering.boost_factor(&mob.entity, DATA_BOOST_TIME);
        (self.attribute_value(id, Attribute::MovementSpeed) * 0.225 * boost_factor as f64) as f32
    }

    pub fn pig_boost(&mut self, id: EntityId) -> bool {
        let mob = self.mob_mut(id).expect("a pig");
        let Mob { entity, kind: MobKind::Pig(pig), .. } = mob else { unreachable!("not a pig") };
        pig.steering.boost(entity, DATA_BOOST_TIME)
    }

    pub fn pig_can_use_slot(&self, id: EntityId, slot: EquipmentSlot) -> bool {
        if slot != EquipmentSlot::Saddle { true } else { self.is_alive(id) && !self.is_baby(id) }
    }

    pub fn pig_thunder_hit(&mut self, id: EntityId, lightning_bolt: EntityId) {
        if self.difficulty != Difficulty::Peaceful {
            let params = ConversionParams::single(self, id, false, true);
            let zombified_piglin = self.convert_to(id, "minecraft:zombified_piglin", params, |level, zombified_piglin| {
                let mut random = std::mem::replace(&mut level.entity_mut(id).random, wafer_core::random::LegacyRandomSource::new(0));
                level.zombified_piglin_populate_default_equipment_slots(zombified_piglin, &mut random);
                level.entity_mut(id).random = random;
                level.mob_mut(zombified_piglin).expect("a zombified piglin").persistence_required = true;
            });
            if zombified_piglin.is_none() {
                self.entity_thunder_hit(id, lightning_bolt);
            }
        } else {
            self.entity_thunder_hit(id, lightning_bolt);
        }
    }

    pub fn pig_get_breed_offspring(&mut self, id: EntityId, partner: EntityId) -> Option<Box<Mob>> {
        let mut baby = Box::new(Pig::create());
        if self.mob(partner).is_some_and(|mob| matches!(mob.kind, MobKind::Pig(_))) {
            let variant = if self.entity_mut(id).random.next_boolean() { get_variant(self.entity(id)) } else { get_variant(self.entity(partner)) };
            set_variant(&mut baby.entity, variant);
        }
        Some(baby)
    }
}

pub fn finalize_spawn(level: &mut (impl ServerLevelAccessor + ?Sized), mob: &mut Mob) {
    let pos = mob.entity.block_position();
    if let Some(variant) = variant_utils::select_variant_to_spawn(level, pos, spawn_selectors()) {
        set_variant(&mut mob.entity, variant);
    }
    let sound_variant = variant_utils::pick_random_sound_variant(SOUND_VARIANT_REGISTRY, level.get_random());
    set_sound_variant(&mut mob.entity, sound_variant);
}
