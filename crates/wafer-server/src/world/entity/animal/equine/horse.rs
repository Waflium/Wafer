use wafer_core::Vec3;
use wafer_core::random::RandomSource;
use wafer_data::{EquipmentSlot, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::abstract_horse::{self, AbstractHorse, abstract_horse_defaults};
use super::markings::Markings;
use super::variant::Variant;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::ageable_mob::AgeableMobGroupData;
use crate::world::entity::attributes::{Attribute, HORSE_ATTRIBUTES};
use crate::world::entity::data::DataValue;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{Entity, EntityDimensions, EntityId, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::{InteractionHand, InteractionResult};

pub const DATA_ID_TYPE_VARIANT: usize = 19;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(1.3964844, 1.6).with_eye_height(1.52).with_passenger_attachments(&[Vec3::new(0.0, 1.44375f32 as f64, 0.0)]);
const BABY_DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(1.3964844, 1.6).with_eye_height(1.52).with_passenger_attachments(&[Vec3::new(0.0, (1.6f32 - 0.125f32) as f64, 0.0)]);
const DEFAULT_VARIANT: i32 = 0;

#[derive(Default)]
pub struct Horse {
    pub abstract_horse: AbstractHorse,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct HorseGroupData {
    pub ageable_mob_group_data: AgeableMobGroupData,
    pub variant: Variant,
}

impl HorseGroupData {
    pub fn new(variant: Variant) -> HorseGroupData {
        HorseGroupData { ageable_mob_group_data: AgeableMobGroupData::new(true), variant }
    }
}

pub fn baby_dimensions() -> EntityDimensions {
    BABY_DIMENSIONS.scale(abstract_horse::BABY_SCALE)
}

pub fn randomize_attributes(mob: &mut Mob, random: &mut (impl RandomSource + ?Sized)) {
    let attributes = &mut mob.living.attributes;
    let max_health = abstract_horse::generate_max_health(|bound| random.next_int_bounded(bound));
    attributes.instance(Attribute::MaxHealth).expect("horses have a max health").set_base_value(max_health as f64);
    let speed = abstract_horse::generate_speed(|| random.next_double());
    attributes.instance(Attribute::MovementSpeed).expect("horses have a movement speed").set_base_value(speed);
    let jump_strength = abstract_horse::generate_jump_strength(|| random.next_double());
    attributes.instance(Attribute::JumpStrength).expect("horses have a jump strength").set_base_value(jump_strength);
}

pub fn get_type_variant(entity: &Entity) -> i32 {
    match entity.data.get(DATA_ID_TYPE_VARIANT) {
        DataValue::Int(variant) => *variant,
        _ => DEFAULT_VARIANT,
    }
}

pub fn set_type_variant(entity: &mut Entity, i: i32) {
    entity.data.set(DATA_ID_TYPE_VARIANT, DataValue::Int(i));
}

pub fn set_variant_and_markings(entity: &mut Entity, variant: Variant, markings: Markings) {
    set_type_variant(entity, variant.get_id() & 0xFF | markings.get_id() << 8 & 0xFF00);
}

pub fn get_variant(entity: &Entity) -> Variant {
    Variant::by_id(get_type_variant(entity) & 0xFF)
}

pub fn set_variant(entity: &mut Entity, variant: Variant) {
    set_type_variant(entity, variant.get_id() & 0xFF | get_type_variant(entity) & -256);
}

pub fn get_markings(entity: &Entity) -> Markings {
    Markings::by_id((get_type_variant(entity) & 0xFF00) >> 8)
}

pub fn get_ambient_sound(mob: &Mob) -> &'static str {
    if mob.is_baby() { "minecraft:entity.baby_horse.ambient" } else { "minecraft:entity.horse.ambient" }
}

pub fn get_death_sound(mob: &Mob) -> &'static str {
    if mob.is_baby() { "minecraft:entity.baby_horse.death" } else { "minecraft:entity.horse.death" }
}

pub fn get_eating_sound(mob: &Mob) -> &'static str {
    if mob.is_baby() { "minecraft:entity.baby_horse.eat" } else { "minecraft:entity.horse.eat" }
}

pub fn get_hurt_sound(mob: &Mob) -> &'static str {
    if mob.is_baby() { "minecraft:entity.baby_horse.hurt" } else { "minecraft:entity.horse.hurt" }
}

