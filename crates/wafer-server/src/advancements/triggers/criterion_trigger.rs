use wafer_protocol::nbt::Nbt;

use super::*;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance as Instance;
use crate::util::codec::DecodeResult;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CriterionTrigger {
    Impossible,
    PlayerKilledEntity,
    EntityKilledPlayer,
    EnterBlock,
    InventoryChanged,
    RecipeUnlocked,
    PlayerHurtEntity,
    EntityHurtPlayer,
    EnchantedItem,
    FilledBucket,
    BrewedPotion,
    ConstructBeacon,
    UsedEnderEye,
    SummonedEntity,
    BredAnimals,
    Location,
    SleptInBed,
    CuredZombieVillager,
    VillagerTrade,
    ItemDurabilityChanged,
    Levitation,
    ChangedDimension,
    Tick,
    TameAnimal,
    PlacedBlock,
    ConsumeItem,
    EffectsChanged,
    UsedTotem,
    NetherTravel,
    FishingRodHooked,
    ChanneledLightning,
    ShotCrossbow,
    SpearMobs,
    KilledByArrow,
    HeroOfTheVillage,
    VoluntaryExile,
    SlideDownBlock,
    BeeNestDestroyed,
    TargetHit,
    ItemUsedOnBlock,
    DefaultBlockUse,
    AnyBlockUse,
    PlayerGeneratesContainerLoot,
    ThrownItemPickedUpByEntity,
    ThrownItemPickedUpByPlayer,
    PlayerInteractedWithEntity,
    PlayerShearedEquipment,
    StartedRiding,
    LightningStrike,
    UsingItem,
    FallFromHeight,
    RideEntityInLava,
    KillMobNearSculkCatalyst,
    AllayDropItemOnBlock,
    AvoidVibration,
    RecipeCrafted,
    CrafterRecipeCrafted,
    FallAfterExplosion,
}

impl CriterionTrigger {
    pub const COUNT: usize = 58;

