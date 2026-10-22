use std::borrow::Cow;

use wafer_core::Vec3;
use wafer_core::random::RandomSource;
use wafer_data::{EquipmentSlot, game_rule, item_tags, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::abstract_piglin::{self, AbstractPiglin};
use super::piglin_ai;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::ai::brain::{self, Brain, Provider};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::sensing::sensor_type::SensorType;
use crate::world::entity::attributes::{Attribute, AttributeModifier, Operation, PIGLIN_ATTRIBUTES};
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::npc::inventory_carrier;
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{Entity, EntityDimensions, EntityId, entity_types};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::enchantment::enchantment_helper;
use crate::world::simple_container::{SimpleContainer, SimpleContainerKind};
use crate::world::{InteractionHand, InteractionResult};

const DATA_BABY_ID: usize = 17;
pub const DATA_IS_CHARGING_CROSSBOW: usize = 18;
const DATA_IS_DANCING: usize = 19;
const SPEED_MODIFIER_BABY_ID: &str = "minecraft:baby";
const SPEED_MODIFIER_BABY: AttributeModifier = AttributeModifier { id: Cow::Borrowed(SPEED_MODIFIER_BABY_ID), amount: 0.2f32 as f64, operation: Operation::AddMultipliedBase };
const CHANCE_OF_WEARING_EACH_ARMOUR_ITEM: f32 = 0.1;
const MAX_PASSENGERS_ON_ONE_HOGLIN: i32 = 3;
const PROBABILITY_OF_SPAWNING_AS_BABY: f32 = 0.2;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.6, 1.95)
    .with_eye_height(1.79)
    .with_passenger_attachments(&[Vec3::new(0.0, 2.0125f32 as f64, 0.0)])
    .with_vehicle_attachment(&[Vec3::new(0.0, 0.7f32 as f64, 0.0)]);
pub const BABY_DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(0.49, 0.98).with_eye_height(0.78).with_vehicle_attachment(&[Vec3::new(0.0, 0.1875f32 as f64, 0.0)]);
const PROBABILITY_OF_SPAWNING_WITH_CROSSBOW_INSTEAD_OF_SWORD: f64 = 0.5;
const DEFAULT_IS_BABY: bool = false;
const DEFAULT_CANNOT_HUNT: bool = false;
pub const INVENTORY_SLOT_OFFSET: i32 = 300;
const INVENTORY_SIZE: usize = 8;

pub static BRAIN_PROVIDER: Provider = brain::provider_with_memories(
    &[
        memory_module_type::UNIVERSAL_ANGER.erase(),
        memory_module_type::ATE_RECENTLY.erase(),
        memory_module_type::SPEAR_FLEEING_TIME.erase(),
        memory_module_type::SPEAR_FLEEING_POSITION.erase(),
        memory_module_type::SPEAR_CHARGE_POSITION.erase(),
        memory_module_type::SPEAR_ENGAGE_TIME.erase(),
    ],
    &[SensorType::NearestLivingEntities, SensorType::NearestPlayers, SensorType::NearestItems, SensorType::HurtBy, SensorType::PiglinSpecificSensor],
    |_| piglin_ai::get_activities(),
);

pub struct Piglin {
    pub abstract_piglin: AbstractPiglin,
    pub inventory: SimpleContainer,
    cannot_hunt: bool,
}

pub fn piglin(mob: &Mob) -> &Piglin {
    match &mob.kind {
        MobKind::Piglin(piglin) => piglin,
        _ => unreachable!("a piglin"),
    }
}

pub fn piglin_mut(mob: &mut Mob) -> &mut Piglin {
    match &mut mob.kind {
        MobKind::Piglin(piglin) => piglin,
        _ => unreachable!("a piglin"),
    }
}

pub fn is_baby(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_BABY_ID), DataValue::Boolean(true))
}

