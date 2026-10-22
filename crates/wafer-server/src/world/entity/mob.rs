use std::cell::Cell;

use wafer_core::random::RandomSource;
use wafer_core::{AABB, BlockPos};
use wafer_data::{EquipmentSlot, EquipmentSlotGroup, Item, damage_types, entity_type_tags, game_rule, items};
use wafer_protocol::nbt::Nbt;

use super::monster::creeper::{self, Creeper};
use super::monster::skeleton::abstract_skeleton::{self, AbstractSkeleton};
use super::monster::skeleton::bogged::Bogged;
use super::monster::skeleton::skeleton::Skeleton;
use super::monster::skeleton::parched::Parched;
use super::monster::skeleton::stray::Stray;
use super::monster::zombie::drowned::{self, Drowned};
use super::monster::zombie::husk::{self, Husk};
use super::monster::illager::evoker::{self, Evoker};
use super::monster::illager::illusioner::{self, Illusioner};
use super::monster::illager::pillager::{self, Pillager};
use super::animal::equine::skeleton_horse::{self, SkeletonHorse};
use super::animal::equine::zombie_horse::{self, ZombieHorse};
use super::animal::equine::llama::{self, Llama};
use super::animal::equine::trader_llama::{self, TraderLlama};
use super::npc::wanderingtrader::wandering_trader::WanderingTrader;
use super::animal::polarbear::polar_bear::{self, PolarBear};
use super::monster::ravager::{self, Ravager};
use super::monster::vex::{self, Vex};
use super::monster::illager::vindicator::{self, Vindicator};
use super::monster::giant::{self, Giant};
use super::monster::skeleton::wither_skeleton::{self, WitherSkeleton};
use super::monster::zombie::zombie_villager::{self, ZombieVillager};
use super::monster::zombie::zombie::{self, Zombie};
use super::monster::zombie::zombified_piglin::ZombifiedPiglin;
use super::monster::blaze::{self, Blaze};
use super::monster::ghast::{self, Ghast};
use super::monster::spider::cave_spider::{self, CaveSpider};
use super::monster::spider::spider::{self, Spider};
use super::monster::endermite::{self, Endermite};
use super::monster::silverfish::{self, Silverfish};
use super::monster::enderman::{self, Enderman};
use super::neutral_mob::NeutralMob;
use super::monster::patrolling_monster::PatrollingMonster;
use super::monster::witch::{self, Witch};
use super::raid::raider::{self, Raider};
use super::monster::cubemob::magma_cube::{self, MagmaCube};
use super::monster::cubemob::slime::{self, Slime};
use super::ambient::bat::{self, Bat};
use super::animal::squid::glow_squid::GlowSquid;
use super::animal::feline::cat::{self, Cat};
use super::animal::feline::ocelot::{self, Ocelot};
use super::animal::fox::fox::{self, Fox};
use super::animal::squid::squid::{self as squid, Squid};
use super::animal::ageable_water_creature;
use super::animal::golem::abstract_golem;
use super::animal::golem::iron_golem::{self, IronGolem};
use super::animal::golem::snow_golem::{self, SnowGolem};
use super::animal::fish::abstract_fish::AbstractFish;
use super::animal::fish::abstract_schooling_fish::AbstractSchoolingFish;
use super::animal::fish::cod::{self, Cod};
use super::animal::fish::pufferfish::Pufferfish;
use super::animal::fish::salmon::{self, Salmon};
use super::animal::fish::tropical_fish::{self, TropicalFish};
use super::animal::fish::water_animal;
use super::entity_spawn_reason::EntitySpawnReason;
use super::spawn_group_data::SpawnGroupData;
use crate::world::attribute::environment_attributes;
use crate::world::difficulty::Difficulty;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::item::component::attack_range::AttackRange;
use crate::world::item::component::values::ItemEnchantments;
use crate::world::item::enchantment::enchantment_helper;
use crate::world::item::enchantment::providers::vanilla_enchantment_providers;

use super::animal::chicken::{self, Chicken};
use super::animal::cow::abstract_cow::{self, AbstractCow};
use super::animal::cow::cow::{self, Cow};
use super::animal::cow::mushroom_cow::{self, MushroomCow};
use super::animal::pig::pig::{self, Pig};
use super::animal::sheep::sheep::{self, Sheep};
use super::animal::wolf::wolf::{self, Wolf};
use super::animal::equine::abstract_chested_horse::AbstractChestedHorse;
use super::animal::equine::abstract_horse::{self, AbstractHorse};
use super::animal::equine::donkey::{self, Donkey};
use super::animal::equine::horse::{self, Horse};
use super::animal::equine::mule::{self, Mule};
use super::animal::rabbit::rabbit::{self, Rabbit};
use super::animal::turtle::turtle::{self, Turtle};
use super::tamable_animal::TamableAnimal;
use super::animal::goat::goat::{self, Goat};
use super::animal::armadillo::armadillo::{self, Armadillo};
use super::animal::axolotl::axolotl::{self, Axolotl};
use super::animal::frog::frog::{self, Frog};
use super::animal::frog::tadpole::{self, Tadpole};
use super::npc::villager::villager::{self, Villager};
use super::ai::brain::{self, Brain};
use super::ai::memory::memory_module_type;
use super::ageable_mob::{self, AgeableMob, BABY_START_AGE};
use super::conversion_params::ConversionParams;
use super::variant::variant_utils;
use crate::world::item::dye_color::DyeColor;
use super::ai::MobAi;
use super::ai::goal::goal::Flag;
use super::ai::control::move_control::MoveControlKind;
use super::ai::navigation::path_navigation::{PathNavigation, PathNavigationKind};
use crate::world::level::pathfinder::path_type::PathType;
use super::animal::Animal;
use super::attributes::Attribute;
use super::data::{DataValue, ids};
use super::entity_save::uuid_from_nbt;
use super::ai::goal::goal_selector::GoalSelector;
use super::living_entity::{LivingEntity, get_comfortable_fall_distance};
use super::{Entity, EntityDimensions, EntityId, LevelEntity};
use crate::core::component::data_component_getter::DataComponentGetter;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::effect::MobEffectInstance;
use crate::world::item::component::values::ItemStackTemplate;
use crate::world::item::component::{self as dc, DataComponentType};
use crate::world::item::{self, ItemStack, spawn_egg_item};
use crate::world::{InteractionHand, InteractionResult};
use crate::world::level::block::entity::int_value;
use crate::world::level::block::state::block_behaviour;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::storage::uuid_ints;
use super::entity_type::EntityType;

pub const DEFAULT_EQUIPMENT_DROP_CHANCE: f32 = 0.085;
const MOB_FLAG_NO_AI: i8 = 1;
const MOB_FLAG_LEFTHANDED: i8 = 2;
const MOB_FLAG_AGGRESSIVE: i8 = 4;
const ITEM_PICKUP_REACH: (f64, f64, f64) = (1.0, 0.0, 1.0);
const EQUIPMENT_POPULATION_ORDER: [EquipmentSlot; 4] = [EquipmentSlot::Head, EquipmentSlot::Chest, EquipmentSlot::Legs, EquipmentSlot::Feet];
pub const MAX_WEARING_ARMOR_CHANCE: f32 = 0.15;
pub const WEARING_ARMOR_UPGRADE_MATERIAL_CHANCE: f32 = 0.1087;
pub const WEARING_ARMOR_UPGRADE_MATERIAL_ATTEMPTS: f32 = 3.0;
pub const MAX_ENCHANTED_ARMOR_CHANCE: f32 = 0.5;
pub const MAX_ENCHANTED_WEAPON_CHANCE: f32 = 0.25;
pub const RANDOM_SPAWN_BONUS_ID: &str = "minecraft:random_spawn_bonus";

fn default_attack_reach() -> f64 {
    (2.04f32 as f64).sqrt() - 0.6f32 as f64
}

pub enum MobKind {
    Chicken(Chicken),
    Cow(Cow),
    MushroomCow(MushroomCow),
    Pig(Pig),
    Sheep(Sheep),
    Goat(Goat),
    PolarBear(PolarBear),
    Armadillo(Armadillo),
    Zombie(Zombie),
    Husk(Husk),
    Vindicator(Vindicator),
    Pillager(Pillager),
    Evoker(Evoker),
    Illusioner(Illusioner),
    Vex(Vex),
    Phantom(crate::world::entity::monster::phantom::Phantom),
    Guardian(crate::world::entity::monster::guardian::Guardian),
    ElderGuardian(crate::world::entity::monster::guardian::Guardian),
    Ravager(Ravager),
    SkeletonHorse(SkeletonHorse),
    ZombieHorse(ZombieHorse),
    Llama(Llama),
    TraderLlama(Llama),
    WanderingTrader(WanderingTrader),
    Giant(Giant),
    WitherSkeleton(WitherSkeleton),
    ZombieVillager(ZombieVillager),
    Creeper(Creeper),
    Spider(Spider),
    CaveSpider(CaveSpider),
    Skeleton(Skeleton),
    Stray(Stray),
    Parched(Parched),
    Bogged(Bogged),
    Drowned(Drowned),
    Silverfish(Silverfish),
    Endermite(Endermite),
    Enderman(Enderman),
    Witch(Witch),
    Slime(Slime),
    MagmaCube(MagmaCube),
    Wolf(Wolf),
    Bat(Bat),
    Squid(Squid),
    GlowSquid(GlowSquid),
    IronGolem(IronGolem),
    SnowGolem(SnowGolem),
    Cod(Cod),
    Salmon(Salmon),
    Pufferfish(Pufferfish),
    TropicalFish(TropicalFish),
    Cat(Cat),
    Ocelot(Ocelot),
    Horse(Horse),
    Donkey(Donkey),
    Mule(Mule),
    Axolotl(Axolotl),
    Frog(Frog),
    Tadpole(Tadpole),
    Villager(Villager),
    Fox(Fox),
    Blaze(Blaze),
    Ghast(Ghast),
    ZombifiedPiglin(ZombifiedPiglin),
    Rabbit(Rabbit),
    Turtle(Turtle),
}

pub struct Mob {
    pub leash_data: Option<super::leashable::LeashData>,
    pub entity: Entity,
    pub living: LivingEntity,
    pub health: f32,
    pub absorption: f32,
    pub equipment: [Option<ItemStack>; 8],
    pub drop_chances: [f32; 8],
    pub ambient_sound_time: i32,
    pub xp_reward: i32,
    pub can_pick_up_loot: bool,
    pub persistence_required: bool,
    pub loot_table: Option<String>,
    pub loot_table_seed: i64,
    pub home_position: BlockPos,
    pub home_radius: i32,
    pub target: Option<EntityId>,
    pub ai: MobAi,
    pathfinding_malus: [Cell<Option<f32>>; PathType::COUNT],
    pub kind: MobKind,
}

impl Mob {
    pub fn new(entity: Entity, living: LivingEntity, kind: MobKind) -> Mob {
        let mut mob = Mob {
            leash_data: None,
            entity,
            living,
            health: 1.0,
            absorption: 0.0,
            equipment: Default::default(),
            drop_chances: [DEFAULT_EQUIPMENT_DROP_CHANCE; 8],
            ambient_sound_time: 0,
            xp_reward: 0,
            can_pick_up_loot: false,
            persistence_required: false,
            loot_table: None,
            loot_table_seed: 0,
            home_position: BlockPos::ZERO,
            home_radius: -1,
            target: None,
            ai: MobAi::default(),
            pathfinding_malus: Default::default(),
            kind,
        };
        let max_health = mob.living.attributes.value(Attribute::MaxHealth) as f32;
        mob.set_health(max_health);
        let e = &mut mob.entity;
        e.reapply_position();
        let y_rot = e.random.next_float() * std::f32::consts::TAU;
        e.set_y_rot(y_rot);
        e.y_head_rot = e.y_rot;
        mob.living.brain = mob.make_brain(brain::Packed::EMPTY);
        mob.ai.navigation = mob.create_navigation();
        mob.register_goals();
        mob
    }

    fn register_goals(&mut self) {
        match self.kind {
            MobKind::Chicken(_) => Chicken::register_goals(&mut self.ai),
            MobKind::Cow(_) | MobKind::MushroomCow(_) => AbstractCow::register_goals(&mut self.ai),
            MobKind::Pig(_) => Pig::register_goals(&mut self.ai),
            MobKind::Sheep(_) => Sheep::register_goals(&mut self.ai),
            MobKind::Goat(_) => {}
            MobKind::PolarBear(_) => PolarBear::register_goals(&mut self.ai),
            MobKind::Armadillo(_) => {}
            MobKind::Zombie(_) | MobKind::ZombieVillager(_) | MobKind::Husk(_) | MobKind::Drowned(_) | MobKind::ZombifiedPiglin(_) => Zombie::register_goals(&mut self.ai, &self.kind),
            MobKind::Blaze(_) => Blaze::register_goals(&mut self.ai),
            MobKind::Ghast(_) => Ghast::register_goals(&mut self.ai),
            MobKind::Phantom(_) => crate::world::entity::monster::phantom::Phantom::register_goals(&mut self.ai),
            MobKind::Creeper(_) => Creeper::register_goals(&mut self.ai),
            MobKind::Spider(_) | MobKind::CaveSpider(_) => Spider::register_goals(&mut self.ai),
            MobKind::Skeleton(_) | MobKind::Stray(_) | MobKind::Parched(_) | MobKind::Bogged(_) => AbstractSkeleton::register_goals(&mut self.ai),
            MobKind::WitherSkeleton(_) => WitherSkeleton::register_goals(&mut self.ai),
            MobKind::Silverfish(_) => Silverfish::register_goals(&mut self.ai),
            MobKind::Endermite(_) => Endermite::register_goals(&mut self.ai),
            MobKind::Giant(_) => {}
            MobKind::Enderman(_) => Enderman::register_goals(&mut self.ai),
            MobKind::Witch(_) => Witch::register_goals(&mut self.ai),
            MobKind::Vindicator(_) => Vindicator::register_goals(&mut self.ai),
            MobKind::Pillager(_) => Pillager::register_goals(&mut self.ai),
            MobKind::Evoker(_) => Evoker::register_goals(&mut self.ai),
            MobKind::Illusioner(_) => Illusioner::register_goals(&mut self.ai),
            MobKind::Ravager(_) => Ravager::register_goals(&mut self.ai),
            MobKind::Vex(_) => Vex::register_goals(&mut self.ai),
            MobKind::Guardian(_) => crate::world::entity::monster::guardian::Guardian::register_goals(&mut self.ai, 80),
            MobKind::ElderGuardian(_) => crate::world::entity::monster::elder_guardian::ElderGuardian::register_goals(&mut self.ai),
            MobKind::Slime(_) => Slime::register_goals(&mut self.ai),
            MobKind::MagmaCube(_) => MagmaCube::register_goals(&mut self.ai),
            MobKind::Wolf(_) => Wolf::register_goals(&mut self.ai, self.living.attributes.value(Attribute::FollowRange)),
            MobKind::Bat(_) => {}
            MobKind::Squid(_) | MobKind::GlowSquid(_) => Squid::register_goals(&mut self.ai),
            MobKind::IronGolem(_) => IronGolem::register_goals(&mut self.ai),
            MobKind::SnowGolem(_) => SnowGolem::register_goals(&mut self.ai),
            MobKind::Cod(_) | MobKind::Salmon(_) | MobKind::TropicalFish(_) => AbstractSchoolingFish::register_goals(&mut self.ai, &mut self.entity.random),
            MobKind::Pufferfish(_) => Pufferfish::register_goals(&mut self.ai),
            MobKind::Cat(_) => Cat::register_goals(&mut self.ai, self.living.attributes.value(Attribute::FollowRange)),
            MobKind::Ocelot(_) => Ocelot::register_goals(&mut self.ai),
            MobKind::Horse(_) | MobKind::Donkey(_) | MobKind::Mule(_) => AbstractHorse::register_goals(&mut self.ai, AbstractHorse::add_behaviour_goals),
            MobKind::SkeletonHorse(_) => AbstractHorse::register_goals(&mut self.ai, |_| {}),
            MobKind::ZombieHorse(_) => AbstractHorse::register_goals(&mut self.ai, ZombieHorse::add_behaviour_goals),
            MobKind::Llama(_) => Llama::register_goals(&mut self.ai),
            MobKind::TraderLlama(_) => TraderLlama::register_goals(&mut self.ai),
            MobKind::WanderingTrader(_) => WanderingTrader::register_goals(&mut self.ai),
            MobKind::Axolotl(_) | MobKind::Frog(_) => {}
            MobKind::Tadpole(_) => AbstractFish::register_goals(&mut self.ai),
            MobKind::Villager(_) => {}
            MobKind::Fox(_) => Fox::register_goals(&mut self.ai, &mut self.entity.random, self.living.attributes.value(Attribute::FollowRange)),
            MobKind::Rabbit(_) => Rabbit::register_goals(&mut self.ai),
            MobKind::Turtle(_) => {
                let is_baby = self.is_baby();
                Turtle::register_goals(&mut self.ai, is_baby)
            }
        }
    }

    pub fn make_brain(&mut self, packed_brain: brain::Packed) -> Brain {
        match self.kind {
            MobKind::Goat(_) => Goat::make_brain(self, packed_brain),
            MobKind::Armadillo(_) => Armadillo::make_brain(self, packed_brain),
            MobKind::Axolotl(_) => Axolotl::make_brain(self, packed_brain),
            MobKind::Frog(_) => Frog::make_brain(self, packed_brain),
            MobKind::Tadpole(_) => Tadpole::make_brain(self, packed_brain),
            MobKind::Villager(_) => Villager::make_brain(self, packed_brain),
            _ => Brain::new(),
        }
    }

    fn age_boundary_reached(&mut self) {
        if let MobKind::Goat(_) = self.kind {
            Goat::age_boundary_reached(self);
        }
    }

    pub fn set_y_head_rot(&mut self, y_head_rot: f32) {
        match self.kind {
            MobKind::Goat(_) => Goat::set_y_head_rot(self, y_head_rot),
            _ => self.entity.y_head_rot = y_head_rot,
        }
    }

    pub fn get_dimensions(&self, pose: super::data::Pose) -> EntityDimensions {
        let default_dimensions = match self.kind {
            MobKind::Goat(_) => goat::get_default_dimensions(self.is_baby(), pose),
            _ => self.default_dimensions(),
        };
        default_dimensions.scale(self.living.attributes.value(Attribute::Scale) as f32)
    }

    pub fn is_monster(&self) -> bool {
        matches!(self.kind, MobKind::Zombie(_) | MobKind::ZombieVillager(_) | MobKind::Husk(_) | MobKind::Drowned(_) | MobKind::Creeper(_) | MobKind::Silverfish(_) | MobKind::Endermite(_) | MobKind::Giant(_))
            || self.is_spider()
            || self.is_abstract_skeleton()
            || matches!(self.kind, MobKind::Enderman(_))
            || self.raider().is_some()
            || matches!(self.kind, MobKind::Blaze(_) | MobKind::ZombifiedPiglin(_))
            || matches!(self.kind, MobKind::Vex(_) | MobKind::Guardian(_) | MobKind::ElderGuardian(_))
    }

    pub fn neutral_mob(&self) -> Option<&dyn NeutralMob> {
        match &self.kind {
            MobKind::Enderman(enderman) => Some(enderman),
            MobKind::Wolf(wolf) => Some(wolf),
            MobKind::IronGolem(iron_golem) => Some(iron_golem),
            MobKind::PolarBear(polar_bear) => Some(polar_bear),
            MobKind::ZombifiedPiglin(zombified_piglin) => Some(zombified_piglin),
            _ => None,
        }
    }

    pub fn neutral_mob_mut(&mut self) -> Option<(&mut dyn NeutralMob, &mut Entity)> {
        match &mut self.kind {
            MobKind::Enderman(enderman) => Some((enderman, &mut self.entity)),
            MobKind::Wolf(wolf) => Some((wolf, &mut self.entity)),
            MobKind::IronGolem(iron_golem) => Some((iron_golem, &mut self.entity)),
            MobKind::PolarBear(polar_bear) => Some((polar_bear, &mut self.entity)),
            MobKind::ZombifiedPiglin(zombified_piglin) => Some((zombified_piglin, &mut self.entity)),
            _ => None,
        }
    }

    pub fn tamable_animal(&self) -> Option<&TamableAnimal> {
        match &self.kind {
            MobKind::Wolf(wolf) => Some(&wolf.tamable_animal),
            MobKind::Cat(cat) => Some(&cat.tamable_animal),
            _ => None,
        }
    }

    pub fn tamable_animal_mut(&mut self) -> Option<&mut TamableAnimal> {
        match &mut self.kind {
            MobKind::Wolf(wolf) => Some(&mut wolf.tamable_animal),
            MobKind::Cat(cat) => Some(&mut cat.tamable_animal),
            _ => None,
        }
    }

    pub fn llama_strength(&self) -> Option<i32> {
        matches!(self.kind, MobKind::Llama(_) | MobKind::TraderLlama(_)).then(|| llama::get_strength(&self.entity))
    }

    pub fn abstract_horse(&self) -> Option<&AbstractHorse> {
        match &self.kind {
            MobKind::Horse(horse) => Some(&horse.abstract_horse),
            MobKind::Donkey(donkey) => Some(&donkey.abstract_chested_horse.abstract_horse),
            MobKind::Mule(mule) => Some(&mule.abstract_chested_horse.abstract_horse),
            MobKind::SkeletonHorse(horse) => Some(&horse.abstract_horse),
            MobKind::ZombieHorse(horse) => Some(&horse.abstract_horse),
            MobKind::Llama(llama) | MobKind::TraderLlama(llama) => Some(&llama.abstract_chested_horse.abstract_horse),
            _ => None,
        }
    }

    pub fn abstract_horse_mut(&mut self) -> Option<&mut AbstractHorse> {
        match &mut self.kind {
            MobKind::Horse(horse) => Some(&mut horse.abstract_horse),
            MobKind::Donkey(donkey) => Some(&mut donkey.abstract_chested_horse.abstract_horse),
            MobKind::Mule(mule) => Some(&mut mule.abstract_chested_horse.abstract_horse),
            MobKind::SkeletonHorse(horse) => Some(&mut horse.abstract_horse),
            MobKind::ZombieHorse(horse) => Some(&mut horse.abstract_horse),
            MobKind::Llama(llama) | MobKind::TraderLlama(llama) => Some(&mut llama.abstract_chested_horse.abstract_horse),
            _ => None,
        }
    }

    pub fn abstract_chested_horse(&self) -> Option<&AbstractChestedHorse> {
        match &self.kind {
            MobKind::Donkey(donkey) => Some(&donkey.abstract_chested_horse),
            MobKind::Mule(mule) => Some(&mule.abstract_chested_horse),
            MobKind::Llama(llama) | MobKind::TraderLlama(llama) => Some(&llama.abstract_chested_horse),
            _ => None,
        }
    }

    pub fn is_zombie(&self) -> bool {
        matches!(self.kind, MobKind::Zombie(_) | MobKind::ZombieVillager(_) | MobKind::Husk(_) | MobKind::Drowned(_) | MobKind::ZombifiedPiglin(_))
    }

    pub fn is_spider(&self) -> bool {
        matches!(self.kind, MobKind::Spider(_) | MobKind::CaveSpider(_))
    }

    pub fn is_abstract_illager(&self) -> bool {
        matches!(self.kind, MobKind::Vindicator(_) | MobKind::Pillager(_) | MobKind::Evoker(_) | MobKind::Illusioner(_))
    }

    pub fn set_custom_name(&mut self, name: Option<Nbt>) {
        self.entity.set_custom_name(name.clone());
        if let MobKind::Vindicator(_) = self.kind {
            Vindicator::on_custom_name_set(self, name.as_ref());
        }
    }

    pub fn is_abstract_skeleton(&self) -> bool {
        matches!(self.kind, MobKind::Skeleton(_) | MobKind::Stray(_) | MobKind::Parched(_) | MobKind::WitherSkeleton(_) | MobKind::Bogged(_))
    }

