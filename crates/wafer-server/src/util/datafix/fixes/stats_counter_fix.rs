use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::block_state_data;
use crate::util::datafix::fixes::item_stack_the_flattening_fix;
use crate::util::datafix::fixes::references::{OBJECTIVE, STATS};
use crate::util::datafix::schemas::v1451_6;

const SPECIAL_OBJECTIVE_CRITERIA: [&str; 43] = [
    "dummy",
    "trigger",
    "deathCount",
    "playerKillCount",
    "totalKillCount",
    "health",
    "food",
    "air",
    "armor",
    "xp",
    "level",
    "killedByTeam.aqua",
    "killedByTeam.black",
    "killedByTeam.blue",
    "killedByTeam.dark_aqua",
    "killedByTeam.dark_blue",
    "killedByTeam.dark_gray",
    "killedByTeam.dark_green",
    "killedByTeam.dark_purple",
    "killedByTeam.dark_red",
    "killedByTeam.gold",
    "killedByTeam.gray",
    "killedByTeam.green",
    "killedByTeam.light_purple",
    "killedByTeam.red",
    "killedByTeam.white",
    "killedByTeam.yellow",
    "teamkill.aqua",
    "teamkill.black",
    "teamkill.blue",
    "teamkill.dark_aqua",
    "teamkill.dark_blue",
    "teamkill.dark_gray",
    "teamkill.dark_green",
    "teamkill.dark_purple",
    "teamkill.dark_red",
    "teamkill.gold",
    "teamkill.gray",
    "teamkill.green",
    "teamkill.light_purple",
    "teamkill.red",
    "teamkill.white",
    "teamkill.yellow",
];

const SKIP: [&str; 5] = [
    "stat.craftItem.minecraft.spawn_egg",
    "stat.useItem.minecraft.spawn_egg",
    "stat.breakItem.minecraft.spawn_egg",
    "stat.pickup.minecraft.spawn_egg",
    "stat.drop.minecraft.spawn_egg",
];

const BLOCK_KEY: &str = "stat.mineBlock";
const NEW_BLOCK_KEY: &str = "minecraft:mined";
const NEW_CUSTOM_KEY: &str = "minecraft:custom";

fn custom_map(key: &str) -> Option<&'static str> {
    Some(match key {
        "stat.leaveGame" => "minecraft:leave_game",
        "stat.playOneMinute" => "minecraft:play_one_minute",
        "stat.timeSinceDeath" => "minecraft:time_since_death",
        "stat.sneakTime" => "minecraft:sneak_time",
        "stat.walkOneCm" => "minecraft:walk_one_cm",
        "stat.crouchOneCm" => "minecraft:crouch_one_cm",
        "stat.sprintOneCm" => "minecraft:sprint_one_cm",
        "stat.swimOneCm" => "minecraft:swim_one_cm",
        "stat.fallOneCm" => "minecraft:fall_one_cm",
        "stat.climbOneCm" => "minecraft:climb_one_cm",
        "stat.flyOneCm" => "minecraft:fly_one_cm",
        "stat.diveOneCm" => "minecraft:dive_one_cm",
        "stat.minecartOneCm" => "minecraft:minecart_one_cm",
        "stat.boatOneCm" => "minecraft:boat_one_cm",
        "stat.pigOneCm" => "minecraft:pig_one_cm",
        "stat.horseOneCm" => "minecraft:horse_one_cm",
        "stat.aviateOneCm" => "minecraft:aviate_one_cm",
        "stat.jump" => "minecraft:jump",
        "stat.drop" => "minecraft:drop",
        "stat.damageDealt" => "minecraft:damage_dealt",
        "stat.damageTaken" => "minecraft:damage_taken",
        "stat.deaths" => "minecraft:deaths",
        "stat.mobKills" => "minecraft:mob_kills",
        "stat.animalsBred" => "minecraft:animals_bred",
        "stat.playerKills" => "minecraft:player_kills",
        "stat.fishCaught" => "minecraft:fish_caught",
        "stat.talkedToVillager" => "minecraft:talked_to_villager",
        "stat.tradedWithVillager" => "minecraft:traded_with_villager",
        "stat.cakeSlicesEaten" => "minecraft:eat_cake_slice",
        "stat.cauldronFilled" => "minecraft:fill_cauldron",
        "stat.cauldronUsed" => "minecraft:use_cauldron",
        "stat.armorCleaned" => "minecraft:clean_armor",
        "stat.bannerCleaned" => "minecraft:clean_banner",
        "stat.brewingstandInteraction" => "minecraft:interact_with_brewingstand",
        "stat.beaconInteraction" => "minecraft:interact_with_beacon",
        "stat.dropperInspected" => "minecraft:inspect_dropper",
        "stat.hopperInspected" => "minecraft:inspect_hopper",
        "stat.dispenserInspected" => "minecraft:inspect_dispenser",
        "stat.noteblockPlayed" => "minecraft:play_noteblock",
        "stat.noteblockTuned" => "minecraft:tune_noteblock",
        "stat.flowerPotted" => "minecraft:pot_flower",
        "stat.trappedChestTriggered" => "minecraft:trigger_trapped_chest",
        "stat.enderchestOpened" => "minecraft:open_enderchest",
        "stat.itemEnchanted" => "minecraft:enchant_item",
        "stat.recordPlayed" => "minecraft:play_record",
        "stat.furnaceInteraction" => "minecraft:interact_with_furnace",
        "stat.craftingTableInteraction" => "minecraft:interact_with_crafting_table",
        "stat.chestOpened" => "minecraft:open_chest",
        "stat.sleepInBed" => "minecraft:sleep_in_bed",
        "stat.shulkerBoxOpened" => "minecraft:open_shulker_box",
        _ => return None,
    })
}

