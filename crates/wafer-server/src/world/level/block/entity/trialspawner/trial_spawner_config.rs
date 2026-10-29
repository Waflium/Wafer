use std::sync::{LazyLock, OnceLock};

use wafer_protocol::nbt::Nbt;

use crate::core::holder::{Holder, RegistryElement};
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::util::random::{WeightedList, weighted_data_codec, weighted_list_codec};
use crate::world::item::component as dc;
use crate::world::item::component::codec::{self as component_codec, Codec, RangeError};
use crate::world::item::component::types::{self, Pair, nbt};
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
                crate::core::data_registries::json_entries("minecraft:trial_spawner")
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

fn weighted_list<T>(tag: &Nbt, data: Pair, element: impl Fn(&Nbt) -> Option<T>) -> DecodeResult<WeightedList<T>> {
    weighted_list_codec(weighted_data_codec(data)).0.decode_partial(tag).map(|list| {
        let items = list.as_list().iter().filter_map(|entry| Some((element(entry.get("data")?)?, entry.get("weight")?.as_i64()? as i32))).collect();
        WeightedList::new(items)
    })
}

fn spawn_data_codec() -> Pair {
    nbt(Codec::Func(|tag| SpawnData::decode_partial(tag).map(|_| tag.clone())))
}

fn encode_weighted_list<T>(list: &WeightedList<T>, element: impl Fn(&T) -> Nbt) -> Nbt {
    Nbt::List(list.items.iter().map(|(data, weight)| Nbt::Compound(vec![("data".into(), element(data)), ("weight".into(), Nbt::Int(*weight))])).collect())
}

impl TrialSpawnerConfig {
    pub fn decode_direct(tag: &Nbt) -> DataResult<TrialSpawnerConfig> {
        TrialSpawnerConfig::decode_direct_partial(tag).into_result()
    }

    pub fn decode_direct_partial(tag: &Nbt) -> DecodeResult<TrialSpawnerConfig> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let int_range = |min: i32, max: i32| move |tag: &Nbt| Codec::Int(min, max, RangeError::Outside).decode(tag).map(|value| component_codec::as_i32(&value));
        let non_negative_float = |tag: &Nbt| types::codec_float_range(0.0, f32::MAX).decode(tag).map(|value| component_codec::as_f32(&value));
        let mut record = codec::Record::default();
        let spawn_range = record.optional(tag, "spawn_range", int_range(1, 128));
        let total_mobs = record.optional(tag, "total_mobs", non_negative_float);
        let simultaneous_mobs = record.optional(tag, "simultaneous_mobs", non_negative_float);
        let total_mobs_added_per_player = record.optional(tag, "total_mobs_added_per_player", non_negative_float);
        let simultaneous_mobs_added_per_player = record.optional(tag, "simultaneous_mobs_added_per_player", non_negative_float);
        let ticks_between_spawn = record.optional(tag, "ticks_between_spawn", int_range(0, i32::MAX));
        let spawn_potentials = record.optional(tag, "spawn_potentials", |tag| weighted_list(tag, spawn_data_codec(), |data| SpawnData::decode(data).ok()));
        let loot_tables_to_eject = record.optional(tag, "loot_tables_to_eject", |tag| weighted_list(tag, types::identifier(), |data| data.as_str().map(str::to_string)));
        let items_to_drop_when_ominous = record.optional(tag, "items_to_drop_when_ominous", |tag| codec::identifier(tag));
        record.build(|| {
            Some(TrialSpawnerConfig {
                spawn_range: spawn_range.unwrap_or(DEFAULT.spawn_range),
                total_mobs: total_mobs.unwrap_or(DEFAULT.total_mobs),
                simultaneous_mobs: simultaneous_mobs.unwrap_or(DEFAULT.simultaneous_mobs),
                total_mobs_added_per_player: total_mobs_added_per_player.unwrap_or(DEFAULT.total_mobs_added_per_player),
                simultaneous_mobs_added_per_player: simultaneous_mobs_added_per_player.unwrap_or(DEFAULT.simultaneous_mobs_added_per_player),
                ticks_between_spawn: ticks_between_spawn.unwrap_or(DEFAULT.ticks_between_spawn),
                spawn_potentials_definition: spawn_potentials.unwrap_or_else(WeightedList::empty),
                loot_tables_to_eject: loot_tables_to_eject.unwrap_or_else(|| DEFAULT.loot_tables_to_eject.clone()),
                items_to_drop_when_ominous: items_to_drop_when_ominous.unwrap_or_else(|| DEFAULT.items_to_drop_when_ominous.clone()),
            })
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
