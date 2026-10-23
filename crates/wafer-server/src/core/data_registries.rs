use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;
use std::sync::atomic::{AtomicPtr, Ordering};

use serde_json::Value;
use wafer_data::SyncedRegistry;

use crate::server::packs::repository::VANILLA_ID;
use crate::server::packs::resources::MultiPackResourceManager;
use crate::server::reloadable_server_registries::LoadingErrors;

pub struct DataRegistry {
    to_network: Vec<i32>,
    from_network: Vec<i32>,
    contents: Vec<Option<Value>>,
    elements: Vec<Value>,
    json: &'static [(&'static str, &'static str)],
}

impl DataRegistry {
    pub fn network_id(&self, id: i32) -> i32 {
        self.to_network.get(id as usize).copied().unwrap_or(id)
    }

    pub fn from_network_id(&self, network_id: i32) -> i32 {
        self.from_network.get(network_id as usize).copied().unwrap_or(network_id)
    }

    pub fn network_order(&self) -> impl Iterator<Item = i32> + '_ {
        self.from_network.iter().copied()
    }

    pub fn network_contents(&self, id: i32) -> Option<&Value> {
        self.contents.get(id as usize).and_then(Option::as_ref)
    }

    pub fn json_entries(&self) -> &'static [(&'static str, &'static str)] {
        self.json
    }
}

static ACTIVE: AtomicPtr<HashMap<&'static str, DataRegistry>> = AtomicPtr::new(std::ptr::null_mut());

fn active() -> Option<&'static HashMap<&'static str, DataRegistry>> {
    let active = ACTIVE.load(Ordering::Acquire);
    if active.is_null() { None } else { Some(unsafe { &*active }) }
}

static LEVEL_STEMS: AtomicPtr<Vec<(&'static str, Value)>> = AtomicPtr::new(std::ptr::null_mut());

pub fn level_stems() -> &'static [(&'static str, Value)] {
    let stems = LEVEL_STEMS.load(Ordering::Acquire);
    if stems.is_null() { &[] } else { unsafe { &*stems } }
}

static BIOMES: AtomicPtr<DataRegistry> = AtomicPtr::new(std::ptr::null_mut());

pub fn biome_network_id(id: u32) -> u32 {
    let biomes = BIOMES.load(Ordering::Acquire);
    if biomes.is_null() { id } else { unsafe { &*biomes }.network_id(id as i32) as u32 }
}

pub fn biome_from_network_id(network_id: u32) -> u32 {
    let biomes = BIOMES.load(Ordering::Acquire);
    if biomes.is_null() { network_id } else { unsafe { &*biomes }.from_network_id(network_id as i32) as u32 }
}

pub fn get(name: &str) -> Option<&'static DataRegistry> {
    active()?.get(name)
}

pub fn network_id(registry: &str, id: i32) -> i32 {
    get(registry).map_or(id, |registry| registry.network_id(id))
}

pub fn from_network_id(registry: &str, network_id: i32) -> i32 {
    get(registry).map_or(network_id, |registry| registry.from_network_id(network_id))
}

pub fn json_entries(registry: &str) -> &'static [(&'static str, &'static str)] {
    static VANILLA: std::sync::Once = std::sync::Once::new();
    if active().is_none() {
        VANILLA.call_once(|| {
            let mut repository = crate::server::packs::repository::PackRepository::create_vanilla_trusted_repository();
            repository.reload();
            repository.set_selected(&[VANILLA_ID]);
            load(&MultiPackResourceManager::new(repository.open_all_selected()));
        });
    }
    get(registry).map_or(&[], DataRegistry::json_entries)
}

