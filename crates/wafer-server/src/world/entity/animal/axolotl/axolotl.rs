use wafer_core::random::RandomSource;
use wafer_core::Vec3;
use wafer_data::{damage_types, item_tags, items, mob_effects, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::axolotl_ai;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::ageable_mob::{AgeableMobGroupData, BABY_START_AGE};
use crate::world::entity::ai::brain::{self, Brain, Provider};
use crate::world::entity::ai::control::look_control::LookControlKind;
use crate::world::entity::ai::control::move_control::MoveControlKind;
use crate::world::entity::ai::control::smooth_swimming_move_control::SmoothSwimmingMoveControl;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::sensing::sensor_type::SensorType;
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::AXOLOTL_ATTRIBUTES;
use crate::world::entity::bucketable;
use crate::world::entity::data::{DataValue, ageable_mob_defaults, ids};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{Entity, EntityDimensions, EntityId, MoverType};
use crate::world::item::component as dc;
use crate::world::item::{ItemStack, item_utils};
use crate::world::level::pathfinder::path_type::PathType;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::{InteractionHand, InteractionResult};
use crate::world::level::storage::tag_value_input;

pub const TOTAL_PLAYDEAD_TIME: i32 = 200;
pub const PLAYER_REGEN_DETECTION_RANGE: f64 = 20.0;
pub const RARE_VARIANT_CHANCE: i32 = 1200;
pub const AXOLOTL_TOTAL_AIR_SUPPLY: i32 = 6000;
pub const VARIANT_TAG: &str = "Variant";
const REHYDRATE_AIR_SUPPLY: i32 = 1800;
const REGEN_BUFF_MAX_DURATION: i32 = 2400;
const REGEN_BUFF_BASE_DURATION: i32 = 100;
const DEFAULT_FROM_BUCKET: bool = false;
pub const DATA_VARIANT: usize = 18;
pub const DATA_PLAYING_DEAD: usize = 19;
pub const FROM_BUCKET: usize = 20;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.75, 0.42).with_eye_height(0.2751);
pub const BABY_DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.375, 0.21).with_eye_height(0.09375).with_passenger_attachments(&[Vec3::new(0.0, 0.1875, 0.0)]);
pub const MOVE_CONTROL: SmoothSwimmingMoveControl = SmoothSwimmingMoveControl::new(85, 10, 0.1, 0.5, false);

pub static BRAIN_PROVIDER: Provider = brain::provider(
    &[SensorType::NearestLivingEntities, SensorType::NearestAdult, SensorType::HurtBy, SensorType::AxolotlAttackables, SensorType::FoodTemptations],
    |_| axolotl_ai::get_activities(),
);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Variant {
    Lucy,
    Wild,
    Gold,
    Cyan,
    Blue,
}

impl Variant {
    pub const DEFAULT: Variant = Variant::Lucy;
    pub const VALUES: [Variant; 5] = [Variant::Lucy, Variant::Wild, Variant::Gold, Variant::Cyan, Variant::Blue];

    pub fn get_id(self) -> i32 {
        self as i32
    }

