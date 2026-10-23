use wafer_protocol::nbt::Nbt;

use crate::world::item::component::types::Pair;

pub fn type_codec(id: &str) -> Option<Pair> {
    use super::{
        attribute_modifiers_predicate, bundle_predicate, container_predicate, custom_data_predicate, damage_predicate, enchantments_predicate,
        firework_explosion_predicate, fireworks_predicate, jukebox_playable_predicate, potions_predicate, trim_predicate, villager_type_predicate,
        writable_book_predicate, written_book_predicate,
    };
    Some(match id {
        "minecraft:damage" => damage_predicate::codec(),
        "minecraft:enchantments" | "minecraft:stored_enchantments" => enchantments_predicate::codec(),
        "minecraft:potion_contents" => potions_predicate::codec(),
        "minecraft:custom_data" => custom_data_predicate::codec(),
        "minecraft:container" => container_predicate::codec(),
        "minecraft:bundle_contents" => bundle_predicate::codec(),
        "minecraft:firework_explosion" => firework_explosion_predicate::codec(),
        "minecraft:fireworks" => fireworks_predicate::codec(),
        "minecraft:writable_book_content" => writable_book_predicate::codec(),
        "minecraft:written_book_content" => written_book_predicate::codec(),
        "minecraft:attribute_modifiers" => attribute_modifiers_predicate::codec(),
        "minecraft:trim" => trim_predicate::codec(),
        "minecraft:jukebox_playable" => jukebox_playable_predicate::codec(),
        "minecraft:villager/variant" => villager_type_predicate::codec(),
        _ => return None,
    })
}

use super::any_value::AnyValue;
use super::attribute_modifiers_predicate::AttributeModifiersPredicate;
use super::bundle_predicate::BundlePredicate;
use super::container_predicate::ContainerPredicate;
use super::custom_data_predicate::CustomDataPredicate;
use super::damage_predicate::DamagePredicate;
use super::enchantments_predicate::EnchantmentsPredicate;
use super::firework_explosion_predicate::FireworkExplosionPredicate;
use super::fireworks_predicate::FireworksPredicate;
use super::jukebox_playable_predicate::JukeboxPlayablePredicate;
use super::potions_predicate::PotionsPredicate;
use super::trim_predicate::TrimPredicate;
use super::villager_type_predicate::VillagerTypePredicate;
use super::writable_book_predicate::WritableBookPredicate;
use super::written_book_predicate::WrittenBookPredicate;
use crate::core::component::data_component_getter::DataComponentGetter;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::{self as dc, DataComponentType};

#[derive(Debug)]
pub enum DataComponentPredicate {
    AnyValue(AnyValue),
    Damage(DamagePredicate),
    Enchantments(EnchantmentsPredicate),
    StoredEnchantments(EnchantmentsPredicate),
    Potions(PotionsPredicate),
    CustomData(CustomDataPredicate),
    Container(ContainerPredicate),
    BundleContents(BundlePredicate),
    FireworkExplosion(FireworkExplosionPredicate),
    Fireworks(FireworksPredicate),
    WritableBook(WritableBookPredicate),
    WrittenBook(WrittenBookPredicate),
    AttributeModifiers(AttributeModifiersPredicate),
    ArmorTrim(TrimPredicate),
    JukeboxPlayable(JukeboxPlayablePredicate),
    VillagerVariant(VillagerTypePredicate),
}

impl DataComponentPredicate {
    pub fn decode_map(tag: &Nbt) -> DataResult<Vec<DataComponentPredicate>> {
        Ok(codec::dispatched_map(tag, |key| DataComponentPredicate::codec(&crate::resources::identifier::read(key)?), |decode, value| decode(value))?
            .into_iter()
            .map(|(_, predicate)| predicate)
            .collect())
    }