    pub fn client_tracking_range(&self) -> i32 {
        if matches!(self.kind, MobKind::Tadpole(_) | MobKind::Ravager(_) | MobKind::PolarBear(_) | MobKind::SkeletonHorse(_) | MobKind::ZombieHorse(_) | MobKind::Llama(_) | MobKind::TraderLlama(_) | MobKind::WanderingTrader(_)) {
            10
        } else if self.is_abstract_fish() {
            4
        } else if matches!(self.kind, MobKind::Bat(_)) {
            5
        } else if self.is_zombie()
            || matches!(self.kind, MobKind::Creeper(_) | MobKind::Silverfish(_) | MobKind::Endermite(_) | MobKind::Enderman(_) | MobKind::Witch(_) | MobKind::Vindicator(_) | MobKind::Pillager(_) | MobKind::Evoker(_) | MobKind::Illusioner(_) | MobKind::Vex(_) | MobKind::Ravager(_) | MobKind::MagmaCube(_) | MobKind::Phantom(_) | MobKind::Guardian(_))
            || matches!(self.kind, MobKind::Squid(_))
            || matches!(self.kind, MobKind::SnowGolem(_))
            || matches!(self.kind, MobKind::Cat(_))
            || matches!(self.kind, MobKind::Mule(_))
            || matches!(self.kind, MobKind::Fox(_))
            || matches!(self.kind, MobKind::Blaze(_))
            || matches!(self.kind, MobKind::Rabbit(_))
            || self.is_spider()
            || self.is_abstract_skeleton()
        {
            8
        } else {
            10
        }
    }

    pub fn track_deltas(&self) -> bool {
        !matches!(self.kind, MobKind::Bat(_))
    }

    fn create_navigation(&self) -> PathNavigation {
        let follow_range_base = self.living.attributes.base_value(Attribute::FollowRange);
        match self.kind {
            MobKind::Spider(_) | MobKind::CaveSpider(_) => PathNavigation::wall_climber(follow_range_base),
            MobKind::Drowned(_) | MobKind::Axolotl(_) => PathNavigation::of_kind(PathNavigationKind::Amphibious, follow_range_base),
            MobKind::Frog(_) => PathNavigation::of_kind(PathNavigationKind::Frog, follow_range_base),
            MobKind::Cod(_) | MobKind::Salmon(_) | MobKind::Pufferfish(_) | MobKind::TropicalFish(_) | MobKind::Tadpole(_) => PathNavigation::of_kind(PathNavigationKind::WaterBound, follow_range_base),
            MobKind::Guardian(_) | MobKind::ElderGuardian(_) => PathNavigation::of_kind(PathNavigationKind::WaterBound, follow_range_base),
            MobKind::Turtle(_) => Turtle::create_navigation(follow_range_base),
            _ => PathNavigation::new(follow_range_base),
        }
    }

    pub fn get_pathfinding_malus(&self, path_type: PathType) -> f32 {
        self.pathfinding_malus[path_type as usize].get().unwrap_or_else(|| path_type.get_malus())
    }

    pub fn set_pathfinding_malus(&self, path_type: PathType, cost: f32) {
        self.pathfinding_malus[path_type as usize].set(Some(cost));
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.living.speed = speed;
        self.living.zza = speed;
    }

    pub fn get_max_head_x_rot(&self) -> i32 {
        match self.kind {
            MobKind::Wolf(_) => wolf::get_max_head_x_rot(self),
            MobKind::Axolotl(_) => axolotl::get_max_head_x_rot(),
            MobKind::Guardian(_) | MobKind::ElderGuardian(_) => crate::world::entity::monster::guardian::Guardian::get_max_head_x_rot(),
            _ if self.is_cube_mob() => 0,
            _ => 40,
        }
    }

    fn update_control_flags(&mut self, no_controller: bool, not_in_boat: bool) {
        let goal_selector = &mut self.ai.goal_selector;
        goal_selector.set_control_flag(Flag::Move, no_controller);
        goal_selector.set_control_flag(Flag::Jump, no_controller && not_in_boat);
        goal_selector.set_control_flag(Flag::Look, no_controller);
        if let MobKind::Ravager(_) = self.kind {
            goal_selector.set_control_flag(Flag::Target, no_controller);
        }
    }

    pub fn get_head_rot_speed(&self) -> i32 {
        match self.kind {
            MobKind::Frog(_) => frog::get_head_rot_speed(),
            _ => 10,
        }
    }