    pub fn get_name(self) -> &'static str {
        ["lucy", "wild", "gold", "cyan", "blue"][self as usize]
    }

    pub fn get_serialized_name(self) -> &'static str {
        self.get_name()
    }

    fn common(self) -> bool {
        self != Variant::Blue
    }

    pub fn by_id(id: i32) -> Variant {
        if (0..Variant::VALUES.len() as i32).contains(&id) { Variant::VALUES[id as usize] } else { Variant::VALUES[0] }
    }

    pub fn by_name(name: &str) -> Option<Variant> {
        Variant::VALUES.into_iter().find(|variant| variant.get_serialized_name() == name)
    }

    pub fn get_common_spawn_variant(random: &mut (impl RandomSource + ?Sized)) -> Variant {
        Variant::get_spawn_variant(random, true)
    }

    pub fn get_rare_spawn_variant(random: &mut (impl RandomSource + ?Sized)) -> Variant {
        Variant::get_spawn_variant(random, false)
    }

    fn get_spawn_variant(random: &mut (impl RandomSource + ?Sized), common: bool) -> Variant {
        let valid_variants: Vec<Variant> = Variant::VALUES.into_iter().filter(|variant| variant.common() == common).collect();
        valid_variants[random.next_int_bounded(valid_variants.len() as i32) as usize]
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct AxolotlGroupData {
    pub ageable_mob_group_data: AgeableMobGroupData,
    pub types: [Variant; 2],
}

impl AxolotlGroupData {
    pub fn new(types: [Variant; 2]) -> AxolotlGroupData {
        AxolotlGroupData { ageable_mob_group_data: AgeableMobGroupData::new(false), types }
    }

    pub fn get_variant(&self, random: &mut (impl RandomSource + ?Sized)) -> Variant {
        self.types[random.next_int_bounded(self.types.len() as i32) as usize]
    }
}

#[derive(Default)]
pub struct Axolotl {
    pub animal: Animal,
}

pub fn get_variant(entity: &Entity) -> Variant {
    match entity.data.get(DATA_VARIANT) {
        DataValue::Int(id) => Variant::by_id(*id),
        _ => Variant::DEFAULT,
    }
}

pub fn set_variant(entity: &mut Entity, variant: Variant) {
    entity.data.set(DATA_VARIANT, DataValue::Int(variant.get_id()));
}

pub fn is_playing_dead(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_PLAYING_DEAD), DataValue::Boolean(true))
}

pub fn set_playing_dead(entity: &mut Entity, playing_dead: bool) {
    entity.data.set(DATA_PLAYING_DEAD, DataValue::Boolean(playing_dead));
}

pub fn from_bucket(entity: &Entity) -> bool {
    matches!(entity.data.get(FROM_BUCKET), DataValue::Boolean(true))
}

pub fn set_from_bucket(entity: &mut Entity, from_bucket: bool) {
    entity.data.set(FROM_BUCKET, DataValue::Boolean(from_bucket));
}

fn use_rare_variant(random: &mut impl RandomSource) -> bool {
    random.next_int_bounded(RARE_VARIANT_CHANCE) == 0
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::AXOLOTL_FOOD)
}

pub fn get_walk_target_value() -> f32 {
    0.0
}

pub fn is_pushed_by_fluid() -> bool {
    false
}

pub fn get_max_head_x_rot() -> i32 {
    1
}

pub fn get_max_head_y_rot() -> i32 {
    1
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.axolotl.hurt"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.axolotl.death"
}

pub fn get_ambient_sound(entity: &Entity) -> &'static str {
    if entity.is_in_water() { "minecraft:entity.axolotl.idle_water" } else { "minecraft:entity.axolotl.idle_air" }
}

pub fn get_swim_splash_sound() -> &'static str {
    "minecraft:entity.axolotl.splash"
}

pub fn get_swim_sound() -> &'static str {
    "minecraft:entity.axolotl.swim"
}

pub fn get_pickup_sound() -> &'static str {
    "minecraft:item.bucket.fill_axolotl"
}

pub fn get_bucket_item_stack() -> ItemStack {
    ItemStack::new(items::AXOLOTL_BUCKET, 1)
}

pub fn check_spawn_obstruction(mob: &Mob, level: &mut (impl ServerLevelAccessor + ?Sized)) -> bool {
    level.is_unobstructed(&mob.entity.bounding_box())
}

impl Axolotl {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        data[ids::AIR_SUPPLY] = DataValue::Int(AXOLOTL_TOTAL_AIR_SUPPLY);
        data.extend([DataValue::Int(0), DataValue::Boolean(false), DataValue::Boolean(DEFAULT_FROM_BUCKET)]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:axolotl"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&AXOLOTL_ATTRIBUTES), MobKind::Axolotl(Axolotl::default()));
        Animal::init(&mut mob);
        mob.set_pathfinding_malus(PathType::Water, 0.0);
        mob.ai.move_control.kind = MoveControlKind::Axolotl(MOVE_CONTROL);
        mob.ai.look_control.kind = LookControlKind::Axolotl(20);
        mob
    }

