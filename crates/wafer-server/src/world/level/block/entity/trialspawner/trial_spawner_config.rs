use std::sync::{LazyLock, OnceLock};

use wafer_protocol::nbt::Nbt;

use crate::core::holder::{Holder, RegistryElement};
use crate::util::codec::{self, DataResult};
use crate::util::random::WeightedList;
use crate::world::item::component as dc;
use crate::world::level::spawn_data::SpawnData;

const SPAWNER_TRIAL_CHAMBER_KEY: &str = "minecraft:spawners/trial_chamber/key";
const SPAWNER_TRIAL_CHAMBER_CONSUMABLES: &str = "minecraft:spawners/trial_chamber/consumables";
const SPAWNER_TRIAL_ITEMS_TO_DROP_WHEN_OMINOUS: &str = "minecraft:spawners/trial_chamber/items_to_drop_when_ominous";

#[derive(Clone, PartialEq, Debug)]
pub struct TrialSpawnerConfig {
    pub spawn_range: i32,
    pub total_mobs: f32,
    pub simultaneous_mobs: f32,
    pub total_mobs_added_per_player: f32,
    pub simultaneous_mobs_added_per_player: f32,
    pub ticks_between_spawn: i32,
    pub spawn_potentials_definition: WeightedList<SpawnData>,
    pub loot_tables_to_eject: WeightedList<String>,
    pub items_to_drop_when_ominous: String,
}

pub static DEFAULT: LazyLock<TrialSpawnerConfig> = LazyLock::new(|| TrialSpawnerConfig {
    spawn_range: 4,
    total_mobs: 6.0,
    simultaneous_mobs: 2.0,
    total_mobs_added_per_player: 2.0,
    simultaneous_mobs_added_per_player: 1.0,
    ticks_between_spawn: 40,
    spawn_potentials_definition: WeightedList::empty(),
    loot_tables_to_eject: WeightedList::new(vec![(SPAWNER_TRIAL_CHAMBER_CONSUMABLES.into(), 1), (SPAWNER_TRIAL_CHAMBER_KEY.into(), 1)]),
    items_to_drop_when_ominous: SPAWNER_TRIAL_ITEMS_TO_DROP_WHEN_OMINOUS.into(),
});

impl RegistryElement for TrialSpawnerConfig {
    const REGISTRY: &'static str = "minecraft:trial_spawner";

    fn contains_key(key: &str) -> bool {
        TrialSpawnerConfig::lookup(key).is_some()
    }

    fn lookup(key: &str) -> Option<&'static TrialSpawnerConfig> {
        static CONFIGS: OnceLock<Vec<(&'static str, TrialSpawnerConfig)>> = OnceLock::new();
        CONFIGS
            .get_or_init(|| {
                wafer_data::TRIAL_SPAWNER_CONFIGS
                    .iter()
                    .map(|&(name, json)| {
                        let tag = dc::json_to_nbt(&serde_json::from_str(json).expect("vanilla JSON"));
                        (name, TrialSpawnerConfig::decode_direct(&tag).expect("vanilla trial spawner config"))
                    })
                    .collect()
            })
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, config)| config)
    }
}

fn weighted_list<T>(tag: &Nbt, element: impl Fn(&Nbt) -> DataResult<T>) -> DataResult<WeightedList<T>> {
    let entries = codec::list(tag)?;
    let mut items = Vec::with_capacity(entries.len());
    for entry in &entries {
        let Ok(data) = codec::field(entry, "data").and_then(&element) else { continue };
        let Ok(weight) = codec::field(entry, "weight").and_then(codec::int) else { continue };
        if weight >= 0 {
            items.push((data, weight));
        }
    }
    Ok(WeightedList::new(items))
}

fn encode_weighted_list<T>(list: &WeightedList<T>, element: impl Fn(&T) -> Nbt) -> Nbt {
    Nbt::List(list.items.iter().map(|(data, weight)| Nbt::Compound(vec![("data".into(), element(data)), ("weight".into(), Nbt::Int(*weight))])).collect())
}