fn leak(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn dimension_type(name: &'static str, v: &Value) -> wafer_data::DimensionType {
    let int = |key: &str| v[key].as_i64().unwrap_or(0);
    let infiniburn = v["infiniburn"].as_str().unwrap_or("").trim_start_matches('#');
    wafer_data::DimensionType {
        name,
        min_y: int("min_y") as i32,
        height: int("height") as u32,
        logical_height: int("logical_height") as u32,
        coordinate_scale: v["coordinate_scale"].as_f64().unwrap_or(1.0),
        has_skylight: v["has_skylight"].as_bool().unwrap_or(false),
        has_ceiling: v["has_ceiling"].as_bool().unwrap_or(false),
        has_ender_dragon_fight: v["has_ender_dragon_fight"].as_bool().unwrap_or(false),
        has_fixed_time: v["has_fixed_time"].as_bool().unwrap_or(false),
        ambient_light: v["ambient_light"].as_f64().unwrap_or(0.0) as f32,
        infiniburn: wafer_data::dynamic_tag("minecraft:block", infiniburn),
        default_clock: v["default_clock"].as_str().map(|clock| leak(clock.to_owned())),
        attributes: leak(v.get("attributes").map_or("{}".to_owned(), Value::to_string)),
        timelines: leak(v["timelines"].to_string()),
        monster_spawn_light_level: leak(v["monster_spawn_light_level"].to_string()),
        monster_spawn_block_light_limit: int("monster_spawn_block_light_limit") as i32,
    }
}

fn file_key(id: &str) -> (Vec<u16>, Vec<u16>) {
    let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
    (format!("{path}.json").encode_utf16().collect(), namespace.encode_utf16().collect())
}

const UNSYNCHRONIZED: &[(&str, &[(&str, &str)])] = &[
    ("minecraft:worldgen/density_function", wafer_data::DENSITY_FUNCTIONS),
    ("minecraft:worldgen/noise", wafer_data::NOISES),
    ("minecraft:worldgen/noise_settings", wafer_data::NOISE_SETTINGS),
    ("minecraft:worldgen/material_rule", wafer_data::MATERIAL_RULES),
    ("minecraft:worldgen/material_condition", wafer_data::MATERIAL_CONDITIONS),
    ("minecraft:worldgen/world_preset", wafer_data::WORLD_PRESETS),
    ("minecraft:worldgen/multi_noise_biome_source_parameter_list", wafer_data::BIOME_SOURCE_PARAMETER_LISTS),
    ("minecraft:worldgen/carver", wafer_data::CARVERS),
    ("minecraft:worldgen/structure", wafer_data::STRUCTURES),
    ("minecraft:worldgen/structure_set", wafer_data::STRUCTURE_SETS),
    ("minecraft:worldgen/template_pool", wafer_data::TEMPLATE_POOLS),
    ("minecraft:worldgen/feature", wafer_data::FEATURES),
    ("minecraft:worldgen/placed_feature", wafer_data::PLACED_FEATURES),
    ("minecraft:worldgen/processor_list", wafer_data::PROCESSOR_LISTS),
    ("minecraft:trial_spawner", wafer_data::TRIAL_SPAWNER_CONFIGS),
    ("minecraft:villager_trade", wafer_data::VILLAGER_TRADES),
    ("minecraft:trade_set", wafer_data::TRADE_SETS),
    ("minecraft:enchantment_provider", &[]),
];

struct PackElement {
    registry: &'static str,
    id: String,
    pack: String,
    json: Result<Value, String>,
}

static PACK_ELEMENTS: Mutex<Vec<PackElement>> = Mutex::new(Vec::new());

thread_local! {
    static UNBOUND: RefCell<Option<Vec<(String, String)>>> = const { RefCell::new(None) };
}

pub fn unknown_reference(registry: &str, key: &str) -> String {
    UNBOUND.with(|unbound| {
        if let Some(unbound) = unbound.borrow_mut().as_mut() {
            unbound.push((registry.to_string(), key.to_string()));
        }
    });
    unknown_reference_message(registry, key)
}

fn unknown_reference_message(registry: &str, key: &str) -> String {
    format!("Unknown registry key in ResourceKey[minecraft:root / {registry}]: {key}")
}

fn read_element(registry: &'static str, id: &str, resource: &crate::server::packs::Resource, pack_elements: &mut Vec<PackElement>) -> Option<Value> {
    let json = resource.supplier.read().map_err(|e| e.to_string()).and_then(|bytes| serde_json::from_slice::<Value>(&bytes).map_err(|e| format!("com.google.gson.JsonSyntaxException: {e}")));
    let parsed = json.as_ref().ok().cloned();
    if resource.source_pack_id != VANILLA_ID {
        pack_elements.push(PackElement { registry, id: id.to_string(), pack: resource.source_pack_id.clone(), json });
    }
    parsed
}

fn load_registry(
    resource_manager: &MultiPackResourceManager,
    name: &'static str,
    builtin: &[&'static str],
    pack_elements: &mut Vec<PackElement>,
) -> (DataRegistry, &'static [&'static str]) {
    let directory = name.strip_prefix("minecraft:").unwrap_or(name);
    let resources = resource_manager.list_resources(directory, ".json");
    let mut entries: Vec<&'static str> = builtin.to_vec();
    let mut contents: Vec<Option<Value>> = vec![None; entries.len()];
    let mut elements: Vec<Value> = vec![Value::Null; entries.len()];
    for (location, resource) in &resources {
        let (namespace, path) = location.split_once(':').unwrap();
        let id = format!("{namespace}:{}", &path[directory.len() + 1..path.len() - ".json".len()]);
        let Some(json) = read_element(name, &id, resource, pack_elements) else { continue };
        let index = match entries.iter().position(|&entry| entry == id) {
            Some(index) => index,
            None => {
                entries.push(Box::leak(id.into_boxed_str()));
                contents.push(None);
                elements.push(Value::Null);
                entries.len() - 1
            }
        };
        if resource.source_pack_id != VANILLA_ID {
            contents[index] = Some(json.clone());
        }
        elements[index] = json;
    }
    let mut from_network: Vec<i32> = (0..entries.len() as i32).collect();
    from_network.sort_by_key(|&id| file_key(entries[id as usize]));
    let mut to_network = vec![0; entries.len()];
    for (network_id, &id) in from_network.iter().enumerate() {
        to_network[id as usize] = network_id as i32;
    }
    let json: Vec<(&'static str, &'static str)> = entries.iter().zip(&elements).map(|(&name, element)| (name, &*Box::leak(element.to_string().into_boxed_str()))).collect();
    (DataRegistry { to_network, from_network, contents, elements, json: Box::leak(json.into_boxed_slice()) }, Box::leak(entries.into_boxed_slice()))
}

pub fn load(resource_manager: &MultiPackResourceManager) {
    let mut pack_elements = Vec::new();
    let mut synced = Vec::new();
    let mut registries = HashMap::new();
    for builtin in wafer_data::builtin_synced_registries() {
        let (registry, entries) = load_registry(resource_manager, builtin.name, builtin.entries, &mut pack_elements);
        synced.push(SyncedRegistry { name: builtin.name, entries });
        registries.insert(builtin.name, registry);
    }
    wafer_data::set_synced_registries(synced);
    let mut unsynchronized = Vec::new();
    for &(name, table) in UNSYNCHRONIZED {
        let builtin: Vec<&'static str> = table.iter().map(|&(entry, _)| entry).collect();
        let (registry, entries) = load_registry(resource_manager, name, &builtin, &mut pack_elements);
        unsynchronized.push((name, entries));
        registries.insert(name, registry);
    }
    wafer_data::set_data_registry_entries(unsynchronized);
    let mut stems: Vec<(&'static str, Value)> = Vec::new();
    for (location, resource) in resource_manager.list_resources("dimension", ".json") {
        let (namespace, path) = location.split_once(':').unwrap();
        let id = format!("{namespace}:{}", &path["dimension".len() + 1..path.len() - ".json".len()]);
        if let Some(json) = read_element("minecraft:dimension", &id, &resource, &mut pack_elements) {
            stems.push((leak(id), json));
        }
    }
    stems.sort_by_key(|(id, _)| file_key(id));
    LEVEL_STEMS.store(Box::into_raw(Box::new(stems)), Ordering::Release);
    let dimension_registry = &registries["minecraft:dimension_type"];
    let dimension_entries = wafer_data::SyncedRegistry::get("minecraft:dimension_type").entries;
    wafer_data::set_dimension_types(dimension_entries.iter().zip(&dimension_registry.elements).map(|(&name, json)| dimension_type(name, json)).collect());
    let registries: &'static HashMap<&'static str, DataRegistry> = Box::leak(Box::new(registries));
    BIOMES.store((&registries["minecraft:worldgen/biome"] as *const DataRegistry).cast_mut(), Ordering::Release);
    ACTIVE.store((registries as *const HashMap<&'static str, DataRegistry>).cast_mut(), Ordering::Release);
    *PACK_ELEMENTS.lock().unwrap() = pack_elements;
}

pub fn validate() -> Result<(), String> {
    let pack_elements = std::mem::take(&mut *PACK_ELEMENTS.lock().unwrap());
    let mut errors = LoadingErrors(Default::default());
    UNBOUND.with(|unbound| unbound.replace(Some(Vec::new())));
    for element in &pack_elements {
        let decoded = element.json.as_ref().map_err(Clone::clone).and_then(|json| crate::nbt::with_json_source(|| decode(element.registry, json)));
        let unbound = UNBOUND.with(|unbound| unbound.borrow().as_ref().and_then(|unbound| unbound.last().map(|(registry, key)| unknown_reference_message(registry, key))));
        match decoded {
            Err(cause) if unbound.as_ref() == Some(&cause) => {}
            Err(cause) if element.json.is_err() => errors.parse_error(element.registry, &element.id, &element.pack, cause),
            Err(cause) => errors.parse_error(element.registry, &element.id, &element.pack, format!("java.lang.IllegalStateException: {cause}")),
            Ok(()) => {}
        }
    }
    let mut unbound_by_registry: BTreeMap<String, Vec<(String, ())>> = BTreeMap::new();
    for (registry, key) in UNBOUND.with(|unbound| unbound.take()).unwrap_or_default() {
        let keys = unbound_by_registry.entry(registry).or_default();
        if !keys.iter().any(|(k, _)| *k == key) {
            keys.push((key, ()));
        }
    }
    for (registry, mut keys) in unbound_by_registry {
        crate::tags::identifier_order(&mut keys);
        let keys: Vec<String> = keys.into_iter().map(|(key, _)| key).collect();
        errors.0.insert(("minecraft:root".to_string(), registry.clone()), format!("java.lang.IllegalStateException: Unbound values in registry ResourceKey[minecraft:root / {registry}]: [{}]", keys.join(", ")));
    }
    if errors.0.is_empty() {
        return Ok(());
    }
    let report = errors.report();
    error!("{report}");
    Err(report)
}

fn decode(registry: &str, json: &Value) -> Result<(), String> {
    use crate::world::item::component::json_to_nbt;
    use crate::world::level::levelgen;
    match registry {
        "minecraft:worldgen/noise" | "minecraft:worldgen/density_function" | "minecraft:worldgen/noise_settings" | "minecraft:worldgen/material_rule" | "minecraft:worldgen/material_condition" => {
            levelgen::validate(registry, json)
        }
        "minecraft:worldgen/feature" => levelgen::feature::feature::Feature::parse(json).map(drop),
        "minecraft:worldgen/placed_feature" => levelgen::placement::placed_feature::PlacedFeature::parse(json).map(drop),
        "minecraft:worldgen/structure" => levelgen::structure::structure::Structure::parse(json).map(drop),
        "minecraft:worldgen/structure_set" => levelgen::structure::structure_set::StructureSet::parse(json).map(drop),
        "minecraft:worldgen/template_pool" => levelgen::structure::pools::structure_template_pool::StructureTemplatePool::parse(json).map(drop),
        "minecraft:worldgen/processor_list" => levelgen::structure::templatesystem::structure_processor::StructureProcessorList::parse(json).map(drop),
        "minecraft:worldgen/configured_carver" | "minecraft:worldgen/carver" => levelgen::carver::WorldCarver::parse(json).map(drop),
        "minecraft:worldgen/world_preset" => levelgen::world_dimensions::WorldDimensions::from_json(&json["dimensions"], 0).map(drop),
        "minecraft:worldgen/biome" => crate::world::level::biome::validate(json),
        "minecraft:dimension" => levelgen::world_dimensions::WorldDimensions::stem_from_json(json, 0).map(drop),
        "minecraft:enchantment" => crate::world::item::enchantment::enchantment::Enchantment::decode(&json_to_nbt(json)).map(drop),
        "minecraft:enchantment_provider" => crate::world::item::enchantment::providers::enchantment_provider::EnchantmentProvider::parse(json).map(drop),
        "minecraft:villager_trade" => crate::world::item::trading::villager_trade::VillagerTrade::decode(&json_to_nbt(json)).map(drop),
        "minecraft:trade_set" => crate::world::item::trading::trade_set::TradeSet::decode(&json_to_nbt(json)).map(drop),
        "minecraft:trial_spawner" => crate::world::level::block::entity::trialspawner::trial_spawner_config::TrialSpawnerConfig::decode_direct(&json_to_nbt(json)).map(drop),
        "minecraft:painting_variant" => crate::world::entity::decoration::painting::painting_variant::PaintingVariant::decode(&json_to_nbt(json)).map(drop),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use crate::server::packs::repository::{PackRepository, VANILLA_ID};
    use crate::server::packs::resources::MultiPackResourceManager;

    #[test]
    fn vanilla_entries_keep_their_builtin_ids_and_send_no_contents() {
        let mut repository = PackRepository::create_vanilla_trusted_repository();
        repository.reload();
        repository.set_selected(&[VANILLA_ID]);
        super::load(&MultiPackResourceManager::new(repository.open_all_selected()));
        for builtin in wafer_data::builtin_synced_registries() {
            let registry = super::get(builtin.name).unwrap();
            assert_eq!(wafer_data::SyncedRegistry::get(builtin.name).entries, builtin.entries, "{}", builtin.name);
            for id in 0..builtin.entries.len() as i32 {
                assert_eq!(registry.network_id(id), id, "{} {}", builtin.name, builtin.entries[id as usize]);
                assert!(registry.network_contents(id).is_none(), "{} {}", builtin.name, builtin.entries[id as usize]);
                assert!(registry.json_entries()[id as usize].1 != "null", "{} {}", builtin.name, builtin.entries[id as usize]);
            }
        }
    }
}