    pub fn make_brain(mob: &mut Mob, packed_brain: brain::Packed) -> Brain {
        BRAIN_PROVIDER.make_brain(mob, packed_brain)
    }

    pub fn get_default_dimensions(mob: &Mob) -> EntityDimensions {
        if mob.is_baby() { BABY_DIMENSIONS } else { DIMENSIONS }
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        out.push((VARIANT_TAG.into(), Nbt::Int(get_variant(&mob.entity).get_id())));
        out.push(("FromBucket".into(), Nbt::Byte(from_bucket(&mob.entity) as i8)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, nbt: &Nbt) {
        let variant = tag_value_input::get_i64(nbt, VARIANT_TAG).map_or(Variant::DEFAULT, |id| Variant::by_id(id as i32));
        set_variant(&mut mob.entity, variant);
        set_from_bucket(&mut mob.entity, tag_value_input::get_i64(nbt, "FromBucket").is_some_and(|v| v != 0));
    }

    pub fn save_to_bucket_tag(mob: &Mob, bucket: &mut ItemStack) {
        bucketable::save_default_data_to_bucket_tag(mob, bucket);
        bucket.set(dc::AXOLOTL_VARIANT, Some(Nbt::String(get_variant(&mob.entity).get_serialized_name().into())));
        let mut tag = match bucket.get(dc::BUCKET_ENTITY_DATA) {
            Some(Nbt::Compound(tag)) => tag.clone(),
            _ => Vec::new(),
        };
        let mut put = |key: &str, value: Nbt| {
            tag.retain(|(k, _)| k != key);
            tag.push((key.to_string(), value));
        };
        put("Age", Nbt::Int(mob.ageable().expect("an axolotl").age));
        put("AgeLocked", Nbt::Byte(mob.is_age_locked() as i8));
        let brain = &mob.living.brain;
        if brain.has_memory_value(memory_module_type::HAS_HUNTING_COOLDOWN) {
            put("HuntingCooldown", Nbt::Long(brain.get_time_until_expiry(memory_module_type::HAS_HUNTING_COOLDOWN)));
        }
        bucket.set(dc::BUCKET_ENTITY_DATA, Some(Nbt::Compound(tag)));
    }

    pub fn load_from_bucket_tag(mob: &mut Mob, tag: &Nbt) {
        bucketable::load_default_data_from_bucket_tag(mob, tag);
        let age = tag_value_input::get_i64(tag, "Age").map_or(0, |v| v as i32);
        if mob.set_age(age) {
            mob.refresh_dimensions();
        }
        let age_locked = tag_value_input::get_i64(tag, "AgeLocked").is_some_and(|v| v != 0);
        mob.entity.data.set(ids::AGE_LOCKED, DataValue::Boolean(age_locked));
        match tag.get("HuntingCooldown").and_then(|nbt| match nbt {
            Nbt::Long(v) => Some(*v),
            _ => None,
        }) {
            Some(hunting_cooldown) => mob.living.brain.set_memory_with_expiry(memory_module_type::HAS_HUNTING_COOLDOWN, true, hunting_cooldown),
            None => mob.living.brain.set_memory_optional(memory_module_type::HAS_HUNTING_COOLDOWN, None),
        }
    }
}

pub fn finalize_spawn(level: &mut (impl ServerLevelAccessor + ?Sized), mob: &mut Mob, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
    let mut is_baby = false;
    let random = level.get_random();
    let group_data = match group_data {
        Some(SpawnGroupData::Axolotl(axolotl_group_data)) => {
            if axolotl_group_data.ageable_mob_group_data.get_group_size() >= 2 {
                is_baby = true;
            }
            SpawnGroupData::Axolotl(axolotl_group_data)
        }
        _ => {
            let first = Variant::get_common_spawn_variant(random);
            let second = Variant::get_common_spawn_variant(random);
            SpawnGroupData::Axolotl(AxolotlGroupData::new([first, second]))
        }
    };
    let SpawnGroupData::Axolotl(axolotl_group_data) = &group_data else { unreachable!("axolotl group data") };
    let variant = axolotl_group_data.get_variant(level.get_random());
    set_variant(&mut mob.entity, variant);
    if is_baby && mob.set_age(BABY_START_AGE) {
        mob.refresh_dimensions();
    }
    Some(group_data)
}

impl ServerLevel {
    pub fn axolotl_play_ambient_sound(&mut self, id: EntityId) {
        if !is_playing_dead(self.entity(id)) {
            self.mob_play_ambient_sound(id);
        }
    }

    pub fn axolotl_base_tick(&mut self, id: EntityId) {
        let air_supply = self.entity(id).air_supply();
        self.mob_base_tick(id);
        if self.mob(id).is_some_and(|mob| !mob.is_no_ai()) {
            self.axolotl_handle_air_supply(id, air_supply);
        }
    }

    fn axolotl_handle_air_supply(&mut self, id: EntityId, pre_tick_air_supply: i32) {
        if self.is_alive(id) && !self.is_in_water_or_rain(id) {
            self.entity_mut(id).set_air_supply(pre_tick_air_supply - 1);
            if self.should_take_drowning_damage(id) {
                self.entity_mut(id).set_air_supply(0);
                self.hurt_server(id, DamageSource::new(damage_types::DRY_OUT), 2.0);
            }
        } else {
            let max_air_supply = self.entity(id).get_max_air_supply();
            self.entity_mut(id).set_air_supply(max_air_supply);
        }
    }

    pub fn rehydrate(&mut self, id: EntityId) {
        let e = self.entity_mut(id);
        let new_air_supply = e.air_supply() + REHYDRATE_AIR_SUPPLY;
        let max_air_supply = e.get_max_air_supply();
        e.set_air_supply(new_air_supply.min(max_air_supply));
    }

    pub fn axolotl_get_breed_offspring(&mut self, id: EntityId, partner: EntityId) -> Option<Box<Mob>> {
        let mut baby = Box::new(Axolotl::create());
        let (own_variant, partner_variant) = (get_variant(self.entity(id)), get_variant(self.entity(partner)));
        let random = &mut self.entity_mut(id).random;
        let variant = if use_rare_variant(random) {
            Variant::get_rare_spawn_variant(random)
        } else if random.next_boolean() {
            own_variant
        } else {
            partner_variant
        };
        set_variant(&mut baby.entity, variant);
        baby.persistence_required = true;
        Some(baby)
    }

    pub fn axolotl_custom_server_ai_step(&mut self, id: EntityId) {
        Brain::tick(self, id);
        if self.mob(id).is_none() {
            return;
        }
        axolotl_ai::update_activity(self.get_brain_mut(id));
        let mob = self.mob_mut(id).expect("an axolotl");
        if !mob.is_no_ai() {
            let play_dead_ticks = mob.living.brain.get_memory(memory_module_type::PLAY_DEAD_TICKS).copied();
            set_playing_dead(&mut mob.entity, play_dead_ticks.is_some_and(|ticks| ticks > 0));
        }
    }

    pub fn axolotl_play_attack_sound(&mut self, id: EntityId) {
        self.entity_play_sound(id, crate::world::entity::sound_event("minecraft:entity.axolotl.attack"), 1.0, 1.0);
    }

    pub fn axolotl_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        let current_health = self.health(id);
        let max_health = self.max_health(id);
        let no_ai = self.mob(id).expect("an axolotl").is_no_ai();
        if !no_ai
            && self.entity_mut(id).random.next_int_bounded(3) == 0
            && ((self.entity_mut(id).random.next_int_bounded(3) as f32) < damage || current_health / max_health < 0.5)
            && damage < current_health
            && self.entity(id).is_in_water()
            && (source.causing_entity.is_some() || source.direct_entity.is_some())
            && !is_playing_dead(self.entity(id))
        {
            self.get_brain_mut(id).set_memory(memory_module_type::PLAY_DEAD_TICKS, TOTAL_PLAYDEAD_TIME);
        }
        self.living_entity_hurt_server(id, source, damage)
    }

    pub fn axolotl_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        match bucketable::bucket_mob_pickup(self, player, hand, id) {
            Some(result) => result,
            None => self.animal_mob_interact(id, player, hand),
        }
    }

