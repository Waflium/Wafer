use wafer_core::random::{LegacyRandomSource, RandomSource};
use wafer_core::{Vec3, mth};
use wafer_data::{item_tags, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::goat_ai;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::ai::brain::{self, Brain, Provider};
use crate::world::entity::ai::sensing::sensor_type::SensorType;
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::{Attribute, GOAT_ATTRIBUTES};
use crate::world::entity::data::{DataValue, Pose, ageable_mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{Entity, EntityDimensions, EntityId, ItemEntity, LevelEntity, sound_event};
use crate::world::item::component as dc;
use crate::world::item::{ItemStack, item_utils};
use crate::world::level::pathfinder::path_type::PathType;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::{InteractionHand, InteractionResult};

pub const LONG_JUMPING_DIMENSION_SCALE_FACTOR: f32 = 0.7;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.9, 1.3).with_passenger_attachments(&[Vec3::new(0.0, 1.1125f32 as f64, 0.0)]);
pub const BABY_DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.45, 0.65).with_eye_height(0.59375).with_passenger_attachments(&[Vec3::new(0.0, 0.53125, 0.0)]);
const ADULT_ATTACK_DAMAGE: i32 = 2;
const BABY_ATTACK_DAMAGE: i32 = 1;
pub const GOAT_FALL_DAMAGE_REDUCTION: i32 = 10;
pub const GOAT_SCREAMING_CHANCE: f64 = 0.02;
pub const UNIHORN_CHANCE: f64 = 0.1f32 as f64;
pub const DATA_IS_SCREAMING_GOAT: usize = 18;
pub const DATA_HAS_LEFT_HORN: usize = 19;
pub const DATA_HAS_RIGHT_HORN: usize = 20;
const DEFAULT_IS_SCREAMING: bool = false;
const DEFAULT_HAS_LEFT_HORN: bool = true;
const DEFAULT_HAS_RIGHT_HORN: bool = true;

pub static BRAIN_PROVIDER: Provider = brain::provider(
    &[SensorType::NearestLivingEntities, SensorType::NearestPlayers, SensorType::NearestItems, SensorType::NearestAdult, SensorType::HurtBy, SensorType::FoodTemptations],
    |_| goat_ai::get_activities(),
);

#[derive(Default)]
pub struct Goat {
    pub animal: Animal,
    is_lowering_head: bool,
    lower_head_tick: i32,
}

fn flag(entity: &Entity, id: usize) -> bool {
    matches!(entity.data.get(id), DataValue::Boolean(true))
}

pub fn is_screaming_goat(entity: &Entity) -> bool {
    flag(entity, DATA_IS_SCREAMING_GOAT)
}

pub fn set_screaming_goat(entity: &mut Entity, is_screaming_goat: bool) {
    entity.data.set(DATA_IS_SCREAMING_GOAT, DataValue::Boolean(is_screaming_goat));
}

pub fn has_left_horn(entity: &Entity) -> bool {
    flag(entity, DATA_HAS_LEFT_HORN)
}

pub fn has_right_horn(entity: &Entity) -> bool {
    flag(entity, DATA_HAS_RIGHT_HORN)
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::GOAT_FOOD)
}

pub fn get_default_dimensions(baby: bool, pose: Pose) -> EntityDimensions {
    let entity_dimensions = if baby { BABY_DIMENSIONS } else { DIMENSIONS };
    if pose == Pose::LongJumping { entity_dimensions.scale(LONG_JUMPING_DIMENSION_SCALE_FACTOR) } else { entity_dimensions }
}

pub fn get_ambient_sound(entity: &Entity) -> &'static str {
    if is_screaming_goat(entity) { "minecraft:entity.goat.screaming.ambient" } else { "minecraft:entity.goat.ambient" }
}

pub fn get_hurt_sound(entity: &Entity) -> &'static str {
    if is_screaming_goat(entity) { "minecraft:entity.goat.screaming.hurt" } else { "minecraft:entity.goat.hurt" }
}

pub fn get_death_sound(entity: &Entity) -> &'static str {
    if is_screaming_goat(entity) { "minecraft:entity.goat.screaming.death" } else { "minecraft:entity.goat.death" }
}

pub fn get_step_sound() -> &'static str {
    "minecraft:entity.goat.step"
}

fn get_milking_sound(entity: &Entity) -> &'static str {
    if is_screaming_goat(entity) { "minecraft:entity.goat.screaming.milk" } else { "minecraft:entity.goat.milk" }
}