pub fn set_baby(mob: &mut Mob, baby: bool) {
    let changed = is_baby(&mob.entity) != baby;
    mob.entity.data.set(DATA_BABY_ID, DataValue::Boolean(baby));
    if changed {
        mob.refresh_dimensions();
    }
    let speed = mob.living.attributes.instance(Attribute::MovementSpeed).expect("piglins have movement speed");
    speed.remove_modifier(SPEED_MODIFIER_BABY_ID);
    if baby {
        speed.add_transient_modifier(SPEED_MODIFIER_BABY);
    }
}

pub fn is_dancing(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_IS_DANCING), DataValue::Boolean(true))
}

pub fn can_use_non_melee_weapon(item: &ItemStack) -> bool {
    item.get_item() == items::CROSSBOW || item.has(dc::KINETIC_WEAPON)
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.piglin.hurt"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.piglin.death"
}

pub fn get_step_sound() -> &'static str {
    "minecraft:entity.piglin.step"
}

impl Piglin {
    pub fn create() -> Mob {
        let mut data = mob_defaults();
        data.push(DataValue::Boolean(false));
        data.push(DataValue::Boolean(DEFAULT_IS_BABY));
        data.push(DataValue::Boolean(false));
        data.push(DataValue::Boolean(false));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:piglin"), DIMENSIONS, data);
        let piglin = Piglin { abstract_piglin: AbstractPiglin::default(), inventory: SimpleContainer::new(SimpleContainerKind::Plain, INVENTORY_SIZE), cannot_hunt: DEFAULT_CANNOT_HUNT };
        let mut mob = Mob::new(entity, LivingEntity::new(&PIGLIN_ATTRIBUTES), MobKind::Piglin(piglin));
        abstract_piglin::init(&mut mob);
        mob.xp_reward = 5;
        mob
    }

    pub fn make_brain(mob: &mut Mob, packed_brain: brain::Packed) -> Brain {
        BRAIN_PROVIDER.make_brain(mob, packed_brain)
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        abstract_piglin::add_additional_save_data(mob, out);
        out.push(("IsBaby".into(), Nbt::Byte(is_baby(&mob.entity) as i8)));
        out.push(("CannotHunt".into(), Nbt::Byte(piglin(mob).cannot_hunt as i8)));
        inventory_carrier::write_inventory_to_tag(&piglin(mob).inventory, out);
    }

    pub fn read_additional_save_data(mob: &mut Mob, nbt: &Nbt) {
        use crate::world::level::block::entity::byte_or;
        abstract_piglin::read_additional_save_data(mob, nbt);
        set_baby(mob, byte_or(nbt, "IsBaby", DEFAULT_IS_BABY as i8) != 0);
        piglin_mut(mob).cannot_hunt = byte_or(nbt, "CannotHunt", DEFAULT_CANNOT_HUNT as i8) != 0;
        inventory_carrier::read_inventory_from_tag(&mut piglin_mut(mob).inventory, nbt);
    }
}

impl ServerLevel {
    pub fn piglin_can_hunt(&self, id: EntityId) -> bool {
        !piglin(self.mob(id).expect("a piglin")).cannot_hunt
    }

    pub fn piglin_is_dancing(&self, id: EntityId) -> bool {
        is_dancing(self.entity(id))
    }

    pub fn piglin_set_dancing(&mut self, id: EntityId, dancing: bool) {
        self.entity_mut(id).data.set(DATA_IS_DANCING, DataValue::Boolean(dancing));
    }

    pub fn piglin_drop_custom_death_loot(&mut self, id: EntityId, source: &DamageSource, killed_by_player: bool) {
        self.mob_drop_custom_death_loot(id, source, killed_by_player);
        for item_stack in piglin_mut(self.mob_mut(id).expect("a piglin")).inventory.remove_all_items() {
            self.spawn_at_location(id, item_stack, 0.0);
        }
    }

    pub fn piglin_add_to_inventory(&mut self, id: EntityId, item_stack: &ItemStack) -> ItemStack {
        piglin_mut(self.mob_mut(id).expect("a piglin")).inventory.add_item(item_stack)
    }