    fn codec(id: &str) -> DataResult<Box<dyn Fn(&Nbt) -> DataResult<DataComponentPredicate>>> {
        let concrete: Option<fn(&Nbt) -> DataResult<DataComponentPredicate>> = match id {
            "minecraft:damage" => Some(|tag| Ok(DataComponentPredicate::Damage(DamagePredicate::decode(tag)?))),
            "minecraft:enchantments" => Some(|tag| Ok(DataComponentPredicate::Enchantments(EnchantmentsPredicate::decode(dc::ENCHANTMENTS, tag)?))),
            "minecraft:stored_enchantments" => Some(|tag| Ok(DataComponentPredicate::StoredEnchantments(EnchantmentsPredicate::decode(dc::STORED_ENCHANTMENTS, tag)?))),
            "minecraft:potion_contents" => Some(|tag| Ok(DataComponentPredicate::Potions(PotionsPredicate::decode(tag)?))),
            "minecraft:custom_data" => Some(|tag| Ok(DataComponentPredicate::CustomData(CustomDataPredicate::decode(tag)?))),
            "minecraft:container" => Some(|tag| Ok(DataComponentPredicate::Container(ContainerPredicate::decode(tag)?))),
            "minecraft:bundle_contents" => Some(|tag| Ok(DataComponentPredicate::BundleContents(BundlePredicate::decode(tag)?))),
            "minecraft:firework_explosion" => Some(|tag| Ok(DataComponentPredicate::FireworkExplosion(FireworkExplosionPredicate::decode(tag)?))),
            "minecraft:fireworks" => Some(|tag| Ok(DataComponentPredicate::Fireworks(FireworksPredicate::decode(tag)?))),
            "minecraft:writable_book_content" => Some(|tag| Ok(DataComponentPredicate::WritableBook(WritableBookPredicate::decode(tag)?))),
            "minecraft:written_book_content" => Some(|tag| Ok(DataComponentPredicate::WrittenBook(WrittenBookPredicate::decode(tag)?))),
            "minecraft:attribute_modifiers" => Some(|tag| Ok(DataComponentPredicate::AttributeModifiers(AttributeModifiersPredicate::decode(tag)?))),
            "minecraft:trim" => Some(|tag| Ok(DataComponentPredicate::ArmorTrim(TrimPredicate::decode(tag)?))),
            "minecraft:jukebox_playable" => Some(|tag| Ok(DataComponentPredicate::JukeboxPlayable(JukeboxPlayablePredicate::decode(tag)?))),
            "minecraft:villager/variant" => Some(|tag| Ok(DataComponentPredicate::VillagerVariant(VillagerTypePredicate::decode(tag)?))),
            _ => None,
        };
        if let Some(concrete) = concrete {
            return Ok(Box::new(concrete));
        }
        match DataComponentType::by_name(id) {
            Some(ty) => Ok(Box::new(move |tag| {
                codec::map(tag)?;
                Ok(DataComponentPredicate::AnyValue(AnyValue { ty }))
            })),
            None => Err(format!("Failed to parse either. First: Unknown registry key in ResourceKey[minecraft:root / minecraft:data_component_predicate_type]: {id}; Second: Unknown registry key in ResourceKey[minecraft:root / minecraft:data_component_type]: {id}")),
        }
    }

    pub fn matches(&self, components: &(impl DataComponentGetter + ?Sized)) -> bool {
        let single = |ty: DataComponentType, matches: &dyn Fn(&Nbt) -> bool| components.get(ty).is_some_and(|value| matches(&value));
        match self {
            DataComponentPredicate::AnyValue(p) => p.matches(components),
            DataComponentPredicate::Damage(p) => p.matches(components),
            DataComponentPredicate::Enchantments(p) | DataComponentPredicate::StoredEnchantments(p) => single(p.component_type(), &|v| p.matches(v)),
            DataComponentPredicate::Potions(p) => single(dc::POTION_CONTENTS, &|v| p.matches(v)),
            DataComponentPredicate::CustomData(p) => p.matches(components),
            DataComponentPredicate::Container(p) => single(dc::CONTAINER, &|v| p.matches(v)),
            DataComponentPredicate::BundleContents(p) => single(dc::BUNDLE_CONTENTS, &|v| p.matches(v)),
            DataComponentPredicate::FireworkExplosion(p) => single(dc::FIREWORK_EXPLOSION, &|v| p.matches(v)),
            DataComponentPredicate::Fireworks(p) => single(dc::FIREWORKS, &|v| p.matches(v)),
            DataComponentPredicate::WritableBook(p) => single(dc::WRITABLE_BOOK_CONTENT, &|v| p.matches(v)),
            DataComponentPredicate::WrittenBook(p) => single(dc::WRITTEN_BOOK_CONTENT, &|v| p.matches(v)),
            DataComponentPredicate::AttributeModifiers(p) => single(dc::ATTRIBUTE_MODIFIERS, &|v| p.matches(v)),
            DataComponentPredicate::ArmorTrim(p) => single(dc::TRIM, &|v| p.matches(v)),
            DataComponentPredicate::JukeboxPlayable(p) => single(dc::JUKEBOX_PLAYABLE, &|v| p.matches(v)),
            DataComponentPredicate::VillagerVariant(p) => single(dc::VILLAGER_VARIANT, &|v| p.matches(v)),
        }
    }
}
