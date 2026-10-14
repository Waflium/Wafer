use wafer_data::block_tags;

use super::entity_type::{Builder, EntityType};
use super::mob_category::MobCategory;

pub const ACACIA_BOAT: EntityType = Builder::of(0, MobCategory::Misc).sized(1.375, 0.5625).build();
pub const ACACIA_CHEST_BOAT: EntityType = Builder::of(1, MobCategory::Misc).sized(1.375, 0.5625).build();
pub const ALLAY: EntityType = Builder::of(2, MobCategory::Creature).sized(0.35, 0.6).build();
pub const AREA_EFFECT_CLOUD: EntityType = Builder::of(3, MobCategory::Misc).sized(6.0, 0.5).fire_immune().build();
pub const ARMADILLO: EntityType = Builder::of(4, MobCategory::Creature).sized(0.7, 0.65).build();
pub const ARMOR_STAND: EntityType = Builder::of(5, MobCategory::Misc).sized(0.5, 1.975).build();
pub const ARROW: EntityType = Builder::of(6, MobCategory::Misc).sized(0.5, 0.5).build();
pub const AXOLOTL: EntityType = Builder::of(7, MobCategory::Axolotls).sized(0.75, 0.42).build();
pub const BAMBOO_CHEST_RAFT: EntityType = Builder::of(8, MobCategory::Misc).sized(1.375, 0.5625).build();
pub const BAMBOO_RAFT: EntityType = Builder::of(9, MobCategory::Misc).sized(1.375, 0.5625).build();
pub const BAT: EntityType = Builder::of(10, MobCategory::Ambient).sized(0.5, 0.9).build();
pub const BEE: EntityType = Builder::of(11, MobCategory::Creature).sized(0.55, 0.5).build();
pub const BIRCH_BOAT: EntityType = Builder::of(12, MobCategory::Misc).sized(1.375, 0.5625).build();
pub const BIRCH_CHEST_BOAT: EntityType = Builder::of(13, MobCategory::Misc).sized(1.375, 0.5625).build();
pub const BLAZE: EntityType = Builder::of(14, MobCategory::Monster).sized(0.6, 1.8).fire_immune().not_in_peaceful().build();
pub const BLOCK_DISPLAY: EntityType = Builder::of(15, MobCategory::Misc).sized(0.0, 0.0).build();
pub const BOGGED: EntityType = Builder::of(16, MobCategory::Monster).sized(0.6, 1.99).not_in_peaceful().build();
pub const BREEZE: EntityType = Builder::of(17, MobCategory::Monster).sized(0.6, 1.77).not_in_peaceful().build();
pub const BREEZE_WIND_CHARGE: EntityType = Builder::of(18, MobCategory::Misc).sized(0.3125, 0.3125).build();
pub const CAMEL: EntityType = Builder::of(19, MobCategory::Creature).sized(1.7, 2.375).build();
pub const CAMEL_HUSK: EntityType = Builder::of(20, MobCategory::Monster).sized(1.7, 2.375).build();
pub const CAT: EntityType = Builder::of(21, MobCategory::Creature).sized(0.6, 0.7).build();
pub const CAVE_SPIDER: EntityType = Builder::of(22, MobCategory::Monster).sized(0.7, 0.5).not_in_peaceful().build();
pub const CHERRY_BOAT: EntityType = Builder::of(23, MobCategory::Misc).sized(1.375, 0.5625).build();
pub const CHERRY_CHEST_BOAT: EntityType = Builder::of(24, MobCategory::Misc).sized(1.375, 0.5625).build();
pub const CHEST_MINECART: EntityType = Builder::of(25, MobCategory::Misc).sized(0.98, 0.7).build();
pub const CHICKEN: EntityType = Builder::of(26, MobCategory::Creature).sized(0.4, 0.7).build();
pub const COD: EntityType = Builder::of(27, MobCategory::WaterAmbient).sized(0.5, 0.3).build();
pub const COPPER_GOLEM: EntityType = Builder::of(28, MobCategory::Misc).sized(0.49, 0.98).build();
pub const COMMAND_BLOCK_MINECART: EntityType = Builder::of(29, MobCategory::Misc).sized(0.98, 0.7).build();
pub const COW: EntityType = Builder::of(30, MobCategory::Creature).sized(0.9, 1.4).build();
pub const CREAKING: EntityType = Builder::of(31, MobCategory::Monster).sized(0.9, 2.7).not_in_peaceful().build();
pub const CREEPER: EntityType = Builder::of(32, MobCategory::Monster).sized(0.6, 1.7).not_in_peaceful().build();
pub const CUSHION: EntityType = Builder::of(33, MobCategory::Misc).sized(1.0, 0.25).build();
pub const DARK_OAK_BOAT: EntityType = Builder::of(34, MobCategory::Misc).sized(1.375, 0.5625).build();
pub const DARK_OAK_CHEST_BOAT: EntityType = Builder::of(35, MobCategory::Misc).sized(1.375, 0.5625).build();
pub const DOLPHIN: EntityType = Builder::of(36, MobCategory::WaterCreature).sized(0.9, 0.6).build();
pub const DONKEY: EntityType = Builder::of(37, MobCategory::Creature).sized(1.3964844, 1.5).build();
pub const DRAGON_FIREBALL: EntityType = Builder::of(38, MobCategory::Misc).sized(1.0, 1.0).build();
pub const DROWNED: EntityType = Builder::of(39, MobCategory::Monster).sized(0.6, 1.95).not_in_peaceful().build();
pub const EGG: EntityType = Builder::of(40, MobCategory::Misc).sized(0.25, 0.25).build();
pub const ELDER_GUARDIAN: EntityType = Builder::of(41, MobCategory::Monster).sized(1.9975, 1.9975).not_in_peaceful().build();
pub const ENDERMAN: EntityType = Builder::of(42, MobCategory::Monster).sized(0.6, 2.9).not_in_peaceful().build();
pub const ENDERMITE: EntityType = Builder::of(43, MobCategory::Monster).sized(0.4, 0.3).not_in_peaceful().build();
pub const ENDER_DRAGON: EntityType = Builder::of(44, MobCategory::Monster).sized(16.0, 8.0).fire_immune().build();
pub const ENDER_PEARL: EntityType = Builder::of(45, MobCategory::Misc).sized(0.25, 0.25).build();
pub const END_CRYSTAL: EntityType = Builder::of(46, MobCategory::Misc).sized(2.0, 2.0).fire_immune().build();
pub const EVOKER: EntityType = Builder::of(47, MobCategory::Monster).sized(0.6, 1.95).not_in_peaceful().build();
pub const EVOKER_FANGS: EntityType = Builder::of(48, MobCategory::Misc).sized(0.5, 0.8).build();
pub const EXPERIENCE_BOTTLE: EntityType = Builder::of(49, MobCategory::Misc).sized(0.25, 0.25).build();
pub const EXPERIENCE_ORB: EntityType = Builder::of(50, MobCategory::Misc).sized(0.5, 0.5).build();
pub const EYE_OF_ENDER: EntityType = Builder::of(51, MobCategory::Misc).sized(0.25, 0.25).build();
pub const FALLING_BLOCK: EntityType = Builder::of(52, MobCategory::Misc).sized(0.98, 0.98).build();
pub const FIREBALL: EntityType = Builder::of(53, MobCategory::Misc).sized(1.0, 1.0).build();
pub const FIREWORK_ROCKET: EntityType = Builder::of(54, MobCategory::Misc).sized(0.25, 0.25).build();
pub const FOX: EntityType = Builder::of(55, MobCategory::Creature).sized(0.6, 0.7).immune_to(|| &block_tags::FOX_IMMUNE_TO).build();
pub const FROG: EntityType = Builder::of(56, MobCategory::Creature).sized(0.5, 0.5).build();
pub const FURNACE_MINECART: EntityType = Builder::of(57, MobCategory::Misc).sized(0.98, 0.7).build();
pub const GHAST: EntityType = Builder::of(58, MobCategory::Monster).sized(4.0, 4.0).fire_immune().not_in_peaceful().build();
pub const HAPPY_GHAST: EntityType = Builder::of(59, MobCategory::Creature).sized(4.0, 4.0).build();
pub const GIANT: EntityType = Builder::of(60, MobCategory::Monster).sized(3.6, 12.0).not_in_peaceful().build();
pub const GLOW_ITEM_FRAME: EntityType = Builder::of(61, MobCategory::Misc).sized(0.5, 0.5).build();
pub const GLOW_SQUID: EntityType = Builder::of(62, MobCategory::UndergroundWaterCreature).sized(0.8, 0.8).build();
pub const GOAT: EntityType = Builder::of(63, MobCategory::Creature).sized(0.9, 1.3).build();
pub const GUARDIAN: EntityType = Builder::of(64, MobCategory::Monster).sized(0.85, 0.85).not_in_peaceful().build();
pub const HOGLIN: EntityType = Builder::of(65, MobCategory::Monster).sized(1.3964844, 1.4).not_in_peaceful().build();
pub const HOPPER_MINECART: EntityType = Builder::of(66, MobCategory::Misc).sized(0.98, 0.7).build();
pub const HORSE: EntityType = Builder::of(67, MobCategory::Creature).sized(1.3964844, 1.6).build();
pub const HUSK: EntityType = Builder::of(68, MobCategory::Monster).sized(0.6, 1.95).not_in_peaceful().build();
pub const ILLUSIONER: EntityType = Builder::of(69, MobCategory::Monster).sized(0.6, 1.95).not_in_peaceful().build();
pub const INTERACTION: EntityType = Builder::of(70, MobCategory::Misc).sized(0.0, 0.0).build();
pub const IRON_GOLEM: EntityType = Builder::of(71, MobCategory::Misc).sized(1.4, 2.7).build();
pub const ITEM: EntityType = Builder::of(72, MobCategory::Misc).sized(0.25, 0.25).build();
pub const ITEM_DISPLAY: EntityType = Builder::of(73, MobCategory::Misc).sized(0.0, 0.0).build();
pub const ITEM_FRAME: EntityType = Builder::of(74, MobCategory::Misc).sized(0.5, 0.5).build();
pub const JUNGLE_BOAT: EntityType = Builder::of(75, MobCategory::Misc).sized(1.375, 0.5625).build();
pub const JUNGLE_CHEST_BOAT: EntityType = Builder::of(76, MobCategory::Misc).sized(1.375, 0.5625).build();
pub const LEASH_KNOT: EntityType = Builder::of(77, MobCategory::Misc).sized(0.375, 0.5).build();
pub const LIGHTNING_BOLT: EntityType = Builder::of(78, MobCategory::Misc).sized(0.0, 0.0).build();
pub const LLAMA: EntityType = Builder::of(79, MobCategory::Creature).sized(0.9, 1.87).build();
pub const LLAMA_SPIT: EntityType = Builder::of(80, MobCategory::Misc).sized(0.25, 0.25).build();
pub const MAGMA_CUBE: EntityType = Builder::of(81, MobCategory::Monster).sized(0.52, 0.52).fire_immune().spawn_dimensions_scale(4.0).not_in_peaceful().build();
pub const MANGROVE_BOAT: EntityType = Builder::of(82, MobCategory::Misc).sized(1.375, 0.5625).build();
pub const MANGROVE_CHEST_BOAT: EntityType = Builder::of(83, MobCategory::Misc).sized(1.375, 0.5625).build();
pub const MANNEQUIN: EntityType = Builder::of(84, MobCategory::Misc).sized(0.6, 1.8).build();
pub const MARKER: EntityType = Builder::of(85, MobCategory::Misc).sized(0.0, 0.0).build();
pub const MINECART: EntityType = Builder::of(86, MobCategory::Misc).sized(0.98, 0.7).build();
pub const MOOSHROOM: EntityType = Builder::of(87, MobCategory::Creature).sized(0.9, 1.4).build();
pub const MULE: EntityType = Builder::of(88, MobCategory::Creature).sized(1.3964844, 1.6).build();
pub const NAUTILUS: EntityType = Builder::of(89, MobCategory::WaterCreature).sized(0.875, 0.95).build();
pub const OAK_BOAT: EntityType = Builder::of(90, MobCategory::Misc).sized(1.375, 0.5625).build();
pub const OAK_CHEST_BOAT: EntityType = Builder::of(91, MobCategory::Misc).sized(1.375, 0.5625).build();
pub const OCELOT: EntityType = Builder::of(92, MobCategory::Creature).sized(0.6, 0.7).build();
pub const OMINOUS_ITEM_SPAWNER: EntityType = Builder::of(93, MobCategory::Misc).sized(0.25, 0.25).build();
pub const PAINTING: EntityType = Builder::of(94, MobCategory::Misc).sized(0.5, 0.5).build();
pub const PALE_OAK_BOAT: EntityType = Builder::of(95, MobCategory::Misc).sized(1.375, 0.5625).build();
pub const PALE_OAK_CHEST_BOAT: EntityType = Builder::of(96, MobCategory::Misc).sized(1.375, 0.5625).build();
pub const PANDA: EntityType = Builder::of(97, MobCategory::Creature).sized(1.3, 1.25).build();
pub const PARCHED: EntityType = Builder::of(98, MobCategory::Monster).sized(0.6, 1.99).not_in_peaceful().build();
pub const PARROT: EntityType = Builder::of(99, MobCategory::Creature).sized(0.5, 0.9).build();
pub const PHANTOM: EntityType = Builder::of(100, MobCategory::Monster).sized(0.9, 0.5).not_in_peaceful().build();
pub const PIG: EntityType = Builder::of(101, MobCategory::Creature).sized(0.9, 0.9).build();
pub const PIGLIN: EntityType = Builder::of(102, MobCategory::Monster).sized(0.6, 1.95).build();
pub const PIGLIN_BRUTE: EntityType = Builder::of(103, MobCategory::Monster).sized(0.6, 1.95).not_in_peaceful().build();
pub const PILLAGER: EntityType = Builder::of(104, MobCategory::Monster).sized(0.6, 1.95).can_spawn_far_from_player().not_in_peaceful().build();
pub const POLAR_BEAR: EntityType = Builder::of(105, MobCategory::Creature).sized(1.4, 1.4).immune_to(|| &block_tags::POLAR_BEAR_IMMUNE_TO).build();
pub const POPLAR_BOAT: EntityType = Builder::of(106, MobCategory::Misc).sized(1.375, 0.5625).build();
pub const POPLAR_CHEST_BOAT: EntityType = Builder::of(107, MobCategory::Misc).sized(1.375, 0.5625).build();
pub const SPLASH_POTION: EntityType = Builder::of(108, MobCategory::Misc).sized(0.25, 0.25).build();
pub const LINGERING_POTION: EntityType = Builder::of(109, MobCategory::Misc).sized(0.25, 0.25).build();
pub const PUFFERFISH: EntityType = Builder::of(110, MobCategory::WaterAmbient).sized(0.7, 0.7).build();
pub const RABBIT: EntityType = Builder::of(111, MobCategory::Creature).sized(0.49, 0.6).build();
pub const RAVAGER: EntityType = Builder::of(112, MobCategory::Monster).sized(1.95, 2.2).not_in_peaceful().build();
pub const SALMON: EntityType = Builder::of(113, MobCategory::WaterAmbient).sized(0.7, 0.4).build();
pub const SHEEP: EntityType = Builder::of(114, MobCategory::Creature).sized(0.9, 1.3).build();
pub const SHULKER: EntityType = Builder::of(115, MobCategory::Monster).sized(1.0, 1.0).fire_immune().can_spawn_far_from_player().build();
pub const SHULKER_BULLET: EntityType = Builder::of(116, MobCategory::Misc).sized(0.3125, 0.3125).build();
pub const SILVERFISH: EntityType = Builder::of(117, MobCategory::Monster).sized(0.4, 0.3).not_in_peaceful().build();
pub const SKELETON: EntityType = Builder::of(118, MobCategory::Monster).sized(0.6, 1.99).not_in_peaceful().build();
pub const SKELETON_HORSE: EntityType = Builder::of(119, MobCategory::Creature).sized(1.3964844, 1.6).build();
pub const SLIME: EntityType = Builder::of(120, MobCategory::Monster).sized(0.52, 0.52).spawn_dimensions_scale(4.0).not_in_peaceful().build();
pub const SMALL_FIREBALL: EntityType = Builder::of(121, MobCategory::Misc).sized(0.3125, 0.3125).build();
pub const SNIFFER: EntityType = Builder::of(122, MobCategory::Creature).sized(1.9, 1.75).build();
pub const SNOWBALL: EntityType = Builder::of(123, MobCategory::Misc).sized(0.25, 0.25).build();
pub const SNOW_GOLEM: EntityType = Builder::of(124, MobCategory::Misc).sized(0.7, 1.9).immune_to(|| &block_tags::SNOW_GOLEM_IMMUNE_TO).build();
pub const SPAWNER_MINECART: EntityType = Builder::of(125, MobCategory::Misc).sized(0.98, 0.7).build();
pub const SPECTRAL_ARROW: EntityType = Builder::of(126, MobCategory::Misc).sized(0.5, 0.5).build();
pub const SPIDER: EntityType = Builder::of(127, MobCategory::Monster).sized(1.4, 0.9).not_in_peaceful().build();
pub const SPRUCE_BOAT: EntityType = Builder::of(128, MobCategory::Misc).sized(1.375, 0.5625).build();
pub const SPRUCE_CHEST_BOAT: EntityType = Builder::of(129, MobCategory::Misc).sized(1.375, 0.5625).build();
pub const SQUID: EntityType = Builder::of(130, MobCategory::WaterCreature).sized(0.8, 0.8).build();
pub const STRAY: EntityType = Builder::of(131, MobCategory::Monster).sized(0.6, 1.99).immune_to(|| &block_tags::STRAY_IMMUNE_TO).not_in_peaceful().build();
pub const STRIDER: EntityType = Builder::of(132, MobCategory::Creature).sized(0.9, 1.7).fire_immune().build();
pub const SULFUR_CUBE: EntityType = Builder::of(133, MobCategory::Monster).sized(0.49, 0.49).spawn_dimensions_scale(2.0).build();
pub const TADPOLE: EntityType = Builder::of(134, MobCategory::Creature).sized(0.4, 0.3).build();
pub const TEXT_DISPLAY: EntityType = Builder::of(135, MobCategory::Misc).sized(0.0, 0.0).build();
pub const TNT: EntityType = Builder::of(136, MobCategory::Misc).sized(0.98, 0.98).fire_immune().build();
pub const TNT_MINECART: EntityType = Builder::of(137, MobCategory::Misc).sized(0.98, 0.7).build();
pub const TRADER_LLAMA: EntityType = Builder::of(138, MobCategory::Creature).sized(0.9, 1.87).build();
pub const TRIDENT: EntityType = Builder::of(139, MobCategory::Misc).sized(0.5, 0.5).build();
pub const TROPICAL_FISH: EntityType = Builder::of(140, MobCategory::WaterAmbient).sized(0.5, 0.4).build();
pub const TURTLE: EntityType = Builder::of(141, MobCategory::Creature).sized(1.2, 0.4).build();
pub const VEX: EntityType = Builder::of(142, MobCategory::Monster).sized(0.4, 0.8).fire_immune().not_in_peaceful().build();
pub const VILLAGER: EntityType = Builder::of(143, MobCategory::Misc).sized(0.6, 1.95).build();
pub const VINDICATOR: EntityType = Builder::of(144, MobCategory::Monster).sized(0.6, 1.95).not_in_peaceful().build();
pub const WANDERING_TRADER: EntityType = Builder::of(145, MobCategory::Creature).sized(0.6, 1.95).build();
pub const WARDEN: EntityType = Builder::of(146, MobCategory::Monster).sized(0.9, 2.9).fire_immune().not_in_peaceful().build();
pub const WIND_CHARGE: EntityType = Builder::of(147, MobCategory::Misc).sized(0.3125, 0.3125).build();
pub const WITCH: EntityType = Builder::of(148, MobCategory::Monster).sized(0.6, 1.95).not_in_peaceful().build();
pub const WITHER: EntityType = Builder::of(149, MobCategory::Monster).sized(0.9, 3.5).fire_immune().immune_to(|| &block_tags::WITHER_IMMUNE_TO).not_in_peaceful().build();
pub const WITHER_SKELETON: EntityType = Builder::of(150, MobCategory::Monster).sized(0.7, 2.4).fire_immune().immune_to(|| &block_tags::WITHER_SKELETON_IMMUNE_TO).not_in_peaceful().build();
pub const WITHER_SKULL: EntityType = Builder::of(151, MobCategory::Misc).sized(0.3125, 0.3125).build();
pub const WOLF: EntityType = Builder::of(152, MobCategory::Creature).sized(0.6, 0.85).build();
pub const ZOGLIN: EntityType = Builder::of(153, MobCategory::Monster).sized(1.3964844, 1.4).fire_immune().not_in_peaceful().build();
pub const ZOMBIE: EntityType = Builder::of(154, MobCategory::Monster).sized(0.6, 1.95).not_in_peaceful().build();
pub const ZOMBIE_HORSE: EntityType = Builder::of(155, MobCategory::Monster).sized(1.3964844, 1.6).build();
pub const ZOMBIE_NAUTILUS: EntityType = Builder::of(156, MobCategory::Monster).sized(0.875, 0.95).build();
pub const ZOMBIE_VILLAGER: EntityType = Builder::of(157, MobCategory::Monster).sized(0.6, 1.95).not_in_peaceful().build();
pub const ZOMBIFIED_PIGLIN: EntityType = Builder::of(158, MobCategory::Monster).sized(0.6, 1.95).fire_immune().not_in_peaceful().build();
pub const PLAYER: EntityType = Builder::of(159, MobCategory::Misc).sized(0.6, 1.8).no_summon().build();
pub const FISHING_BOBBER: EntityType = Builder::of(160, MobCategory::Misc).sized(0.25, 0.25).no_summon().build();