    pub fn get_type(&self) -> &'static EntityType {
        EntityType::by_id(self.entity.entity_type)
    }

    pub fn is_persistence_required(&self) -> bool {
        self.persistence_required
    }

    pub fn remove_when_far_away(&self, dist_sqr: f64) -> bool {
        match &self.kind {
            MobKind::WanderingTrader(_) => false,
            MobKind::ZombieHorse(_) => true,
            MobKind::Chicken(chicken) => chicken::remove_when_far_away(chicken),
            MobKind::ZombieVillager(_) => zombie_villager::remove_when_far_away(self),
            MobKind::Zombie(_) | MobKind::Husk(_) | MobKind::Drowned(_) | MobKind::Creeper(_) | MobKind::Spider(_) | MobKind::CaveSpider(_) | MobKind::Skeleton(_) | MobKind::Stray(_) | MobKind::Parched(_) | MobKind::WitherSkeleton(_) | MobKind::Bogged(_) | MobKind::Silverfish(_) | MobKind::Endermite(_) | MobKind::Giant(_) => true,
            MobKind::Enderman(_) => true,
            MobKind::Witch(witch) => witch.raider.remove_when_far_away(dist_sqr),
            MobKind::Ravager(ravager) => ravager.raider.remove_when_far_away(dist_sqr),
            MobKind::Vindicator(vindicator) => vindicator.raider.remove_when_far_away(dist_sqr),
            MobKind::Pillager(pillager) => pillager.raider.remove_when_far_away(dist_sqr),
            MobKind::Evoker(evoker) => evoker.spellcaster_illager.raider.remove_when_far_away(dist_sqr),
            MobKind::Illusioner(illusioner) => illusioner.spellcaster_illager.raider.remove_when_far_away(dist_sqr),
            MobKind::Slime(_) | MobKind::MagmaCube(_) => true,
            MobKind::Bat(_) | MobKind::Squid(_) | MobKind::GlowSquid(_) => true,
            MobKind::IronGolem(_) | MobKind::SnowGolem(_) => abstract_golem::remove_when_far_away(dist_sqr),
            MobKind::Cod(_) | MobKind::Salmon(_) | MobKind::Pufferfish(_) | MobKind::TropicalFish(_) | MobKind::Tadpole(_) => self.abstract_fish_remove_when_far_away(),
            MobKind::Cat(_) => cat::remove_when_far_away(self),
            MobKind::Ocelot(_) => ocelot::remove_when_far_away(self),
            MobKind::Axolotl(_) => !axolotl::from_bucket(&self.entity) && !self.entity.has_custom_name(),
            MobKind::Villager(_) => false,
            MobKind::Blaze(_) | MobKind::Ghast(_) | MobKind::ZombifiedPiglin(_) | MobKind::Phantom(_) | MobKind::Guardian(_) | MobKind::ElderGuardian(_) => true,
            _ => super::animal::remove_when_far_away(dist_sqr),
        }
    }

    pub fn get_max_spawn_cluster_size(&self) -> i32 {
        match self.kind {
            MobKind::Wolf(_) => 8,
            MobKind::Cod(_) | MobKind::Salmon(_) | MobKind::TropicalFish(_) => self.abstract_schooling_fish_get_max_spawn_cluster_size(),
            MobKind::Pufferfish(_) | MobKind::Tadpole(_) => self.abstract_fish_get_max_spawn_cluster_size(),
            MobKind::Horse(_) | MobKind::Donkey(_) | MobKind::Mule(_) | MobKind::SkeletonHorse(_) | MobKind::ZombieHorse(_) | MobKind::Llama(_) | MobKind::TraderLlama(_) => abstract_horse::get_max_spawn_cluster_size(),
            MobKind::Ghast(_) => Ghast::get_max_spawn_cluster_size(),
            MobKind::Pillager(_) => Pillager::get_max_spawn_cluster_size(),
            _ => self.mob_get_max_spawn_cluster_size(),
        }
    }

    pub fn mob_get_max_spawn_cluster_size(&self) -> i32 {
        4
    }

    pub fn is_max_group_size_reached(&self, group_size: i32) -> bool {
        match &self.kind {
            MobKind::TropicalFish(tropical_fish) => TropicalFish::is_max_group_size_reached(tropical_fish),
            _ => self.mob_is_max_group_size_reached(group_size),
        }
    }

    pub fn mob_is_max_group_size_reached(&self, _group_size: i32) -> bool {
        false
    }

    pub fn has_home(&self) -> bool {
        self.home_radius != -1
    }

    pub fn set_home_to(&mut self, new_center: BlockPos, radius: i32) {
        self.home_position = new_center;
        self.home_radius = radius;
    }

    pub fn clear_home(&mut self) {
        self.home_radius = -1;
    }

    pub fn is_within_home(&self, pos: BlockPos) -> bool {
        self.home_radius == -1 || self.home_position.dist_sqr(pos) < (self.home_radius * self.home_radius) as f64
    }

    pub fn set_health(&mut self, health: f32) {
        self.health = wafer_core::mth::clamp_f(health, 0.0, self.living.attributes.value(Attribute::MaxHealth) as f32);
        self.entity.data.set(ids::HEALTH, DataValue::Float(self.health));
    }

    pub fn item_by_slot(&self, slot: EquipmentSlot) -> Option<&ItemStack> {
        self.equipment[slot as usize].as_ref().filter(|s| !s.is_empty())
    }

    fn mob_flags(&self) -> i8 {
        match self.entity.data.get(ids::MOB_FLAGS) {
            DataValue::Byte(flags) => *flags,
            _ => 0,
        }
    }

    fn set_mob_flag(&mut self, flag: i8, value: bool) {
        let flags = self.mob_flags();
        self.entity.data.set(ids::MOB_FLAGS, DataValue::Byte(if value { flags | flag } else { flags & !flag }));
    }

    pub fn is_no_ai(&self) -> bool {
        self.mob_flags() & MOB_FLAG_NO_AI != 0
    }

    pub fn set_no_ai(&mut self, no_ai: bool) {
        self.set_mob_flag(MOB_FLAG_NO_AI, no_ai);
    }

    pub fn is_left_handed(&self) -> bool {
        self.mob_flags() & MOB_FLAG_LEFTHANDED != 0
    }

    pub fn set_left_handed(&mut self, left_handed: bool) {
        self.set_mob_flag(MOB_FLAG_LEFTHANDED, left_handed);
    }

    pub fn is_aggressive(&self) -> bool {
        self.mob_flags() & MOB_FLAG_AGGRESSIVE != 0
    }

    pub fn set_aggressive(&mut self, aggressive: bool) {
        self.set_mob_flag(MOB_FLAG_AGGRESSIVE, aggressive);
    }

    pub fn set_drop_chance(&mut self, slot: EquipmentSlot, percent: f32) {
        self.drop_chances[slot as usize] = percent;
    }

    pub fn is_baby(&self) -> bool {
        match &self.kind {
            MobKind::Zombie(_) | MobKind::ZombieVillager(_) | MobKind::Husk(_) | MobKind::Drowned(_) => zombie::is_baby(&self.entity),
            _ => self.can_be_a_baby() && self.ageable().is_some_and(|a| a.age < 0),
        }
    }

    pub fn can_be_a_baby(&self) -> bool {
        match self.kind {
            MobKind::Frog(_) => frog::can_be_a_baby(),
            _ => !self.is_cube_mob(),
        }
    }

    pub fn cube_mob_size(&self) -> Option<i32> {
        self.is_cube_mob().then(|| self.get_size())
    }

    pub fn raider(&self) -> Option<&Raider> {
        match &self.kind {
            MobKind::Witch(witch) => Some(&witch.raider),
            MobKind::Ravager(ravager) => Some(&ravager.raider),
            MobKind::Vindicator(vindicator) => Some(&vindicator.raider),
            MobKind::Pillager(pillager) => Some(&pillager.raider),
            MobKind::Evoker(evoker) => Some(&evoker.spellcaster_illager.raider),
            MobKind::Illusioner(illusioner) => Some(&illusioner.spellcaster_illager.raider),
            _ => None,
        }
    }

    pub fn raider_mut(&mut self) -> Option<&mut Raider> {
        match &mut self.kind {
            MobKind::Witch(witch) => Some(&mut witch.raider),
            MobKind::Ravager(ravager) => Some(&mut ravager.raider),
            MobKind::Vindicator(vindicator) => Some(&mut vindicator.raider),
            MobKind::Pillager(pillager) => Some(&mut pillager.raider),
            MobKind::Evoker(evoker) => Some(&mut evoker.spellcaster_illager.raider),
            MobKind::Illusioner(illusioner) => Some(&mut illusioner.spellcaster_illager.raider),
            _ => None,
        }
    }

    pub fn patrolling_monster(&self) -> Option<&PatrollingMonster> {
        self.raider().map(|raider| &raider.patrolling_monster)
    }

    pub fn patrolling_monster_mut(&mut self) -> Option<&mut PatrollingMonster> {
        self.raider_mut().map(|raider| &mut raider.patrolling_monster)
    }

    pub fn can_be_leader(&self) -> bool {
        match self.kind {
            MobKind::Witch(_) => Witch::can_be_leader(),
            MobKind::Ravager(_) => Ravager::can_be_leader(),
            MobKind::Vindicator(_) | MobKind::Pillager(_) | MobKind::Evoker(_) | MobKind::Illusioner(_) => true,
            _ => true,
        }
    }

    pub fn sheep_sheared(&self) -> Option<bool> {
        match &self.kind {
            MobKind::Sheep(_) => Some(sheep::is_sheared(&self.entity)),
            _ => None,
        }
    }

    pub fn ageable(&self) -> Option<&AgeableMob> {
        if self.is_squid() {
            return Some(&squid::squid(self).ageable_water_creature.ageable);
        }
        if let Some(abstract_villager) = self.abstract_villager() {
            return Some(&abstract_villager.ageable);
        }
        match self.abstract_cube_mob() {
            Some(cube_mob) => Some(&cube_mob.ageable),
            None => self.animal().map(|animal| &animal.ageable),
        }
    }

    pub fn ageable_mut(&mut self) -> Option<&mut AgeableMob> {
        if self.is_squid() {
            return Some(&mut squid::squid_mut(self).ageable_water_creature.ageable);
        }
        if self.is_cube_mob() {
            return self.abstract_cube_mob_mut().map(|cube_mob| &mut cube_mob.ageable);
        }
        if self.abstract_villager().is_some() {
            return self.abstract_villager_mut().map(|abstract_villager| &mut abstract_villager.ageable);
        }
        self.animal_mut().map(|animal| &mut animal.ageable)
    }

    pub fn animal(&self) -> Option<&Animal> {
        match &self.kind {
            MobKind::Chicken(chicken) => Some(&chicken.animal),
            MobKind::Cow(cow) => Some(&cow.abstract_cow.animal),
            MobKind::MushroomCow(mushroom_cow) => Some(&mushroom_cow.abstract_cow.animal),
            MobKind::Pig(pig) => Some(&pig.animal),
            MobKind::Sheep(sheep) => Some(&sheep.animal),
            MobKind::Wolf(wolf) => Some(&wolf.tamable_animal.animal),
            MobKind::Goat(goat) => Some(&goat.animal),
            MobKind::PolarBear(polar_bear) => Some(&polar_bear.animal),
            MobKind::Horse(_) | MobKind::Donkey(_) | MobKind::Mule(_) | MobKind::SkeletonHorse(_) | MobKind::ZombieHorse(_) | MobKind::Llama(_) | MobKind::TraderLlama(_) => self.abstract_horse().map(|horse| &horse.animal),
            MobKind::Armadillo(armadillo) => Some(&armadillo.animal),
            MobKind::Axolotl(axolotl) => Some(&axolotl.animal),
            MobKind::Frog(frog) => Some(&frog.animal),
            MobKind::Tadpole(_) => None,
            MobKind::Zombie(_) | MobKind::ZombieVillager(_) | MobKind::Husk(_) | MobKind::Drowned(_) | MobKind::Creeper(_) | MobKind::Spider(_) | MobKind::CaveSpider(_) | MobKind::Skeleton(_) | MobKind::Stray(_) | MobKind::Parched(_) | MobKind::WitherSkeleton(_) | MobKind::Bogged(_) | MobKind::Silverfish(_) | MobKind::Endermite(_) | MobKind::Giant(_) => None,
            MobKind::Enderman(_) => None,
            MobKind::Blaze(_) | MobKind::Ghast(_) | MobKind::ZombifiedPiglin(_) | MobKind::Phantom(_) => None,
            MobKind::Witch(_) | MobKind::Vindicator(_) | MobKind::Pillager(_) | MobKind::Evoker(_) | MobKind::Illusioner(_) | MobKind::Vex(_) | MobKind::Ravager(_) | MobKind::Guardian(_) | MobKind::ElderGuardian(_) => None,
            MobKind::Slime(_) | MobKind::MagmaCube(_) => None,
            MobKind::Bat(_) | MobKind::Squid(_) | MobKind::GlowSquid(_) => None,
            MobKind::IronGolem(_) | MobKind::SnowGolem(_) => None,
            MobKind::Cod(_) | MobKind::Salmon(_) | MobKind::Pufferfish(_) | MobKind::TropicalFish(_) => None,
            MobKind::Cat(cat) => Some(&cat.tamable_animal.animal),
            MobKind::Ocelot(ocelot) => Some(&ocelot.animal),
            MobKind::Villager(_) | MobKind::WanderingTrader(_) => None,
            MobKind::Fox(fox) => Some(&fox.animal),
            MobKind::Rabbit(rabbit) => Some(&rabbit.animal),
            MobKind::Turtle(turtle) => Some(&turtle.animal),
        }
    }

    pub fn animal_mut(&mut self) -> Option<&mut Animal> {
        match &mut self.kind {
            MobKind::Chicken(chicken) => Some(&mut chicken.animal),
            MobKind::Cow(cow) => Some(&mut cow.abstract_cow.animal),
            MobKind::MushroomCow(mushroom_cow) => Some(&mut mushroom_cow.abstract_cow.animal),
            MobKind::Pig(pig) => Some(&mut pig.animal),
            MobKind::Sheep(sheep) => Some(&mut sheep.animal),
            MobKind::Wolf(wolf) => Some(&mut wolf.tamable_animal.animal),
            MobKind::Goat(goat) => Some(&mut goat.animal),
            MobKind::PolarBear(polar_bear) => Some(&mut polar_bear.animal),
            MobKind::Horse(horse) => Some(&mut horse.abstract_horse.animal),
            MobKind::Donkey(donkey) => Some(&mut donkey.abstract_chested_horse.abstract_horse.animal),
            MobKind::Mule(mule) => Some(&mut mule.abstract_chested_horse.abstract_horse.animal),
            MobKind::SkeletonHorse(horse) => Some(&mut horse.abstract_horse.animal),
            MobKind::ZombieHorse(horse) => Some(&mut horse.abstract_horse.animal),
            MobKind::Llama(llama) | MobKind::TraderLlama(llama) => Some(&mut llama.abstract_chested_horse.abstract_horse.animal),
            MobKind::Armadillo(armadillo) => Some(&mut armadillo.animal),
            MobKind::Axolotl(axolotl) => Some(&mut axolotl.animal),
            MobKind::Frog(frog) => Some(&mut frog.animal),
            MobKind::Tadpole(_) => None,
            MobKind::Zombie(_) | MobKind::ZombieVillager(_) | MobKind::Husk(_) | MobKind::Drowned(_) | MobKind::Creeper(_) | MobKind::Spider(_) | MobKind::CaveSpider(_) | MobKind::Skeleton(_) | MobKind::Stray(_) | MobKind::Parched(_) | MobKind::WitherSkeleton(_) | MobKind::Bogged(_) | MobKind::Silverfish(_) | MobKind::Endermite(_) | MobKind::Giant(_) => None,
            MobKind::Enderman(_) => None,
            MobKind::Blaze(_) | MobKind::Ghast(_) | MobKind::ZombifiedPiglin(_) | MobKind::Phantom(_) => None,
            MobKind::Witch(_) | MobKind::Vindicator(_) | MobKind::Pillager(_) | MobKind::Evoker(_) | MobKind::Illusioner(_) | MobKind::Vex(_) | MobKind::Ravager(_) | MobKind::Guardian(_) | MobKind::ElderGuardian(_) => None,
            MobKind::Slime(_) | MobKind::MagmaCube(_) => None,
            MobKind::Bat(_) | MobKind::Squid(_) | MobKind::GlowSquid(_) => None,
            MobKind::IronGolem(_) | MobKind::SnowGolem(_) => None,
            MobKind::Cod(_) | MobKind::Salmon(_) | MobKind::Pufferfish(_) | MobKind::TropicalFish(_) => None,
            MobKind::Cat(cat) => Some(&mut cat.tamable_animal.animal),
            MobKind::Ocelot(ocelot) => Some(&mut ocelot.animal),
            MobKind::Villager(_) | MobKind::WanderingTrader(_) => None,
            MobKind::Fox(fox) => Some(&mut fox.animal),
            MobKind::Rabbit(rabbit) => Some(&mut rabbit.animal),
            MobKind::Turtle(turtle) => Some(&mut turtle.animal),
        }
    }

    pub fn is_age_locked(&self) -> bool {
        matches!(self.entity.data.get(ids::AGE_LOCKED), DataValue::Boolean(true))
    }

    pub fn set_age(&mut self, new_age: i32) -> bool {
        let Some(ageable) = self.ageable_mut() else { return false };
        let old_age = ageable.age;
        ageable.age = new_age;
        if old_age < 0 && new_age >= 0 || old_age >= 0 && new_age < 0 {
            self.entity.data.set(ids::AGEABLE_BABY, DataValue::Boolean(new_age < 0));
            self.age_boundary_reached();
            return true;
        }
        false
    }

    pub fn set_baby(&mut self, baby: bool) {
        if self.is_zombie() {
            Zombie::set_baby(self, baby);
        } else if self.ageable().is_some() && self.can_be_a_baby() && self.set_age(if baby { BABY_START_AGE } else { 0 }) {
            self.refresh_dimensions();
        }
    }

    pub fn apply_components_from_item_stack(&mut self, stack: &ItemStack) {
        self.apply_implicit_components(stack);
    }

    fn apply_implicit_components(&mut self, components: &impl DataComponentGetter) {
        match self.kind {
            MobKind::Chicken(_) => {
                self.apply_implicit_component_if_present(components, dc::CHICKEN_VARIANT);
                self.apply_implicit_component_if_present(components, dc::CHICKEN_SOUND_VARIANT);
            }
            MobKind::Cow(_) => {
                self.apply_implicit_component_if_present(components, dc::COW_VARIANT);
                self.apply_implicit_component_if_present(components, dc::COW_SOUND_VARIANT);
            }
            MobKind::MushroomCow(_) => {
                self.apply_implicit_component_if_present(components, dc::MOOSHROOM_VARIANT);
            }
            MobKind::Pig(_) => {
                self.apply_implicit_component_if_present(components, dc::PIG_VARIANT);
                self.apply_implicit_component_if_present(components, dc::PIG_SOUND_VARIANT);
            }
            MobKind::Sheep(_) => {
                self.apply_implicit_component_if_present(components, dc::SHEEP_COLOR);
            }
            MobKind::Wolf(_) => {
                self.apply_implicit_component_if_present(components, dc::WOLF_VARIANT);
                self.apply_implicit_component_if_present(components, dc::WOLF_SOUND_VARIANT);
                self.apply_implicit_component_if_present(components, dc::WOLF_COLLAR);
            }
            MobKind::Goat(_) | MobKind::PolarBear(_) => {}
            MobKind::Armadillo(_) => {}
            MobKind::Zombie(_) | MobKind::Husk(_) | MobKind::Drowned(_) | MobKind::Creeper(_) | MobKind::Spider(_) | MobKind::CaveSpider(_) | MobKind::Skeleton(_) | MobKind::Stray(_) | MobKind::Parched(_) | MobKind::WitherSkeleton(_) | MobKind::Bogged(_) | MobKind::Silverfish(_) | MobKind::Endermite(_) | MobKind::Giant(_) => {}
            MobKind::Enderman(_) => {}
            MobKind::Blaze(_) | MobKind::Ghast(_) | MobKind::ZombifiedPiglin(_) | MobKind::Phantom(_) => {}
            MobKind::Witch(_) | MobKind::Vindicator(_) | MobKind::Pillager(_) | MobKind::Evoker(_) | MobKind::Illusioner(_) | MobKind::Vex(_) | MobKind::Ravager(_) | MobKind::Guardian(_) | MobKind::ElderGuardian(_) => {}
            MobKind::Slime(_) | MobKind::MagmaCube(_) => {}
            MobKind::Bat(_) | MobKind::Squid(_) | MobKind::GlowSquid(_) => {}
            MobKind::IronGolem(_) | MobKind::SnowGolem(_) => {}
            MobKind::Salmon(_) => {
                self.apply_implicit_component_if_present(components, dc::SALMON_SIZE);
            }
            MobKind::TropicalFish(_) => {
                self.apply_implicit_component_if_present(components, dc::TROPICAL_FISH_PATTERN);
                self.apply_implicit_component_if_present(components, dc::TROPICAL_FISH_BASE_COLOR);
                self.apply_implicit_component_if_present(components, dc::TROPICAL_FISH_PATTERN_COLOR);
            }
            MobKind::Cod(_) | MobKind::Pufferfish(_) => {}
            MobKind::Cat(_) => {
                self.apply_implicit_component_if_present(components, dc::CAT_VARIANT);
                self.apply_implicit_component_if_present(components, dc::CAT_SOUND_VARIANT);
                self.apply_implicit_component_if_present(components, dc::CAT_COLLAR);
            }
            MobKind::Ocelot(_) => {}
            MobKind::Horse(_) => {
                self.apply_implicit_component_if_present(components, dc::HORSE_VARIANT);
            }
            MobKind::Donkey(_) | MobKind::Mule(_) | MobKind::SkeletonHorse(_) | MobKind::ZombieHorse(_) => {}
            MobKind::Llama(_) | MobKind::TraderLlama(_) => {
                self.apply_implicit_component_if_present(components, dc::LLAMA_VARIANT);
            }
            MobKind::WanderingTrader(_) => {}
            MobKind::Axolotl(_) => {
                self.apply_implicit_component_if_present(components, dc::AXOLOTL_VARIANT);
            }
            MobKind::Frog(_) => {
                self.apply_implicit_component_if_present(components, dc::FROG_VARIANT);
            }
            MobKind::Tadpole(_) => {}
            MobKind::Villager(_) | MobKind::ZombieVillager(_) => {
                self.apply_implicit_component_if_present(components, dc::VILLAGER_VARIANT);
            }
            MobKind::Fox(_) => {
                self.apply_implicit_component_if_present(components, dc::FOX_VARIANT);
            }
            MobKind::Rabbit(_) => {
                self.apply_implicit_component_if_present(components, dc::RABBIT_VARIANT);
            }
            MobKind::Turtle(_) => {}
        }
        self.apply_implicit_component_if_present(components, dc::CUSTOM_NAME);
        self.apply_implicit_component_if_present(components, dc::CUSTOM_DATA);
    }

    pub fn apply_implicit_component(&mut self, ty: DataComponentType, value: &Nbt) -> bool {
        if ty == dc::CUSTOM_NAME {
            self.set_custom_name(Some(value.clone()));
            return true;
        }
        match self.kind {
            MobKind::Chicken(_) if ty == dc::CHICKEN_VARIANT => {
                if let Some(variant) = value.as_str().and_then(chicken::variant_id) {
                    self.entity.data.set(chicken::DATA_VARIANT, DataValue::ChickenVariant(variant));
                }
                return true;
            }
            MobKind::Chicken(_) if ty == dc::CHICKEN_SOUND_VARIANT => {
                if let Some(sound_variant) = value.as_str().and_then(chicken::sound_variant_id) {
                    self.entity.data.set(chicken::DATA_SOUND_VARIANT, DataValue::ChickenSoundVariant(sound_variant));
                }
                return true;
            }
            MobKind::Cow(_) if ty == dc::COW_VARIANT => {
                if let Some(variant) = value.as_str().and_then(|name| variant_utils::read_variant(cow::VARIANT_REGISTRY, name)) {
                    cow::set_variant(&mut self.entity, variant);
                }
                return true;
            }
            MobKind::Cow(_) if ty == dc::COW_SOUND_VARIANT => {
                if let Some(sound_variant) = value.as_str().and_then(|name| variant_utils::read_variant(cow::SOUND_VARIANT_REGISTRY, name)) {
                    cow::set_sound_variant(&mut self.entity, sound_variant);
                }
                return true;
            }
            MobKind::MushroomCow(_) if ty == dc::MOOSHROOM_VARIANT => {
                if let Some(variant) = value.as_str().and_then(mushroom_cow::Variant::by_name) {
                    mushroom_cow::set_variant(&mut self.entity, variant);
                }
                return true;
            }
            MobKind::Pig(_) if ty == dc::PIG_VARIANT => {
                if let Some(variant) = value.as_str().and_then(|name| variant_utils::read_variant(pig::VARIANT_REGISTRY, name)) {
                    pig::set_variant(&mut self.entity, variant);
                }
                return true;
            }
            MobKind::Pig(_) if ty == dc::PIG_SOUND_VARIANT => {
                if let Some(sound_variant) = value.as_str().and_then(|name| variant_utils::read_variant(pig::SOUND_VARIANT_REGISTRY, name)) {
                    pig::set_sound_variant(&mut self.entity, sound_variant);
                }
                return true;
            }
            MobKind::Wolf(_) if ty == dc::WOLF_VARIANT => {
                if let Some(variant) = value.as_str().and_then(|name| variant_utils::read_variant(wolf::VARIANT_REGISTRY, name)) {
                    wolf::set_variant(&mut self.entity, variant);
                }
                return true;
            }
            MobKind::Wolf(_) if ty == dc::WOLF_SOUND_VARIANT => {
                if let Some(sound_variant) = value.as_str().and_then(|name| variant_utils::read_variant(super::animal::wolf::wolf_sound_variant::REGISTRY, name)) {
                    wolf::set_sound_variant(&mut self.entity, sound_variant);
                }
                return true;
            }
            MobKind::Wolf(_) if ty == dc::WOLF_COLLAR => {
                if let Some(color) = value.as_str().and_then(DyeColor::by_name) {
                    wolf::set_collar_color(&mut self.entity, color);
                }
                return true;
            }
            MobKind::Cat(_) if ty == dc::CAT_VARIANT => {
                if let Some(variant) = value.as_str().and_then(|name| variant_utils::read_variant(cat::VARIANT_REGISTRY, name)) {
                    cat::set_variant(&mut self.entity, variant);
                }
                return true;
            }
            MobKind::Cat(_) if ty == dc::CAT_SOUND_VARIANT => {
                if let Some(sound_variant) = value.as_str().and_then(|name| variant_utils::read_variant(super::animal::feline::cat_sound_variant::REGISTRY, name)) {
                    cat::set_sound_variant(&mut self.entity, sound_variant);
                }
                return true;
            }
            MobKind::Cat(_) if ty == dc::CAT_COLLAR => {
                if let Some(color) = value.as_str().and_then(DyeColor::by_name) {
                    cat::set_collar_color(&mut self.entity, color);
                }
                return true;
            }
            MobKind::Llama(_) | MobKind::TraderLlama(_) if ty == dc::LLAMA_VARIANT => {
                if let Some(variant) = value.as_str().and_then(llama::Variant::by_name) {
                    llama::set_variant(&mut self.entity, variant);
                }
                return true;
            }
            MobKind::Horse(_) if ty == dc::HORSE_VARIANT => {
                if let Some(variant) = value.as_str().and_then(super::animal::equine::variant::Variant::by_name) {
                    horse::set_variant(&mut self.entity, variant);
                }
                return true;
            }
            MobKind::Fox(_) if ty == dc::FOX_VARIANT => {
                if let Some(variant) = value.as_str().and_then(fox::Variant::by_name) {
                    fox::set_variant(&mut self.entity, variant);
                }
                return true;
            }
            MobKind::Sheep(_) if ty == dc::SHEEP_COLOR => {
                if let Some(color) = value.as_str().and_then(DyeColor::by_name) {
                    sheep::set_color(&mut self.entity, color);
                }
                return true;
            }
            MobKind::Salmon(_) if ty == dc::SALMON_SIZE => {
                if let Some(variant) = value.as_str().and_then(salmon::Variant::by_name) {
                    self.set_salmon_variant(variant);
                }
                return true;
            }
            MobKind::TropicalFish(_) if ty == dc::TROPICAL_FISH_PATTERN => {
                if let Some(pattern) = value.as_str().and_then(tropical_fish::Pattern::by_name) {
                    self.set_tropical_fish_pattern(pattern);
                }
                return true;
            }
            MobKind::TropicalFish(_) if ty == dc::TROPICAL_FISH_BASE_COLOR => {
                if let Some(color) = value.as_str().and_then(DyeColor::by_name) {
                    self.set_tropical_fish_base_color(color);
                }
                return true;
            }
            MobKind::Rabbit(_) if ty == dc::RABBIT_VARIANT => {
                if let Some(variant) = value.as_str().and_then(rabbit::Variant::by_name) {
                    rabbit::set_variant(self, variant);
                }
                return true;
            }
            MobKind::TropicalFish(_) if ty == dc::TROPICAL_FISH_PATTERN_COLOR => {
                if let Some(color) = value.as_str().and_then(DyeColor::by_name) {
                    self.set_tropical_fish_pattern_color(color);
                }
                return true;
            }
            MobKind::Axolotl(_) if ty == dc::AXOLOTL_VARIANT => {
                if let Some(variant) = value.as_str().and_then(axolotl::Variant::by_name) {
                    axolotl::set_variant(&mut self.entity, variant);
                }
                return true;
            }
            MobKind::Frog(_) if ty == dc::FROG_VARIANT => {
                if let Some(variant) = value.as_str().and_then(|name| variant_utils::read_variant(frog::VARIANT_REGISTRY, name)) {
                    frog::set_variant(&mut self.entity, variant);
                }
                return true;
            }
            MobKind::Villager(_) if ty == dc::VILLAGER_VARIANT => {
                villager::apply_villager_variant(self, value);
                return true;
            }
            MobKind::ZombieVillager(_) if ty == dc::VILLAGER_VARIANT => {
                zombie_villager::apply_villager_variant(self, value);
                return true;
            }
            _ => {}
        }
        self.entity.apply_implicit_component(ty, value)
    }

    fn apply_implicit_component_if_present(&mut self, components: &impl DataComponentGetter, ty: DataComponentType) -> bool {
        match components.get(ty) {
            Some(value) => self.apply_implicit_component(ty, &value),
            None => false,
        }
    }

    pub fn default_dimensions(&self) -> EntityDimensions {
        match &self.kind {
            MobKind::Chicken(_) if self.is_baby() => chicken::BABY_DIMENSIONS,
            MobKind::Chicken(_) => chicken::DIMENSIONS,
            MobKind::Cow(_) if self.is_baby() => cow::BABY_DIMENSIONS,
            MobKind::MushroomCow(_) if self.is_baby() => mushroom_cow::BABY_DIMENSIONS,
            MobKind::Cow(_) | MobKind::MushroomCow(_) => abstract_cow::DIMENSIONS,
            MobKind::Pig(_) if self.is_baby() => pig::BABY_DIMENSIONS,
            MobKind::Pig(_) => pig::DIMENSIONS,
            MobKind::Sheep(_) if self.is_baby() => sheep::BABY_DIMENSIONS,
            MobKind::Sheep(_) => sheep::DIMENSIONS,
            MobKind::Goat(_) => goat::get_default_dimensions(self.is_baby(), self.entity.pose()),
            MobKind::PolarBear(_) if self.is_baby() => polar_bear::BABY_DIMENSIONS,
            MobKind::PolarBear(_) => polar_bear::DIMENSIONS,
            MobKind::Armadillo(_) => armadillo::get_default_dimensions(self.is_baby()),
            MobKind::Zombie(_) if self.is_baby() => zombie::BABY_DIMENSIONS,
            MobKind::Zombie(_) => zombie::DIMENSIONS,
            MobKind::ZombieVillager(_) => zombie_villager::get_default_dimensions(self),
            MobKind::Husk(_) if self.is_baby() => husk::BABY_DIMENSIONS,
            MobKind::Husk(_) => husk::DIMENSIONS,
            MobKind::Drowned(_) if self.is_baby() => drowned::BABY_DIMENSIONS,
            MobKind::Drowned(_) => zombie::DIMENSIONS,
            MobKind::Creeper(_) => creeper::DIMENSIONS,
            MobKind::Spider(_) => spider::DIMENSIONS,
            MobKind::CaveSpider(_) => cave_spider::DIMENSIONS,
            MobKind::Skeleton(_) | MobKind::Stray(_) | MobKind::Parched(_) | MobKind::Bogged(_) => abstract_skeleton::DIMENSIONS,
            MobKind::WitherSkeleton(_) => wither_skeleton::DIMENSIONS,
            MobKind::Silverfish(_) => silverfish::DIMENSIONS,
            MobKind::Endermite(_) => endermite::DIMENSIONS,
            MobKind::Giant(_) => giant::DIMENSIONS,
            MobKind::Enderman(_) => enderman::DIMENSIONS,
            MobKind::Blaze(_) => blaze::DIMENSIONS,
            MobKind::Ghast(_) => ghast::DIMENSIONS,
            MobKind::Phantom(_) => crate::world::entity::monster::phantom::Phantom::get_default_dimensions(self),
            MobKind::ZombifiedPiglin(_) => ZombifiedPiglin::get_default_dimensions(self),
            MobKind::Witch(_) => witch::DIMENSIONS,
            MobKind::Ravager(_) => ravager::DIMENSIONS,
            MobKind::Vindicator(_) => vindicator::DIMENSIONS,
            MobKind::Pillager(_) => pillager::DIMENSIONS,
            MobKind::Evoker(_) => evoker::DIMENSIONS,
            MobKind::Illusioner(_) => illusioner::DIMENSIONS,
            MobKind::Vex(_) => vex::DIMENSIONS,
            MobKind::Guardian(_) => crate::world::entity::monster::guardian::DIMENSIONS,
            MobKind::ElderGuardian(_) => crate::world::entity::monster::elder_guardian::DIMENSIONS,
            MobKind::Slime(_) => slime::DIMENSIONS.scale(self.get_size() as f32),
            MobKind::MagmaCube(_) => magma_cube::DIMENSIONS.scale(self.get_size() as f32),
            MobKind::Wolf(_) if self.is_baby() => wolf::BABY_DIMENSIONS,
            MobKind::Wolf(_) => wolf::DIMENSIONS,
            MobKind::Bat(_) => bat::DIMENSIONS,
            MobKind::Squid(_) | MobKind::GlowSquid(_) if self.is_baby() => squid::BABY_DIMENSIONS,
            MobKind::Squid(_) | MobKind::GlowSquid(_) => squid::DIMENSIONS,
            MobKind::IronGolem(_) => iron_golem::DIMENSIONS,
            MobKind::SnowGolem(_) => snow_golem::DIMENSIONS,
            MobKind::Cod(_) => cod::DIMENSIONS,
            MobKind::Salmon(_) => salmon::DIMENSIONS.scale(self.get_salmon_scale()),
            MobKind::Pufferfish(_) => Pufferfish::get_default_dimensions(self),
            MobKind::TropicalFish(_) => tropical_fish::DIMENSIONS,
            MobKind::Cat(_) if self.is_baby() => cat::BABY_DIMENSIONS,
            MobKind::Cat(_) => cat::DIMENSIONS,
            MobKind::Ocelot(_) if self.is_baby() => ocelot::BABY_DIMENSIONS,
            MobKind::Ocelot(_) => ocelot::DIMENSIONS,
            MobKind::Horse(_) if self.is_baby() => horse::baby_dimensions(),
            MobKind::Horse(_) => horse::DIMENSIONS,
            MobKind::Llama(_) | MobKind::TraderLlama(_) if self.is_baby() => llama::baby_dimensions(),
            MobKind::Llama(_) | MobKind::TraderLlama(_) => llama::DIMENSIONS,
            MobKind::WanderingTrader(_) => super::npc::wanderingtrader::wandering_trader::DIMENSIONS,
            MobKind::SkeletonHorse(_) if self.is_baby() => skeleton_horse::baby_dimensions(),
            MobKind::SkeletonHorse(_) => skeleton_horse::DIMENSIONS,
            MobKind::ZombieHorse(_) if self.is_baby() => zombie_horse::baby_dimensions(),
            MobKind::ZombieHorse(_) => zombie_horse::DIMENSIONS,
            MobKind::Donkey(_) if self.is_baby() => donkey::baby_dimensions(),
            MobKind::Donkey(_) => donkey::DIMENSIONS,
            MobKind::Mule(_) if self.is_baby() => mule::baby_dimensions(),
            MobKind::Mule(_) => mule::DIMENSIONS,
            MobKind::Axolotl(_) => Axolotl::get_default_dimensions(self),
            MobKind::Frog(_) => frog::DIMENSIONS,
            MobKind::Tadpole(_) => tadpole::DIMENSIONS,
            MobKind::Villager(_) => Villager::get_default_dimensions(self),
            MobKind::Fox(_) if self.is_baby() => fox::BABY_DIMENSIONS,
            MobKind::Fox(_) => fox::DIMENSIONS,
            MobKind::Rabbit(_) if self.is_baby() => rabbit::BABY_DIMENSIONS,
            MobKind::Rabbit(_) => rabbit::DIMENSIONS,
            MobKind::Turtle(_) if self.is_baby() => turtle::baby_dimensions(),
            MobKind::Turtle(_) => turtle::DIMENSIONS,
        }
    }

    pub fn passenger_attachment(&self) -> wafer_core::Vec3 {
        let baby = self.is_baby();
        let (x, y, z) = match &self.kind {
            MobKind::Chicken(_) if baby => (0.0, 0.375f32 as f64, 0.0),
            MobKind::Chicken(_) => (0.0, 0.7, -0.1),
            MobKind::Cow(_) | MobKind::MushroomCow(_) if baby => (0.0, 0.75f32 as f64, 0.0),
            MobKind::Cow(_) | MobKind::MushroomCow(_) => (0.0, 1.36875f32 as f64, 0.0),
            MobKind::Pig(_) if baby => (0.0, 0.5f32 as f64, 0.0),
            MobKind::Pig(_) => (0.0, 0.86875f32 as f64, 0.0),
            MobKind::Sheep(_) if baby => (0.0, 0.5625f32 as f64, 0.0),
            MobKind::Sheep(_) => (0.0, 1.2375f32 as f64, 0.0),
            MobKind::PolarBear(_) if baby => (0.0, 0.625, 0.0),
            MobKind::PolarBear(_) => (0.0, 1.4f32 as f64, 0.0),
            MobKind::Goat(_) if baby => (0.0, 0.53125f32 as f64, 0.0),
            MobKind::Goat(_) => (0.0, 1.1125f32 as f64, 0.0),
            MobKind::Armadillo(_) => (0.0, self.default_dimensions().height as f64, 0.0),
            MobKind::Zombie(_) | MobKind::ZombieVillager(_) | MobKind::Husk(_) | MobKind::Drowned(_) if baby => (0.0, 0.98f32 as f64, 0.0),
            MobKind::Zombie(_) | MobKind::Drowned(_) => (0.0, 2.0125f32 as f64, 0.0),
            MobKind::Husk(_) => (0.0, 2.075f32 as f64, 0.0),
            MobKind::ZombieVillager(_) => (0.0, 2.125f32 as f64, 0.0),
            MobKind::Creeper(_) => (0.0, 1.7f32 as f64, 0.0),
            MobKind::Spider(_) => (0.0, 0.765f32 as f64, 0.0),
            MobKind::CaveSpider(_) => (0.0, 0.5f32 as f64, 0.0),
            MobKind::Skeleton(_) | MobKind::Stray(_) | MobKind::Parched(_) | MobKind::Bogged(_) => (0.0, 1.99f32 as f64, 0.0),
            MobKind::WitherSkeleton(_) => (0.0, 2.4f32 as f64, 0.0),
            MobKind::Silverfish(_) | MobKind::Endermite(_) => (0.0, 0.2375f32 as f64, 0.0),
            MobKind::Giant(_) => (0.0, 12.0, 0.0),
            MobKind::Enderman(_) => (0.0, 2.80625f32 as f64, 0.0),
            MobKind::Blaze(_) => (0.0, 1.8f32 as f64, 0.0),
            MobKind::Ghast(_) => (0.0, 4.0625f32 as f64, 0.0),
            MobKind::Phantom(_) => {
                let point = self.default_dimensions().attachments.get(super::entity_attachments::EntityAttachment::Passenger, 0, 0.0);
                (point.x, point.y, point.z)
            }
            MobKind::ZombifiedPiglin(_) if baby => (0.0, 0.98f32 as f64, 0.0),
            MobKind::ZombifiedPiglin(_) => (0.0, 2.0f32 as f64, 0.0),
            MobKind::Witch(_) => (0.0, 2.2625f32 as f64, 0.0),
            MobKind::Ravager(_) => (0.0, 2.2625f32 as f64, -0.0625f32 as f64),
            MobKind::Vindicator(_) | MobKind::Pillager(_) | MobKind::Evoker(_) | MobKind::Illusioner(_) => (0.0, 2.0, 0.0),
            MobKind::Vex(_) => (0.0, 0.7375f32 as f64, 0.0),
            MobKind::Guardian(_) => (0.0, 0.975f32 as f64, 0.0),
            MobKind::ElderGuardian(_) => (0.0, 2.350625f32 as f64, 0.0),
            MobKind::Slime(_) | MobKind::MagmaCube(_) => return self.abstract_cube_mob_get_passenger_attachment_point(),
            MobKind::Wolf(_) if baby => (0.0, 0.4375f32 as f64, 0.0),
            MobKind::Wolf(_) => (0.0, 0.81875, -0.0625),
            MobKind::Bat(_) => (0.0, 0.9f32 as f64, 0.0),
            MobKind::Squid(_) | MobKind::GlowSquid(_) if baby => (0.0, 0.5f32 as f64, 0.0),
            MobKind::Squid(_) | MobKind::GlowSquid(_) => (0.0, 0.8f32 as f64, 0.0),
            MobKind::IronGolem(_) => (0.0, 2.7f32 as f64, 0.0),
            MobKind::SnowGolem(_) => (0.0, 1.9f32 as f64, 0.0),
            MobKind::Cod(_) | MobKind::Salmon(_) | MobKind::Pufferfish(_) | MobKind::TropicalFish(_) => (0.0, self.default_dimensions().height as f64, 0.0),
            MobKind::Cat(_) | MobKind::Ocelot(_) if baby => (0.0, 0.3125f32 as f64, 0.0),
            MobKind::Cat(_) => (0.0, 0.5125f32 as f64, 0.0),
            MobKind::Ocelot(_) => (0.0, 0.6375f32 as f64, 0.0),
            MobKind::Fox(_) if baby => (0.0, 0.375f32 as f64, 0.0),
            MobKind::Fox(_) => (0.0, 0.6375, -0.25),
            MobKind::Horse(_) | MobKind::Donkey(_) | MobKind::Mule(_) | MobKind::SkeletonHorse(_) | MobKind::ZombieHorse(_) | MobKind::Llama(_) | MobKind::TraderLlama(_) | MobKind::Rabbit(_) | MobKind::Turtle(_) => {
                let point = self.default_dimensions().attachments.get(super::entity_attachments::EntityAttachment::Passenger, 0, 0.0);
                (point.x, point.y, point.z)
            }
            MobKind::Axolotl(_) if baby => (0.0, 0.1875f32 as f64, 0.0),
            MobKind::Axolotl(_) | MobKind::Tadpole(_) => (0.0, self.default_dimensions().height as f64, 0.0),
            MobKind::Frog(_) => (0.0, 0.375, -0.25),
            MobKind::Villager(_) | MobKind::WanderingTrader(_) => (0.0, self.default_dimensions().height as f64, 0.0),
        };
        let scale = self.living.attributes.value(Attribute::Scale) as f32 as f64;
        wafer_core::Vec3::new(x * scale, y * scale, z * scale)
    }

    pub fn dimensions(&self) -> EntityDimensions {
        let scale = self.living.attributes.value(Attribute::Scale) as f32;
        self.default_dimensions().scale(scale)
    }

    pub fn loot_table_id(&self) -> Option<String> {
        if self.loot_table.is_some() {
            return self.loot_table.clone();
        }
        let id = wafer_data::registry::ENTITY_TYPE[self.entity.entity_type as usize];
        Some(format!("minecraft:entities/{}", id.trim_start_matches("minecraft:")))
    }

    pub fn save(&self, out: &mut Vec<(String, Nbt)>) {
        let living = &self.living;
        out.push(("Health".into(), Nbt::Float(self.health)));
        out.push(("HurtTime".into(), Nbt::Short(living.hurt_time as i16)));
        out.push(("DeathTime".into(), Nbt::Short(living.death_time as i16)));
        out.push(("AbsorptionAmount".into(), Nbt::Float(self.absorption)));
        out.push(("current_impulse_context_reset_grace_time".into(), Nbt::Int(living.current_impulse_context_reset_grace_time)));
        if let Some(pos) = living.current_impulse_impact_pos {
            out.push(("current_explosion_impact_pos".into(), Nbt::List(vec![Nbt::Double(pos.x), Nbt::Double(pos.y), Nbt::Double(pos.z)])));
        }
        out.push(("attributes".into(), living.attributes.to_nbt()));
        if !living.active_effects.is_empty() {
            out.push(("active_effects".into(), Nbt::List(living.active_effects.iter().map(|e| e.to_nbt()).collect())));
        }
        out.push(("FallFlying".into(), Nbt::Byte(self.entity.is_fall_flying() as i8)));
        if let DataValue::OptionalBlockPos(Some(pos)) = self.entity.data.get(ids::SLEEPING_POS) {
            out.push(("sleeping_pos".into(), Nbt::IntArray(vec![pos.x, pos.y, pos.z])));
        }
        out.push(("Brain".into(), self.living.brain.pack().to_nbt()));
        if let Some(player) = living.last_hurt_by_player {
            out.push(("last_hurt_by_player".into(), uuid_ints(player)));
            out.push(("last_hurt_by_player_memory_time".into(), Nbt::Int(living.last_hurt_by_player_memory_time)));
        }
        if let Some(mob) = living.last_hurt_by_mob {
            out.push(("last_hurt_by_mob".into(), uuid_ints(mob)));
            out.push(("ticks_since_last_hurt_by_mob".into(), Nbt::Int(self.entity.tick_count - living.last_hurt_by_mob_timestamp)));
        }
        if let Some(equipment) = self.save_equipment() {
            out.push(("equipment".into(), equipment));
        }
        if living.locator_bar_icon.has_data() {
            out.push(("locator_bar_icon".into(), living.locator_bar_icon.to_nbt()));
        }
        out.push(("CanPickUpLoot".into(), Nbt::Byte(self.can_pick_up_loot as i8)));
        out.push(("PersistenceRequired".into(), Nbt::Byte(self.persistence_required as i8)));
        let drop_chances: Vec<(String, Nbt)> = EquipmentSlot::VALUES
            .into_iter()
            .filter(|&slot| self.drop_chances[slot as usize] != DEFAULT_EQUIPMENT_DROP_CHANCE)
            .map(|slot| (slot_name(slot).to_string(), Nbt::Float(self.drop_chances[slot as usize])))
            .collect();
        if !drop_chances.is_empty() {
            out.push(("drop_chances".into(), Nbt::Compound(drop_chances)));
        }
        super::leashable::LeashData::write(self.leash_data.as_ref(), out);
        if self.home_radius > -1 {
            out.push(("home_radius".into(), Nbt::Int(self.home_radius)));
            out.push(("home_pos".into(), Nbt::IntArray(vec![self.home_position.x, self.home_position.y, self.home_position.z])));
        }
        out.push(("LeftHanded".into(), Nbt::Byte(self.is_left_handed() as i8)));
        if let Some(table) = &self.loot_table {
            out.push(("DeathLootTable".into(), Nbt::String(table.clone())));
        }
        if self.loot_table_seed != 0 {
            out.push(("DeathLootTableSeed".into(), Nbt::Long(self.loot_table_seed)));
        }
        if self.is_no_ai() {
            out.push(("NoAI".into(), Nbt::Byte(1)));
        }
        if let Some(ageable) = self.ageable().filter(|_| self.can_be_a_baby()) {
            out.push(("Age".into(), Nbt::Int(ageable.age)));
            out.push(("ForcedAge".into(), Nbt::Int(ageable.forced_age)));
            out.push(("AgeLocked".into(), Nbt::Byte(self.is_age_locked() as i8)));
        }
        if let Some(animal) = self.animal() {
            out.push(("InLove".into(), Nbt::Int(animal.in_love)));
            if let Some(cause) = animal.love_cause {
                out.push(("LoveCause".into(), uuid_ints(cause)));
            }
        }
        match &self.kind {
            MobKind::Chicken(chicken) => {
                out.push(("IsChickenJockey".into(), Nbt::Byte(chicken.is_chicken_jockey as i8)));
                out.push(("EggLayTime".into(), Nbt::Int(chicken.egg_time)));
                if let DataValue::ChickenVariant(variant) = self.entity.data.get(chicken::DATA_VARIANT) {
                    out.push(("variant".into(), Nbt::String(chicken::variant_name(*variant).into())));
                }
                if let DataValue::ChickenSoundVariant(variant) = self.entity.data.get(chicken::DATA_SOUND_VARIANT) {
                    out.push(("sound_variant".into(), Nbt::String(chicken::sound_variant_name(*variant).into())));
                }
            }
            MobKind::Cow(_) => {
                out.push(("variant".into(), Nbt::String(variant_utils::variant_name(cow::VARIANT_REGISTRY, cow::get_variant(&self.entity)).into())));
                out.push(("sound_variant".into(), Nbt::String(variant_utils::variant_name(cow::SOUND_VARIANT_REGISTRY, cow::get_sound_variant(&self.entity)).into())));
            }
            MobKind::MushroomCow(mushroom_cow) => {
                out.push(("Type".into(), Nbt::String(mushroom_cow::get_variant(&self.entity).get_serialized_name().into())));
                if let Some(stew_effects) = &mushroom_cow.stew_effects {
                    out.push(("stew_effects".into(), stew_effects.clone()));
                }
            }
            MobKind::Pig(_) => {
                out.push(("variant".into(), Nbt::String(variant_utils::variant_name(pig::VARIANT_REGISTRY, pig::get_variant(&self.entity)).into())));
                out.push(("sound_variant".into(), Nbt::String(variant_utils::variant_name(pig::SOUND_VARIANT_REGISTRY, pig::get_sound_variant(&self.entity)).into())));
            }
            MobKind::Sheep(_) => {
                out.push(("Sheared".into(), Nbt::Byte(sheep::is_sheared(&self.entity) as i8)));
                out.push(("Color".into(), Nbt::Byte(sheep::get_color(&self.entity).id as i8)));
            }
            MobKind::Goat(_) => Goat::add_additional_save_data(self, out),
            MobKind::PolarBear(_) => PolarBear::add_additional_save_data(self, out),
            MobKind::Armadillo(_) => Armadillo::add_additional_save_data(self, out),
            MobKind::ZombieVillager(_) => ZombieVillager::add_additional_save_data(self, out),
            MobKind::Zombie(_) | MobKind::Husk(_) | MobKind::Drowned(_) => {
                let is_afflicted = self.entity.is_eye_in_fluid(&wafer_data::fluid_tags::WATER);
                Zombie::add_additional_save_data(self, is_afflicted, out);
            }
            MobKind::Creeper(_) => Creeper::add_additional_save_data(self, out),
            MobKind::Spider(_) | MobKind::CaveSpider(_) => {}
            MobKind::Skeleton(_) => Skeleton::add_additional_save_data(self, out),
            MobKind::Stray(_) | MobKind::Parched(_) | MobKind::WitherSkeleton(_) => {}
            MobKind::Bogged(_) => Bogged::add_additional_save_data(self, out),
            MobKind::Silverfish(_) => {}
            MobKind::Endermite(_) => Endermite::add_additional_save_data(self, out),
            MobKind::Giant(_) => {}
            MobKind::Enderman(_) => Enderman::add_additional_save_data(self, out),
            MobKind::Blaze(_) => {}
            MobKind::Ghast(_) => Ghast::add_additional_save_data(self, out),
            MobKind::Phantom(_) => crate::world::entity::monster::phantom::Phantom::add_additional_save_data(self, out),
            MobKind::ZombifiedPiglin(_) => {
                let is_afflicted = self.entity.is_eye_in_fluid(&wafer_data::fluid_tags::WATER);
                Zombie::add_additional_save_data(self, is_afflicted, out);
                ZombifiedPiglin::add_additional_save_data(self, out);
            }
            MobKind::Witch(witch) => witch.raider.add_additional_save_data(out),
            MobKind::Ravager(_) => Ravager::add_additional_save_data(self, out),
            MobKind::Vindicator(_) => Vindicator::add_additional_save_data(self, out),
            MobKind::Pillager(_) => Pillager::add_additional_save_data(self, out),
            MobKind::Evoker(evoker) => evoker.spellcaster_illager.add_additional_save_data(out),
            MobKind::Illusioner(illusioner) => illusioner.spellcaster_illager.add_additional_save_data(out),
            MobKind::Vex(_) => Vex::add_additional_save_data(self, out),
            MobKind::Guardian(_) | MobKind::ElderGuardian(_) => {}
            MobKind::Slime(_) | MobKind::MagmaCube(_) => self.abstract_cube_mob_add_additional_save_data(out),
            MobKind::Wolf(_) => Wolf::add_additional_save_data(self, out),
            MobKind::Bat(_) => Bat::add_additional_save_data(self, out),
            MobKind::Squid(_) => {}
            MobKind::GlowSquid(_) => GlowSquid::add_additional_save_data(self, out),
            MobKind::IronGolem(_) => IronGolem::add_additional_save_data(self, out),
            MobKind::SnowGolem(_) => SnowGolem::add_additional_save_data(self, out),
            MobKind::Cod(_) => AbstractFish::add_additional_save_data(self, out),
            MobKind::Salmon(_) => {
                AbstractFish::add_additional_save_data(self, out);
                Salmon::add_additional_save_data(self, out);
            }
            MobKind::Pufferfish(_) => {
                AbstractFish::add_additional_save_data(self, out);
                Pufferfish::add_additional_save_data(self, out);
            }
            MobKind::TropicalFish(_) => {
                AbstractFish::add_additional_save_data(self, out);
                TropicalFish::add_additional_save_data(self, out);
            }
            MobKind::Cat(_) => Cat::add_additional_save_data(self, out),
            MobKind::Ocelot(_) => Ocelot::add_additional_save_data(self, out),
            MobKind::Horse(_) => Horse::add_additional_save_data(self, out),
            MobKind::SkeletonHorse(_) => SkeletonHorse::add_additional_save_data(self, out),
            MobKind::Llama(_) => Llama::add_additional_save_data(self, out),
            MobKind::TraderLlama(_) => TraderLlama::add_additional_save_data(self, out),
            MobKind::WanderingTrader(_) => WanderingTrader::add_additional_save_data(self, out),
            MobKind::ZombieHorse(_) => AbstractHorse::add_additional_save_data(self, out),
            MobKind::Donkey(_) | MobKind::Mule(_) => AbstractChestedHorse::add_additional_save_data(self, out),
            MobKind::Axolotl(_) => Axolotl::add_additional_save_data(self, out),
            MobKind::Frog(_) => Frog::add_additional_save_data(self, out),
            MobKind::Tadpole(_) => {
                AbstractFish::add_additional_save_data(self, out);
                Tadpole::add_additional_save_data(self, out);
            }
            MobKind::Villager(_) => Villager::add_additional_save_data(self, out),
            MobKind::Fox(_) => Fox::add_additional_save_data(self, out),
            MobKind::Rabbit(_) => Rabbit::add_additional_save_data(self, out),
            MobKind::Turtle(_) => Turtle::add_additional_save_data(self, out),
        }
    }

    pub fn load(&mut self, nbt: &Nbt) {
        let float = |key: &str| nbt.get(key).and_then(Nbt::as_f64).map(|v| v as f32);
        let int = |key: &str| nbt.get(key).and_then(int_value);
        let boolean = |key: &str| nbt.get(key).and_then(Nbt::as_i64).is_some_and(|v| v != 0);
        let short = |key: &str| nbt.get(key).and_then(Nbt::as_i64).map_or(0, |v| v as i16 as i32);
        let y_rot = self.entity.y_rot;
        self.set_y_head_rot(y_rot);
        self.living.y_body_rot = y_rot;
        if self.is_cube_mob() {
            self.abstract_cube_mob_read_size(nbt);
        }
        self.absorption = float("AbsorptionAmount").unwrap_or(0.0);
        if let Some(attributes) = nbt.get("attributes") {
            self.living.attributes.load(attributes);
        }
        self.living.active_effects.clear();
        for effect in nbt.get("active_effects").map(Nbt::as_list).unwrap_or(&[]).iter().filter_map(MobEffectInstance::from_nbt) {
            match self.living.active_effects.binary_search_by_key(&effect.effect, |e| e.effect) {
                Ok(index) => self.living.active_effects[index] = effect,
                Err(index) => self.living.active_effects.insert(index, effect),
            }
            self.living.effects_dirty = true;
        }
        let max_health = self.living.attributes.value(Attribute::MaxHealth) as f32;
        self.set_health(float("Health").unwrap_or(max_health));
        self.living.hurt_time = short("HurtTime");
        self.living.death_time = short("DeathTime");
        self.entity.set_shared_flag(super::flags::FALL_FLYING, boolean("FallFlying"));
        if let Some(packed_brain) = nbt.get("Brain").and_then(brain::Packed::from_nbt) {
            self.living.brain = self.make_brain(packed_brain);
        }
        self.living.last_hurt_by_player =nbt.get("last_hurt_by_player").and_then(uuid_from_nbt);
        self.living.last_hurt_by_player_memory_time = int("last_hurt_by_player_memory_time").unwrap_or(0);
        self.living.last_hurt_by_mob = nbt.get("last_hurt_by_mob").and_then(uuid_from_nbt);
        self.living.last_hurt_by_mob_timestamp = self.entity.tick_count - int("ticks_since_last_hurt_by_mob").unwrap_or(0);
        self.load_equipment(nbt.get("equipment"));
        self.living.locator_bar_icon = nbt.get("locator_bar_icon").and_then(crate::world::waypoints::waypoint::Icon::from_nbt).unwrap_or_default();
        self.living.current_impulse_context_reset_grace_time = int("current_impulse_context_reset_grace_time").unwrap_or(0);
        self.living.current_impulse_impact_pos = match nbt.get("current_explosion_impact_pos") {
            Some(Nbt::List(v)) if v.len() == 3 => Some(wafer_core::Vec3::new(v[0].as_f64().unwrap_or(0.0), v[1].as_f64().unwrap_or(0.0), v[2].as_f64().unwrap_or(0.0))),
            _ => None,
        };
        self.can_pick_up_loot = boolean("CanPickUpLoot");
        self.persistence_required = boolean("PersistenceRequired");
        self.drop_chances = [DEFAULT_EQUIPMENT_DROP_CHANCE; 8];
        if let Some(Nbt::Compound(chances)) = nbt.get("drop_chances") {
            for (name, chance) in chances {
                if let (Some(slot), Some(chance)) = (EquipmentSlot::by_name(name), chance.as_f64()) {
                    self.drop_chances[slot as usize] = chance as f32;
                }
            }
        }
        self.leash_data = super::leashable::LeashData::read(nbt);
        self.home_radius = int("home_radius").unwrap_or(-1);
        if self.home_radius >= 0 {
            self.home_position = match nbt.get("home_pos") {
                Some(Nbt::IntArray(v)) if v.len() == 3 => BlockPos::new(v[0], v[1], v[2]),
                _ => BlockPos::ZERO,
            };
        }
        self.set_left_handed(boolean("LeftHanded"));
        self.loot_table = nbt.get("DeathLootTable").and_then(Nbt::as_str).map(str::to_string);
        self.loot_table_seed = nbt.get("DeathLootTableSeed").and_then(Nbt::as_i64).unwrap_or(0);
        self.set_no_ai(boolean("NoAI"));
        if self.ageable().is_some() && self.can_be_a_baby() {
            if self.set_age(int("Age").unwrap_or(0)) {
                self.refresh_dimensions();
                if let MobKind::Villager(_) = self.kind {
                    Villager::refresh_brain_detached(self);
                }
            }
            let forced_age = int("ForcedAge").unwrap_or(0);
            self.ageable_mut().expect("ageable").forced_age = forced_age;
            self.entity.data.set(ids::AGE_LOCKED, DataValue::Boolean(boolean("AgeLocked")));
        }
        if let Some(animal) = self.animal_mut() {
            animal.in_love = int("InLove").unwrap_or(0);
            animal.love_cause = nbt.get("LoveCause").and_then(uuid_from_nbt);
        }
        let read = |key: &str, registry: &str| nbt.get(key).and_then(Nbt::as_str).and_then(|name| variant_utils::read_variant(registry, name));
        match &mut self.kind {
            MobKind::Cow(_) => {
                if let Some(variant) = read("variant", cow::VARIANT_REGISTRY) {
                    cow::set_variant(&mut self.entity, variant);
                }
                if let Some(sound_variant) = read("sound_variant", cow::SOUND_VARIANT_REGISTRY) {
                    cow::set_sound_variant(&mut self.entity, sound_variant);
                }
            }
            MobKind::MushroomCow(mushroom_cow) => {
                let variant = nbt.get("Type").and_then(Nbt::as_str).and_then(mushroom_cow::Variant::by_name).unwrap_or(mushroom_cow::Variant::DEFAULT);
                mushroom_cow::set_variant(&mut self.entity, variant);
                mushroom_cow.stew_effects = nbt.get("stew_effects").filter(|effects| matches!(effects, Nbt::List(_))).cloned();
            }
            MobKind::Pig(_) => {
                if let Some(variant) = read("variant", pig::VARIANT_REGISTRY) {
                    pig::set_variant(&mut self.entity, variant);
                }
                if let Some(sound_variant) = read("sound_variant", pig::SOUND_VARIANT_REGISTRY) {
                    pig::set_sound_variant(&mut self.entity, sound_variant);
                }
            }
            MobKind::Sheep(_) => {
                sheep::set_sheared(&mut self.entity, boolean("Sheared"));
                let color = nbt.get("Color").and_then(Nbt::as_i64).map_or(DyeColor::VALUES[0], |id| DyeColor::by_id(id as i8 as i32));
                sheep::set_color(&mut self.entity, color);
            }
            MobKind::Goat(_) | MobKind::PolarBear(_) => {}
            MobKind::Armadillo(_) => {}
            MobKind::Chicken(chicken) => {
                let variant = nbt.get("variant").and_then(Nbt::as_str).and_then(chicken::variant_id);
                let sound_variant = nbt.get("sound_variant").and_then(Nbt::as_str).and_then(chicken::sound_variant_id);
                chicken.is_chicken_jockey = boolean("IsChickenJockey");
                if let Some(egg_time) = int("EggLayTime") {
                    chicken.egg_time = egg_time;
                }
                if let Some(variant) = variant {
                    self.entity.data.set(chicken::DATA_VARIANT, DataValue::ChickenVariant(variant));
                }
                if let Some(sound_variant) = sound_variant {
                    self.entity.data.set(chicken::DATA_SOUND_VARIANT, DataValue::ChickenSoundVariant(sound_variant));
                }
            }
            MobKind::Zombie(_) | MobKind::ZombieVillager(_) | MobKind::Husk(_) | MobKind::Drowned(_) | MobKind::Creeper(_) | MobKind::Spider(_) | MobKind::CaveSpider(_) | MobKind::Skeleton(_) | MobKind::Stray(_) | MobKind::Parched(_) | MobKind::WitherSkeleton(_) | MobKind::Bogged(_) | MobKind::Silverfish(_) | MobKind::Endermite(_) | MobKind::Giant(_) => {}
            MobKind::Enderman(_) => {}
            MobKind::Blaze(_) | MobKind::Ghast(_) | MobKind::ZombifiedPiglin(_) | MobKind::Phantom(_) => {}
            MobKind::Witch(witch) => witch.raider.read_additional_save_data(nbt),
            MobKind::Ravager(_) => Ravager::read_additional_save_data(self, nbt),
            MobKind::Vindicator(_) => Vindicator::read_additional_save_data(self, nbt),
            MobKind::Pillager(_) => Pillager::read_additional_save_data(self, nbt),
            MobKind::Evoker(evoker) => evoker.spellcaster_illager.read_additional_save_data(nbt),
            MobKind::Illusioner(illusioner) => illusioner.spellcaster_illager.read_additional_save_data(nbt),
            MobKind::Vex(_) => Vex::read_additional_save_data(self, nbt),
            MobKind::Guardian(_) | MobKind::ElderGuardian(_) => {}
            MobKind::Slime(_) | MobKind::MagmaCube(_) => self.abstract_cube_mob_read_was_on_ground(nbt),
            MobKind::Wolf(_) => {}
            MobKind::Bat(_) | MobKind::Squid(_) | MobKind::GlowSquid(_) => {}
            MobKind::IronGolem(_) | MobKind::SnowGolem(_) => {}
            MobKind::Cod(_) => AbstractFish::read_additional_save_data(self, nbt),
            MobKind::Salmon(_) => {
                AbstractFish::read_additional_save_data(self, nbt);
                Salmon::read_additional_save_data(self, nbt);
            }
            MobKind::Pufferfish(_) => {
                AbstractFish::read_additional_save_data(self, nbt);
                Pufferfish::read_additional_save_data(self, nbt);
            }
            MobKind::TropicalFish(_) => {
                AbstractFish::read_additional_save_data(self, nbt);
                TropicalFish::read_additional_save_data(self, nbt);
            }
            MobKind::Cat(_) | MobKind::Ocelot(_) => {}
            MobKind::Horse(_) | MobKind::Donkey(_) | MobKind::Mule(_) | MobKind::SkeletonHorse(_) | MobKind::ZombieHorse(_) | MobKind::Llama(_) | MobKind::TraderLlama(_) | MobKind::WanderingTrader(_) => {}
            MobKind::Axolotl(_) => Axolotl::read_additional_save_data(self, nbt),
            MobKind::Frog(_) => Frog::read_additional_save_data(self, nbt),
            MobKind::Tadpole(_) => {
                AbstractFish::read_additional_save_data(self, nbt);
                Tadpole::read_additional_save_data(self, nbt);
            }
            MobKind::Villager(_) => Villager::read_additional_save_data(self, nbt),
            MobKind::Fox(_) => {}
            MobKind::Rabbit(_) | MobKind::Turtle(_) => {}
        }
        match self.kind {
            MobKind::Wolf(_) => Wolf::read_additional_save_data(self, nbt),
            MobKind::Cat(_) => Cat::read_additional_save_data(self, nbt),
            MobKind::Ocelot(_) => Ocelot::read_additional_save_data(self, nbt),
            MobKind::Horse(_) => Horse::read_additional_save_data(self, nbt),
            MobKind::SkeletonHorse(_) => SkeletonHorse::read_additional_save_data(self, nbt),
            MobKind::Llama(_) => Llama::read_additional_save_data(self, nbt),
            MobKind::TraderLlama(_) => TraderLlama::read_additional_save_data(self, nbt),
            MobKind::WanderingTrader(_) => WanderingTrader::read_additional_save_data(self, nbt),
            MobKind::ZombieHorse(_) => AbstractHorse::read_additional_save_data(self, nbt),
            MobKind::Donkey(_) | MobKind::Mule(_) => AbstractChestedHorse::read_additional_save_data(self, nbt),
            MobKind::ZombieVillager(_) => ZombieVillager::read_additional_save_data(self, nbt),
            MobKind::Zombie(_) | MobKind::Husk(_) | MobKind::Drowned(_) | MobKind::ZombifiedPiglin(_) => Zombie::read_additional_save_data(self, nbt),
            MobKind::Ghast(_) => Ghast::read_additional_save_data(self, nbt),
            MobKind::Phantom(_) => crate::world::entity::monster::phantom::Phantom::read_additional_save_data(self, nbt),
            MobKind::IronGolem(_) => IronGolem::read_additional_save_data(self, nbt),
            MobKind::SnowGolem(_) => SnowGolem::read_additional_save_data(self, nbt),
            MobKind::Skeleton(_) => Skeleton::read_additional_save_data(self, nbt),
            MobKind::Bogged(_) => Bogged::read_additional_save_data(self, nbt),
            MobKind::Endermite(_) => Endermite::read_additional_save_data(self, nbt),
            MobKind::Giant(_) => {}
            MobKind::Bat(_) => Bat::read_additional_save_data(self, nbt),
            MobKind::GlowSquid(_) => GlowSquid::read_additional_save_data(self, nbt),
            MobKind::Goat(_) => Goat::read_additional_save_data(self, nbt),
            MobKind::PolarBear(_) => {}
            MobKind::Armadillo(_) => Armadillo::read_additional_save_data(self, nbt),
            MobKind::Fox(_) => Fox::read_additional_save_data(self, nbt),
            MobKind::Rabbit(_) => Rabbit::read_additional_save_data(self, nbt),
            MobKind::Turtle(_) => {
                Turtle::read_home_pos(self, nbt);
                Turtle::read_additional_save_data(self, nbt);
            }
            _ => {}
        }
        if matches!(self.kind, MobKind::Creeper(_)) {
            Creeper::read_additional_save_data(self, nbt);
        }
        if matches!(self.kind, MobKind::Enderman(_)) {
            Enderman::read_additional_save_data(self, nbt);
        }
    }

    pub fn refresh_dimensions(&mut self) -> EntityDimensions {
        let old = self.entity.dimensions;
        self.entity.dimensions = self.dimensions();
        self.entity.reapply_position();
        old
    }

    fn chicken_sound_set(&self) -> &'static chicken::SoundSet {
        let id = match self.entity.data.get(chicken::DATA_SOUND_VARIANT) {
            DataValue::ChickenSoundVariant(id) => *id,
            _ => 0,
        };
        let variant = chicken::sound_variant(id);
        if self.is_baby() { &variant.baby_sounds } else { &variant.adult_sounds }
    }

    fn cow_sound_set(&self) -> &'static abstract_cow::CowSoundVariant {
        match &self.kind {
            MobKind::Cow(_) => cow::sound_variant(cow::get_sound_variant(&self.entity)),
            _ => mushroom_cow::get_sound_set(),
        }
    }

    pub fn get_ambient_sound(&self) -> Option<&'static str> {
        Some(match &self.kind {
            MobKind::Chicken(_) => &self.chicken_sound_set().ambient_sound,
            MobKind::Cow(_) | MobKind::MushroomCow(_) => &self.cow_sound_set().ambient_sound,
            MobKind::Pig(_) => &Pig::get_sound_set(self).ambient_sound,
            MobKind::Sheep(_) => "minecraft:entity.sheep.ambient",
            MobKind::Goat(_) => goat::get_ambient_sound(&self.entity),
            MobKind::PolarBear(_) => polar_bear::get_ambient_sound(self),
            MobKind::Armadillo(_) => return armadillo::get_ambient_sound(&self.entity),
            MobKind::Zombie(_) | MobKind::ZombieVillager(_) | MobKind::Husk(_) => Zombie::get_ambient_sound(self),
            MobKind::Drowned(_) => Drowned::get_ambient_sound(self),
            MobKind::Creeper(_) => return None,
            MobKind::Spider(_) | MobKind::CaveSpider(_) => Spider::get_ambient_sound(),
            MobKind::Skeleton(_) => Skeleton::get_ambient_sound(),
            MobKind::Stray(_) => Stray::get_ambient_sound(),
            MobKind::Parched(_) => Parched::get_ambient_sound(),
            MobKind::WitherSkeleton(_) => WitherSkeleton::get_ambient_sound(),
            MobKind::Bogged(_) => Bogged::get_ambient_sound(),
            MobKind::Silverfish(_) => Silverfish::get_ambient_sound(),
            MobKind::Endermite(_) => Endermite::get_ambient_sound(),
            MobKind::Giant(_) => return None,
            MobKind::Enderman(_) => Enderman::get_ambient_sound(self),
            MobKind::Blaze(_) => Blaze::get_ambient_sound(),
            MobKind::Ghast(_) => Ghast::get_ambient_sound(),
            MobKind::Phantom(_) => crate::world::entity::monster::phantom::Phantom::get_ambient_sound(),
            MobKind::ZombifiedPiglin(_) => unreachable!("zombified piglins pick their ambient sound with their anger"),
            MobKind::Witch(_) => Witch::get_ambient_sound(),
            MobKind::Ravager(_) => Ravager::get_ambient_sound(),
            MobKind::Vindicator(_) => Vindicator::get_ambient_sound(),
            MobKind::Pillager(_) => Pillager::get_ambient_sound(),
            MobKind::Evoker(_) => Evoker::get_ambient_sound(),
            MobKind::Illusioner(_) => Illusioner::get_ambient_sound(),
            MobKind::Vex(_) => Vex::get_ambient_sound(),
            MobKind::Guardian(_) | MobKind::ElderGuardian(_) => crate::world::entity::monster::guardian::Guardian::get_ambient_sound(self),
            MobKind::Slime(_) | MobKind::MagmaCube(_) => return None,
            MobKind::Wolf(_) => unreachable!("wolves pick their ambient sound with their random"),
            MobKind::Bat(_) => unreachable!("bats pick their ambient sound with their random"),
            MobKind::Squid(_) => Squid::get_ambient_sound(),
            MobKind::GlowSquid(_) => GlowSquid::get_ambient_sound(),
            MobKind::IronGolem(_) => return None,
            MobKind::SnowGolem(_) => SnowGolem::get_ambient_sound(),
            MobKind::Cod(_) => Cod::get_ambient_sound(),
            MobKind::Salmon(_) => Salmon::get_ambient_sound(),
            MobKind::Pufferfish(_) => return None,
            MobKind::TropicalFish(_) => TropicalFish::get_ambient_sound(),
            MobKind::Cat(_) => unreachable!("cats pick their ambient sound with their random"),
            MobKind::Ocelot(_) => Ocelot::get_ambient_sound(),
            MobKind::Horse(_) => horse::get_ambient_sound(self),
            MobKind::Donkey(_) => donkey::get_ambient_sound(),
            MobKind::Mule(_) => mule::get_ambient_sound(),
            MobKind::SkeletonHorse(_) => skeleton_horse::get_ambient_sound(self),
            MobKind::ZombieHorse(_) => zombie_horse::get_ambient_sound(),
            MobKind::Llama(_) | MobKind::TraderLlama(_) => llama::get_ambient_sound(),
            MobKind::WanderingTrader(_) => {
                if self.abstract_villager().is_some_and(|villager| villager.trading_player.is_some()) {
                    "minecraft:entity.wandering_trader.trade"
                } else {
                    "minecraft:entity.wandering_trader.ambient"
                }
            }
            MobKind::Axolotl(_) => axolotl::get_ambient_sound(&self.entity),
            MobKind::Frog(_) => frog::get_ambient_sound(),
            MobKind::Tadpole(_) => return None,
            MobKind::Villager(_) => return villager::get_ambient_sound(self),
            MobKind::Fox(_) => unreachable!("foxes pick their ambient sound with their random"),
            MobKind::Rabbit(_) => Rabbit::get_ambient_sound(),
            MobKind::Turtle(_) => return Turtle::get_ambient_sound(self),
        })
    }

    pub fn get_hurt_sound(&self) -> &'static str {
        match &self.kind {
            MobKind::Chicken(_) => &self.chicken_sound_set().hurt_sound,
            MobKind::Cow(_) | MobKind::MushroomCow(_) => &self.cow_sound_set().hurt_sound,
            MobKind::Pig(_) => &Pig::get_sound_set(self).hurt_sound,
            MobKind::Sheep(_) => "minecraft:entity.sheep.hurt",
            MobKind::Goat(_) => goat::get_hurt_sound(&self.entity),
            MobKind::PolarBear(_) => polar_bear::get_hurt_sound(),
            MobKind::Armadillo(_) => armadillo::get_hurt_sound(&self.entity),
            MobKind::Zombie(_) | MobKind::ZombieVillager(_) | MobKind::Husk(_) => Zombie::get_hurt_sound(self),
            MobKind::Drowned(_) => Drowned::get_hurt_sound(self),
            MobKind::Creeper(_) => Creeper::get_hurt_sound(),
            MobKind::Spider(_) | MobKind::CaveSpider(_) => Spider::get_hurt_sound(),
            MobKind::Skeleton(_) => Skeleton::get_hurt_sound(),
            MobKind::Stray(_) => Stray::get_hurt_sound(),
            MobKind::Parched(_) => Parched::get_hurt_sound(),
            MobKind::WitherSkeleton(_) => WitherSkeleton::get_hurt_sound(),
            MobKind::Bogged(_) => Bogged::get_hurt_sound(),
            MobKind::Silverfish(_) => Silverfish::get_hurt_sound(),
            MobKind::Endermite(_) => Endermite::get_hurt_sound(),
            MobKind::Giant(_) => "minecraft:entity.hostile.hurt",
            MobKind::Enderman(_) => Enderman::get_hurt_sound(),
            MobKind::Blaze(_) => Blaze::get_hurt_sound(),
            MobKind::Ghast(_) => Ghast::get_hurt_sound(),
            MobKind::Phantom(_) => crate::world::entity::monster::phantom::Phantom::get_hurt_sound(),
            MobKind::ZombifiedPiglin(_) => ZombifiedPiglin::get_hurt_sound(),
            MobKind::Witch(_) => Witch::get_hurt_sound(),
            MobKind::Ravager(_) => Ravager::get_hurt_sound(),
            MobKind::Vindicator(_) => Vindicator::get_hurt_sound(),
            MobKind::Pillager(_) => Pillager::get_hurt_sound(),
            MobKind::Evoker(_) => Evoker::get_hurt_sound(),
            MobKind::Illusioner(_) => Illusioner::get_hurt_sound(),
            MobKind::Vex(_) => Vex::get_hurt_sound(),
            MobKind::Guardian(_) | MobKind::ElderGuardian(_) => crate::world::entity::monster::guardian::Guardian::get_hurt_sound(self),
            MobKind::Slime(_) => Slime::get_hurt_sound(self),
            MobKind::MagmaCube(_) => MagmaCube::get_hurt_sound(self),
            MobKind::Wolf(_) => Wolf::get_hurt_sound(self),
            MobKind::Bat(_) => Bat::get_hurt_sound(),
            MobKind::Squid(_) => Squid::get_hurt_sound(),
            MobKind::GlowSquid(_) => GlowSquid::get_hurt_sound(),
            MobKind::IronGolem(_) => IronGolem::get_hurt_sound(),
            MobKind::SnowGolem(_) => SnowGolem::get_hurt_sound(),
            MobKind::Cod(_) => Cod::get_hurt_sound(),
            MobKind::Salmon(_) => Salmon::get_hurt_sound(),
            MobKind::Pufferfish(_) => Pufferfish::get_hurt_sound(),
            MobKind::TropicalFish(_) => TropicalFish::get_hurt_sound(),
            MobKind::Cat(_) => Cat::get_hurt_sound(self),
            MobKind::Ocelot(_) => Ocelot::get_hurt_sound(),
            MobKind::Horse(_) => horse::get_hurt_sound(self),
            MobKind::Donkey(_) => donkey::get_hurt_sound(),
            MobKind::Mule(_) => mule::get_hurt_sound(),
            MobKind::SkeletonHorse(_) => skeleton_horse::get_hurt_sound(),
            MobKind::ZombieHorse(_) => zombie_horse::get_hurt_sound(),
            MobKind::Llama(_) | MobKind::TraderLlama(_) => llama::get_hurt_sound(),
            MobKind::WanderingTrader(_) => WanderingTrader::get_hurt_sound(),
            MobKind::Axolotl(_) => axolotl::get_hurt_sound(),
            MobKind::Frog(_) => frog::get_hurt_sound(),
            MobKind::Tadpole(_) => tadpole::get_hurt_sound(),
            MobKind::Villager(_) => villager::get_hurt_sound(),
            MobKind::Fox(_) => Fox::get_hurt_sound(),
            MobKind::Rabbit(_) => Rabbit::get_hurt_sound(),
            MobKind::Turtle(_) => Turtle::get_hurt_sound(self),
        }
    }

    pub fn get_death_sound(&self) -> &'static str {
        match &self.kind {
            MobKind::Chicken(_) => &self.chicken_sound_set().death_sound,
            MobKind::Cow(_) | MobKind::MushroomCow(_) => &self.cow_sound_set().death_sound,
            MobKind::Pig(_) => &Pig::get_sound_set(self).death_sound,
            MobKind::Sheep(_) => "minecraft:entity.sheep.death",
            MobKind::Goat(_) => goat::get_death_sound(&self.entity),
            MobKind::PolarBear(_) => polar_bear::get_death_sound(),
            MobKind::Armadillo(_) => armadillo::get_death_sound(),
            MobKind::Zombie(_) | MobKind::ZombieVillager(_) | MobKind::Husk(_) => Zombie::get_death_sound(self),
            MobKind::Drowned(_) => Drowned::get_death_sound(self),
            MobKind::Creeper(_) => Creeper::get_death_sound(),
            MobKind::Spider(_) | MobKind::CaveSpider(_) => Spider::get_death_sound(),
            MobKind::Skeleton(_) => Skeleton::get_death_sound(),
            MobKind::Stray(_) => Stray::get_death_sound(),
            MobKind::Parched(_) => Parched::get_death_sound(),
            MobKind::WitherSkeleton(_) => WitherSkeleton::get_death_sound(),
            MobKind::Bogged(_) => Bogged::get_death_sound(),
            MobKind::Silverfish(_) => Silverfish::get_death_sound(),
            MobKind::Endermite(_) => Endermite::get_death_sound(),
            MobKind::Giant(_) => "minecraft:entity.hostile.death",
            MobKind::Enderman(_) => Enderman::get_death_sound(),
            MobKind::Blaze(_) => Blaze::get_death_sound(),
            MobKind::Ghast(_) => Ghast::get_death_sound(),
            MobKind::Phantom(_) => crate::world::entity::monster::phantom::Phantom::get_death_sound(),
            MobKind::ZombifiedPiglin(_) => ZombifiedPiglin::get_death_sound(),
            MobKind::Witch(_) => Witch::get_death_sound(),
            MobKind::Ravager(_) => Ravager::get_death_sound(),
            MobKind::Vindicator(_) => Vindicator::get_death_sound(),
            MobKind::Pillager(_) => Pillager::get_death_sound(),
            MobKind::Evoker(_) => Evoker::get_death_sound(),
            MobKind::Illusioner(_) => Illusioner::get_death_sound(),
            MobKind::Vex(_) => Vex::get_death_sound(),
            MobKind::Guardian(_) | MobKind::ElderGuardian(_) => crate::world::entity::monster::guardian::Guardian::get_death_sound(self),
            MobKind::Slime(_) => Slime::get_death_sound(self),
            MobKind::MagmaCube(_) => MagmaCube::get_death_sound(self),
            MobKind::Wolf(_) => Wolf::get_death_sound(self),
            MobKind::Bat(_) => Bat::get_death_sound(),
            MobKind::Squid(_) => Squid::get_death_sound(),
            MobKind::GlowSquid(_) => GlowSquid::get_death_sound(),
            MobKind::IronGolem(_) => IronGolem::get_death_sound(),
            MobKind::SnowGolem(_) => SnowGolem::get_death_sound(),
            MobKind::Cod(_) => Cod::get_death_sound(),
            MobKind::Salmon(_) => Salmon::get_death_sound(),
            MobKind::Pufferfish(_) => Pufferfish::get_death_sound(),
            MobKind::TropicalFish(_) => TropicalFish::get_death_sound(),
            MobKind::Cat(_) => Cat::get_death_sound(self),
            MobKind::Ocelot(_) => Ocelot::get_death_sound(),
            MobKind::Horse(_) => horse::get_death_sound(self),
            MobKind::Donkey(_) => donkey::get_death_sound(),
            MobKind::Mule(_) => mule::get_death_sound(),
            MobKind::SkeletonHorse(_) => skeleton_horse::get_death_sound(),
            MobKind::ZombieHorse(_) => zombie_horse::get_death_sound(),
            MobKind::Llama(_) | MobKind::TraderLlama(_) => llama::get_death_sound(),
            MobKind::WanderingTrader(_) => WanderingTrader::get_death_sound(),
            MobKind::Axolotl(_) => axolotl::get_death_sound(),
            MobKind::Frog(_) => frog::get_death_sound(),
            MobKind::Tadpole(_) => tadpole::get_death_sound(),
            MobKind::Villager(_) => villager::get_death_sound(),
            MobKind::Fox(_) => Fox::get_death_sound(),
            MobKind::Rabbit(_) => Rabbit::get_death_sound(),
            MobKind::Turtle(_) => Turtle::get_death_sound(self),
        }
    }

    pub fn get_step_sound(&self) -> Option<&'static str> {
        Some(match &self.kind {
            MobKind::Chicken(_) => &self.chicken_sound_set().step_sound,
            MobKind::Cow(_) | MobKind::MushroomCow(_) => &self.cow_sound_set().step_sound,
            MobKind::Pig(_) => &Pig::get_sound_set(self).step_sound,
            MobKind::Sheep(_) => "minecraft:entity.sheep.step",
            MobKind::Goat(_) => goat::get_step_sound(),
            MobKind::PolarBear(_) => polar_bear::get_step_sound(),
            MobKind::Armadillo(_) => armadillo::get_step_sound(),
            MobKind::Zombie(_) | MobKind::ZombieVillager(_) | MobKind::Husk(_) => Zombie::get_step_sound(self),
            MobKind::Drowned(_) => Drowned::get_step_sound(self),
            MobKind::Creeper(_) => return None,
            MobKind::Spider(_) | MobKind::CaveSpider(_) => Spider::get_step_sound(),
            MobKind::Skeleton(_) => Skeleton::get_step_sound(),
            MobKind::Stray(_) => Stray::get_step_sound(),
            MobKind::Parched(_) => Parched::get_step_sound(),
            MobKind::WitherSkeleton(_) => WitherSkeleton::get_step_sound(),
            MobKind::Bogged(_) => Bogged::get_step_sound(),
            MobKind::Silverfish(_) => Silverfish::get_step_sound(),
            MobKind::Endermite(_) => Endermite::get_step_sound(),
            MobKind::Giant(_) => return None,
            MobKind::Enderman(_) => return None,
            MobKind::Blaze(_) | MobKind::Ghast(_) | MobKind::Phantom(_) => return None,
            MobKind::ZombifiedPiglin(_) => Zombie::get_step_sound(self),
            MobKind::Witch(_) | MobKind::Vindicator(_) | MobKind::Pillager(_) | MobKind::Evoker(_) | MobKind::Illusioner(_) | MobKind::Vex(_) | MobKind::Guardian(_) | MobKind::ElderGuardian(_) => return None,
            MobKind::Ravager(_) => Ravager::get_step_sound(),
            MobKind::Slime(_) | MobKind::MagmaCube(_) => return None,
            MobKind::Wolf(_) => Wolf::get_step_sound(self),
            MobKind::Bat(_) | MobKind::Squid(_) | MobKind::GlowSquid(_) => return None,
            MobKind::IronGolem(_) | MobKind::SnowGolem(_) => return None,
            MobKind::Cod(_) | MobKind::Salmon(_) | MobKind::Pufferfish(_) | MobKind::TropicalFish(_) => return None,
            MobKind::Cat(_) | MobKind::Ocelot(_) => return None,
            MobKind::Horse(_) | MobKind::Donkey(_) | MobKind::Mule(_) | MobKind::SkeletonHorse(_) | MobKind::ZombieHorse(_) => return None,
            MobKind::Llama(_) | MobKind::TraderLlama(_) => llama::get_step_sound(),
            MobKind::WanderingTrader(_) => return None,
            MobKind::Axolotl(_) | MobKind::Tadpole(_) => return None,
            MobKind::Frog(_) => frog::get_step_sound(),
            MobKind::Villager(_) => return None,
            MobKind::Fox(_) => return None,
            MobKind::Rabbit(_) => return None,
            MobKind::Turtle(_) => Turtle::get_step_sound(self),
        })
    }

    pub fn get_sound_volume(&self) -> f32 {
        match &self.kind {
            MobKind::Cow(_) | MobKind::MushroomCow(_) | MobKind::Wolf(_) => 0.4,
            MobKind::Slime(_) | MobKind::MagmaCube(_) => self.abstract_cube_mob_get_sound_volume(),
            MobKind::Bat(_) => Bat::get_sound_volume(),
            MobKind::Squid(_) | MobKind::GlowSquid(_) => Squid::get_sound_volume(),
            MobKind::Horse(_) | MobKind::Donkey(_) | MobKind::Mule(_) | MobKind::SkeletonHorse(_) | MobKind::ZombieHorse(_) | MobKind::Llama(_) | MobKind::TraderLlama(_) => abstract_horse::get_sound_volume(),
            MobKind::Ghast(_) => Ghast::get_sound_volume(),
            _ => 1.0,
        }
    }

    pub fn max_head_y_rot(&self) -> i32 {
        match self.kind {
            MobKind::Ravager(_) => Ravager::get_max_head_y_rot(),
            MobKind::Goat(_) => goat::get_max_head_y_rot(),
            MobKind::Armadillo(_) => armadillo::get_max_head_y_rot(&self.entity),
            MobKind::Axolotl(_) => axolotl::get_max_head_y_rot(),
            MobKind::Frog(_) => frog::get_max_head_y_rot(),
            _ => 75,
        }
    }

    pub fn max_head_x_rot(&self) -> i32 {
        40
    }

    pub fn head_rot_speed(&self) -> i32 {
        10
    }

    pub fn ambient_sound_interval(&self) -> i32 {
        match self.kind {
            MobKind::IronGolem(_) => IronGolem::get_ambient_sound_interval(),
            MobKind::SnowGolem(_) => SnowGolem::get_ambient_sound_interval(),
            MobKind::Ocelot(_) => Ocelot::get_ambient_sound_interval(),
            MobKind::Horse(_) | MobKind::Donkey(_) | MobKind::Mule(_) | MobKind::SkeletonHorse(_) | MobKind::ZombieHorse(_) | MobKind::Llama(_) | MobKind::TraderLlama(_) => abstract_horse::get_ambient_sound_interval(),
            MobKind::Turtle(_) => Turtle::get_ambient_sound_interval(),
            MobKind::Guardian(_) | MobKind::ElderGuardian(_) => crate::world::entity::monster::guardian::Guardian::get_ambient_sound_interval(),
            _ if self.is_squid() => ageable_water_creature::get_ambient_sound_interval(),
            _ if self.is_water_animal() => water_animal::get_ambient_sound_interval(),
            _ if self.animal().is_some() => 120,
            _ => 80,
        }
    }

    pub fn save_equipment(&self) -> Option<Nbt> {
        let slots: Vec<(String, Nbt)> = EquipmentSlot::VALUES
            .into_iter()
            .filter_map(|slot| self.item_by_slot(slot).map(|stack| (slot_name(slot).to_string(), stack.to_nbt())))
            .collect();
        (!slots.is_empty()).then_some(Nbt::Compound(slots))
    }

    pub fn load_equipment(&mut self, nbt: Option<&Nbt>) {
        self.equipment = Default::default();
        let Some(Nbt::Compound(slots)) = nbt else { return };
        for (name, stack) in slots {
            if let Some(slot) = EquipmentSlot::by_name(name)
                && let Ok(stack) = ItemStack::from_nbt(stack)
            {
                self.equipment[slot as usize] = Some(stack);
            }
        }
    }
}

