use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::{ENTITY, ITEM_STACK};

static ID_MAP: [(&str, &str); 75] = [
    ("AreaEffectCloud", "minecraft:area_effect_cloud"),
    ("ArmorStand", "minecraft:armor_stand"),
    ("Arrow", "minecraft:arrow"),
    ("Bat", "minecraft:bat"),
    ("Blaze", "minecraft:blaze"),
    ("Boat", "minecraft:boat"),
    ("CaveSpider", "minecraft:cave_spider"),
    ("Chicken", "minecraft:chicken"),
    ("Cow", "minecraft:cow"),
    ("Creeper", "minecraft:creeper"),
    ("Donkey", "minecraft:donkey"),
    ("DragonFireball", "minecraft:dragon_fireball"),
    ("ElderGuardian", "minecraft:elder_guardian"),
    ("EnderCrystal", "minecraft:ender_crystal"),
    ("EnderDragon", "minecraft:ender_dragon"),
    ("Enderman", "minecraft:enderman"),
    ("Endermite", "minecraft:endermite"),
    ("EyeOfEnderSignal", "minecraft:eye_of_ender_signal"),
    ("FallingSand", "minecraft:falling_block"),
    ("Fireball", "minecraft:fireball"),
    ("FireworksRocketEntity", "minecraft:fireworks_rocket"),
    ("Ghast", "minecraft:ghast"),
    ("Giant", "minecraft:giant"),
    ("Guardian", "minecraft:guardian"),
    ("Horse", "minecraft:horse"),
    ("Husk", "minecraft:husk"),
    ("Item", "minecraft:item"),
    ("ItemFrame", "minecraft:item_frame"),
    ("LavaSlime", "minecraft:magma_cube"),
    ("LeashKnot", "minecraft:leash_knot"),
    ("MinecartChest", "minecraft:chest_minecart"),
    ("MinecartCommandBlock", "minecraft:commandblock_minecart"),
    ("MinecartFurnace", "minecraft:furnace_minecart"),
    ("MinecartHopper", "minecraft:hopper_minecart"),
    ("MinecartRideable", "minecraft:minecart"),
    ("MinecartSpawner", "minecraft:spawner_minecart"),
    ("MinecartTNT", "minecraft:tnt_minecart"),
    ("Mule", "minecraft:mule"),
    ("MushroomCow", "minecraft:mooshroom"),
    ("Ozelot", "minecraft:ocelot"),
    ("Painting", "minecraft:painting"),
    ("Pig", "minecraft:pig"),
    ("PigZombie", "minecraft:zombie_pigman"),
    ("PolarBear", "minecraft:polar_bear"),
    ("PrimedTnt", "minecraft:tnt"),
    ("Rabbit", "minecraft:rabbit"),
    ("Sheep", "minecraft:sheep"),
    ("Shulker", "minecraft:shulker"),
    ("ShulkerBullet", "minecraft:shulker_bullet"),
    ("Silverfish", "minecraft:silverfish"),
    ("Skeleton", "minecraft:skeleton"),
    ("SkeletonHorse", "minecraft:skeleton_horse"),
    ("Slime", "minecraft:slime"),
    ("SmallFireball", "minecraft:small_fireball"),
    ("SnowMan", "minecraft:snowman"),
    ("Snowball", "minecraft:snowball"),
    ("SpectralArrow", "minecraft:spectral_arrow"),
    ("Spider", "minecraft:spider"),
    ("Squid", "minecraft:squid"),
    ("Stray", "minecraft:stray"),
    ("ThrownEgg", "minecraft:egg"),
    ("ThrownEnderpearl", "minecraft:ender_pearl"),
    ("ThrownExpBottle", "minecraft:xp_bottle"),
    ("ThrownPotion", "minecraft:potion"),
    ("Villager", "minecraft:villager"),
    ("VillagerGolem", "minecraft:villager_golem"),
    ("Witch", "minecraft:witch"),
    ("WitherBoss", "minecraft:wither"),
    ("WitherSkeleton", "minecraft:wither_skeleton"),
    ("WitherSkull", "minecraft:wither_skull"),
    ("Wolf", "minecraft:wolf"),
    ("XPOrb", "minecraft:xp_orb"),
    ("Zombie", "minecraft:zombie"),
    ("ZombieHorse", "minecraft:zombie_horse"),
    ("ZombieVillager", "minecraft:zombie_villager"),
];

pub struct EntityIdFix {
    base: DataFixBase,
}

impl EntityIdFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> EntityIdFix {
        EntityIdFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for EntityIdFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let old_type = self.get_input_schema().find_choice_type(ENTITY);
        let new_type = self.get_output_schema().find_choice_type(ENTITY);
        let old_item_stack_type = self.get_input_schema().get_type(ITEM_STACK);
        let new_item_stack_type = self.get_output_schema().get_type(ITEM_STACK);
        TypeRewriteRule::seq(vec![
            self.convert_unchecked("item stack entity name hook converter", old_item_stack_type, new_item_stack_type),
            self.fix_type_everywhere_to("EntityIdFix", old_type, new_type, |input| match input {
                Value::Choice(id, value) => Value::Choice(ID_MAP.iter().find(|(old, _)| *old == id).map_or(id, |(_, new)| new.to_string()), value),
                other => other,
            }),
        ])
    }
}