pub fn get_max_head_y_rot() -> i32 {
    15
}

impl Goat {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        data.extend([DataValue::Boolean(DEFAULT_IS_SCREAMING), DataValue::Boolean(DEFAULT_HAS_LEFT_HORN), DataValue::Boolean(DEFAULT_HAS_RIGHT_HORN)]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:goat"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&GOAT_ATTRIBUTES), MobKind::Goat(Goat::default()));
        Animal::init(&mut mob);
        mob.ai.navigation.set_can_float(true);
        mob.set_pathfinding_malus(PathType::PowderSnow, -1.0);
        mob.set_pathfinding_malus(PathType::OnTopOfPowderSnow, -1.0);
        mob
    }

    pub fn make_brain(mob: &mut Mob, packed_brain: brain::Packed) -> Brain {
        BRAIN_PROVIDER.make_brain(mob, packed_brain)
    }

    pub fn age_boundary_reached(mob: &mut Mob) {
        let attack_damage = if mob.is_baby() { BABY_ATTACK_DAMAGE } else { ADULT_ATTACK_DAMAGE };
        mob.living.attributes.instance(Attribute::AttackDamage).expect("goats have an attack damage").set_base_value(attack_damage as f64);
    }

    pub fn set_y_head_rot(mob: &mut Mob, y_head_rot: f32) {
        let max_head_y_rot = get_max_head_y_rot() as f32;
        let delta_from_body = mth::degrees_difference(mob.living.y_body_rot, y_head_rot);
        let delta_from_body_clamped = mth::clamp_f(delta_from_body, -max_head_y_rot, max_head_y_rot);
        mob.entity.y_head_rot = mob.living.y_body_rot + delta_from_body_clamped;
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        out.push(("IsScreamingGoat".into(), Nbt::Byte(is_screaming_goat(&mob.entity) as i8)));
        out.push(("HasLeftHorn".into(), Nbt::Byte(has_left_horn(&mob.entity) as i8)));
        out.push(("HasRightHorn".into(), Nbt::Byte(has_right_horn(&mob.entity) as i8)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, nbt: &Nbt) {
        let boolean = |key: &str, default: bool| nbt.get(key).and_then(Nbt::as_i64).map_or(default, |v| v != 0);
        set_screaming_goat(&mut mob.entity, boolean("IsScreamingGoat", DEFAULT_IS_SCREAMING));
        mob.entity.data.set(DATA_HAS_LEFT_HORN, DataValue::Boolean(boolean("HasLeftHorn", DEFAULT_HAS_LEFT_HORN)));
        mob.entity.data.set(DATA_HAS_RIGHT_HORN, DataValue::Boolean(boolean("HasRightHorn", DEFAULT_HAS_RIGHT_HORN)));
    }
}

fn goat_mut(mob: &mut Mob) -> &mut Goat {
    match &mut mob.kind {
        MobKind::Goat(goat) => goat,
        _ => unreachable!("a goat"),
    }
}

pub fn finalize_spawn(level: &mut (impl ServerLevelAccessor + ?Sized), mob: &mut Mob) {
    let random = level.get_random();
    goat_ai::init_memories(&mut mob.living.brain, random);
    set_screaming_goat(&mut mob.entity, random.next_double() < GOAT_SCREAMING_CHANCE);
    Goat::age_boundary_reached(mob);
    if !mob.is_baby() && random.next_float() < UNIHORN_CHANCE as f32 {
        let horn_to_remove = if random.next_boolean() { DATA_HAS_LEFT_HORN } else { DATA_HAS_RIGHT_HORN };
        mob.entity.data.set(horn_to_remove, DataValue::Boolean(false));
    }
}

fn uuid_hash_code(uuid: u128) -> i32 {
    let hilo = (uuid >> 64) as u64 ^ uuid as u64;
    ((hilo >> 32) as i32) ^ (hilo as i32)
}

impl ServerLevel {
    pub fn create_horn(&self, id: EntityId) -> ItemStack {
        let entity = self.entity(id);
        let mut random = LegacyRandomSource::new(uuid_hash_code(entity.uuid) as i64);
        let key = if is_screaming_goat(entity) { "minecraft:screaming_goat_horns" } else { "minecraft:regular_goat_horns" };
        let contents = wafer_data::tag("minecraft:instrument", key);
        let mut horn = ItemStack::new(items::GOAT_HORN, 1);
        if !contents.is_empty() {
            let instrument = contents[random.next_int_bounded(contents.len() as i32) as usize];
            let name = crate::world::item::component::codec::registry("minecraft:instrument")[instrument as usize];
            horn.set(dc::INSTRUMENT, Some(Nbt::String(name.to_string())));
        }
        horn
    }

    pub fn goat_drop_horn(&mut self, id: EntityId) -> bool {
        if self.is_baby(id) {
            return false;
        }
        let entity = self.entity(id);
        let (has_left, has_right) = (has_left_horn(entity), has_right_horn(entity));
        if !has_left && !has_right {
            return false;
        }
        let horn_to_drop = if !has_left {
            DATA_HAS_RIGHT_HORN
        } else if !has_right {
            DATA_HAS_LEFT_HORN
        } else if self.entity_mut(id).random.next_boolean() {
            DATA_HAS_LEFT_HORN
        } else {
            DATA_HAS_RIGHT_HORN
        };
        self.entity_mut(id).data.set(horn_to_drop, DataValue::Boolean(false));
        let body_position = self.entity(id).position();
        let item = self.create_horn(id);
        let random = &mut self.entity_mut(id).random;
        let delta_x = mth::random_between(random, -0.2, 0.2) as f64;
        let delta_y = mth::random_between(random, 0.3, 0.7) as f64;
        let delta_z = mth::random_between(random, -0.2, 0.2) as f64;
        let item_entity = ItemEntity::with_movement(body_position, item, Vec3::new(delta_x, delta_y, delta_z));
        self.add_fresh_entity(LevelEntity::Item(item_entity));
        true
    }

    pub fn goat_get_breed_offspring(&mut self, id: EntityId, partner: EntityId) -> Option<Box<Mob>> {
        let mut new_goat = Box::new(Goat::create());
        goat_ai::init_memories(&mut new_goat.living.brain, &mut self.random);
        let parent = if self.random.next_boolean() { id } else { partner };
        let baby_is_screaming = self.mob(parent).is_some_and(|mob| matches!(mob.kind, MobKind::Goat(_)) && is_screaming_goat(&mob.entity)) || self.random.next_double() < GOAT_SCREAMING_CHANCE;
        set_screaming_goat(&mut new_goat.entity, baby_is_screaming);
        Some(new_goat)
    }

    pub fn goat_calculate_fall_damage(&self, id: EntityId, fall_distance: f64, damage_modifier: f32) -> i32 {
        self.living_entity_calculate_fall_damage(id, fall_distance, damage_modifier) - GOAT_FALL_DAMAGE_REDUCTION
    }

    pub fn goat_custom_server_ai_step(&mut self, id: EntityId) {
        Brain::tick(self, id);
        if self.mob(id).is_none() {
            return;
        }
        goat_ai::update_activity(self.get_brain_mut(id));
        self.animal_custom_server_ai_step(id);
    }

    pub fn goat_play_eating_sound(&mut self, id: EntityId) {
        let sound = if is_screaming_goat(self.entity(id)) { "minecraft:entity.goat.screaming.eat" } else { "minecraft:entity.goat.eat" };
        let pitch = mth::random_between(&mut self.random, 0.8, 1.2);
        self.play_sound_from_entity(None, id, sound_event(sound), SoundSource::Neutral, 1.0, pitch);
    }

    pub fn goat_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let held_item = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if held_item.is(items::BUCKET) && !self.is_baby(id) {
            let sound = get_milking_sound(self.entity(id));
            self.entity_play_sound(player, sound_event(sound), 1.0, 1.0);
            let bucket_or_milk_bucket = item_utils::create_filled_result(self, player, hand, held_item, ItemStack::new(items::MILK_BUCKET, 1), true);
            self.player_mut(player).set_item_in_hand(hand, Some(bucket_or_milk_bucket));
            return InteractionResult::SUCCESS;
        }
        let interaction_result = self.animal_mob_interact(id, player, hand);
        let held_item = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if interaction_result.consumes_action() && is_food(&held_item) {
            self.goat_play_eating_sound(id);
        }
        interaction_result
    }

    pub fn goat_ai_step(&mut self, id: EntityId) {
        let goat = goat_mut(self.mob_mut(id).expect("a goat"));
        if goat.is_lowering_head {
            goat.lower_head_tick += 1;
        } else {
            goat.lower_head_tick -= 2;
        }
        goat.lower_head_tick = goat.lower_head_tick.clamp(0, 20);
        self.animal_ai_step(id);
    }
}