pub fn check_mob_spawn_rules(entity_type: &EntityType, level: &mut (impl ServerLevelAccessor + ?Sized), spawn_reason: EntitySpawnReason, pos: BlockPos) -> bool {
    let below = pos.below();
    EntitySpawnReason::is_spawner(spawn_reason) || block_behaviour::is_valid_spawn(level.block_state(below), entity_type)
}

pub fn slot_name(slot: EquipmentSlot) -> &'static str {
    match slot {
        EquipmentSlot::Mainhand => "mainhand",
        EquipmentSlot::Offhand => "offhand",
        EquipmentSlot::Feet => "feet",
        EquipmentSlot::Legs => "legs",
        EquipmentSlot::Chest => "chest",
        EquipmentSlot::Head => "head",
        EquipmentSlot::Body => "body",
        EquipmentSlot::Saddle => "saddle",
    }
}

impl ServerLevel {
    pub fn mob(&self, id: EntityId) -> Option<&Mob> {
        match self.entities.get(&id) {
            Some(LevelEntity::Mob(mob)) => Some(mob),
            _ => None,
        }
    }

    pub fn mob_mut(&mut self, id: EntityId) -> Option<&mut Mob> {
        match self.entities.get_mut(&id) {
            Some(LevelEntity::Mob(mob)) => Some(mob),
            _ => None,
        }
    }