impl TrialSpawnerConfig {
    pub fn decode_direct(tag: &Nbt) -> DataResult<TrialSpawnerConfig> {
        codec::map(tag)?;
        Ok(TrialSpawnerConfig {
            spawn_range: codec::decode_or(tag, "spawn_range", DEFAULT.spawn_range, codec::int)?,
            total_mobs: codec::decode_or(tag, "total_mobs", DEFAULT.total_mobs, codec::float)?,
            simultaneous_mobs: codec::decode_or(tag, "simultaneous_mobs", DEFAULT.simultaneous_mobs, codec::float)?,
            total_mobs_added_per_player: codec::decode_or(tag, "total_mobs_added_per_player", DEFAULT.total_mobs_added_per_player, codec::float)?,
            simultaneous_mobs_added_per_player: codec::decode_or(tag, "simultaneous_mobs_added_per_player", DEFAULT.simultaneous_mobs_added_per_player, codec::float)?,
            ticks_between_spawn: codec::decode_or(tag, "ticks_between_spawn", DEFAULT.ticks_between_spawn, codec::int)?,
            spawn_potentials_definition: codec::decode_or(tag, "spawn_potentials", WeightedList::empty(), |t| weighted_list(t, SpawnData::decode))?,
            loot_tables_to_eject: match codec::optional_field(tag, "loot_tables_to_eject") {
                Some(t) => weighted_list(t, codec::identifier)?,
                None => DEFAULT.loot_tables_to_eject.clone(),
            },
            items_to_drop_when_ominous: match codec::optional_field(tag, "items_to_drop_when_ominous") {
                Some(t) => codec::identifier(t)?,
                None => DEFAULT.items_to_drop_when_ominous.clone(),
            },
        })
    }

    pub fn decode(tag: &Nbt) -> DataResult<Holder<TrialSpawnerConfig>> {
        Holder::decode(tag, TrialSpawnerConfig::decode_direct)
    }

    pub fn encode_direct(&self) -> Nbt {
        let mut output = Vec::new();
        if self.spawn_range != DEFAULT.spawn_range {
            output.push(("spawn_range".into(), Nbt::Int(self.spawn_range)));
        }
        if self.total_mobs != DEFAULT.total_mobs {
            output.push(("total_mobs".into(), Nbt::Float(self.total_mobs)));
        }
        if self.simultaneous_mobs != DEFAULT.simultaneous_mobs {
            output.push(("simultaneous_mobs".into(), Nbt::Float(self.simultaneous_mobs)));
        }
        if self.total_mobs_added_per_player != DEFAULT.total_mobs_added_per_player {
            output.push(("total_mobs_added_per_player".into(), Nbt::Float(self.total_mobs_added_per_player)));
        }
        if self.simultaneous_mobs_added_per_player != DEFAULT.simultaneous_mobs_added_per_player {
            output.push(("simultaneous_mobs_added_per_player".into(), Nbt::Float(self.simultaneous_mobs_added_per_player)));
        }
        if self.ticks_between_spawn != DEFAULT.ticks_between_spawn {
            output.push(("ticks_between_spawn".into(), Nbt::Int(self.ticks_between_spawn)));
        }
        if !self.spawn_potentials_definition.items.is_empty() {
            output.push(("spawn_potentials".into(), encode_weighted_list(&self.spawn_potentials_definition, SpawnData::encode)));
        }
        if self.loot_tables_to_eject != DEFAULT.loot_tables_to_eject {
            output.push(("loot_tables_to_eject".into(), encode_weighted_list(&self.loot_tables_to_eject, |key| Nbt::String(key.clone()))));
        }
        if self.items_to_drop_when_ominous != DEFAULT.items_to_drop_when_ominous {
            output.push(("items_to_drop_when_ominous".into(), Nbt::String(self.items_to_drop_when_ominous.clone())));
        }
        Nbt::Compound(output)
    }

    pub fn encode(holder: &Holder<TrialSpawnerConfig>) -> Nbt {
        match holder.unwrap_key() {
            Some(key) => Nbt::String(key.to_string()),
            None => holder.value().encode_direct(),
        }
    }

    pub fn calculate_target_total_mobs(&self, additional_players: i32) -> i32 {
        (self.total_mobs + self.total_mobs_added_per_player * additional_players as f32).floor() as i32
    }

    pub fn calculate_target_simultaneous_mobs(&self, additional_players: i32) -> i32 {
        (self.simultaneous_mobs + self.simultaneous_mobs_added_per_player * additional_players as f32).floor() as i32
    }

    pub fn ticks_between_item_spawners(&self) -> i64 {
        160
    }

    pub fn with_spawning(&self, spawn_data: SpawnData) -> TrialSpawnerConfig {
        TrialSpawnerConfig { spawn_potentials_definition: WeightedList::new(vec![(spawn_data, 1)]), ..self.clone() }
    }
}