    pub const VALUES: [CriterionTrigger; CriterionTrigger::COUNT] = [
        CriterionTrigger::Impossible,
        CriterionTrigger::PlayerKilledEntity,
        CriterionTrigger::EntityKilledPlayer,
        CriterionTrigger::EnterBlock,
        CriterionTrigger::InventoryChanged,
        CriterionTrigger::RecipeUnlocked,
        CriterionTrigger::PlayerHurtEntity,
        CriterionTrigger::EntityHurtPlayer,
        CriterionTrigger::EnchantedItem,
        CriterionTrigger::FilledBucket,
        CriterionTrigger::BrewedPotion,
        CriterionTrigger::ConstructBeacon,
        CriterionTrigger::UsedEnderEye,
        CriterionTrigger::SummonedEntity,
        CriterionTrigger::BredAnimals,
        CriterionTrigger::Location,
        CriterionTrigger::SleptInBed,
        CriterionTrigger::CuredZombieVillager,
        CriterionTrigger::VillagerTrade,
        CriterionTrigger::ItemDurabilityChanged,
        CriterionTrigger::Levitation,
        CriterionTrigger::ChangedDimension,
        CriterionTrigger::Tick,
        CriterionTrigger::TameAnimal,
        CriterionTrigger::PlacedBlock,
        CriterionTrigger::ConsumeItem,
        CriterionTrigger::EffectsChanged,
        CriterionTrigger::UsedTotem,
        CriterionTrigger::NetherTravel,
        CriterionTrigger::FishingRodHooked,
        CriterionTrigger::ChanneledLightning,
        CriterionTrigger::ShotCrossbow,
        CriterionTrigger::SpearMobs,
        CriterionTrigger::KilledByArrow,
        CriterionTrigger::HeroOfTheVillage,
        CriterionTrigger::VoluntaryExile,
        CriterionTrigger::SlideDownBlock,
        CriterionTrigger::BeeNestDestroyed,
        CriterionTrigger::TargetHit,
        CriterionTrigger::ItemUsedOnBlock,
        CriterionTrigger::DefaultBlockUse,
        CriterionTrigger::AnyBlockUse,
        CriterionTrigger::PlayerGeneratesContainerLoot,
        CriterionTrigger::ThrownItemPickedUpByEntity,
        CriterionTrigger::ThrownItemPickedUpByPlayer,
        CriterionTrigger::PlayerInteractedWithEntity,
        CriterionTrigger::PlayerShearedEquipment,
        CriterionTrigger::StartedRiding,
        CriterionTrigger::LightningStrike,
        CriterionTrigger::UsingItem,
        CriterionTrigger::FallFromHeight,
        CriterionTrigger::RideEntityInLava,
        CriterionTrigger::KillMobNearSculkCatalyst,
        CriterionTrigger::AllayDropItemOnBlock,
        CriterionTrigger::AvoidVibration,
        CriterionTrigger::RecipeCrafted,
        CriterionTrigger::CrafterRecipeCrafted,
        CriterionTrigger::FallAfterExplosion,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            CriterionTrigger::Impossible => "minecraft:impossible",
            CriterionTrigger::PlayerKilledEntity => "minecraft:player_killed_entity",
            CriterionTrigger::EntityKilledPlayer => "minecraft:entity_killed_player",
            CriterionTrigger::EnterBlock => "minecraft:enter_block",
            CriterionTrigger::InventoryChanged => "minecraft:inventory_changed",
            CriterionTrigger::RecipeUnlocked => "minecraft:recipe_unlocked",
            CriterionTrigger::PlayerHurtEntity => "minecraft:player_hurt_entity",
            CriterionTrigger::EntityHurtPlayer => "minecraft:entity_hurt_player",
            CriterionTrigger::EnchantedItem => "minecraft:enchanted_item",
            CriterionTrigger::FilledBucket => "minecraft:filled_bucket",
            CriterionTrigger::BrewedPotion => "minecraft:brewed_potion",
            CriterionTrigger::ConstructBeacon => "minecraft:construct_beacon",
            CriterionTrigger::UsedEnderEye => "minecraft:used_ender_eye",
            CriterionTrigger::SummonedEntity => "minecraft:summoned_entity",
            CriterionTrigger::BredAnimals => "minecraft:bred_animals",
            CriterionTrigger::Location => "minecraft:location",
            CriterionTrigger::SleptInBed => "minecraft:slept_in_bed",
            CriterionTrigger::CuredZombieVillager => "minecraft:cured_zombie_villager",
            CriterionTrigger::VillagerTrade => "minecraft:villager_trade",
            CriterionTrigger::ItemDurabilityChanged => "minecraft:item_durability_changed",
            CriterionTrigger::Levitation => "minecraft:levitation",
            CriterionTrigger::ChangedDimension => "minecraft:changed_dimension",
            CriterionTrigger::Tick => "minecraft:tick",
            CriterionTrigger::TameAnimal => "minecraft:tame_animal",
            CriterionTrigger::PlacedBlock => "minecraft:placed_block",
            CriterionTrigger::ConsumeItem => "minecraft:consume_item",
            CriterionTrigger::EffectsChanged => "minecraft:effects_changed",
            CriterionTrigger::UsedTotem => "minecraft:used_totem",
            CriterionTrigger::NetherTravel => "minecraft:nether_travel",
            CriterionTrigger::FishingRodHooked => "minecraft:fishing_rod_hooked",
            CriterionTrigger::ChanneledLightning => "minecraft:channeled_lightning",
            CriterionTrigger::ShotCrossbow => "minecraft:shot_crossbow",
            CriterionTrigger::SpearMobs => "minecraft:spear_mobs",
            CriterionTrigger::KilledByArrow => "minecraft:killed_by_arrow",
            CriterionTrigger::HeroOfTheVillage => "minecraft:hero_of_the_village",
            CriterionTrigger::VoluntaryExile => "minecraft:voluntary_exile",
            CriterionTrigger::SlideDownBlock => "minecraft:slide_down_block",
            CriterionTrigger::BeeNestDestroyed => "minecraft:bee_nest_destroyed",
            CriterionTrigger::TargetHit => "minecraft:target_hit",
            CriterionTrigger::ItemUsedOnBlock => "minecraft:item_used_on_block",
            CriterionTrigger::DefaultBlockUse => "minecraft:default_block_use",
            CriterionTrigger::AnyBlockUse => "minecraft:any_block_use",
            CriterionTrigger::PlayerGeneratesContainerLoot => "minecraft:player_generates_container_loot",
            CriterionTrigger::ThrownItemPickedUpByEntity => "minecraft:thrown_item_picked_up_by_entity",
            CriterionTrigger::ThrownItemPickedUpByPlayer => "minecraft:thrown_item_picked_up_by_player",
            CriterionTrigger::PlayerInteractedWithEntity => "minecraft:player_interacted_with_entity",
            CriterionTrigger::PlayerShearedEquipment => "minecraft:player_sheared_equipment",
            CriterionTrigger::StartedRiding => "minecraft:started_riding",
            CriterionTrigger::LightningStrike => "minecraft:lightning_strike",
            CriterionTrigger::UsingItem => "minecraft:using_item",
            CriterionTrigger::FallFromHeight => "minecraft:fall_from_height",
            CriterionTrigger::RideEntityInLava => "minecraft:ride_entity_in_lava",
            CriterionTrigger::KillMobNearSculkCatalyst => "minecraft:kill_mob_near_sculk_catalyst",
            CriterionTrigger::AllayDropItemOnBlock => "minecraft:allay_drop_item_on_block",
            CriterionTrigger::AvoidVibration => "minecraft:avoid_vibration",
            CriterionTrigger::RecipeCrafted => "minecraft:recipe_crafted",
            CriterionTrigger::CrafterRecipeCrafted => "minecraft:crafter_recipe_crafted",
            CriterionTrigger::FallAfterExplosion => "minecraft:fall_after_explosion",
        }
    }

    pub fn by_name(name: &str) -> Option<CriterionTrigger> {
        CriterionTrigger::VALUES.into_iter().find(|trigger| trigger.name() == name)
    }

    pub fn decode_instance(self, tag: &Nbt) -> DecodeResult<Instance> {
        match self {
            CriterionTrigger::Impossible => match crate::util::codec::map(tag) {
                Ok(_) => DecodeResult::success(Instance::Impossible),
                Err(error) => DecodeResult::error(error),
            },
            CriterionTrigger::PlayerKilledEntity | CriterionTrigger::EntityKilledPlayer | CriterionTrigger::KillMobNearSculkCatalyst => {
                killed_trigger::TriggerInstance::decode(tag).map(Instance::Killed)
            }
            CriterionTrigger::EnterBlock => enter_block_trigger::TriggerInstance::decode(tag).map(Instance::EnterBlock),
            CriterionTrigger::InventoryChanged => inventory_change_trigger::TriggerInstance::decode(tag).map(Instance::InventoryChange),
            CriterionTrigger::RecipeUnlocked => recipe_unlocked_trigger::TriggerInstance::decode(tag).map(Instance::RecipeUnlocked),
            CriterionTrigger::PlayerHurtEntity => player_hurt_entity_trigger::TriggerInstance::decode(tag).map(Instance::PlayerHurtEntity),
            CriterionTrigger::EntityHurtPlayer => entity_hurt_player_trigger::TriggerInstance::decode(tag).map(Instance::EntityHurtPlayer),
            CriterionTrigger::EnchantedItem => enchanted_item_trigger::TriggerInstance::decode(tag).map(Instance::EnchantedItem),
            CriterionTrigger::FilledBucket => filled_bucket_trigger::TriggerInstance::decode(tag).map(Instance::FilledBucket),
            CriterionTrigger::BrewedPotion => brewed_potion_trigger::TriggerInstance::decode(tag).map(Instance::BrewedPotion),
            CriterionTrigger::ConstructBeacon => construct_beacon_trigger::TriggerInstance::decode(tag).map(Instance::ConstructBeacon),
            CriterionTrigger::UsedEnderEye => used_ender_eye_trigger::TriggerInstance::decode(tag).map(Instance::UsedEnderEye),
            CriterionTrigger::SummonedEntity => summoned_entity_trigger::TriggerInstance::decode(tag).map(Instance::SummonedEntity),
            CriterionTrigger::BredAnimals => bred_animals_trigger::TriggerInstance::decode(tag).map(Instance::BredAnimals),
            CriterionTrigger::Location
            | CriterionTrigger::SleptInBed
            | CriterionTrigger::Tick
            | CriterionTrigger::HeroOfTheVillage
            | CriterionTrigger::VoluntaryExile
            | CriterionTrigger::AvoidVibration => player_trigger::TriggerInstance::decode(tag).map(Instance::Player),
            CriterionTrigger::CuredZombieVillager => cured_zombie_villager_trigger::TriggerInstance::decode(tag).map(Instance::CuredZombieVillager),
            CriterionTrigger::VillagerTrade => trade_trigger::TriggerInstance::decode(tag).map(Instance::Trade),
            CriterionTrigger::ItemDurabilityChanged => item_durability_trigger::TriggerInstance::decode(tag).map(Instance::ItemDurability),
            CriterionTrigger::Levitation => levitation_trigger::TriggerInstance::decode(tag).map(Instance::Levitation),
            CriterionTrigger::ChangedDimension => change_dimension_trigger::TriggerInstance::decode(tag).map(Instance::ChangeDimension),
            CriterionTrigger::TameAnimal => tame_animal_trigger::TriggerInstance::decode(tag).map(Instance::TameAnimal),
            CriterionTrigger::PlacedBlock | CriterionTrigger::ItemUsedOnBlock | CriterionTrigger::AllayDropItemOnBlock => {
                item_used_on_location_trigger::TriggerInstance::decode(tag).map(Instance::ItemUsedOnLocation)
            }
            CriterionTrigger::ConsumeItem => consume_item_trigger::TriggerInstance::decode(tag).map(Instance::ConsumeItem),
            CriterionTrigger::EffectsChanged => effects_changed_trigger::TriggerInstance::decode(tag).map(Instance::EffectsChanged),
            CriterionTrigger::UsedTotem => used_totem_trigger::TriggerInstance::decode(tag).map(Instance::UsedTotem),
            CriterionTrigger::NetherTravel | CriterionTrigger::FallFromHeight | CriterionTrigger::RideEntityInLava => {
                distance_trigger::TriggerInstance::decode(tag).map(Instance::Distance)
            }
            CriterionTrigger::FishingRodHooked => fishing_rod_hooked_trigger::TriggerInstance::decode(tag).map(Instance::FishingRodHooked),
            CriterionTrigger::ChanneledLightning => channeled_lightning_trigger::TriggerInstance::decode(tag).map(Instance::ChanneledLightning),
            CriterionTrigger::ShotCrossbow => shot_crossbow_trigger::TriggerInstance::decode(tag).map(Instance::ShotCrossbow),
            CriterionTrigger::SpearMobs => spear_mobs_trigger::TriggerInstance::decode(tag).map(Instance::SpearMobs),
            CriterionTrigger::KilledByArrow => killed_by_arrow_trigger::TriggerInstance::decode(tag).map(Instance::KilledByArrow),
            CriterionTrigger::SlideDownBlock => slide_down_block_trigger::TriggerInstance::decode(tag).map(Instance::SlideDownBlock),
            CriterionTrigger::BeeNestDestroyed => bee_nest_destroyed_trigger::TriggerInstance::decode(tag).map(Instance::BeeNestDestroyed),
            CriterionTrigger::TargetHit => target_block_trigger::TriggerInstance::decode(tag).map(Instance::TargetBlock),
            CriterionTrigger::DefaultBlockUse => default_block_interaction_trigger::TriggerInstance::decode(tag).map(Instance::DefaultBlockInteraction),
            CriterionTrigger::AnyBlockUse => any_block_interaction_trigger::TriggerInstance::decode(tag).map(Instance::AnyBlockInteraction),
            CriterionTrigger::PlayerGeneratesContainerLoot => loot_table_trigger::TriggerInstance::decode(tag).map(Instance::LootTable),
            CriterionTrigger::ThrownItemPickedUpByEntity | CriterionTrigger::ThrownItemPickedUpByPlayer => {
                picked_up_item_trigger::TriggerInstance::decode(tag).map(Instance::PickedUpItem)
            }
            CriterionTrigger::PlayerInteractedWithEntity | CriterionTrigger::PlayerShearedEquipment => {
                player_interact_trigger::TriggerInstance::decode(tag).map(Instance::PlayerInteract)
            }
            CriterionTrigger::StartedRiding => start_riding_trigger::TriggerInstance::decode(tag).map(Instance::StartRiding),
            CriterionTrigger::LightningStrike => lightning_strike_trigger::TriggerInstance::decode(tag).map(Instance::LightningStrike),
            CriterionTrigger::UsingItem => using_item_trigger::TriggerInstance::decode(tag).map(Instance::UsingItem),
            CriterionTrigger::RecipeCrafted | CriterionTrigger::CrafterRecipeCrafted => {
                recipe_crafted_trigger::TriggerInstance::decode(tag).map(Instance::RecipeCrafted)
            }
            CriterionTrigger::FallAfterExplosion => fall_after_explosion_trigger::TriggerInstance::decode(tag).map(Instance::FallAfterExplosion),
        }
    }
}