    pub fn piglin_can_add_to_inventory(&self, id: EntityId, item_stack: &ItemStack) -> bool {
        piglin(self.mob(id).expect("a piglin")).inventory.can_add_item(item_stack)
    }

    pub fn piglin_finalize_spawn(&mut self, id: EntityId, difficulty: &DifficultyInstance, spawn_reason: EntitySpawnReason, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
        if spawn_reason != EntitySpawnReason::Structure {
            if self.random.next_float() < PROBABILITY_OF_SPAWNING_AS_BABY {
                set_baby(self.mob_mut(id).expect("a piglin"), true);
            } else if !self.is_baby(id) {
                let spawn_weapon = self.piglin_create_spawn_weapon(id);
                self.living_set_item_slot(id, EquipmentSlot::Mainhand, spawn_weapon);
            }
        }
        piglin_ai::init_memories(self, id);
        self.piglin_populate_default_equipment_slots(id);
        self.populate_default_equipment_enchantments(id, difficulty);
        self.mob_finalize_spawn(id, difficulty, spawn_reason, group_data)
    }

    fn piglin_populate_default_equipment_slots(&mut self, id: EntityId) {
        if !self.is_baby(id) {
            self.piglin_maybe_wear_armor(id, EquipmentSlot::Head, ItemStack::new(items::GOLDEN_HELMET, 1));
            self.piglin_maybe_wear_armor(id, EquipmentSlot::Chest, ItemStack::new(items::GOLDEN_CHESTPLATE, 1));
            self.piglin_maybe_wear_armor(id, EquipmentSlot::Legs, ItemStack::new(items::GOLDEN_LEGGINGS, 1));
            self.piglin_maybe_wear_armor(id, EquipmentSlot::Feet, ItemStack::new(items::GOLDEN_BOOTS, 1));
        }
    }

    fn piglin_maybe_wear_armor(&mut self, id: EntityId, slot: EquipmentSlot, item_stack: ItemStack) {
        if self.random.next_float() < CHANCE_OF_WEARING_EACH_ARMOUR_ITEM {
            self.living_set_item_slot(id, slot, item_stack);
        }
    }

