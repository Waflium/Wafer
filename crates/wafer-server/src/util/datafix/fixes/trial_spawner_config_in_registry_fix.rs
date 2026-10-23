use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

pub struct TrialSpawnerConfigInRegistryFix;

impl TrialSpawnerConfigInRegistryFix {
    pub fn new(output_schema: &Arc<Schema>) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, false, "TrialSpawnerConfigInRegistryFix", BLOCK_ENTITY, "minecraft:trial_spawner", fix)
    }
}

pub fn fix_tag(input: Dynamic) -> Dynamic {
    let Some(normal_config) = input.get("normal_config").result() else { return input };
    let Some(ominous_config) = input.get("ominous_config").result() else { return input };
    let registry_location = vanilla_trial_chambers::configs_to_key().iter().rev().find(|(normal, ominous, _)| normal_config.value.as_ref() == Some(normal) && ominous_config.value.as_ref() == Some(ominous));
    match registry_location {
        Some((_, _, location)) => {
            let normal_config = input.create_string(format!("{location}/normal"));
            let ominous_config = input.create_string(format!("{location}/ominous"));
            input.set("normal_config", normal_config).set("ominous_config", ominous_config)
        }
        None => input,
    }
}

fn fix(entity: Typed) -> Typed {
    entity.update_remainder(fix_tag)
}

mod vanilla_trial_chambers {
    use std::sync::OnceLock;

    use wafer_protocol::nbt::Nbt;

    type Config = (Nbt, Nbt, &'static str);

    pub fn configs_to_key() -> &'static [Config] {
        static CONFIGS_TO_KEY: OnceLock<Vec<Config>> = OnceLock::new();
        CONFIGS_TO_KEY.get_or_init(|| {
            let mut configs = Vec::new();
            register(&mut configs, "minecraft:trial_chamber/breeze", super::BREEZE_NORMAL, super::BREEZE_OMINOUS);
            register(&mut configs, "minecraft:trial_chamber/melee/husk", super::HUSK_NORMAL, super::HUSK_OMINOUS);
            register(&mut configs, "minecraft:trial_chamber/melee/spider", super::SPIDER_NORMAL, super::SPIDER_OMINOUS);
            register(&mut configs, "minecraft:trial_chamber/melee/zombie", super::ZOMBIE_NORMAL, super::ZOMBIE_OMINOUS);
            register(&mut configs, "minecraft:trial_chamber/ranged/poison_skeleton", super::POISON_SKELETON_NORMAL, super::POISON_SKELETON_OMINOUS);
            register(&mut configs, "minecraft:trial_chamber/ranged/skeleton", super::SKELETON_NORMAL, super::SKELETON_OMINOUS);
            register(&mut configs, "minecraft:trial_chamber/ranged/stray", super::STRAY_NORMAL, super::STRAY_OMINOUS);
            register(&mut configs, "minecraft:trial_chamber/slow_ranged/poison_skeleton", super::SLOW_POISON_SKELETON_NORMAL, super::SLOW_POISON_SKELETON_OMINOUS);
            register(&mut configs, "minecraft:trial_chamber/slow_ranged/skeleton", super::SLOW_SKELETON_NORMAL, super::SLOW_SKELETON_OMINOUS);
            register(&mut configs, "minecraft:trial_chamber/slow_ranged/stray", super::SLOW_STRAY_NORMAL, super::SLOW_STRAY_OMINOUS);
            register(&mut configs, "minecraft:trial_chamber/small_melee/baby_zombie", super::BABY_ZOMBIE_NORMAL, super::BABY_ZOMBIE_OMINOUS);
            register(&mut configs, "minecraft:trial_chamber/small_melee/cave_spider", super::CAVE_SPIDER_NORMAL, super::CAVE_SPIDER_OMINOUS);
            register(&mut configs, "minecraft:trial_chamber/small_melee/silverfish", super::SILVERFISH_NORMAL, super::SILVERFISH_OMINOUS);
            register(&mut configs, "minecraft:trial_chamber/small_melee/slime", super::SLIME_NORMAL, super::SLIME_OMINOUS);
            configs
        })
    }

