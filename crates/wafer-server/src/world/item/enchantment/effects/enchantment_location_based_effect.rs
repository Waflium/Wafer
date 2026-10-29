use std::sync::LazyLock;

use wafer_core::Vec3;
use wafer_protocol::nbt::Nbt;

use super::enchantment_attribute_effect::{self, EnchantmentAttributeEffect};
use super::enchantment_entity_effect::{self, EnchantmentEntityEffect};
use crate::world::item::component::codec::{Codec, Dispatch, Selector, Stream};
use crate::world::item::component::types::{self, Pair, req};
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::entity::EntityId;
use crate::world::item::enchantment::enchanted_item_in_use::EnchantedItemInUse;
use crate::world::item::enchantment::enchantment_helper::equipment_slot_name;

fn codec_lazy() -> &'static Pair {
    static CODEC: LazyLock<Pair> = LazyLock::new(codec);
    &CODEC
}

pub fn codec() -> Pair {
    let mut cases = enchantment_entity_effect::shared_cases();
    cases.push(("minecraft:all_of", vec![req("effects", types::list((Codec::Lazy(codec_lazy), Stream::Lazy)))]));
    cases.push(("minecraft:attribute", enchantment_attribute_effect::fields()));
    (Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:enchantment_location_based_effect_type"), cases })), Stream::Dispatch)
}

pub enum EnchantmentLocationBasedEffect {
    AllOf(Vec<EnchantmentLocationBasedEffect>),
    Attribute(EnchantmentAttributeEffect),
    Entity(EnchantmentEntityEffect),
}

impl EnchantmentLocationBasedEffect {
    pub fn decode_partial(tag: &Nbt) -> DecodeResult<EnchantmentLocationBasedEffect> {
        codec::checked(&codec_lazy().0, tag, EnchantmentLocationBasedEffect::decode)
    }

    pub fn decode(tag: &Nbt) -> DataResult<EnchantmentLocationBasedEffect> {
        let ty = codec::decode_field(tag, "type", codec::identifier)?;
        Ok(match ty.as_str() {
            "minecraft:all_of" => EnchantmentLocationBasedEffect::AllOf(codec::decode_field(tag, "effects", |t| codec::list_of(t, EnchantmentLocationBasedEffect::decode))?),
            "minecraft:attribute" => EnchantmentLocationBasedEffect::Attribute(EnchantmentAttributeEffect::decode(tag)?),
            _ => EnchantmentLocationBasedEffect::Entity(EnchantmentEntityEffect::decode(tag)?),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn on_changed_block(&self, level: &mut ServerLevel, enchantment_level: i32, item: &EnchantedItemInUse, entity: EntityId, position: Vec3, became_active: bool) {
        match self {
            EnchantmentLocationBasedEffect::AllOf(effects) => {
                for effect in effects {
                    effect.on_changed_block(level, enchantment_level, item, entity, position, became_active);
                }
            }
            EnchantmentLocationBasedEffect::Attribute(effect) => {
                if became_active
                    && level.is_living(entity)
                    && let Some(slot) = item.in_slot
                {
                    let modifier = effect.get_modifier(enchantment_level, equipment_slot_name(slot));
                    if let Some(instance) = level.living_mut(entity).attributes.instance(effect.attribute) {
                        instance.remove_modifier(&modifier.id);
                        instance.add_transient_modifier(modifier);
                    }
                }
            }
            EnchantmentLocationBasedEffect::Entity(effect) => effect.apply(level, enchantment_level, item, entity, position),
        }
    }

    pub fn on_deactivated(&self, level: &mut ServerLevel, item: &EnchantedItemInUse, entity: EntityId, position: Vec3, enchantment_level: i32) {
        match self {
            EnchantmentLocationBasedEffect::AllOf(effects) => {
                for effect in effects {
                    effect.on_deactivated(level, item, entity, position, enchantment_level);
                }
            }
            EnchantmentLocationBasedEffect::Attribute(effect) => {
                if level.is_living(entity)
                    && let Some(slot) = item.in_slot
                {
                    let modifier = effect.get_modifier(enchantment_level, equipment_slot_name(slot));
                    if let Some(instance) = level.living_mut(entity).attributes.instance(effect.attribute) {
                        instance.remove_modifier(&modifier.id);
                    }
                }
            }
            EnchantmentLocationBasedEffect::Entity(_) => {}
        }
    }
}