    pub fn is_mob(&self, id: EntityId) -> bool {
        self.mob(id).is_some()
    }

    pub fn check_and_handle_important_interactions(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if item_stack.is(items::NAME_TAG) {
            let name_tag_interaction_result = item::interact_living_entity(self, &mut item_stack, player, id, hand);
            self.player_mut(player).update_item_in_hand(hand, &item_stack);
            if name_tag_interaction_result.consumes_action() {
                return name_tag_interaction_result;
            }
        }
        if item_stack.get_item().is_instance_of("SpawnEggItem") {
            let entity = self.entity(id);
            let (entity_type, position) = (entity.entity_type, entity.position());
            let offspring = spawn_egg_item::spawn_offspring_from_spawn_egg(self, player, id, entity_type, position, &mut item_stack);
            self.player_mut(player).update_item_in_hand(hand, &item_stack);
            let Some(offspring) = offspring else { return InteractionResult::Pass };
            self.on_offspring_spawned_from_egg(id, player, offspring);
            InteractionResult::SUCCESS_SERVER
        } else {
            InteractionResult::Pass
        }
    }

    fn on_offspring_spawned_from_egg(&mut self, id: EntityId, spawner: EntityId, offspring: EntityId) {
        if self.mob(id).is_some_and(Mob::is_zombie) {
            self.zombie_on_offspring_spawned_from_egg(id, offspring);
        } else if self.is_fox(id) {
            self.fox_on_offspring_spawned_from_egg(spawner, offspring);
        }
    }

