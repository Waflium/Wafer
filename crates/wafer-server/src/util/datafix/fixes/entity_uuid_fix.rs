use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::abstract_uuid_fix::{AbstractUUIDFix, NamedChoiceUpdate, create_uuid_from_ml, replace_uuid_least_most, replace_uuid_ml_tag, replace_uuid_string};
use crate::util::datafix::fixes::references::ENTITY;

const ABSTRACT_HORSES: [&str; 7] = [
    "minecraft:donkey",
    "minecraft:horse",
    "minecraft:llama",
    "minecraft:mule",
    "minecraft:skeleton_horse",
    "minecraft:trader_llama",
    "minecraft:zombie_horse",
];
const TAMEABLE_ANIMALS: [&str; 3] = ["minecraft:cat", "minecraft:parrot", "minecraft:wolf"];
const ANIMALS: [&str; 13] = [
    "minecraft:bee",
    "minecraft:chicken",
    "minecraft:cow",
    "minecraft:fox",
    "minecraft:mooshroom",
    "minecraft:ocelot",
    "minecraft:panda",
    "minecraft:pig",
    "minecraft:polar_bear",
    "minecraft:rabbit",
    "minecraft:sheep",
    "minecraft:turtle",
    "minecraft:hoglin",
];
const MOBS: [&str; 44] = [
    "minecraft:bat",
    "minecraft:blaze",
    "minecraft:cave_spider",
    "minecraft:cod",
    "minecraft:creeper",
    "minecraft:dolphin",
    "minecraft:drowned",
    "minecraft:elder_guardian",
    "minecraft:ender_dragon",
    "minecraft:enderman",
    "minecraft:endermite",
    "minecraft:evoker",
    "minecraft:ghast",
    "minecraft:giant",
    "minecraft:guardian",
    "minecraft:husk",
    "minecraft:illusioner",
    "minecraft:magma_cube",
    "minecraft:pufferfish",
    "minecraft:zombified_piglin",
    "minecraft:salmon",
    "minecraft:shulker",
    "minecraft:silverfish",
    "minecraft:skeleton",
    "minecraft:slime",
    "minecraft:snow_golem",
    "minecraft:spider",
    "minecraft:squid",
    "minecraft:stray",
    "minecraft:tropical_fish",
    "minecraft:vex",
    "minecraft:villager",
    "minecraft:iron_golem",
    "minecraft:vindicator",
    "minecraft:pillager",
    "minecraft:wandering_trader",
    "minecraft:witch",
    "minecraft:wither",
    "minecraft:wither_skeleton",
    "minecraft:zombie",
    "minecraft:zombie_villager",
    "minecraft:phantom",
    "minecraft:ravager",
    "minecraft:piglin",
];
const LIVING_ENTITIES: [&str; 1] = ["minecraft:armor_stand"];
const PROJECTILES: [&str; 14] = [
    "minecraft:arrow",
    "minecraft:dragon_fireball",
    "minecraft:firework_rocket",
    "minecraft:fireball",
    "minecraft:llama_spit",
    "minecraft:small_fireball",
    "minecraft:snowball",
    "minecraft:spectral_arrow",
    "minecraft:egg",
    "minecraft:ender_pearl",
    "minecraft:experience_bottle",
    "minecraft:potion",
    "minecraft:trident",
    "minecraft:wither_skull",
];

pub struct EntityUUIDFix {
    abstract_uuid_fix: AbstractUUIDFix,
}

impl EntityUUIDFix {
    pub fn new(output_schema: &Arc<Schema>) -> EntityUUIDFix {
        EntityUUIDFix { abstract_uuid_fix: AbstractUUIDFix::new(output_schema, ENTITY) }
    }
}

impl DataFix for EntityUUIDFix {
    fn base(&self) -> &DataFixBase {
        &self.abstract_uuid_fix.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let input_schema = self.get_input_schema();
        let output_schema = self.get_output_schema();
        let mut updates: Vec<(NamedChoiceUpdate, fn(Dynamic) -> Dynamic)> = Vec::new();
        let mut add = |name: &str, function: fn(Dynamic) -> Dynamic| updates.push((NamedChoiceUpdate::new(input_schema, output_schema, ENTITY, name), function));
        for name in ABSTRACT_HORSES {
            add(name, update_animal_owner);
        }
        for name in TAMEABLE_ANIMALS {
            add(name, update_animal_owner);
        }
        for name in ANIMALS {
            add(name, update_animal);
        }
        for name in MOBS {
            add(name, update_mob);
        }
        for name in LIVING_ENTITIES {
            add(name, update_living_entity);
        }
        for name in PROJECTILES {
            add(name, update_projectile);
        }
        add("minecraft:bee", update_hurt_by);
        add("minecraft:zombified_piglin", update_hurt_by);
        add("minecraft:fox", update_fox);
        add("minecraft:item", update_item);
        add("minecraft:shulker_bullet", update_shulker_bullet);
        add("minecraft:area_effect_cloud", update_area_effect_cloud);
        add("minecraft:zombie_villager", update_zombie_villager);
        add("minecraft:evoker_fangs", update_evoker_fangs);
        add("minecraft:piglin", update_piglin);
        self.fix_type_everywhere_typed("EntityUUIDFixes", input_schema.get_type(self.abstract_uuid_fix.type_reference), move |input| {
            let input = input.update_remainder(update_entity_uuid);
            updates.iter().fold(input, |input, (update, function)| update.update(input, function))
        })
    }
}