pub fn get_angry_sound(mob: &Mob) -> &'static str {
    if mob.is_baby() { "minecraft:entity.baby_horse.angry" } else { "minecraft:entity.horse.angry" }
}

pub fn finalize_spawn(level: &mut (impl ServerLevelAccessor + ?Sized), mob: &mut Mob, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
    let random = level.get_random();
    let (variant, group_data) = match group_data {
        Some(SpawnGroupData::Horse(horse_group_data)) => (horse_group_data.variant, group_data),
        _ => {
            let variant = Variant::VALUES[random.next_int_bounded(Variant::VALUES.len() as i32) as usize];
            (variant, Some(SpawnGroupData::Horse(HorseGroupData::new(variant))))
        }
    };
    let markings = Markings::VALUES[level.get_random().next_int_bounded(Markings::VALUES.len() as i32) as usize];
    set_variant_and_markings(&mut mob.entity, variant, markings);
    abstract_horse::finalize_spawn(level, mob, group_data)
}

impl Horse {
    pub fn create() -> Mob {
        let mut data = abstract_horse_defaults();
        data.push(DataValue::Int(DEFAULT_VARIANT));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:horse"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&HORSE_ATTRIBUTES), MobKind::Horse(Horse::default()));
        AbstractHorse::init(&mut mob);
        mob
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        AbstractHorse::add_additional_save_data(mob, output);
        output.push(("Variant".into(), Nbt::Int(get_type_variant(&mob.entity))));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        AbstractHorse::read_additional_save_data(mob, input);
        let variant = input.get("Variant").and_then(crate::world::level::block::entity::int_value).unwrap_or(DEFAULT_VARIANT);
        set_type_variant(&mut mob.entity, variant);
    }
}

impl ServerLevel {
    pub fn is_horse(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::Horse(_)))
    }

    pub fn horse_play_gallop_sound(&mut self, id: EntityId, sound_type: &wafer_data::SoundType) {
        if self.entity_mut(id).random.next_int_bounded(10) == 0 {
            let sound = if self.is_baby(id) { "minecraft:entity.baby_horse.breathe" } else { "minecraft:entity.horse.breathe" };
            self.entity_play_sound(id, sound_event(sound), sound_type.volume * 0.6, sound_type.pitch);
        }
    }

    pub fn horse_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let should_open_inventory = !self.is_baby(id) && self.is_tamed(id) && self.player(player).is_secondary_use_active();
        if !self.is_vehicle(id) && !should_open_inventory && (!self.is_baby(id) || !self.is_holding(player, items::GOLDEN_DANDELION)) {
            let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
            if !item_stack.is_empty() {
                if abstract_horse::is_food(&item_stack) {
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

    pub fn horse_can_mate(&self, id: EntityId, partner: EntityId) -> bool {
        if partner == id {
            return false;
        }
        if !self.is_donkey(partner) && !self.is_horse(partner) { false } else { self.can_parent(id) && self.can_parent(partner) }
    }

    pub fn horse_get_breed_offspring(&mut self, id: EntityId, partner: EntityId) -> Option<Box<Mob>> {
        if self.is_donkey(partner) {
            let mut baby = Box::new(super::mule::Mule::create());
            self.set_offspring_attributes(id, partner, &mut baby);
            return Some(baby);
        }
        let mut baby = Box::new(Horse::create());
        let random = &mut self.entity_mut(id).random;
        let select_skin = random.next_int_bounded(9);
        let variant = if select_skin < 4 {
            get_variant(self.entity(id))
        } else if select_skin < 8 {
            get_variant(self.entity(partner))
        } else {
            Variant::VALUES[self.entity_mut(id).random.next_int_bounded(Variant::VALUES.len() as i32) as usize]
        };
        let select_marking = self.entity_mut(id).random.next_int_bounded(5);
        let markings = if select_marking < 2 {
            get_markings(self.entity(id))
        } else if select_marking < 4 {
            get_markings(self.entity(partner))
        } else {
            Markings::VALUES[self.entity_mut(id).random.next_int_bounded(Markings::VALUES.len() as i32) as usize]
        };
        set_variant_and_markings(&mut baby.entity, variant, markings);
        self.set_offspring_attributes(id, partner, &mut baby);
        Some(baby)
    }

    pub fn horse_hurt_armor(&mut self, id: EntityId, damage_source: &DamageSource, damage: f32) {
        self.do_hurt_equipment(id, damage_source, damage, &[EquipmentSlot::Body]);
    }
}