    fn register(configs: &mut Vec<Config>, location: &'static str, normal_nbt: &str, ominous_nbt: &str) {
        let normal_tag = parse(normal_nbt);
        let ominous_tag = parse(ominous_nbt);
        let Nbt::Compound(mut ominous_merged_entries) = normal_tag.clone() else { unreachable!() };
        let Nbt::Compound(ominous_entries) = &ominous_tag else { unreachable!() };
        crate::nbt::merge(&mut ominous_merged_entries, ominous_entries);
        let ominous_merged_tag = Nbt::Compound(ominous_merged_entries);
        let ominous_merged_tag_defaults_omitted = remove_defaults(ominous_merged_tag.clone());
        put(configs, (normal_tag.clone(), ominous_tag, location));
        put(configs, (normal_tag.clone(), ominous_merged_tag, location));
        put(configs, (normal_tag, ominous_merged_tag_defaults_omitted, location));
    }

    fn put(configs: &mut Vec<Config>, config: Config) {
        match configs.iter_mut().find(|(normal, ominous, _)| *normal == config.0 && *ominous == config.1) {
            Some(existing) => existing.2 = config.2,
            None => configs.push(config),
        }
    }

    fn parse(nbt: &str) -> Nbt {
        match crate::nbt::parse_fully(nbt) {
            Ok(tag @ Nbt::Compound(_)) => tag,
            _ => panic!("Failed to parse Trial Spawner NBT config: {nbt}"),
        }
    }

    fn int_or(entries: &[(String, Nbt)], key: &str, default: i32) -> i32 {
        match entries.iter().find(|(name, _)| name == key) {
            Some((_, Nbt::Byte(value))) => *value as i32,
            Some((_, Nbt::Short(value))) => *value as i32,
            Some((_, Nbt::Int(value))) => *value,
            Some((_, Nbt::Long(value))) => *value as i32,
            Some((_, Nbt::Float(value))) => *value as i32,
            Some((_, Nbt::Double(value))) => *value as i32,
            _ => default,
        }
    }

    fn float_or(entries: &[(String, Nbt)], key: &str, default: f32) -> f32 {
        match entries.iter().find(|(name, _)| name == key) {
            Some((_, Nbt::Byte(value))) => *value as f32,
            Some((_, Nbt::Short(value))) => *value as f32,
            Some((_, Nbt::Int(value))) => *value as f32,
            Some((_, Nbt::Long(value))) => *value as f32,
            Some((_, Nbt::Float(value))) => *value,
            Some((_, Nbt::Double(value))) => *value as f32,
            _ => default,
        }
    }

    fn remove_defaults(tag: Nbt) -> Nbt {
        let Nbt::Compound(mut entries) = tag else { unreachable!() };
        if int_or(&entries, "spawn_range", 0) == 4 {
            entries.retain(|(name, _)| name != "spawn_range");
        }
        if float_or(&entries, "total_mobs", 0.0) == 6.0 {
            entries.retain(|(name, _)| name != "total_mobs");
        }
        if float_or(&entries, "simultaneous_mobs", 0.0) == 2.0 {
            entries.retain(|(name, _)| name != "simultaneous_mobs");
        }
        if float_or(&entries, "total_mobs_added_per_player", 0.0) == 2.0 {
            entries.retain(|(name, _)| name != "total_mobs_added_per_player");
        }
        if float_or(&entries, "simultaneous_mobs_added_per_player", 0.0) == 1.0 {
            entries.retain(|(name, _)| name != "simultaneous_mobs_added_per_player");
        }
        if int_or(&entries, "ticks_between_spawn", 0) == 40 {
            entries.retain(|(name, _)| name != "ticks_between_spawn");
        }
        Nbt::Compound(entries)
    }
}