fn item_keys(prefix: &str) -> Option<&'static str> {
    Some(match prefix {
        "stat.craftItem" => "minecraft:crafted",
        "stat.useItem" => "minecraft:used",
        "stat.breakItem" => "minecraft:broken",
        "stat.pickup" => "minecraft:picked_up",
        "stat.drop" => "minecraft:dropped",
        _ => return None,
    })
}

fn entity_keys(prefix: &str) -> Option<&'static str> {
    Some(match prefix {
        "stat.entityKilledBy" => "minecraft:killed_by",
        "stat.killEntity" => "minecraft:killed",
        _ => return None,
    })
}

fn entities(old_entity: &str) -> Option<&'static str> {
    Some(match old_entity {
        "Bat" => "minecraft:bat",
        "Blaze" => "minecraft:blaze",
        "CaveSpider" => "minecraft:cave_spider",
        "Chicken" => "minecraft:chicken",
        "Cow" => "minecraft:cow",
        "Creeper" => "minecraft:creeper",
        "Donkey" => "minecraft:donkey",
        "ElderGuardian" => "minecraft:elder_guardian",
        "Enderman" => "minecraft:enderman",
        "Endermite" => "minecraft:endermite",
        "EvocationIllager" => "minecraft:evocation_illager",
        "Ghast" => "minecraft:ghast",
        "Guardian" => "minecraft:guardian",
        "Horse" => "minecraft:horse",
        "Husk" => "minecraft:husk",
        "Llama" => "minecraft:llama",
        "LavaSlime" => "minecraft:magma_cube",
        "MushroomCow" => "minecraft:mooshroom",
        "Mule" => "minecraft:mule",
        "Ozelot" => "minecraft:ocelot",
        "Parrot" => "minecraft:parrot",
        "Pig" => "minecraft:pig",
        "PolarBear" => "minecraft:polar_bear",
        "Rabbit" => "minecraft:rabbit",
        "Sheep" => "minecraft:sheep",
        "Shulker" => "minecraft:shulker",
        "Silverfish" => "minecraft:silverfish",
        "SkeletonHorse" => "minecraft:skeleton_horse",
        "Skeleton" => "minecraft:skeleton",
        "Slime" => "minecraft:slime",
        "Spider" => "minecraft:spider",
        "Squid" => "minecraft:squid",
        "Stray" => "minecraft:stray",
        "Vex" => "minecraft:vex",
        "Villager" => "minecraft:villager",
        "VindicationIllager" => "minecraft:vindication_illager",
        "Witch" => "minecraft:witch",
        "WitherSkeleton" => "minecraft:wither_skeleton",
        "Wolf" => "minecraft:wolf",
        "ZombieHorse" => "minecraft:zombie_horse",
        "PigZombie" => "minecraft:zombie_pigman",
        "ZombieVillager" => "minecraft:zombie_villager",
        "Zombie" => "minecraft:zombie",
        _ => return None,
    })
}