fn update_piglin(tag: Dynamic) -> Dynamic {
    tag.update("Brain", |brain| {
        brain.update("memories", |memories| {
            memories.update("minecraft:angry_at", |angry_at| match replace_uuid_string(&angry_at, "value", "value") {
                Some(replaced) => replaced,
                None => {
                    warn!("angry_at has no value.");
                    angry_at
                }
            })
        })
    })
}

fn update_evoker_fangs(tag: Dynamic) -> Dynamic {
    replace_uuid_least_most(&tag, "OwnerUUID", "Owner").unwrap_or(tag)
}

fn update_zombie_villager(tag: Dynamic) -> Dynamic {
    replace_uuid_least_most(&tag, "ConversionPlayer", "ConversionPlayer").unwrap_or(tag)
}

fn update_area_effect_cloud(tag: Dynamic) -> Dynamic {
    replace_uuid_least_most(&tag, "OwnerUUID", "Owner").unwrap_or(tag)
}

fn update_shulker_bullet(tag: Dynamic) -> Dynamic {
    let tag = replace_uuid_ml_tag(&tag, "Owner", "Owner").unwrap_or(tag);
    replace_uuid_ml_tag(&tag, "Target", "Target").unwrap_or(tag)
}

fn update_item(tag: Dynamic) -> Dynamic {
    let tag = replace_uuid_ml_tag(&tag, "Owner", "Owner").unwrap_or(tag);
    replace_uuid_ml_tag(&tag, "Thrower", "Thrower").unwrap_or(tag)
}

fn update_fox(tag: Dynamic) -> Dynamic {
    match tag.get("TrustedUUIDs").result() {
        Some(uuid_tags) => {
            let trusted = tag.create_list(uuid_tags.as_stream().into_iter().map(|uuid_tag| {
                create_uuid_from_ml(&uuid_tag).unwrap_or_else(|| {
                    warn!("Trusted contained invalid data.");
                    uuid_tag
                })
            }));
            tag.remove("TrustedUUIDs").set("Trusted", trusted)
        }
        None => tag,
    }
}

fn update_hurt_by(tag: Dynamic) -> Dynamic {
    replace_uuid_string(&tag, "HurtBy", "HurtBy").unwrap_or(tag)
}

fn update_animal_owner(tag: Dynamic) -> Dynamic {
    let fixed = update_animal(tag);
    replace_uuid_string(&fixed, "OwnerUUID", "Owner").unwrap_or(fixed)
}

fn update_animal(tag: Dynamic) -> Dynamic {
    let fixed = update_mob(tag);
    replace_uuid_least_most(&fixed, "LoveCause", "LoveCause").unwrap_or(fixed)
}

fn update_mob(tag: Dynamic) -> Dynamic {
    update_living_entity(tag).update("Leash", |leash_tag| replace_uuid_least_most(&leash_tag, "UUID", "UUID").unwrap_or(leash_tag))
}

pub fn update_living_entity(tag: Dynamic) -> Dynamic {
    tag.update("Attributes", |attributes| {
        let updated = attributes.as_stream().into_iter().map(|attribute| {
            attribute.update("Modifiers", |modifiers| {
                modifiers.create_list(modifiers.as_stream().into_iter().map(|modifier| replace_uuid_least_most(&modifier, "UUID", "UUID").unwrap_or(modifier)))
            })
        });
        attributes.create_list(updated)
    })
}

fn update_projectile(tag: Dynamic) -> Dynamic {
    match tag.get("OwnerUUID").result() {
        Some(owner) => tag.remove("OwnerUUID").set("Owner", owner),
        None => tag,
    }
}

pub fn update_entity_uuid(tag: Dynamic) -> Dynamic {
    replace_uuid_least_most(&tag, "UUID", "UUID").unwrap_or(tag)
}