    pub fn mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        match self.mob(id).expect("a mob").kind {
            MobKind::Chicken(_) => self.animal_mob_interact(id, player, hand),
            MobKind::Cow(_) => self.abstract_cow_mob_interact(id, player, hand),
            MobKind::MushroomCow(_) => self.mushroom_cow_mob_interact(id, player, hand),
            MobKind::Pig(_) => self.pig_mob_interact(id, player, hand),
            MobKind::Sheep(_) => self.sheep_mob_interact(id, player, hand),
            MobKind::Goat(_) => self.goat_mob_interact(id, player, hand),
            MobKind::PolarBear(_) => self.animal_mob_interact(id, player, hand),
            MobKind::Armadillo(_) => self.armadillo_mob_interact(id, player, hand),
            MobKind::ZombieVillager(_) => self.zombie_villager_mob_interact(id, player, hand),
            MobKind::Zombie(_) | MobKind::Husk(_) | MobKind::Drowned(_) | MobKind::Spider(_) | MobKind::CaveSpider(_) | MobKind::Skeleton(_) | MobKind::Stray(_) | MobKind::Parched(_) | MobKind::WitherSkeleton(_) | MobKind::Silverfish(_) | MobKind::Endermite(_) | MobKind::Giant(_) => {
                self.mob_mob_interact(id, player, hand)
            }
            MobKind::Creeper(_) => self.creeper_mob_interact(id, player, hand),
            MobKind::Bogged(_) => self.bogged_mob_interact(id, player, hand),
            MobKind::Enderman(_) => self.mob_mob_interact(id, player, hand),
            MobKind::Blaze(_) | MobKind::Ghast(_) | MobKind::ZombifiedPiglin(_) | MobKind::Phantom(_) => self.mob_mob_interact(id, player, hand),
            MobKind::Witch(_) | MobKind::Vindicator(_) | MobKind::Pillager(_) | MobKind::Evoker(_) | MobKind::Illusioner(_) | MobKind::Vex(_) | MobKind::Ravager(_) | MobKind::Guardian(_) | MobKind::ElderGuardian(_) => self.mob_mob_interact(id, player, hand),
            MobKind::Slime(_) | MobKind::MagmaCube(_) => self.ageable_mob_mob_interact(id, player, hand),
            MobKind::Wolf(_) => self.wolf_mob_interact(id, player, hand),
            MobKind::Bat(_) => self.mob_mob_interact(id, player, hand),
            MobKind::Squid(_) | MobKind::GlowSquid(_) => self.ageable_mob_mob_interact(id, player, hand),
            MobKind::IronGolem(_) => self.iron_golem_mob_interact(id, player, hand),
            MobKind::SnowGolem(_) => self.snow_golem_mob_interact(id, player, hand),
            MobKind::Cod(_) | MobKind::Salmon(_) | MobKind::Pufferfish(_) | MobKind::TropicalFish(_) => self.abstract_fish_mob_interact(id, player, hand),
            MobKind::Cat(_) => self.cat_mob_interact(id, player, hand),
            MobKind::Ocelot(_) => self.ocelot_mob_interact(id, player, hand),
            MobKind::Horse(_) => self.horse_mob_interact(id, player, hand),
            MobKind::SkeletonHorse(_) => self.skeleton_horse_mob_interact(id, player, hand),
            MobKind::ZombieHorse(_) => self.zombie_horse_mob_interact(id, player, hand),
            MobKind::Llama(_) | MobKind::TraderLlama(_) => self.abstract_chested_horse_mob_interact(id, player, hand),
            MobKind::WanderingTrader(_) => self.wandering_trader_mob_interact(id, player, hand),
            MobKind::Donkey(_) | MobKind::Mule(_) => self.abstract_chested_horse_mob_interact(id, player, hand),
            MobKind::Axolotl(_) => self.axolotl_mob_interact(id, player, hand),
            MobKind::Frog(_) => self.animal_mob_interact(id, player, hand),
            MobKind::Tadpole(_) => self.tadpole_mob_interact(id, player, hand),
            MobKind::Villager(_) => self.villager_mob_interact(id, player, hand),
            MobKind::Fox(_) => self.animal_mob_interact(id, player, hand),
            MobKind::Rabbit(_) | MobKind::Turtle(_) => self.animal_mob_interact(id, player, hand),
        }
    }

    pub fn mob_mob_interact(&mut self, _id: EntityId, _player: EntityId, _hand: InteractionHand) -> InteractionResult {
        InteractionResult::Pass
    }

    pub fn ate(&mut self, id: EntityId) {
        if matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Sheep(_))) {
            self.sheep_ate(id);
        }
    }

    pub fn is_saddled(&self, id: EntityId) -> bool {
        self.has_valid_equippable_item_for_slot(id, EquipmentSlot::Saddle)
    }

    fn has_valid_equippable_item_for_slot(&self, id: EntityId, slot: EquipmentSlot) -> bool {
        self.item_by_slot(id, slot).is_some_and(|item_stack| self.is_equippable_in_slot(id, &item_stack, slot))
    }

    pub fn set_guaranteed_drop(&mut self, id: EntityId, slot: EquipmentSlot) {
        self.mob_mut(id).expect("a mob").drop_chances[slot as usize] = 2.0;
    }

    pub fn can_shear_equipment(&self, id: EntityId, player: EntityId) -> bool {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Wolf(_)) => self.wolf_can_shear_equipment(id, player),
            _ => self.mob_can_shear_equipment(id, player),
        }
    }

    pub fn mob_can_shear_equipment(&self, id: EntityId, _player: EntityId) -> bool {
        !self.is_vehicle(id)
    }

    pub fn attempt_to_shear_equipment(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> bool {
        for slot in EquipmentSlot::VALUES {
            let Some(item_stack) = self.item_by_slot(id, slot) else { continue };
            let Some(equippable) = item_stack.get(dc::EQUIPPABLE).map(item::component::values::Equippable::from_nbt) else { continue };
            let creative = self.players.get(&player).is_some_and(|p| p.game_mode.game_type.is_creative());
            if equippable.can_be_sheared && (!crate::world::item::enchantment::enchantment_helper::has_prevent_armor_change(&item_stack) || creative) {
                self.shear_item(id, player, hand, slot, item_stack);
                let sound = equippable.shearing_sound.as_str().map_or(super::sound_event("minecraft:item.shears.snip"), super::sound_event);
                self.entity_play_sound(id, sound, 1.0, 1.0);
                return true;
            }
        }
        false
    }

    fn shear_item(&mut self, id: EntityId, player: EntityId, hand: InteractionHand, slot: EquipmentSlot, item_stack_to_shear: ItemStack) {
        self.hurt_and_break(player, hand.as_equipment_slot(), 1);
        let equipment_spawn_offset = self.mob(id).expect("a mob").passenger_attachment();
        self.living_set_item_slot(id, slot, ItemStack::empty());
        self.spawn_at_location_with_offset(id, item_stack_to_shear.clone(), equipment_spawn_offset);
        crate::advancements::triggers::criteria_triggers::PLAYER_SHEARED_EQUIPMENT.trigger(self, player, &item_stack_to_shear, id);
    }

    pub fn convert_to(&mut self, id: EntityId, entity_type: &str, params: ConversionParams, after_conversion: impl FnOnce(&mut ServerLevel, EntityId)) -> Option<EntityId> {
        if self.entity(id).is_removed() {
            return None;
        }
        let LevelEntity::Mob(new_mob) = ServerLevel::create_entity(entity_type)? else { return None };
        let new_id = new_mob.entity.id;
        self.entities.insert(new_id, LevelEntity::Mob(new_mob));
        params.ty.convert(self, id, new_id, &params);
        after_conversion(self, new_id);
        let new_mob = self.entities.shift_remove(&new_id).expect("the converted mob");
        self.add_fresh_entity(new_mob);
        if params.ty.should_discard_after_conversion() {
            self.remove(id, super::RemovalReason::Discarded);
        }
        Some(new_id)
    }

    pub fn mob_drop_custom_death_loot(&mut self, id: EntityId, source: &DamageSource, killed_by_player: bool) {
        for slot in EquipmentSlot::VALUES {
            let item_stack = self.item_by_slot(id, slot).unwrap_or_else(ItemStack::empty);
            let mut drop_chance = self.mob(id).expect("a mob").drop_chances[slot as usize];
            if drop_chance != 0.0 {
                let preserve = drop_chance > 1.0;
                if let Some(living_source) = source.causing_entity.filter(|&e| self.is_living(e)) {
                    drop_chance = self.process_equipment_drop_chance(living_source, source, drop_chance);
                }
                if !item_stack.is_empty()
                    && !crate::world::item::enchantment::enchantment_helper::has_prevent_equipment_drop(&item_stack)
                    && (killed_by_player || preserve)
                    && self.entity_mut(id).random.next_float() < drop_chance
                {
                    let mut item_stack = item_stack;
                    if !preserve && item_stack.is_damageable_item() {
                        let max_damage = item_stack.max_damage();
                        let random = &mut self.entity_mut(id).random;
                        let bound = random.next_int_bounded((max_damage - 3).max(1));
                        let damage = max_damage - random.next_int_bounded(1 + bound);
                        item_stack.set_damage_value(damage);
                    }
                    self.spawn_at_location(id, item_stack, 0.0);
                    self.living_set_item_slot(id, slot, ItemStack::empty());
                }
            }
        }
    }

    pub fn use_player_item(&mut self, player: EntityId, hand: InteractionHand, item_stack: &mut ItemStack) {
        let before_use_count = item_stack.count;
        let use_remainder = item_stack.get(dc::USE_REMAINDER).map(ItemStackTemplate::from_nbt);
        item_stack.consume(1, self, Some(player));
        self.player_mut(player).update_item_in_hand(hand, item_stack);
        if let Some(use_remainder) = use_remainder {
            let new_hand_stack = if self.has_infinite_materials(player) || item_stack.count >= before_use_count {
                item_stack.clone()
            } else {
                let remainder_stack = use_remainder.create().unwrap_or_else(ItemStack::empty);
                if item_stack.is_empty() {
                    remainder_stack
                } else {
                    self.handle_extra_items_created_on_use(player, remainder_stack);
                    item_stack.clone()
                }
            };
            self.player_mut(player).update_item_in_hand(hand, &new_hand_stack);
        }
    }

    pub fn mob_tick_head_turn(&mut self, id: EntityId) {
        match self.mob(id).expect("a mob").kind {
            MobKind::Armadillo(_) => self.armadillo_body_rotation_control_client_tick(id),
            MobKind::Phantom(_) => self.phantom_body_rotation_control_client_tick(id),
            _ => self.body_rotation_control_tick(id),
        }
    }

    pub fn stop_in_place(&mut self, id: EntityId) {
        self.get_navigation(id, |navigation, _| navigation.stop());
        let mob = self.mob_mut(id).expect("a mob");
        mob.living.xxa = 0.0;
        mob.living.yya = 0.0;
        mob.set_speed(0.0);
        mob.entity.set_delta_movement(wafer_core::Vec3::ZERO);
        if let Some(leash_data) = mob.leash_data.as_mut() {
            leash_data.angular_momentum = 0.0;
        }
    }

    pub fn mob_tick(&mut self, id: EntityId) {
        self.living_entity_tick(id);
        let no_controller = !self.controlling_passenger(id).is_some_and(|passenger| self.is_mob(passenger))
            || self.is_ravager(id) && self.controlling_passenger(id).is_some_and(|passenger| wafer_data::entity_type_tags::RAIDERS.contains(self.entity(passenger).entity_type as usize));
        let not_in_boat = !self.vehicle(id).is_some_and(|vehicle| self.is_boat(vehicle));
        if let Some(mob) = self.mob_mut(id)
            && mob.entity.tick_count % 5 == 0
        {
            mob.update_control_flags(no_controller, not_in_boat);
        }
    }

    pub fn mob_base_tick(&mut self, id: EntityId) {
        self.living_entity_base_tick(id);
        if self.mob(id).is_none() || !self.is_alive(id) {
            return;
        }
        let mob = self.mob_mut(id).expect("a mob");
        let ambient_sound_time = mob.ambient_sound_time;
        let roll = mob.entity.random.next_int_bounded(1000);
        mob.ambient_sound_time += 1;
        if roll < ambient_sound_time {
            mob.ambient_sound_time = -mob.ambient_sound_interval();
            self.play_ambient_sound(id);
        }
    }

    pub fn play_ambient_sound(&mut self, id: EntityId) {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Axolotl(_)) => self.axolotl_play_ambient_sound(id),
            Some(MobKind::Fox(_)) => self.fox_play_ambient_sound(id),
            _ => self.mob_play_ambient_sound(id),
        }
    }

    pub fn mob_play_ambient_sound(&mut self, id: EntityId) {
        if let Some(sound) = self.get_ambient_sound(id) {
            self.make_sound(id, super::sound_event(sound));
        }
    }

    pub fn get_ambient_sound(&mut self, id: EntityId) -> Option<&'static str> {
        match self.mob(id).expect("a mob").kind {
            MobKind::Wolf(_) => Some(self.wolf_get_ambient_sound(id)),
            MobKind::Bat(_) => self.bat_get_ambient_sound(id),
            MobKind::Cat(_) => Some(self.cat_get_ambient_sound(id)),
            MobKind::Fox(_) => self.fox_get_ambient_sound(id),
            MobKind::ZombifiedPiglin(_) => Some(self.zombified_piglin_get_ambient_sound(id)),
            _ => self.mob(id).expect("a mob").get_ambient_sound(),
        }
    }

    pub fn play_attack_sound(&mut self, id: EntityId) {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Axolotl(_)) => self.axolotl_play_attack_sound(id),
            Some(MobKind::Rabbit(_)) => self.rabbit_play_attack_sound(id),
            _ => {}
        }
    }

    pub fn spawn_anim(&mut self, id: EntityId) {
        self.broadcast_entity_event(id, super::entity_event::SILVERFISH_MERGE_ANIM);
    }

    pub fn mob_ai_step(&mut self, id: EntityId) {
        self.living_entity_ai_step(id);
        if !self.is_mob(id) {
            return;
        }
        if entity_type_tags::BURN_IN_DAYLIGHT.contains(self.entity(id).entity_type as usize) {
            self.burn_undead(id);
        }
        let mob = self.mob(id).expect("a mob");
        if mob.can_pick_up_loot && self.is_alive(id) && !mob.living.dead && self.game_rules.get_bool(game_rule::MOB_GRIEFING) {
            let (x, y, z) = self.get_pickup_reach(id);
            let area = self.entity(id).bounding_box().inflate(x, y, z);
            for entity in self.items_in(&area, None) {
                let Some(LevelEntity::Item(item)) = self.entities.get(&entity) else { continue };
                if !item.entity.is_removed() && !item.item.is_empty() && !item.has_pickup_delay() {
                    let stack = item.item.clone();
                    if self.wants_to_pick_up(id, &stack) {
                        self.pick_up_item(id, entity);
                    }
                }
            }
        }
    }

    fn sun_protection_slot(&self, id: EntityId) -> EquipmentSlot {
        if self.is_zombie_horse(id) { zombie_horse::sun_protection_slot() } else { EquipmentSlot::Head }
    }

    fn burn_undead(&mut self, id: EntityId) {
        if self.is_alive(id) && self.is_sun_burn_tick(id) {
            let slot = self.sun_protection_slot(id);
            match self.item_by_slot(id, slot) {
                Some(sun_blocker) => {
                    if sun_blocker.is_damageable_item() {
                        let amount = self.entity_mut(id).random.next_int_bounded(2);
                        self.hurt_and_break(id, slot, amount);
                    }
                }
                None => self.ignite_for_seconds(id, 8.0),
            }
        }
    }

    fn is_sun_burn_tick(&mut self, id: EntityId) -> bool {
        let position = self.entity(id).position();
        if self.environment_attributes().get_value(environment_attributes::MONSTERS_BURN, position) {
            let br = self.get_entity_light_level_dependent_magic_value(id);
            let e = self.entity(id);
            let rounded_pos = BlockPos::containing(e.x(), e.eye_y(), e.z());
            let (is_in_powder_snow, was_in_powder_snow) = (e.is_in_powder_snow, e.was_in_powder_snow);
            let is_in_non_burnable_block = self.is_in_water_or_rain(id) || is_in_powder_snow || was_in_powder_snow;
            if br > 0.5 && self.entity_mut(id).random.next_float() * 30.0 < (br - 0.4) * 2.0 && !is_in_non_burnable_block && self.can_see_sky(rounded_pos) {
                return true;
            }
        }
        false
    }

    fn get_pickup_reach(&self, _id: EntityId) -> (f64, f64, f64) {
        ITEM_PICKUP_REACH
    }

    pub fn pick_up_item(&mut self, id: EntityId, entity: EntityId) {
        if matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Pillager(_))) {
            self.pillager_pick_up_item(id, entity);
        } else if self.mob(id).and_then(Mob::raider).is_some() {
            self.raider_pick_up_item(id, entity);
        } else if self.is_villager(id) {
            self.villager_pick_up_item(id, entity);
        } else if self.is_fox(id) {
            self.fox_pick_up_item(id, entity);
        } else {
            self.mob_pick_up_item(id, entity);
        }
    }

    pub fn mob_pick_up_item(&mut self, id: EntityId, entity: EntityId) {
        let Some(LevelEntity::Item(item)) = self.entities.get(&entity) else { return };
        let item_stack = item.item.copy();
        let equipped_with_stack = self.equip_item_if_possible(id, item_stack);
        if !equipped_with_stack.is_empty() {
            self.on_item_pickup(id, entity);
            self.take(id, entity, equipped_with_stack.count);
            let Some(LevelEntity::Item(item)) = self.entities.get_mut(&entity) else { return };
            item.item.shrink(equipped_with_stack.count);
            if item.item.is_empty() {
                self.discard(entity);
            }
        }
    }

    pub fn mob_equip(&mut self, id: EntityId, equipment: &super::equipment_table::EquipmentTable) {
        self.mob_equip_loot_table(id, &equipment.loot_table, &equipment.slot_drop_chances);
    }

    pub fn mob_equip_loot_table(&mut self, id: EntityId, loot_table: &str, drop_chances: &[(EquipmentSlot, f32)]) {
        self.equip_loot_table(id, loot_table, create_equipment_params, drop_chances);
    }

    pub fn mob_equip_detached(&mut self, entity: LevelEntity, equipment: &super::equipment_table::EquipmentTable) -> LevelEntity {
        let id = entity.entity().id;
        self.entities.insert(id, entity);
        self.mob_equip(id, equipment);
        self.entities.shift_remove(&id).expect("the equipped mob")
    }

    pub fn equip_item_if_possible(&mut self, id: EntityId, item_stack: ItemStack) -> ItemStack {
        let mut slot = self.get_equipment_slot_for_item(id, &item_stack);
        if !self.is_equippable_in_slot(id, &item_stack, slot) {
            return ItemStack::empty();
        }
        let mut current = self.item_by_slot(id, slot).unwrap_or_else(ItemStack::empty);
        let mut can_replace = self.can_replace_current_item(id, &item_stack, &current, slot);
        if slot.is_armor() && !can_replace {
            slot = EquipmentSlot::Mainhand;
            current = self.item_by_slot(id, slot).unwrap_or_else(ItemStack::empty);
            can_replace = current.is_empty();
        }
        if can_replace && self.can_hold_item(id, &item_stack) {
            let mob = self.mob_mut(id).expect("a mob");
            let drop_chance = mob.drop_chances[slot as usize];
            if !current.is_empty() && (mob.entity.random.next_float() - 0.1).max(0.0) < drop_chance {
                self.spawn_at_location(id, current, 0.0);
            }
            let to_equip = limit(slot, item_stack);
            self.set_item_slot_and_drop_when_killed(id, slot, to_equip.clone());
            self.mob_mut(id).expect("a mob").persistence_required = true;
            to_equip
        } else {
            ItemStack::empty()
        }
    }

    pub fn set_item_slot_and_drop_when_killed(&mut self, id: EntityId, slot: EquipmentSlot, item_stack: ItemStack) {
        self.living_set_item_slot(id, slot, item_stack);
        self.set_guaranteed_drop(id, slot);
    }

    pub fn get_body_armor_item(&self, id: EntityId) -> ItemStack {
        self.item_by_slot(id, EquipmentSlot::Body).unwrap_or_else(ItemStack::empty)
    }

    pub fn is_wearing_body_armor(&self, id: EntityId) -> bool {
        self.item_by_slot_ref(id, EquipmentSlot::Body).is_some()
    }

    fn can_replace_current_item(&self, id: EntityId, new_item_stack: &ItemStack, current_item_stack: &ItemStack, slot: EquipmentSlot) -> bool {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Drowned(_)) => self.drowned_can_replace_current_item(id, new_item_stack, current_item_stack, slot),
            _ => self.mob_can_replace_current_item(id, new_item_stack, current_item_stack, slot),
        }
    }

    pub fn mob_can_replace_current_item(&self, id: EntityId, new_item_stack: &ItemStack, current_item_stack: &ItemStack, slot: EquipmentSlot) -> bool {
        if current_item_stack.is_empty() {
            true
        } else if slot.is_armor() {
            self.compare_armor(id, new_item_stack, current_item_stack, slot)
        } else if slot == EquipmentSlot::Mainhand {
            self.compare_weapons(id, new_item_stack, current_item_stack, slot)
        } else {
            false
        }
    }

    fn compare_armor(&self, id: EntityId, new_item_stack: &ItemStack, current_item_stack: &ItemStack, slot: EquipmentSlot) -> bool {
        if enchantment_helper::has_prevent_armor_change(current_item_stack) {
            return false;
        }
        let new_defense = self.get_approximate_attribute_with(id, new_item_stack, Attribute::Armor, slot);
        let old_defense = self.get_approximate_attribute_with(id, current_item_stack, Attribute::Armor, slot);
        let new_toughness = self.get_approximate_attribute_with(id, new_item_stack, Attribute::ArmorToughness, slot);
        let old_toughness = self.get_approximate_attribute_with(id, current_item_stack, Attribute::ArmorToughness, slot);
        if new_defense != old_defense {
            new_defense > old_defense
        } else if new_toughness != old_toughness {
            new_toughness > old_toughness
        } else {
            can_replace_equal_item(new_item_stack, current_item_stack)
        }
    }

    fn compare_weapons(&self, id: EntityId, new_item_stack: &ItemStack, current_item_stack: &ItemStack, slot: EquipmentSlot) -> bool {
        if let Some(preferred_weapon_type) = self.get_preferred_weapon_type(id) {
            if current_item_stack.get_item().is(preferred_weapon_type) && !new_item_stack.get_item().is(preferred_weapon_type) {
                return false;
            }
            if !current_item_stack.get_item().is(preferred_weapon_type) && new_item_stack.get_item().is(preferred_weapon_type) {
                return true;
            }
        }
        let new_attack_damage = self.get_approximate_attribute_with(id, new_item_stack, Attribute::AttackDamage, slot);
        let old_attack_damage = self.get_approximate_attribute_with(id, current_item_stack, Attribute::AttackDamage, slot);
        if new_attack_damage != old_attack_damage { new_attack_damage > old_attack_damage } else { can_replace_equal_item(new_item_stack, current_item_stack) }
    }

    fn get_approximate_attribute_with(&self, id: EntityId, item_stack: &ItemStack, attribute: Attribute, slot: EquipmentSlot) -> f64 {
        let attributes = &self.living(id).attributes;
        let base_value = if attributes.has_attribute(attribute) { attributes.base_value(attribute) } else { 0.0 };
        compute_attribute_modifiers(item_stack, attribute, base_value, slot)
    }

    pub fn can_hold_item(&self, id: EntityId, item_stack: &ItemStack) -> bool {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Zombie(_) | MobKind::ZombieVillager(_) | MobKind::Husk(_) | MobKind::Drowned(_) | MobKind::ZombifiedPiglin(_)) => self.zombie_can_hold_item(id, item_stack),
            Some(MobKind::Fox(_)) => self.fox_can_hold_item(id, item_stack),
            Some(MobKind::WitherSkeleton(_)) => self.wither_skeleton_can_hold_item(id, item_stack),
            _ => self.mob_can_hold_item(id, item_stack),
        }
    }

    pub fn mob_can_hold_item(&self, _id: EntityId, _item_stack: &ItemStack) -> bool {
        true
    }

    pub fn wants_to_pick_up(&self, id: EntityId, item_stack: &ItemStack) -> bool {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Zombie(_) | MobKind::ZombieVillager(_) | MobKind::Husk(_)) => self.zombie_wants_to_pick_up(id, item_stack),
            Some(MobKind::Drowned(_)) => self.drowned_wants_to_pick_up(id, item_stack),
            Some(MobKind::ZombifiedPiglin(_)) => self.zombified_piglin_wants_to_pick_up(id, item_stack),
            Some(MobKind::Skeleton(_) | MobKind::Stray(_) | MobKind::Parched(_) | MobKind::WitherSkeleton(_) | MobKind::Bogged(_)) => self.abstract_skeleton_wants_to_pick_up(id, item_stack),
            Some(MobKind::Villager(_)) => self.villager_wants_to_pick_up(id, item_stack),
            _ => self.mob_wants_to_pick_up(id, item_stack),
        }
    }

    pub fn mob_wants_to_pick_up(&self, id: EntityId, item_stack: &ItemStack) -> bool {
        self.can_hold_item(id, item_stack)
    }

    pub fn can_use_non_melee_weapon(&self, id: EntityId, item: &ItemStack) -> bool {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Skeleton(_) | MobKind::Stray(_) | MobKind::Parched(_) | MobKind::WitherSkeleton(_) | MobKind::Bogged(_)) => item.get_item() == items::BOW,
            Some(MobKind::Pillager(_)) => Pillager::can_use_non_melee_weapon(item),
            _ => self.mob_can_use_non_melee_weapon(id, item),
        }
    }

    pub fn mob_can_use_non_melee_weapon(&self, _id: EntityId, _item: &ItemStack) -> bool {
        false
    }

    fn get_preferred_weapon_type(&self, id: EntityId) -> Option<&'static wafer_data::Tag> {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Skeleton(_) | MobKind::Stray(_) | MobKind::Parched(_) | MobKind::Bogged(_)) => Some(&wafer_data::item_tags::SKELETON_PREFERRED_WEAPONS),
            Some(MobKind::Drowned(_)) => Some(Drowned::get_preferred_weapon_type()),
            Some(MobKind::Pillager(_)) => Some(Pillager::get_preferred_weapon_type()),
            _ => None,
        }
    }

    pub fn is_monster(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(Mob::is_monster)
    }

    pub fn get_target(&self, id: EntityId) -> Option<EntityId> {
        match self.mob(id)?.kind {
            MobKind::Axolotl(_) => self.axolotl_get_target(id),
            MobKind::Frog(_) => self.frog_get_target(id),
            _ => self.mob_get_target(id),
        }
    }

    pub fn mob_get_target(&self, id: EntityId) -> Option<EntityId> {
        self.as_valid_target(id, self.mob(id)?.target)
    }

    pub fn get_target_from_brain(&self, id: EntityId) -> Option<EntityId> {
        self.get_brain(id).get_memory(memory_module_type::ATTACK_TARGET).copied()
    }

    fn as_valid_target(&self, id: EntityId, target: Option<EntityId>) -> Option<EntityId> {
        let target = target.filter(|&target| self.is_living(target))?;
        if self.is_player(target) && (self.player(target).game_mode.game_type.is_creative() || self.is_spectator(target)) {
            None
        } else if !self.can_attack(id, target) {
            None
        } else {
            Some(target)
        }
    }

    pub fn set_target(&mut self, id: EntityId, target: Option<EntityId>) {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Creeper(_)) => self.creeper_set_target(id, target),
            Some(MobKind::Enderman(_)) => self.enderman_set_target(id, target),
            Some(MobKind::Fox(_)) => self.fox_set_target(id, target),
            Some(MobKind::ZombifiedPiglin(_)) => self.zombified_piglin_set_target(id, target),
            _ => self.mob_set_target(id, target),
        }
    }

    pub fn mob_set_target(&mut self, id: EntityId, target: Option<EntityId>) {
        let target = self.as_valid_target(id, target);
        if let Some(mob) = self.mob_mut(id) {
            mob.target = target;
        }
    }

    pub fn look_at(&mut self, id: EntityId, entity: EntityId, y_max: f32, x_max: f32) {
        let (e, t) = (self.entity(id), self.entity(entity));
        let xd = t.x() - e.x();
        let zd = t.z() - e.z();
        let yd = if self.is_living(entity) { t.eye_y() - e.eye_y() } else { (t.bounding_box().min_y + t.bounding_box().max_y) / 2.0 - e.eye_y() };
        let sd = (xd * xd + zd * zd).sqrt();
        let y_rot_d = (wafer_core::mth::atan2(zd, xd) * wafer_core::mth::RAD_TO_DEG as f64) as f32 - 90.0;
        let x_rot_d = (-(wafer_core::mth::atan2(yd, sd) * wafer_core::mth::RAD_TO_DEG as f64)) as f32;
        let e = self.entity_mut(id);
        let x_rot = rotlerp(e.x_rot, x_rot_d, x_max);
        e.set_x_rot(x_rot);
        let y_rot = rotlerp(e.y_rot, y_rot_d, y_max);
        e.set_y_rot(y_rot);
    }

    pub fn mob_can_attack(&self, id: EntityId, target: EntityId) -> bool {
        !(wafer_data::registry::ENTITY_TYPE[self.entity(target).entity_type as usize] == "minecraft:ghast") && self.living_entity_can_attack(id, target)
    }

    pub fn is_within_melee_attack_range(&self, id: EntityId, target: EntityId) -> bool {
        let (max_range, min_range) = match self.get_active_item(id).get(dc::ATTACK_RANGE).map(AttackRange::from_nbt) {
            None => (default_attack_reach(), 0.0),
            Some(attack_range) => (attack_range.effective_max_range(self, id) as f64, attack_range.effective_min_range(self, id) as f64),
        };
        let hitbox = self.get_hitbox(target);
        self.get_attack_bounding_box(id, max_range).intersects(&hitbox) && (min_range <= 0.0 || !self.get_attack_bounding_box(id, min_range).intersects(&hitbox))
    }

    fn get_attack_bounding_box(&self, id: EntityId, horizontal_expansion: f64) -> AABB {
        let aabb = match self.vehicle(id) {
            Some(vehicle) => {
                let mount_aabb = self.entity(vehicle).bounding_box();
                let own_aabb = self.entity(id).bounding_box();
                AABB::new(
                    own_aabb.min_x.min(mount_aabb.min_x),
                    own_aabb.min_y,
                    own_aabb.min_z.min(mount_aabb.min_z),
                    own_aabb.max_x.max(mount_aabb.max_x),
                    own_aabb.max_y,
                    own_aabb.max_z.max(mount_aabb.max_z),
                )
            }
            None => self.entity(id).bounding_box(),
        };
        let default_bb = aabb.inflate(horizontal_expansion, 0.0, horizontal_expansion);
        if self.is_ravager(id) { Ravager::get_attack_bounding_box(default_bb) } else { default_bb }
    }

    pub fn get_hitbox(&self, id: EntityId) -> AABB {
        let aabb = self.entity(id).bounding_box();
        match self.vehicle(id) {
            Some(vehicle) => {
                let pos = self.passenger_riding_position(vehicle, id);
                aabb.set_min_y(pos.y.max(aabb.min_y))
            }
            None => aabb,
        }
    }

    pub fn do_hurt_target(&mut self, id: EntityId, target: EntityId) -> bool {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Zombie(_) | MobKind::ZombieVillager(_) | MobKind::Drowned(_) | MobKind::ZombifiedPiglin(_)) => self.zombie_do_hurt_target(id, target),
            Some(MobKind::Husk(_)) => self.husk_do_hurt_target(id, target),
            Some(MobKind::WitherSkeleton(_)) => self.wither_skeleton_do_hurt_target(id, target),
            Some(MobKind::Creeper(_)) => self.creeper_do_hurt_target(id, target),
            Some(MobKind::CaveSpider(_)) => self.cave_spider_do_hurt_target(id, target),
            Some(MobKind::IronGolem(_)) => self.iron_golem_do_hurt_target(id, target),
            Some(MobKind::Ravager(_)) => self.ravager_do_hurt_target(id, target),
            _ => self.mob_do_hurt_target(id, target),
        }
    }

    pub fn mob_do_hurt_target(&mut self, id: EntityId, target: EntityId) -> bool {
        let mut dmg = self.attribute_value(id, Attribute::AttackDamage) as f32;
        let weapon_item = self.get_weapon_item(id);
        let damage_source = self.get_damage_source(&weapon_item, id);
        dmg = self.modify_damage(&weapon_item, target, &damage_source, dmg);
        let old_movement = self.entity(target).delta_movement;
        let was_hurt = self.hurt_server(target, damage_source, dmg);
        if was_hurt {
            let knockback = self.get_knockback(id, target, &damage_source);
            self.living_entity_cause_extra_knockback(id, target, knockback, old_movement);
            self.do_post_attack_effects(target, &damage_source);
            self.set_last_hurt_mob(id, target);
            self.play_attack_sound(id);
        }
        self.post_piercing_attack(id);
        was_hurt
    }

    fn get_damage_source(&self, item_stack: &ItemStack, attacker: EntityId) -> DamageSource {
        let ty = item_stack
            .get(dc::DAMAGE_TYPE)
            .and_then(Nbt::as_str)
            .and_then(wafer_data::DamageType::by_name)
            .unwrap_or(damage_types::MOB_ATTACK);
        DamageSource { causing_entity: Some(attacker), direct_entity: Some(attacker), ..DamageSource::new(ty) }
    }

    pub fn swing_for_attack(&mut self, id: EntityId, hand: InteractionHand) {
        let swing_animation = self.item_by_slot_ref(id, hand.as_equipment_slot()).map_or(crate::world::item::component::swing_animation::SwingAnimation::DEFAULT, ItemStack::attack_animation);
        self.swing(id, hand, swing_animation, false);
    }

    pub fn mob_get_base_experience_reward(&mut self, id: EntityId) -> i32 {
        let xp_reward = self.mob(id).expect("a mob").xp_reward;
        if xp_reward > 0 {
            let mut result = xp_reward;
            for slot in EquipmentSlot::VALUES {
                if slot != EquipmentSlot::Saddle {
                    let mob = self.mob_mut(id).expect("a mob");
                    if mob.item_by_slot(slot).is_some() && mob.drop_chances[slot as usize] <= 1.0 {
                        result += 1 + mob.entity.random.next_int_bounded(3);
                    }
                }
            }
            result
        } else {
            xp_reward
        }
    }

    pub fn set_chicken_jockey(&mut self, id: EntityId, is_chicken_jockey: bool) {
        if let Some(MobKind::Chicken(chicken)) = self.mob_mut(id).map(|mob| &mut mob.kind) {
            chicken.is_chicken_jockey = is_chicken_jockey;
        }
    }

    pub fn mob_server_ai_step(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a mob");
        mob.living.no_action_time += 1;
        mob.ai.sensing.tick();
        let id_based_tick_count = mob.entity.tick_count.wrapping_add(id);
        if id_based_tick_count % 2 != 0 && mob.entity.tick_count > 1 {
            self.with_target_selector(id, |selector, level| selector.tick_running_goals(level, id, false));
            self.with_goal_selector(id, |selector, level| selector.tick_running_goals(level, id, false));
        } else {
            self.with_target_selector(id, |selector, level| selector.tick(level, id));
            self.with_goal_selector(id, |selector, level| selector.tick(level, id));
        }
        if self.mob(id).is_none() {
            return;
        }
        self.get_navigation(id, |navigation, level| navigation.tick(level, id));
        self.custom_server_ai_step(id);
        match self.mob(id).expect("a mob").ai.move_control.kind {
            MoveControlKind::Default => self.move_control_tick(id),
            MoveControlKind::Drowned => self.drowned_move_control_tick(id),
            MoveControlKind::CubeMob => self.cube_mob_move_control_tick(id),
            MoveControlKind::Fish => self.fish_move_control_tick(id),
            MoveControlKind::SmoothSwimming(control) => self.smooth_swimming_move_control_tick(id, control),
            MoveControlKind::Axolotl(control) => self.axolotl_move_control_tick(id, control),
            MoveControlKind::Fox => self.fox_move_control_tick(id),
            MoveControlKind::Ghast => self.ghast_move_control_tick(id),
            MoveControlKind::Vex => self.vex_move_control_tick(id),
            MoveControlKind::Phantom => self.phantom_move_control_tick(id),
            MoveControlKind::Guardian => self.guardian_move_control_tick(id),
            MoveControlKind::Rabbit => self.rabbit_move_control_tick(id),
            MoveControlKind::Turtle => self.turtle_move_control_tick(id),
        }
        self.look_control_tick(id);
        match self.mob(id).expect("a mob").kind {
            MobKind::Rabbit(_) => self.rabbit_jump_control_tick(id),
            _ => self.jump_control_tick(id),
        }
    }

    pub fn with_goal_selector(&mut self, id: EntityId, f: impl FnOnce(&mut GoalSelector, &mut ServerLevel)) {
        let Some(mob) = self.mob_mut(id) else { return };
        let mut selector = std::mem::take(&mut mob.ai.goal_selector);
        f(&mut selector, self);
        if let Some(mob) = self.mob_mut(id) {
            mob.ai.goal_selector = selector;
        }
    }

    fn with_target_selector(&mut self, id: EntityId, f: impl FnOnce(&mut GoalSelector, &mut ServerLevel)) {
        let Some(mob) = self.mob_mut(id) else { return };
        let mut selector = std::mem::take(&mut mob.ai.target_selector);
        f(&mut selector, self);
        if let Some(mob) = self.mob_mut(id) {
            mob.ai.target_selector = selector;
        }
    }

    pub fn get_navigation<R>(&mut self, id: EntityId, f: impl FnOnce(&mut PathNavigation, &mut ServerLevel) -> R) -> R {
        let mut navigation = std::mem::take(&mut self.mob_mut(id).expect("a mob").ai.navigation);
        let result = f(&mut navigation, self);
        if let Some(mob) = self.mob_mut(id) {
            mob.ai.navigation = navigation;
        }
        result
    }

    pub fn get_max_fall_distance(&self, id: EntityId) -> i32 {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Creeper(_)) => self.creeper_get_max_fall_distance(id),
            _ => self.mob_get_max_fall_distance(id),
        }
    }

    pub fn mob_get_max_fall_distance(&self, id: EntityId) -> i32 {
        if self.get_target(id).is_none() {
            return get_comfortable_fall_distance(0.0);
        }
        let mob = self.mob(id).expect("a mob");
        let max_health = mob.living.attributes.value(Attribute::MaxHealth) as f32;
        let mut sacrifice = (mob.health - max_health * 0.33) as i32;
        sacrifice -= (3 - self.difficulty as i32) * 4;
        if sacrifice < 0 {
            sacrifice = 0;
        }
        get_comfortable_fall_distance(sacrifice as f32)
    }

    fn custom_server_ai_step(&mut self, id: EntityId) {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Sheep(_)) => self.sheep_custom_server_ai_step(id),
            Some(MobKind::Goat(_)) => self.goat_custom_server_ai_step(id),
            Some(MobKind::Armadillo(_)) => self.armadillo_custom_server_ai_step(id),
            Some(
                MobKind::Zombie(_)
                | MobKind::ZombieVillager(_)
                | MobKind::Husk(_)
                | MobKind::Drowned(_)
                | MobKind::Creeper(_)
                | MobKind::Spider(_)
                | MobKind::CaveSpider(_)
                | MobKind::Skeleton(_)
                | MobKind::Stray(_) | MobKind::Parched(_)
                | MobKind::WitherSkeleton(_)
                | MobKind::Bogged(_)
                | MobKind::Silverfish(_)
                | MobKind::Endermite(_) | MobKind::Giant(_),
            )
            | None => {}
            Some(MobKind::Enderman(_)) => self.enderman_custom_server_ai_step(id),
            Some(MobKind::Blaze(_)) => self.blaze_custom_server_ai_step(id),
            Some(MobKind::Ghast(_) | MobKind::Phantom(_) | MobKind::Guardian(_)) => {}
            Some(MobKind::ElderGuardian(_)) => self.elder_guardian_custom_server_ai_step(id),
            Some(MobKind::ZombifiedPiglin(_)) => self.zombified_piglin_custom_server_ai_step(id),
            Some(MobKind::Witch(_) | MobKind::Ravager(_)) => {}
            Some(MobKind::Vindicator(_)) => self.vindicator_custom_server_ai_step(id),
            Some(MobKind::Pillager(_)) => {}
            Some(MobKind::Evoker(_) | MobKind::Illusioner(_)) => self.spellcaster_illager_custom_server_ai_step(id),
            Some(MobKind::Vex(_)) => {}
            Some(MobKind::Slime(_) | MobKind::MagmaCube(_)) => {}
            Some(MobKind::Bat(_)) => self.bat_custom_server_ai_step(id),
            Some(MobKind::Squid(_) | MobKind::GlowSquid(_)) => {}
            Some(MobKind::IronGolem(_) | MobKind::SnowGolem(_)) => {}
            Some(MobKind::Cod(_) | MobKind::Salmon(_) | MobKind::Pufferfish(_) | MobKind::TropicalFish(_)) => {}
            Some(MobKind::Cat(_)) => self.cat_custom_server_ai_step(id),
            Some(MobKind::Ocelot(_)) => self.ocelot_custom_server_ai_step(id),
            Some(MobKind::Axolotl(_)) => self.axolotl_custom_server_ai_step(id),
            Some(MobKind::Frog(_)) => self.frog_custom_server_ai_step(id),
            Some(MobKind::Tadpole(_)) => self.tadpole_custom_server_ai_step(id),
            Some(MobKind::Villager(_)) => self.villager_custom_server_ai_step(id),
            Some(MobKind::Rabbit(_)) => self.rabbit_custom_server_ai_step(id),
            Some(_) => self.animal_custom_server_ai_step(id),
        }
    }

    pub fn requires_custom_persistence(&self, id: EntityId) -> bool {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Enderman(_)) => self.enderman_requires_custom_persistence(id),
            Some(MobKind::Witch(_) | MobKind::Vindicator(_) | MobKind::Pillager(_) | MobKind::Evoker(_) | MobKind::Illusioner(_) | MobKind::Ravager(_)) => self.raider_requires_custom_persistence(id),
            Some(MobKind::Cod(_) | MobKind::Salmon(_) | MobKind::Pufferfish(_) | MobKind::TropicalFish(_) | MobKind::Tadpole(_)) => self.abstract_fish_requires_custom_persistence(id),
            Some(MobKind::Axolotl(_)) => self.axolotl_requires_custom_persistence(id),
            _ => self.mob_requires_custom_persistence(id),
        }
    }

    pub fn mob_requires_custom_persistence(&self, id: EntityId) -> bool {
        self.is_passenger(id) || self.is_leashed(id)
    }

    pub fn read_level_dependent_save_data(&mut self, id: EntityId, input: &Nbt) {
        if self.is_thrown_enderpearl(id) {
            self.thrown_enderpearl_register_to_current_owner(id);
        }
        if matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Tadpole(_))) {
            self.tadpole_read_level_dependent_save_data(id);
        }
        if matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::ZombieVillager(_))) {
            self.zombie_villager_read_level_dependent_save_data(id, input);
        }
        if matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Enderman(_) | MobKind::Wolf(_) | MobKind::IronGolem(_) | MobKind::ZombifiedPiglin(_) | MobKind::PolarBear(_))) {
            self.read_persistent_anger_save_data(id, input);
        }
    }

    pub fn read_level_dependent_save_data_detached(&mut self, mut entity: LevelEntity, input: &Nbt) -> LevelEntity {
        if crate::world::entity::projectile::throwableitemprojectile::thrown_enderpearl::is_thrown_enderpearl(&entity) {
            self.thrown_enderpearl_register_detached(&entity);
        }
        if let LevelEntity::Mob(mob) = &mut entity
            && let MobKind::Villager(_) = mob.kind
        {
            let pos = mob.entity.position();
            self.villager_register_brain_goals_detached(mob, pos);
        }
        if !matches!(&entity, LevelEntity::Mob(mob) if mob.neutral_mob().is_some() || matches!(mob.kind, MobKind::Tadpole(_) | MobKind::ZombieVillager(_))) {
            return entity;
        }
        let id = entity.entity().id;
        self.entities.insert(id, entity);
        self.read_level_dependent_save_data(id, input);
        self.entities.shift_remove(&id).expect("the loaded mob")
    }

    pub fn check_mob_despawn(&mut self, id: EntityId) {
        let mob = self.mob(id).expect("a mob");
        let entity_type = mob.get_type();
        if self.difficulty == Difficulty::Peaceful && !entity_type.is_allowed_in_peaceful() {
            self.remove(id, super::RemovalReason::Discarded);
            return;
        }
        let is_persistent = mob.is_persistence_required() || self.requires_custom_persistence(id);
        let position = self.entity(id).position();
        let Some(player) = self.get_nearest_player_to(position.x, position.y, position.z, -1.0, false) else { return };
        let dist_sqr = self.entity(player).position().distance_to_sqr(position);
        let instant_despawn_distance = entity_type.get_category().get_despawn_distance();
        let despawn_distance_sqr = instant_despawn_distance * instant_despawn_distance;
        if !is_persistent && dist_sqr > despawn_distance_sqr as f64 && self.mob(id).expect("a mob").remove_when_far_away(dist_sqr) {
            self.remove(id, super::RemovalReason::Discarded);
        }
        let no_despawn_distance = entity_type.get_category().get_no_despawn_distance();
        let no_despawn_distance_sqr = (no_despawn_distance * no_despawn_distance) as f64;
        let mob = self.mob_mut(id).expect("a mob");
        if !is_persistent
            && mob.living.no_action_time > 600
            && mob.entity.random.next_int_bounded(800) == 0
            && dist_sqr > no_despawn_distance_sqr
            && mob.remove_when_far_away(dist_sqr)
        {
            self.remove(id, super::RemovalReason::Discarded);
        } else if dist_sqr < no_despawn_distance_sqr {
            mob.living.no_action_time = 0;
        }
    }

    pub fn finalize_spawn_detached(&mut self, entity: LevelEntity, difficulty: &DifficultyInstance, spawn_reason: EntitySpawnReason, group_data: Option<SpawnGroupData>) -> (LevelEntity, Option<SpawnGroupData>) {
        let id = entity.entity().id;
        self.entities.insert(id, entity);
        let group_data = self.finalize_spawn(id, difficulty, spawn_reason, group_data);
        let entity = self.entities.shift_remove(&id).expect("the finalized mob");
        (entity, group_data)
    }

    pub fn finalize_spawn(&mut self, id: EntityId, difficulty: &DifficultyInstance, spawn_reason: EntitySpawnReason, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
        match self.mob(id).expect("a mob").kind {
            MobKind::Zombie(_) => self.zombie_finalize_spawn(id, difficulty, spawn_reason, group_data),
            MobKind::ZombieVillager(_) => self.zombie_villager_finalize_spawn(id, difficulty, spawn_reason, group_data),
            MobKind::Vindicator(_) => self.vindicator_finalize_spawn(id, difficulty, spawn_reason, group_data),
            MobKind::Pillager(_) => self.pillager_finalize_spawn(id, difficulty, spawn_reason, group_data),
            MobKind::Vex(_) => self.vex_finalize_spawn(id, difficulty, spawn_reason, group_data),
            MobKind::ZombieHorse(_) => self.zombie_horse_finalize_spawn(id, difficulty, spawn_reason, group_data),
            MobKind::Illusioner(_) => self.illusioner_finalize_spawn(id, difficulty, spawn_reason, group_data),
            MobKind::Husk(_) => self.husk_finalize_spawn(id, difficulty, spawn_reason, group_data),
            MobKind::ZombifiedPiglin(_) => self.zombie_finalize_spawn(id, difficulty, spawn_reason, group_data),
            MobKind::Drowned(_) => self.drowned_finalize_spawn(id, difficulty, spawn_reason, group_data),
            MobKind::Spider(_) => self.spider_finalize_spawn(id, difficulty, spawn_reason, group_data),
            MobKind::CaveSpider(_) => self.cave_spider_finalize_spawn(id, difficulty, spawn_reason, group_data),
            MobKind::WitherSkeleton(_) => self.wither_skeleton_finalize_spawn(id, difficulty, spawn_reason, group_data),
            MobKind::Skeleton(_) | MobKind::Stray(_) | MobKind::Parched(_) | MobKind::Bogged(_) => self.abstract_skeleton_finalize_spawn(id, difficulty, spawn_reason, group_data),
            MobKind::Cod(_) => self.abstract_schooling_fish_finalize_spawn(id, difficulty, spawn_reason, group_data),
            MobKind::Salmon(_) => {
                self.salmon_finalize_spawn(id);
                self.abstract_schooling_fish_finalize_spawn(id, difficulty, spawn_reason, group_data)
            }
            MobKind::TropicalFish(_) => self.tropical_fish_finalize_spawn(id, difficulty, spawn_reason, group_data),
            MobKind::Pufferfish(_) => self.mob_finalize_spawn(id, difficulty, spawn_reason, group_data),
            MobKind::Fox(_) => self.fox_finalize_spawn(id, difficulty, spawn_reason, group_data),
            _ => {
                let (index, _, entity) = self.entities.shift_remove_full(&id).expect("the mob to finalize");
                let LevelEntity::Mob(mut mob) = entity else { unreachable!("a mob") };
                let group_data = finalize_spawn(self, &mut mob, difficulty, spawn_reason, group_data);
                self.entities.shift_insert(index, id, LevelEntity::Mob(mob));
                group_data
            }
        }
    }

    pub fn mob_finalize_spawn(&mut self, id: EntityId, _difficulty: &DifficultyInstance, _spawn_reason: EntitySpawnReason, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
        let has_modifier = self.mob(id).expect("a mob").living.attributes.has_modifier(Attribute::FollowRange, RANDOM_SPAWN_BONUS_ID);
        if !has_modifier {
            let amount = self.random.triangle(0.0, 0.11485000000000001);
            let mob = self.mob_mut(id).expect("a mob");
            let follow_range = mob.living.attributes.instance(Attribute::FollowRange).expect("mobs have a follow range");
            follow_range.add_permanent_modifier(super::attributes::AttributeModifier {
                id: std::borrow::Cow::Borrowed(RANDOM_SPAWN_BONUS_ID),
                amount,
                operation: super::attributes::Operation::AddMultipliedBase,
            });
        }
        let left_handed = self.random.next_float() < 0.05;
        self.mob_mut(id).expect("a mob").set_left_handed(left_handed);
        group_data
    }

    pub fn populate_default_equipment_slots(&mut self, id: EntityId, difficulty: &DifficultyInstance) {
        match self.mob(id).expect("a mob").kind {
            MobKind::Zombie(_) | MobKind::ZombieVillager(_) | MobKind::Husk(_) => self.zombie_populate_default_equipment_slots(id, difficulty),
            MobKind::Drowned(_) => self.drowned_populate_default_equipment_slots(id, difficulty),
            MobKind::ZombifiedPiglin(_) => self.with_level_random(|level, random| level.zombified_piglin_populate_default_equipment_slots(id, random)),
            MobKind::WitherSkeleton(_) => self.wither_skeleton_populate_default_equipment_slots(id),
            MobKind::Skeleton(_) | MobKind::Stray(_) | MobKind::Parched(_) | MobKind::Bogged(_) => self.abstract_skeleton_populate_default_equipment_slots(id, difficulty),
            _ => self.mob_populate_default_equipment_slots(id, difficulty),
        }
    }

    pub fn mob_populate_default_equipment_slots(&mut self, id: EntityId, difficulty: &DifficultyInstance) {
        if self.random.next_float() < MAX_WEARING_ARMOR_CHANCE * difficulty.get_special_multiplier() {
            let mut armor_type = self.random.next_int_bounded(3);
            let mut i = 1;
            while i as f32 <= WEARING_ARMOR_UPGRADE_MATERIAL_ATTEMPTS {
                if self.random.next_float() < WEARING_ARMOR_UPGRADE_MATERIAL_CHANCE {
                    armor_type += 1;
                }
                i += 1;
            }
            let partial_chance = if self.difficulty == Difficulty::Hard { 0.1 } else { 0.25 };
            let mut first = true;
            for slot in EQUIPMENT_POPULATION_ORDER {
                let item_stack = self.item_by_slot(id, slot);
                if !first && self.random.next_float() < partial_chance {
                    break;
                }
                first = false;
                if item_stack.is_none()
                    && let Some(equip) = get_equipment_for_slot(slot, armor_type)
                {
                    self.living_set_item_slot(id, slot, ItemStack::new(equip, 1));
                }
            }
        }
    }

    pub fn populate_default_equipment_enchantments(&mut self, id: EntityId, local_difficulty: &DifficultyInstance) {
        if matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::WitherSkeleton(_))) {
            return;
        }
        self.enchant_spawned_weapon(id, local_difficulty);
        for slot in EquipmentSlot::VALUES {
            if is_humanoid_armor(slot) {
                self.enchant_spawned_armor(id, slot, local_difficulty);
            }
        }
    }

    fn enchant_spawned_weapon(&mut self, id: EntityId, difficulty: &DifficultyInstance) {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Pillager(_)) => self.pillager_enchant_spawned_weapon(id, difficulty),
            _ => self.mob_enchant_spawned_weapon(id, difficulty),
        }
    }

    pub fn mob_enchant_spawned_weapon(&mut self, id: EntityId, difficulty: &DifficultyInstance) {
        self.enchant_spawned_equipment(id, EquipmentSlot::Mainhand, MAX_ENCHANTED_WEAPON_CHANCE, difficulty);
    }

    fn enchant_spawned_armor(&mut self, id: EntityId, slot: EquipmentSlot, difficulty: &DifficultyInstance) {
        self.enchant_spawned_equipment(id, slot, MAX_ENCHANTED_ARMOR_CHANCE, difficulty);
    }

    fn enchant_spawned_equipment(&mut self, id: EntityId, slot: EquipmentSlot, chance: f32, difficulty: &DifficultyInstance) {
        let Some(mut item_stack) = self.item_by_slot(id, slot) else { return };
        if self.random.next_float() < chance * difficulty.get_special_multiplier() {
            enchantment_helper::enchant_item_from_provider(&mut item_stack, &vanilla_enchantment_providers::MOB_SPAWN_EQUIPMENT, difficulty, &mut self.random);
            self.living_set_item_slot(id, slot, item_stack);
        }
    }
}