struct StatType {
    ty: &'static str,
    type_key: String,
}

pub struct StatsCounterFix {
    base: DataFixBase,
}

impl StatsCounterFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> StatsCounterFix {
        StatsCounterFix { base: DataFixBase::new(output_schema, changes_type) }
    }

    fn make_stat_fixer(&self) -> TypeRewriteRule {
        let input_type = self.get_input_schema().get_type(STATS);
        let output_type = self.get_output_schema().get_type(STATS);
        self.fix_type_everywhere_typed_to("StatsCounterFix", input_type, output_type.clone(), move |input| {
            let tag = input.get_remainder();
            let mut stats: Vec<(&'static str, Dynamic)> = Vec::new();
            if let Ok(map) = tag.get_map_values().into_result() {
                for (entry_key, entry_value) in map {
                    if entry_value.as_number().into_result().is_ok() {
                        let key = entry_key.as_string_or("");
                        if let Some(stat_type) = unpack_legacy_key(&key) {
                            let position = match stats.iter().position(|(ty, _)| *ty == stat_type.ty) {
                                Some(position) => position,
                                None => {
                                    stats.push((stat_type.ty, tag.empty_map()));
                                    stats.len() - 1
                                }
                            };
                            let element = std::mem::take(&mut stats[position].1);
                            stats[position].1 = element.set(&stat_type.type_key, entry_value);
                        }
                    }
                }
            }
            let stats_map = tag.create_map(stats.into_iter().map(|(ty, element)| (tag.create_string(ty), element)).collect::<Vec<_>>());
            let root = tag.empty_map();
            crate::util::read_typed_or_throw(&output_type, root.set("stats", stats_map), false)
        })
    }

    fn make_objective_fixer(&self) -> TypeRewriteRule {
        let input_type = self.get_input_schema().get_type(OBJECTIVE);
        let output_type = self.get_output_schema().get_type(OBJECTIVE);
        self.fix_type_everywhere_typed_to("ObjectiveStatFix", input_type, output_type.clone(), move |input| {
            let tag = input.get_remainder();
            let updated_tag = tag.update("CriteriaName", |name| match name.as_string().into_result() {
                Ok(key) => {
                    let updated = if SPECIAL_OBJECTIVE_CRITERIA.contains(&key.as_str()) {
                        key
                    } else {
                        match unpack_legacy_key(&key) {
                            None => "dummy".to_string(),
                            Some(stat_type) => format!("{}:{}", v1451_6::pack_namespaced_with_dot(stat_type.ty), v1451_6::pack_namespaced_with_dot(&stat_type.type_key)),
                        }
                    };
                    name.create_string(updated)
                }
                Err(_) => name,
            });
            crate::util::read_typed_or_throw(&output_type, updated_tag, false)
        })
    }
}

fn unpack_legacy_key(key: &str) -> Option<StatType> {
    if SKIP.contains(&key) {
        return None;
    }
    if let Some(custom_key) = custom_map(key) {
        return Some(StatType { ty: NEW_CUSTOM_KEY, type_key: custom_key.to_string() });
    }
    let split_index = key.match_indices('.').nth(1).map(|(index, _)| index)?;
    let prefix = &key[..split_index];
    if prefix == BLOCK_KEY {
        let new_key = upgrade_block(&key[split_index + 1..].replace('.', ":"));
        return Some(StatType { ty: NEW_BLOCK_KEY, type_key: new_key });
    }
    if let Some(item_key) = item_keys(prefix) {
        let old_item = key[split_index + 1..].replace('.', ":");
        let new_key = upgrade_item(&old_item).map_or(old_item, str::to_string);
        return Some(StatType { ty: item_key, type_key: new_key });
    }
    if let Some(entity_key) = entity_keys(prefix) {
        let old_entity = key[split_index + 1..].replace('.', ":");
        let new_key = entities(&old_entity).map_or(old_entity, str::to_string);
        return Some(StatType { ty: entity_key, type_key: new_key });
    }
    None
}

fn upgrade_item(name: &str) -> Option<&'static str> {
    item_stack_the_flattening_fix::update_item(name, 0)
}

fn upgrade_block(name: &str) -> String {
    block_state_data::upgrade_block(name)
}

impl DataFix for StatsCounterFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        TypeRewriteRule::seq(vec![self.make_stat_fixer(), self.make_objective_fixer()])
    }
}