    pub fn piglin_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let interaction_result = self.mob_mob_interact(id, player, hand);
        if interaction_result.consumes_action() {
            return interaction_result;
        }
        piglin_ai::mob_interact(self, id, player, hand)
    }

    pub fn piglin_custom_server_ai_step(&mut self, id: EntityId) {
        Brain::tick(self, id);
        if self.mob(id).is_none() {
            return;
        }
        piglin_ai::update_activity(self, id);
        self.abstract_piglin_custom_server_ai_step(id);
    }

    pub fn piglin_finish_conversion(&mut self, id: EntityId) {
        piglin_ai::cancel_admiring(self, id);
        for item_stack in piglin_mut(self.mob_mut(id).expect("a piglin")).inventory.remove_all_items() {
            self.spawn_at_location(id, item_stack, 0.0);
        }
    }

    fn piglin_create_spawn_weapon(&mut self, id: EntityId) -> ItemStack {
        let random = &mut self.entity_mut(id).random;
        if (random.next_float() as f64) < PROBABILITY_OF_SPAWNING_WITH_CROSSBOW_INSTEAD_OF_SWORD {
            ItemStack::new(items::CROSSBOW, 1)
        } else {
            ItemStack::new(if random.next_int_bounded(10) == 0 { items::GOLDEN_SPEAR } else { items::GOLDEN_SWORD }, 1)
        }
    }

    pub fn piglin_get_preferred_weapon_type(&self, id: EntityId) -> Option<&'static wafer_data::Tag> {
        if self.is_baby(id) { None } else { Some(&item_tags::PIGLIN_PREFERRED_WEAPONS) }
    }

    pub fn piglin_set_charging_crossbow(&mut self, id: EntityId, is_charging: bool) {
        self.entity_mut(id).data.set(DATA_IS_CHARGING_CROSSBOW, DataValue::Boolean(is_charging));
    }

    pub fn piglin_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        let source_entity = source.causing_entity;
        let was_hurt = self.living_entity_hurt_server(id, source, damage);
        if was_hurt && let Some(source_entity) = source_entity.filter(|&entity| self.is_living(entity)) {
            piglin_ai::was_hurt_by(self, id, source_entity);
        }
        was_hurt
    }

    pub fn piglin_perform_ranged_attack(&mut self, id: EntityId) {
        self.perform_crossbow_attack(id, 1.6);
    }

    pub fn piglin_hold_in_main_hand(&mut self, id: EntityId, item_stack: ItemStack) {
        self.set_item_slot_and_drop_when_killed(id, EquipmentSlot::Mainhand, item_stack);
        self.mob_mut(id).expect("a piglin").persistence_required = true;
    }

    pub fn piglin_hold_in_off_hand(&mut self, id: EntityId, item_stack: ItemStack) {
        let is_bartering_item = item_stack.is(piglin_ai::BARTERING_ITEM);
        self.set_item_slot_and_drop_when_killed(id, EquipmentSlot::Offhand, item_stack);
        if !is_bartering_item {
            self.mob_mut(id).expect("a piglin").persistence_required = true;
        }
    }

    pub fn piglin_wants_to_pick_up(&self, id: EntityId, item_stack: &ItemStack) -> bool {
        self.game_rules.get_bool(game_rule::MOB_GRIEFING) && self.mob(id).expect("a piglin").can_pick_up_loot && piglin_ai::wants_to_pickup(self, id, item_stack)
    }

    pub fn piglin_can_replace_current_item(&self, id: EntityId, new_item_stack: &ItemStack) -> bool {
        let slot = self.get_equipment_slot_for_item(id, new_item_stack);
        let current_item_stack = self.item_by_slot(id, slot).unwrap_or_else(ItemStack::empty);
        self.piglin_can_replace_current_item_in_slot(id, new_item_stack, &current_item_stack, slot)
    }

    pub fn piglin_can_replace_current_item_in_slot(&self, id: EntityId, new_item_stack: &ItemStack, current_item_stack: &ItemStack, slot: EquipmentSlot) -> bool {
        if enchantment_helper::has_prevent_armor_change(current_item_stack) {
            return false;
        }
        let preferred_weapon_type = self.piglin_get_preferred_weapon_type(id);
        let new_item_wanted = piglin_ai::is_loved_item(new_item_stack) || preferred_weapon_type.is_some_and(|tag| new_item_stack.get_item().is(tag));
        let current_item_wanted = piglin_ai::is_loved_item(current_item_stack) || preferred_weapon_type.is_some_and(|tag| current_item_stack.get_item().is(tag));
        if new_item_wanted && !current_item_wanted {
            true
        } else if !new_item_wanted && current_item_wanted {
            false
        } else {
            self.mob_can_replace_current_item(id, new_item_stack, current_item_stack, slot)
        }
    }

    pub fn piglin_pick_up_item(&mut self, id: EntityId, entity: EntityId) {
        self.on_item_pickup(id, entity);
        piglin_ai::pick_up_item(self, id, entity);
    }

    pub fn piglin_get_entity_to_ride(&self, id: EntityId, entity_to_ride: EntityId) -> EntityId {
        if is_baby(self.entity(id)) && self.entity(entity_to_ride).entity_type == entity_types::HOGLIN.id {
            self.piglin_get_top_passenger(entity_to_ride, MAX_PASSENGERS_ON_ONE_HOGLIN)
        } else {
            entity_to_ride
        }
    }

    fn piglin_get_top_passenger(&self, vehicle: EntityId, counter: i32) -> EntityId {
        match self.passengers(vehicle).first() {
            Some(&first) if counter != 1 => self.piglin_get_top_passenger(first, counter - 1),
            _ => vehicle,
        }
    }

    pub fn piglin_get_ambient_sound(&self, id: EntityId) -> Option<&'static str> {
        piglin_ai::get_sound_for_current_activity(self, id)
    }
}