    pub fn axolotl_can_be_seen_as_enemy(&self, id: EntityId) -> bool {
        !is_playing_dead(self.entity(id)) && self.living_entity_can_be_seen_as_enemy(id)
    }

    pub fn axolotl_on_stop_attacking(level: &mut ServerLevel, body: EntityId, target: EntityId) {
        if level.entity_ref(target).is_none() || !level.is_dead_or_dying(target) {
            return;
        }
        let Some(last_damage_source) = level.get_last_damage_source(target).copied() else { return };
        let Some(player) = last_damage_source.causing_entity.filter(|&entity| level.is_player(entity)) else { return };
        let area = level.entity(body).bounding_box().inflate(PLAYER_REGEN_DETECTION_RANGE, PLAYER_REGEN_DETECTION_RANGE, PLAYER_REGEN_DETECTION_RANGE);
        let players_in_range: Vec<EntityId> = level.level_players().into_iter().filter(|&p| !level.is_spectator(p) && level.entity(p).bounding_box().intersects(&area)).collect();
        if players_in_range.contains(&player) {
            level.apply_supporting_effects(body, player);
        }
    }

    pub fn apply_supporting_effects(&mut self, id: EntityId, player: EntityId) {
        let regen_effect = self.living(player).effect(mob_effects::REGENERATION).cloned();
        if regen_effect.as_ref().is_none_or(|effect| effect.ends_within(REGEN_BUFF_MAX_DURATION - 1)) {
            let previous_duration = regen_effect.map_or(0, |effect| effect.duration);
            let regen_duration = REGEN_BUFF_MAX_DURATION.min(REGEN_BUFF_BASE_DURATION + previous_duration);
            self.add_effect(player, MobEffectInstance::new(mob_effects::REGENERATION, regen_duration, 0), Some(id));
        }
        self.remove_effect(player, mob_effects::MINING_FATIGUE);
    }

