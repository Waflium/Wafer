use wafer_core::random::RandomSource;
use wafer_data::{items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::abstract_horse::{self, AbstractHorse, abstract_horse_defaults, abstract_horse_mut};
use crate::server::level::ServerLevel;
use crate::world::entity::attributes::{Attribute, CHESTED_HORSE_ATTRIBUTES};
use crate::world::entity::data::DataValue;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{Entity, EntityDimensions, EntityId, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::block::entity::base_container_block_entity::{load_all_items, save_all_items};
use crate::world::{InteractionHand, InteractionResult};
use crate::world::level::storage::tag_value_input;

pub const DATA_ID_CHEST: usize = 19;
const DEFAULT_HAS_CHEST: bool = false;

pub struct AbstractChestedHorse {
    pub abstract_horse: AbstractHorse,
}

impl Default for AbstractChestedHorse {
    fn default() -> AbstractChestedHorse {
        let mut abstract_horse = AbstractHorse::default();
        abstract_horse.can_gallop = false;
        AbstractChestedHorse { abstract_horse }
    }
}

pub fn abstract_chested_horse_defaults() -> Vec<DataValue> {
    let mut data = abstract_horse_defaults();
    data.push(DataValue::Boolean(DEFAULT_HAS_CHEST));
    data
}

pub fn baby_dimensions(type_dimensions: EntityDimensions, passenger_attachments: &'static [wafer_core::Vec3]) -> EntityDimensions {
    type_dimensions.with_passenger_attachments(passenger_attachments).scale(0.5)
}

pub fn abstract_chested_horse(mob: &Mob) -> &AbstractChestedHorse {
    match &mob.kind {
        MobKind::Donkey(donkey) => &donkey.abstract_chested_horse,
        MobKind::Mule(mule) => &mule.abstract_chested_horse,
        MobKind::Llama(llama) | MobKind::TraderLlama(llama) => &llama.abstract_chested_horse,
        _ => unreachable!("not an abstract chested horse"),
    }
}

pub fn randomize_attributes(mob: &mut Mob, random: &mut (impl RandomSource + ?Sized)) {
    let max_health = abstract_horse::generate_max_health(|bound| random.next_int_bounded(bound));
    mob.living.attributes.instance(Attribute::MaxHealth).expect("chested horses have a max health").set_base_value(max_health as f64);
}

pub fn has_chest(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_ID_CHEST), DataValue::Boolean(true))
}

pub fn set_chest(entity: &mut Entity, flag: bool) {
    entity.data.set(DATA_ID_CHEST, DataValue::Boolean(flag));
}

pub fn get_inventory_columns(mob: &Mob) -> i32 {
    if has_chest(&mob.entity) { 5 } else { 0 }
}

impl AbstractChestedHorse {
    pub fn create(entity_type: &str, dimensions: EntityDimensions, kind: MobKind) -> Mob {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, entity_type), dimensions, abstract_chested_horse_defaults());
        let mut mob = Mob::new(entity, LivingEntity::new(&CHESTED_HORSE_ATTRIBUTES), kind);
        AbstractHorse::init(&mut mob);
        mob
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        AbstractHorse::add_additional_save_data(mob, output);
        let has_chest = has_chest(&mob.entity);
        output.push(("ChestedHorse".into(), Nbt::Byte(has_chest as i8)));
        if has_chest {
            save_all_items(output, &abstract_chested_horse(mob).abstract_horse.inventory.items, true);
        }
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        AbstractHorse::read_additional_save_data(mob, input);
        let chested = tag_value_input::get_i64(input, "ChestedHorse").is_some_and(|v| v != 0);
        set_chest(&mut mob.entity, chested);
        abstract_horse::create_inventory(mob);
        if has_chest(&mob.entity) {
            load_all_items(input, &mut abstract_horse_mut(mob).inventory.items);
        }
    }
}

impl ServerLevel {
    pub fn is_abstract_chested_horse(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::Donkey(_) | MobKind::Mule(_) | MobKind::Llama(_) | MobKind::TraderLlama(_)))
    }

    pub fn has_chest(&self, id: EntityId) -> bool {
        has_chest(self.entity(id))
    }

    pub fn set_chest(&mut self, id: EntityId, flag: bool) {
        set_chest(self.entity_mut(id), flag);
    }

    pub fn abstract_chested_horse_drop_equipment(&mut self, id: EntityId) {
        self.abstract_horse_drop_equipment(id);
        if self.has_chest(id) {
            self.spawn_at_location(id, ItemStack::new(items::CHEST, 1), 0.0);
            self.set_chest(id, false);
        }
    }

    pub fn chested_horse_chest_slot_get(&self, id: EntityId) -> ItemStack {
        if self.has_chest(id) { ItemStack::new(items::CHEST, 1) } else { ItemStack::empty() }
    }

    pub fn chested_horse_chest_slot_set(&mut self, id: EntityId, item_stack: &ItemStack) -> bool {
        if item_stack.is_empty() {
            if self.has_chest(id) {
                self.set_chest(id, false);
                self.create_inventory(id);
            }
            true
        } else if item_stack.is(items::CHEST) {
            if !self.has_chest(id) {
                self.set_chest(id, true);
                self.create_inventory(id);
            }
            true
        } else {
            false
        }
    }

    pub fn abstract_chested_horse_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let should_open_inventory = !self.is_baby(id) && self.is_tamed(id) && self.player(player).is_secondary_use_active();
        if !self.is_vehicle(id) && !should_open_inventory && (!self.is_baby(id) || !self.is_holding(player, items::GOLDEN_DANDELION)) {
            let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
            if !item_stack.is_empty() {
                if self.is_food(id, &item_stack) {
                    return self.fed_food(id, player, hand, &mut item_stack);
                }
                if !self.is_tamed(id) {
                    self.make_mad(id);
                    return InteractionResult::SUCCESS;
                }
                if !self.has_chest(id) && item_stack.is(items::CHEST) {
                    self.equip_chest(id, player, hand, &mut item_stack);
                    return InteractionResult::SUCCESS;
                }
            }
        }
        self.abstract_horse_mob_interact(id, player, hand)
    }

    fn equip_chest(&mut self, id: EntityId, player: EntityId, hand: InteractionHand, item_stack: &mut ItemStack) {
        self.set_chest(id, true);
        self.play_chest_equips_sound(id);
        item_stack.consume(1, self, Some(player));
        self.player_mut(player).update_item_in_hand(hand, item_stack);
        self.create_inventory(id);
    }

    fn play_chest_equips_sound(&mut self, id: EntityId) {
        if self.is_llama(id) {
            self.llama_play_chest_equips_sound(id);
            return;
        }
        let sound = if self.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::Mule(_))) { "minecraft:entity.mule.chest" } else { "minecraft:entity.donkey.chest" };
        let random = &mut self.entity_mut(id).random;
        let pitch = (random.next_float() - random.next_float()) * 0.2 + 1.0;
        self.entity_play_sound(id, sound_event(sound), 1.0, pitch);
    }
}