fn rotlerp(a: f32, b: f32, max: f32) -> f32 {
    let mut diff = wafer_core::mth::wrap_degrees(b - a);
    if diff > max {
        diff = max;
    }
    if diff < -max {
        diff = -max;
    }
    a + diff
}

fn is_humanoid_armor(slot: EquipmentSlot) -> bool {
    matches!(slot, EquipmentSlot::Feet | EquipmentSlot::Legs | EquipmentSlot::Chest | EquipmentSlot::Head)
}

fn create_equipment_params(level: &mut ServerLevel, id: EntityId) -> crate::world::level::storage::loot::loot_params::LootParams<'_> {
    let origin = level.entity(id).position();
    let mut params = crate::world::level::storage::loot::loot_params::LootParams::new(level);
    params.origin = Some(origin);
    params.this_entity = Some(id);
    params
}

pub fn limit(slot: EquipmentSlot, mut to_equip: ItemStack) -> ItemStack {
    if slot == EquipmentSlot::Mainhand || slot == EquipmentSlot::Offhand { to_equip } else { to_equip.split(1) }
}

fn can_replace_equal_item(new_item_stack: &ItemStack, current_item_stack: &ItemStack) -> bool {
    let enchantments = |stack: &ItemStack| stack.get(dc::ENCHANTMENTS).map_or(0, |enchantments| ItemEnchantments::from_nbt(enchantments).enchantments.len());
    let (current_enchantments, new_enchantments) = (enchantments(current_item_stack), enchantments(new_item_stack));
    if new_enchantments != current_enchantments {
        return new_enchantments > current_enchantments;
    }
    let (new_damage_value, current_damage_value) = (new_item_stack.damage_value(), current_item_stack.damage_value());
    if new_damage_value != current_damage_value {
        new_damage_value < current_damage_value
    } else {
        new_item_stack.has(dc::CUSTOM_NAME) && !current_item_stack.has(dc::CUSTOM_NAME)
    }
}