    pub fn axolotl_requires_custom_persistence(&self, id: EntityId) -> bool {
        self.mob_requires_custom_persistence(id) || from_bucket(self.entity(id))
    }

    pub fn axolotl_travel_in_water(&mut self, id: EntityId, input: Vec3, _base_gravity: f64, _is_falling: bool, _old_y: f64) {
        let speed = self.get_speed(id);
        self.entity_mut(id).move_relative(speed, input);
        let delta = self.entity(id).delta_movement;
        self.move_entity(id, MoverType::SelfMove, delta);
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.scale(0.9));
    }

    pub fn axolotl_use_player_item(&mut self, player: EntityId, hand: InteractionHand, item_stack: &mut ItemStack) {
        if item_stack.is(items::TROPICAL_FISH_BUCKET) {
            let result = item_utils::create_filled_result(self, player, hand, item_stack.clone(), ItemStack::new(items::WATER_BUCKET, 1), true);
            self.player_mut(player).set_item_in_hand(hand, Some(result));
        } else {
            self.use_player_item(player, hand, item_stack);
        }
    }

    pub fn axolotl_get_target(&self, id: EntityId) -> Option<EntityId> {
        self.get_target_from_brain(id)
    }

    pub fn axolotl_move_control_tick(&mut self, id: EntityId, control: SmoothSwimmingMoveControl) {
        if !is_playing_dead(self.entity(id)) {
            self.smooth_swimming_move_control_tick(id, control);
        }
    }

    pub fn axolotl_look_control_tick(&mut self, id: EntityId, max_y_rot_from_center: i32) {
        if !is_playing_dead(self.entity(id)) {
            self.smooth_swimming_look_control_tick(id, max_y_rot_from_center);
        }
    }
}