const BREEZE_NORMAL: &str = r##"{simultaneous_mobs: 1.0f, simultaneous_mobs_added_per_player: 0.5f, spawn_potentials: [{data: {entity: {id: "minecraft:breeze"}}, weight: 1}], ticks_between_spawn: 20, total_mobs: 2.0f, total_mobs_added_per_player: 1.0f}"##;
const BREEZE_OMINOUS: &str = r##"{loot_tables_to_eject: [{data: "minecraft:spawners/ominous/trial_chamber/key", weight: 3}, {data: "minecraft:spawners/ominous/trial_chamber/consumables", weight: 7}], simultaneous_mobs: 2.0f, total_mobs: 4.0f}"##;
const HUSK_NORMAL: &str = r##"{simultaneous_mobs: 3.0f, simultaneous_mobs_added_per_player: 0.5f, spawn_potentials: [{data: {entity: {id: "minecraft:husk"}}, weight: 1}], ticks_between_spawn: 20}"##;
const HUSK_OMINOUS: &str = r##"{loot_tables_to_eject: [{data: "minecraft:spawners/ominous/trial_chamber/key", weight: 3}, {data: "minecraft:spawners/ominous/trial_chamber/consumables", weight: 7}], spawn_potentials: [{data: {entity: {id: "minecraft:husk"}, equipment: {loot_table: "minecraft:equipment/trial_chamber_melee", slot_drop_chances: 0.0f}}, weight: 1}]}"##;
const SPIDER_NORMAL: &str = r##"{simultaneous_mobs: 3.0f, simultaneous_mobs_added_per_player: 0.5f, spawn_potentials: [{data: {entity: {id: "minecraft:spider"}}, weight: 1}], ticks_between_spawn: 20}"##;
const SPIDER_OMINOUS: &str = r##"{loot_tables_to_eject: [{data: "minecraft:spawners/ominous/trial_chamber/key", weight: 3}, {data: "minecraft:spawners/ominous/trial_chamber/consumables", weight: 7}],simultaneous_mobs: 4.0f, total_mobs: 12.0f}"##;
const ZOMBIE_NORMAL: &str = r##"{simultaneous_mobs: 3.0f, simultaneous_mobs_added_per_player: 0.5f, spawn_potentials: [{data: {entity: {id: "minecraft:zombie"}}, weight: 1}], ticks_between_spawn: 20}"##;
const ZOMBIE_OMINOUS: &str = r##"{loot_tables_to_eject: [{data: "minecraft:spawners/ominous/trial_chamber/key", weight: 3}, {data: "minecraft:spawners/ominous/trial_chamber/consumables", weight: 7}],spawn_potentials: [{data: {entity: {id: "minecraft:zombie"}, equipment: {loot_table: "minecraft:equipment/trial_chamber_melee", slot_drop_chances: 0.0f}}, weight: 1}]}"##;
const POISON_SKELETON_NORMAL: &str = r##"{simultaneous_mobs: 3.0f, simultaneous_mobs_added_per_player: 0.5f, spawn_potentials: [{data: {entity: {id: "minecraft:bogged"}}, weight: 1}], ticks_between_spawn: 20}"##;
const POISON_SKELETON_OMINOUS: &str = r##"{loot_tables_to_eject: [{data: "minecraft:spawners/ominous/trial_chamber/key", weight: 3}, {data: "minecraft:spawners/ominous/trial_chamber/consumables", weight: 7}],spawn_potentials: [{data: {entity: {id: "minecraft:bogged"}, equipment: {loot_table: "minecraft:equipment/trial_chamber_ranged", slot_drop_chances: 0.0f}}, weight: 1}]}"##;
const SKELETON_NORMAL: &str = r##"{simultaneous_mobs: 3.0f, simultaneous_mobs_added_per_player: 0.5f, spawn_potentials: [{data: {entity: {id: "minecraft:skeleton"}}, weight: 1}], ticks_between_spawn: 20}"##;
const SKELETON_OMINOUS: &str = r##"{loot_tables_to_eject: [{data: "minecraft:spawners/ominous/trial_chamber/key", weight: 3}, {data: "minecraft:spawners/ominous/trial_chamber/consumables", weight: 7}], spawn_potentials: [{data: {entity: {id: "minecraft:skeleton"}, equipment: {loot_table: "minecraft:equipment/trial_chamber_ranged", slot_drop_chances: 0.0f}}, weight: 1}]}"##;
const STRAY_NORMAL: &str = r##"{simultaneous_mobs: 3.0f, simultaneous_mobs_added_per_player: 0.5f, spawn_potentials: [{data: {entity: {id: "minecraft:stray"}}, weight: 1}], ticks_between_spawn: 20}"##;
const STRAY_OMINOUS: &str = r##"{loot_tables_to_eject: [{data: "minecraft:spawners/ominous/trial_chamber/key", weight: 3}, {data: "minecraft:spawners/ominous/trial_chamber/consumables", weight: 7}], spawn_potentials: [{data: {entity: {id: "minecraft:stray"}, equipment: {loot_table: "minecraft:equipment/trial_chamber_ranged", slot_drop_chances: 0.0f}}, weight: 1}]}"##;
const SLOW_POISON_SKELETON_NORMAL: &str = r##"{simultaneous_mobs: 4.0f, simultaneous_mobs_added_per_player: 2.0f, spawn_potentials: [{data: {entity: {id: "minecraft:bogged"}}, weight: 1}], ticks_between_spawn: 160}"##;
const SLOW_POISON_SKELETON_OMINOUS: &str = r##"{loot_tables_to_eject: [{data: "minecraft:spawners/ominous/trial_chamber/key", weight: 3}, {data: "minecraft:spawners/ominous/trial_chamber/consumables", weight: 7}], spawn_potentials: [{data: {entity: {id: "minecraft:bogged"}, equipment: {loot_table: "minecraft:equipment/trial_chamber_ranged", slot_drop_chances: 0.0f}}, weight: 1}]}"##;
const SLOW_SKELETON_NORMAL: &str = r##"{simultaneous_mobs: 4.0f, simultaneous_mobs_added_per_player: 2.0f, spawn_potentials: [{data: {entity: {id: "minecraft:skeleton"}}, weight: 1}], ticks_between_spawn: 160}"##;
const SLOW_SKELETON_OMINOUS: &str = r##"{loot_tables_to_eject: [{data: "minecraft:spawners/ominous/trial_chamber/key", weight: 3}, {data: "minecraft:spawners/ominous/trial_chamber/consumables", weight: 7}], spawn_potentials: [{data: {entity: {id: "minecraft:skeleton"}, equipment: {loot_table: "minecraft:equipment/trial_chamber_ranged", slot_drop_chances: 0.0f}}, weight: 1}]}"##;
const SLOW_STRAY_NORMAL: &str = r##"{simultaneous_mobs: 4.0f, simultaneous_mobs_added_per_player: 2.0f, spawn_potentials: [{data: {entity: {id: "minecraft:stray"}}, weight: 1}], ticks_between_spawn: 160}"##;
const SLOW_STRAY_OMINOUS: &str = r##"{loot_tables_to_eject: [{data: "minecraft:spawners/ominous/trial_chamber/key", weight: 3}, {data: "minecraft:spawners/ominous/trial_chamber/consumables", weight: 7}],spawn_potentials: [{data: {entity: {id: "minecraft:stray"}, equipment: {loot_table: "minecraft:equipment/trial_chamber_ranged", slot_drop_chances: 0.0f}}, weight: 1}]}"##;
const BABY_ZOMBIE_NORMAL: &str = r##"{simultaneous_mobs: 2.0f, simultaneous_mobs_added_per_player: 0.5f, spawn_potentials: [{data: {entity: {IsBaby: 1b, id: "minecraft:zombie"}}, weight: 1}], ticks_between_spawn: 20}"##;
const BABY_ZOMBIE_OMINOUS: &str = r##"{loot_tables_to_eject: [{data: "minecraft:spawners/ominous/trial_chamber/key", weight: 3}, {data: "minecraft:spawners/ominous/trial_chamber/consumables", weight: 7}], spawn_potentials: [{data: {entity: {IsBaby: 1b, id: "minecraft:zombie"}, equipment: {loot_table: "minecraft:equipment/trial_chamber_melee", slot_drop_chances: 0.0f}}, weight: 1}]}"##;
const CAVE_SPIDER_NORMAL: &str = r##"{simultaneous_mobs: 3.0f, simultaneous_mobs_added_per_player: 0.5f, spawn_potentials: [{data: {entity: {id: "minecraft:cave_spider"}}, weight: 1}], ticks_between_spawn: 20}"##;
const CAVE_SPIDER_OMINOUS: &str = r##"{loot_tables_to_eject: [{data: "minecraft:spawners/ominous/trial_chamber/key", weight: 3}, {data: "minecraft:spawners/ominous/trial_chamber/consumables", weight: 7}], simultaneous_mobs: 4.0f, total_mobs: 12.0f}"##;
const SILVERFISH_NORMAL: &str = r##"{simultaneous_mobs: 3.0f, simultaneous_mobs_added_per_player: 0.5f, spawn_potentials: [{data: {entity: {id: "minecraft:silverfish"}}, weight: 1}], ticks_between_spawn: 20}"##;
const SILVERFISH_OMINOUS: &str = r##"{loot_tables_to_eject: [{data: "minecraft:spawners/ominous/trial_chamber/key", weight: 3}, {data: "minecraft:spawners/ominous/trial_chamber/consumables", weight: 7}], simultaneous_mobs: 4.0f, total_mobs: 12.0f}"##;
const SLIME_NORMAL: &str = r##"{simultaneous_mobs: 3.0f, simultaneous_mobs_added_per_player: 0.5f, spawn_potentials: [{data: {entity: {Size: 1, id: "minecraft:slime"}}, weight: 3}, {data: {entity: {Size: 2, id: "minecraft:slime"}}, weight: 1}], ticks_between_spawn: 20}"##;
const SLIME_OMINOUS: &str = r##"{loot_tables_to_eject: [{data: "minecraft:spawners/ominous/trial_chamber/key", weight: 3}, {data: "minecraft:spawners/ominous/trial_chamber/consumables", weight: 7}], simultaneous_mobs: 4.0f, total_mobs: 12.0f}"##;