fn compute_attribute_modifiers(item_stack: &ItemStack, attribute: Attribute, base_value: f64, slot: EquipmentSlot) -> f64 {
    let mut value = base_value;
    let Some(modifiers) = item_stack.get(dc::ATTRIBUTE_MODIFIERS) else { return value };
    for entry in modifiers.as_list() {
        let entry_attribute = entry.get("type").and_then(Nbt::as_str).and_then(Attribute::by_name);
        let group = entry.get("slot").map_or(Ok(EquipmentSlotGroup::Any), crate::world::item::enchantment::enchantment::slot_group);
        if entry_attribute == Some(attribute) && group.is_ok_and(|group| group.test(slot)) {
            let amount = entry.get("amount").map_or(0.0, crate::world::item::component::codec::as_f64);
            value += match entry.get("operation").and_then(Nbt::as_str).and_then(super::attributes::Operation::by_name) {
                Some(super::attributes::Operation::AddValue) | None => amount,
                Some(super::attributes::Operation::AddMultipliedBase) => amount * base_value,
                Some(super::attributes::Operation::AddMultipliedTotal) => amount * value,
            };
        }
    }
    value
}

pub fn get_equipment_for_slot(slot: EquipmentSlot, ty: i32) -> Option<Item> {
    let by_type = |types: [Item; 6]| (0..6).contains(&ty).then(|| types[ty as usize]);
    match slot {
        EquipmentSlot::Head => by_type([items::LEATHER_HELMET, items::COPPER_HELMET, items::GOLDEN_HELMET, items::CHAINMAIL_HELMET, items::IRON_HELMET, items::DIAMOND_HELMET])
            .or_else(|| get_equipment_for_slot(EquipmentSlot::Chest, ty)),
        EquipmentSlot::Chest => by_type([items::LEATHER_CHESTPLATE, items::COPPER_CHESTPLATE, items::GOLDEN_CHESTPLATE, items::CHAINMAIL_CHESTPLATE, items::IRON_CHESTPLATE, items::DIAMOND_CHESTPLATE])
            .or_else(|| get_equipment_for_slot(EquipmentSlot::Legs, ty)),
        EquipmentSlot::Legs => by_type([items::LEATHER_LEGGINGS, items::COPPER_LEGGINGS, items::GOLDEN_LEGGINGS, items::CHAINMAIL_LEGGINGS, items::IRON_LEGGINGS, items::DIAMOND_LEGGINGS])
            .or_else(|| get_equipment_for_slot(EquipmentSlot::Feet, ty)),
        EquipmentSlot::Feet => by_type([items::LEATHER_BOOTS, items::COPPER_BOOTS, items::GOLDEN_BOOTS, items::CHAINMAIL_BOOTS, items::IRON_BOOTS, items::DIAMOND_BOOTS]),
        _ => None,
    }
}

pub fn finalize_spawn(
    level: &mut (impl ServerLevelAccessor + ?Sized),
    mob: &mut Mob,
    difficulty: &DifficultyInstance,
    spawn_reason: EntitySpawnReason,
    group_data: Option<SpawnGroupData>,
) -> Option<SpawnGroupData> {
    let mut group_data = group_data;
    match mob.kind {
        MobKind::Wolf(_) => group_data = wolf::finalize_spawn(level, mob, group_data),
        MobKind::Chicken(_) => chicken::finalize_spawn(level, mob),
        MobKind::Cow(_) => cow::finalize_spawn(level, mob),
        MobKind::MushroomCow(_) => {}
        MobKind::Pig(_) => pig::finalize_spawn(level, mob),
        MobKind::Sheep(_) => sheep::finalize_spawn(level, mob),
        MobKind::Goat(_) => goat::finalize_spawn(level, mob),
        MobKind::PolarBear(_) => group_data = polar_bear::finalize_spawn(level, group_data),
        MobKind::Armadillo(_) => {}
        MobKind::Zombie(_)
        | MobKind::ZombieVillager(_)
        | MobKind::Husk(_)
        | MobKind::Drowned(_)
        | MobKind::Creeper(_)
        | MobKind::Spider(_)
        | MobKind::Skeleton(_)
        | MobKind::Stray(_) | MobKind::Parched(_)
        | MobKind::WitherSkeleton(_)
        | MobKind::Bogged(_)
        | MobKind::Silverfish(_)
        | MobKind::Endermite(_) | MobKind::Giant(_) => {}
        MobKind::CaveSpider(_) => return group_data,
        MobKind::Enderman(_) => {}
        MobKind::Blaze(_) | MobKind::Ghast(_) | MobKind::ZombifiedPiglin(_) => {}
        MobKind::Phantom(_) => return super::monster::phantom::finalize_spawn(level, mob, difficulty, spawn_reason, group_data),
        MobKind::Witch(_) | MobKind::Vindicator(_) | MobKind::Pillager(_) | MobKind::Evoker(_) | MobKind::Illusioner(_) | MobKind::Ravager(_) => return raider::finalize_spawn(level, mob, difficulty, spawn_reason, group_data),
        MobKind::Vex(_) | MobKind::Guardian(_) | MobKind::ElderGuardian(_) => {}
        MobKind::Slime(_) => return slime::finalize_spawn(level, mob, difficulty, spawn_reason, group_data),
        MobKind::MagmaCube(_) => return magma_cube::finalize_spawn(level, mob, difficulty, spawn_reason, group_data),
        MobKind::Bat(_) => {}
        MobKind::Squid(_) | MobKind::GlowSquid(_) => return squid::finalize_spawn(level, mob, difficulty, spawn_reason, group_data),
        MobKind::IronGolem(_) | MobKind::SnowGolem(_) => {}
        MobKind::Cod(_) | MobKind::Salmon(_) | MobKind::Pufferfish(_) | MobKind::TropicalFish(_) | MobKind::Tadpole(_) => {}
        MobKind::Cat(_) => return cat::finalize_spawn(level, mob, difficulty, spawn_reason, group_data),
        MobKind::Ocelot(_) => return ocelot::finalize_spawn(level, mob, difficulty, spawn_reason, group_data),
        MobKind::Horse(_) => group_data = horse::finalize_spawn(level, mob, group_data),
        MobKind::Donkey(_) | MobKind::Mule(_) | MobKind::SkeletonHorse(_) | MobKind::ZombieHorse(_) => group_data = abstract_horse::finalize_spawn(level, mob, group_data),
        MobKind::Llama(_) => group_data = llama::finalize_spawn(level, mob, group_data),
        MobKind::TraderLlama(_) => group_data = trader_llama::finalize_spawn(level, mob, spawn_reason, group_data),
        MobKind::WanderingTrader(_) => group_data = super::npc::villager::abstract_villager::finalize_spawn(group_data),
        MobKind::Axolotl(_) => {
            if spawn_reason == EntitySpawnReason::Bucket {
                return group_data;
            }
            group_data = axolotl::finalize_spawn(level, mob, group_data);
        }
        MobKind::Frog(_) => frog::finalize_spawn(level, mob),
        MobKind::Villager(_) => group_data = villager::finalize_spawn(level, mob, spawn_reason, group_data),
        MobKind::Fox(_) => return fox::finalize_spawn(level, mob, difficulty, spawn_reason, group_data, false),
        MobKind::Rabbit(_) => group_data = rabbit::finalize_spawn(level, mob, group_data),
        MobKind::Turtle(_) => turtle::finalize_spawn(mob),
    }
    let group_data = if mob.ageable().is_some() { ageable_mob::finalize_spawn(level, mob, group_data) } else { group_data };
    mob_finalize_spawn(level, mob, difficulty, spawn_reason, group_data)
}

pub fn mob_finalize_spawn(
    level: &mut (impl ServerLevelAccessor + ?Sized),
    mob: &mut Mob,
    _difficulty: &DifficultyInstance,
    _spawn_reason: EntitySpawnReason,
    group_data: Option<SpawnGroupData>,
) -> Option<SpawnGroupData> {
    let random = level.get_random();
    if let Some(follow_range) = mob.living.attributes.instance(Attribute::FollowRange)
        && !follow_range.has_modifier(RANDOM_SPAWN_BONUS_ID)
    {
        follow_range.add_permanent_modifier(super::attributes::AttributeModifier {
            id: std::borrow::Cow::Borrowed(RANDOM_SPAWN_BONUS_ID),
            amount: random.triangle(0.0, 0.11485000000000001),
            operation: super::attributes::Operation::AddMultipliedBase,
        });
    }
    mob.set_left_handed(random.next_float() < 0.05);
    group_data
}

pub fn get_walk_target_value(mob: &Mob, pos: BlockPos, level: &mut (impl ServerLevelAccessor + ?Sized)) -> f32 {
    match mob.kind {
        MobKind::MushroomCow(_) => mushroom_cow::get_walk_target_value(pos, level),
        MobKind::Giant(_) => giant::get_walk_target_value(pos, level),
        MobKind::Zombie(_)
        | MobKind::ZombieVillager(_)
        | MobKind::Husk(_)
        | MobKind::Drowned(_)
        | MobKind::Creeper(_)
        | MobKind::Spider(_)
        | MobKind::CaveSpider(_)
        | MobKind::Skeleton(_)
        | MobKind::Stray(_) | MobKind::Parched(_)
        | MobKind::WitherSkeleton(_)
        | MobKind::Bogged(_)
        | MobKind::Endermite(_) => super::monster::monster::get_walk_target_value(pos, level),
        MobKind::Silverfish(_) => silverfish::get_walk_target_value(pos, level),
        MobKind::Enderman(_) => Enderman::get_walk_target_value(),
        MobKind::Blaze(_) | MobKind::ZombifiedPiglin(_) => super::monster::monster::get_walk_target_value(pos, level),
        MobKind::Ghast(_) | MobKind::Phantom(_) => super::pathfinder_mob::get_walk_target_value(),
        MobKind::Guardian(_) | MobKind::ElderGuardian(_) => super::monster::guardian::get_walk_target_value(pos, level),
        MobKind::Witch(_) | MobKind::Vindicator(_) | MobKind::Evoker(_) | MobKind::Illusioner(_) | MobKind::Vex(_) | MobKind::Ravager(_) => super::monster::monster::get_walk_target_value(pos, level),
        MobKind::Pillager(_) => Pillager::get_walk_target_value(),
        MobKind::Slime(_) | MobKind::MagmaCube(_) => super::pathfinder_mob::get_walk_target_value(),
        MobKind::Squid(_) | MobKind::GlowSquid(_) => super::pathfinder_mob::get_walk_target_value(),
        MobKind::IronGolem(_) | MobKind::SnowGolem(_) => super::pathfinder_mob::get_walk_target_value(),
        MobKind::Cod(_) | MobKind::Salmon(_) | MobKind::Pufferfish(_) | MobKind::TropicalFish(_) | MobKind::Tadpole(_) => super::pathfinder_mob::get_walk_target_value(),
        MobKind::Axolotl(_) => axolotl::get_walk_target_value(),
        MobKind::Villager(_) => super::pathfinder_mob::get_walk_target_value(),
        MobKind::Turtle(_) => turtle::get_walk_target_value(mob, pos, level),
        _ => super::animal::get_walk_target_value(pos, level),
    }
}

pub fn check_spawn_rules(mob: &Mob, level: &mut (impl ServerLevelAccessor + ?Sized), _spawn_reason: EntitySpawnReason) -> bool {
    match mob.kind {
        MobKind::Bat(_) => true,
        _ => get_walk_target_value(mob, mob.entity.block_position(), level) >= 0.0,
    }
}

pub fn check_spawn_obstruction(mob: &Mob, level: &mut (impl ServerLevelAccessor + ?Sized)) -> bool {
    let bounding_box = mob.entity.bounding_box();
    match mob.kind {
        MobKind::Drowned(_) | MobKind::Guardian(_) | MobKind::ElderGuardian(_) => level.is_unobstructed(&bounding_box),
        MobKind::Squid(_) | MobKind::GlowSquid(_) => ageable_water_creature::check_spawn_obstruction(mob, level),
        MobKind::IronGolem(_) => IronGolem::check_spawn_obstruction(mob, level),
        MobKind::Ravager(_) => Ravager::check_spawn_obstruction(mob, level),
        MobKind::Cod(_) | MobKind::Salmon(_) | MobKind::Pufferfish(_) | MobKind::TropicalFish(_) | MobKind::Tadpole(_) => water_animal::check_spawn_obstruction(mob, level),
        MobKind::Ocelot(_) => ocelot::check_spawn_obstruction(mob, level),
        MobKind::Axolotl(_) => axolotl::check_spawn_obstruction(mob, level),
        _ => !level.contains_any_liquid(&bounding_box) && level.is_unobstructed(&bounding_box),
    }
}
