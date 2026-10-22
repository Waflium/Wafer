use std::borrow::Cow;

use wafer_protocol::nbt::Nbt;

use super::EntityId;
use super::animal::axolotl::axolotl;
use super::animal::chicken;
use super::animal::frog::frog;
use super::animal::cow::{cow, mushroom_cow};
use super::animal::feline::{cat, cat_sound_variant};
use super::animal::pig::pig;
use super::animal::sheep::sheep;
use super::animal::wolf::{wolf, wolf_sound_variant};
use super::variant::variant_utils;
use super::data::DataValue;
use super::mob::MobKind;
use crate::core::component::data_component_getter::DataComponentGetter;
use crate::server::level::ServerLevel;
use crate::world::item::component::{self as dc, DataComponentType};

pub struct EntityComponents<'a> {
    pub level: &'a ServerLevel,
    pub id: EntityId,
}

impl DataComponentGetter for EntityComponents<'_> {
    fn get(&self, ty: DataComponentType) -> Option<Cow<'_, Nbt>> {
        if let Some(mob) = self.level.mob(self.id) {
            match &mob.kind {
                MobKind::Chicken(_) if ty == dc::CHICKEN_VARIANT => {
                    if let DataValue::ChickenVariant(variant) = mob.entity.data.get(chicken::DATA_VARIANT) {
                        return Some(Cow::Owned(Nbt::String(chicken::variant_name(*variant).to_string())));
                    }
                }
                MobKind::Chicken(_) if ty == dc::CHICKEN_SOUND_VARIANT => {
                    if let DataValue::ChickenSoundVariant(variant) = mob.entity.data.get(chicken::DATA_SOUND_VARIANT) {
                        return Some(Cow::Owned(Nbt::String(chicken::sound_variant_name(*variant).to_string())));
                    }
                }
                MobKind::Cow(_) if ty == dc::COW_VARIANT => {
                    return Some(Cow::Owned(Nbt::String(variant_utils::variant_name(cow::VARIANT_REGISTRY, cow::get_variant(&mob.entity)).to_string())));
                }
                MobKind::Cow(_) if ty == dc::COW_SOUND_VARIANT => {
                    return Some(Cow::Owned(Nbt::String(variant_utils::variant_name(cow::SOUND_VARIANT_REGISTRY, cow::get_sound_variant(&mob.entity)).to_string())));
                }
                MobKind::MushroomCow(_) if ty == dc::MOOSHROOM_VARIANT => {
                    return Some(Cow::Owned(Nbt::String(mushroom_cow::get_variant(&mob.entity).get_serialized_name().to_string())));
                }
                MobKind::Pig(_) if ty == dc::PIG_VARIANT => {
                    return Some(Cow::Owned(Nbt::String(variant_utils::variant_name(pig::VARIANT_REGISTRY, pig::get_variant(&mob.entity)).to_string())));
                }
                MobKind::Pig(_) if ty == dc::PIG_SOUND_VARIANT => {
                    return Some(Cow::Owned(Nbt::String(variant_utils::variant_name(pig::SOUND_VARIANT_REGISTRY, pig::get_sound_variant(&mob.entity)).to_string())));
                }
                MobKind::Wolf(_) if ty == dc::WOLF_VARIANT => {
                    return Some(Cow::Owned(Nbt::String(variant_utils::variant_name(wolf::VARIANT_REGISTRY, wolf::get_variant(&mob.entity)).to_string())));
                }
                MobKind::Wolf(_) if ty == dc::WOLF_SOUND_VARIANT => {
                    return Some(Cow::Owned(Nbt::String(variant_utils::variant_name(wolf_sound_variant::REGISTRY, wolf::get_sound_variant(&mob.entity)).to_string())));
                }
                MobKind::Wolf(_) if ty == dc::WOLF_COLLAR => {
                    return Some(Cow::Owned(Nbt::String(wolf::get_collar_color(&mob.entity).name.to_string())));
                }
                MobKind::Shulker(_) if ty == dc::SHULKER_COLOR => {
                    return crate::world::entity::monster::shulker::get_color(&mob.entity).map(|color| Cow::Owned(Nbt::String(color.name.to_string())));
                }
                MobKind::Parrot(_) if ty == dc::PARROT_VARIANT => {
                    return Some(Cow::Owned(Nbt::String(crate::world::entity::animal::parrot::parrot::get_variant(&mob.entity).get_serialized_name().to_string())));
                }
                MobKind::Cat(_) if ty == dc::CAT_VARIANT => {
                    return Some(Cow::Owned(Nbt::String(variant_utils::variant_name(cat::VARIANT_REGISTRY, cat::get_variant(&mob.entity)).to_string())));
                }
                MobKind::Cat(_) if ty == dc::CAT_SOUND_VARIANT => {
                    return Some(Cow::Owned(Nbt::String(variant_utils::variant_name(cat_sound_variant::REGISTRY, cat::get_sound_variant(&mob.entity)).to_string())));
                }
                MobKind::Cat(_) if ty == dc::CAT_COLLAR => {
                    return Some(Cow::Owned(Nbt::String(cat::get_collar_color(&mob.entity).name.to_string())));
                }
                MobKind::Rabbit(_) if ty == dc::RABBIT_VARIANT => {
                    return Some(Cow::Owned(Nbt::String(super::animal::rabbit::rabbit::get_variant(&mob.entity).get_serialized_name().to_string())));
                }
                MobKind::Sheep(_) if ty == dc::SHEEP_COLOR => {
                    return Some(Cow::Owned(Nbt::String(sheep::get_color(&mob.entity).name.to_string())));
                }
                MobKind::Salmon(_) if ty == dc::SALMON_SIZE => {
                    return Some(Cow::Owned(Nbt::String(mob.get_salmon_variant().get_serialized_name().to_string())));
                }
                MobKind::TropicalFish(_) if ty == dc::TROPICAL_FISH_PATTERN => {
                    return Some(Cow::Owned(Nbt::String(mob.get_tropical_fish_pattern().get_serialized_name().to_string())));
                }
                MobKind::TropicalFish(_) if ty == dc::TROPICAL_FISH_BASE_COLOR => {
                    return Some(Cow::Owned(Nbt::String(mob.get_tropical_fish_base_color().name.to_string())));
                }
                MobKind::TropicalFish(_) if ty == dc::TROPICAL_FISH_PATTERN_COLOR => {
                    return Some(Cow::Owned(Nbt::String(mob.get_tropical_fish_pattern_color().name.to_string())));
                }
                MobKind::Fox(_) if ty == dc::FOX_VARIANT => {
                    return Some(Cow::Owned(Nbt::String(super::animal::fox::fox::get_variant(&mob.entity).get_serialized_name().to_string())));
                }
                MobKind::ZombieNautilus(_) if ty == dc::ZOMBIE_NAUTILUS_VARIANT => {
                    let variant = super::animal::nautilus::zombie_nautilus::get_variant(&mob.entity);
                    return Some(Cow::Owned(Nbt::String(variant_utils::variant_name(super::animal::nautilus::zombie_nautilus::VARIANT_REGISTRY, variant).to_string())));
                }
                MobKind::Llama(_) | MobKind::TraderLlama(_) if ty == dc::LLAMA_VARIANT => {
                    return Some(Cow::Owned(Nbt::String(super::animal::equine::llama::get_variant(&mob.entity).serialized_name().to_string())));
                }
                MobKind::Horse(_) if ty == dc::HORSE_VARIANT => {
                    let variant = super::animal::equine::horse::get_variant(&mob.entity);
                    return Some(Cow::Owned(Nbt::String(variant.get_serialized_name().to_string())));
                }
                MobKind::Axolotl(_) if ty == dc::AXOLOTL_VARIANT => {
                    return Some(Cow::Owned(Nbt::String(axolotl::get_variant(&mob.entity).get_serialized_name().to_string())));
                }
                MobKind::Frog(_) if ty == dc::FROG_VARIANT => {
                    return Some(Cow::Owned(Nbt::String(variant_utils::variant_name(frog::VARIANT_REGISTRY, frog::get_variant(&mob.entity)).to_string())));
                }
                MobKind::Villager(_) if ty == dc::VILLAGER_VARIANT => {
                    return Some(Cow::Owned(super::npc::villager::villager::get_villager_variant(mob)));
                }
                MobKind::ZombieVillager(_) if ty == dc::VILLAGER_VARIANT => {
                    return Some(Cow::Owned(super::monster::zombie::zombie_villager::get_villager_variant(mob)));
                }
                _ => {}
            }
        }
        let entity = self.level.entity(self.id);
        if ty == dc::CUSTOM_NAME {
            return match entity.data.get(super::data::ids::CUSTOM_NAME) {
                DataValue::OptionalComponent(Some(name)) => Some(Cow::Borrowed(name)),
                _ => None,
            };
        }
        if ty == dc::CUSTOM_DATA {
            return entity.custom_data.as_ref().map(Cow::Borrowed);
        }
        None
    }
}
