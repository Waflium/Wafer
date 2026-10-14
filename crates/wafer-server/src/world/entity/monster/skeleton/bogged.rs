use wafer_data::{items, mob_effects};
use wafer_protocol::nbt::Nbt;

use super::abstract_skeleton::{AbstractSkeleton, INCREASED_HARD_ATTACK_INTERVAL, INCREASED_NORMAL_ATTACK_INTERVAL};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::effect::MobEffectInstance;
use crate::world::entity::attributes::BOGGED_ATTRIBUTES;
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind};
use crate::world::entity::{Entity, EntityId, sound_event};
use crate::world::item::ItemStack;
use crate::world::{InteractionHand, InteractionResult};

const DATA_SHEARED: usize = 16;
const SHEARED_TAG_NAME: &str = "sheared";
const DEFAULT_SHEARED: bool = false;
const BOGGED_SHEAR: &str = "minecraft:shearing/bogged";

#[derive(Default)]
pub struct Bogged {
    pub abstract_skeleton: AbstractSkeleton,
}

pub fn is_sheared(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_SHEARED), DataValue::Boolean(true))
}

pub fn set_sheared(entity: &mut Entity, sheared: bool) {
    entity.data.set(DATA_SHEARED, DataValue::Boolean(sheared));
}

impl Bogged {
    pub fn create() -> Mob {
        let mut data = mob_defaults();
        data.push(DataValue::Boolean(false));
        AbstractSkeleton::create_with("minecraft:bogged", &BOGGED_ATTRIBUTES, data, MobKind::Bogged(Bogged::default()))
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        out.push((SHEARED_TAG_NAME.into(), Nbt::Byte(is_sheared(&mob.entity) as i8)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        let sheared = input.get(SHEARED_TAG_NAME).and_then(Nbt::as_i64).map_or(DEFAULT_SHEARED, |v| v != 0);
        set_sheared(&mut mob.entity, sheared);
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.bogged.ambient"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.bogged.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.bogged.death"
    }

    pub fn get_step_sound() -> &'static str {
        "minecraft:entity.bogged.step"
    }

    pub fn get_hard_attack_interval() -> i32 {
        INCREASED_HARD_ATTACK_INTERVAL
    }

    pub fn get_attack_interval() -> i32 {
        INCREASED_NORMAL_ATTACK_INTERVAL
    }
}

impl ServerLevel {
    pub fn bogged_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if item_stack.is(items::SHEARS) && self.bogged_ready_for_shearing(id) {
            self.bogged_shear(id, SoundSource::Players, &item_stack);
            self.hurt_and_break(player, hand.as_equipment_slot(), 1);
            InteractionResult::SUCCESS
        } else {
            self.mob_mob_interact(id, player, hand)
        }
    }

    pub fn bogged_get_arrow(&mut self, id: EntityId, projectile: &ItemStack, power: f32, firing_weapon: Option<&ItemStack>) -> Projectile {
        let mut abstract_arrow = self.abstract_skeleton_get_arrow(id, projectile, power, firing_weapon);
        if let ProjectileKind::Arrow(_) = abstract_arrow.kind {
            abstract_arrow.arrow_add_effect(&MobEffectInstance::new(mob_effects::POISON, 100, 0));
        }
        abstract_arrow
    }

    pub fn bogged_shear(&mut self, id: EntityId, sound_source: SoundSource, tool: &ItemStack) {
        self.play_sound_from_entity(None, id, sound_event("minecraft:entity.bogged.shear"), sound_source, 1.0, 1.0);
        self.spawn_sheared_mushrooms(id, tool);
        set_sheared(self.entity_mut(id), true);
    }

    fn spawn_sheared_mushrooms(&mut self, id: EntityId, tool: &ItemStack) {
        self.drop_from_shearing_loot_table(id, BOGGED_SHEAR, tool, |l, drop| {
            let offset = l.entity(id).dimensions.height;
            l.spawn_at_location(id, drop, offset);
        });
    }

    pub fn bogged_ready_for_shearing(&self, id: EntityId) -> bool {
        !is_sheared(self.entity(id))
    }
}