pub static BY_ID: [&EntityType; 161] = [
    &ACACIA_BOAT,
    &ACACIA_CHEST_BOAT,
    &ALLAY,
    &AREA_EFFECT_CLOUD,
    &ARMADILLO,
    &ARMOR_STAND,
    &ARROW,
    &AXOLOTL,
    &BAMBOO_CHEST_RAFT,
    &BAMBOO_RAFT,
    &BAT,
    &BEE,
    &BIRCH_BOAT,
    &BIRCH_CHEST_BOAT,
    &BLAZE,
    &BLOCK_DISPLAY,
    &BOGGED,
    &BREEZE,
    &BREEZE_WIND_CHARGE,
    &CAMEL,
    &CAMEL_HUSK,
    &CAT,
    &CAVE_SPIDER,
    &CHERRY_BOAT,
    &CHERRY_CHEST_BOAT,
    &CHEST_MINECART,
    &CHICKEN,
    &COD,
    &COPPER_GOLEM,
    &COMMAND_BLOCK_MINECART,
    &COW,
    &CREAKING,
    &CREEPER,
    &CUSHION,
    &DARK_OAK_BOAT,
    &DARK_OAK_CHEST_BOAT,
    &DOLPHIN,
    &DONKEY,
    &DRAGON_FIREBALL,
    &DROWNED,
    &EGG,
    &ELDER_GUARDIAN,
    &ENDERMAN,
    &ENDERMITE,
    &ENDER_DRAGON,
    &ENDER_PEARL,
    &END_CRYSTAL,
    &EVOKER,
    &EVOKER_FANGS,
    &EXPERIENCE_BOTTLE,
    &EXPERIENCE_ORB,
    &EYE_OF_ENDER,
    &FALLING_BLOCK,
    &FIREBALL,
    &FIREWORK_ROCKET,
    &FOX,
    &FROG,
    &FURNACE_MINECART,
    &GHAST,
    &HAPPY_GHAST,
    &GIANT,
    &GLOW_ITEM_FRAME,
    &GLOW_SQUID,
    &GOAT,
    &GUARDIAN,
    &HOGLIN,
    &HOPPER_MINECART,
    &HORSE,
    &HUSK,
    &ILLUSIONER,
    &INTERACTION,
    &IRON_GOLEM,
    &ITEM,
    &ITEM_DISPLAY,
    &ITEM_FRAME,
    &JUNGLE_BOAT,
    &JUNGLE_CHEST_BOAT,
    &LEASH_KNOT,
    &LIGHTNING_BOLT,
    &LLAMA,
    &LLAMA_SPIT,
    &MAGMA_CUBE,
    &MANGROVE_BOAT,
    &MANGROVE_CHEST_BOAT,
    &MANNEQUIN,
    &MARKER,
    &MINECART,
    &MOOSHROOM,
    &MULE,
    &NAUTILUS,
    &OAK_BOAT,
    &OAK_CHEST_BOAT,
    &OCELOT,
    &OMINOUS_ITEM_SPAWNER,
    &PAINTING,
    &PALE_OAK_BOAT,
    &PALE_OAK_CHEST_BOAT,
    &PANDA,
    &PARCHED,
    &PARROT,
    &PHANTOM,
    &PIG,
    &PIGLIN,
    &PIGLIN_BRUTE,
    &PILLAGER,
    &POLAR_BEAR,
    &POPLAR_BOAT,
    &POPLAR_CHEST_BOAT,
    &SPLASH_POTION,
    &LINGERING_POTION,
    &PUFFERFISH,
    &RABBIT,
    &RAVAGER,
    &SALMON,
    &SHEEP,
    &SHULKER,
    &SHULKER_BULLET,
    &SILVERFISH,
    &SKELETON,
    &SKELETON_HORSE,
    &SLIME,
    &SMALL_FIREBALL,
    &SNIFFER,
    &SNOWBALL,
    &SNOW_GOLEM,
    &SPAWNER_MINECART,
    &SPECTRAL_ARROW,
    &SPIDER,
    &SPRUCE_BOAT,
    &SPRUCE_CHEST_BOAT,
    &SQUID,
    &STRAY,
    &STRIDER,
    &SULFUR_CUBE,
    &TADPOLE,
    &TEXT_DISPLAY,
    &TNT,
    &TNT_MINECART,
    &TRADER_LLAMA,
    &TRIDENT,
    &TROPICAL_FISH,
    &TURTLE,
    &VEX,
    &VILLAGER,
    &VINDICATOR,
    &WANDERING_TRADER,
    &WARDEN,
    &WIND_CHARGE,
    &WITCH,
    &WITHER,
    &WITHER_SKELETON,
    &WITHER_SKULL,
    &WOLF,
    &ZOGLIN,
    &ZOMBIE,
    &ZOMBIE_HORSE,
    &ZOMBIE_NAUTILUS,
    &ZOMBIE_VILLAGER,
    &ZOMBIFIED_PIGLIN,
    &PLAYER,
    &FISHING_BOBBER,
];
