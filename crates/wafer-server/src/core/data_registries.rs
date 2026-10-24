use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{Mutex, OnceLock};

use serde_json::Value;
use wafer_data::SyncedRegistry;
use wafer_protocol::nbt::Nbt;

use crate::server::packs::repository::VANILLA_ID;
use crate::server::packs::resources::MultiPackResourceManager;
use crate::resources::registry_load_task::{self, ElementFailure};
use crate::server::reloadable_server_registries::LoadingErrors;
use crate::util::java_throwable::JavaThrowable;

pub struct DataRegistry {
    name: &'static str,
    to_network: Vec<i32>,
    from_network: Vec<i32>,
    contents: Vec<Option<Value>>,
    network: Vec<OnceLock<Nbt>>,
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

    pub fn network_contents(&self, id: i32) -> Option<&Nbt> {
        let json = self.contents.get(id as usize)?.as_ref()?;
        Some(self.network[id as usize].get_or_init(|| encode_network(self.name, json)))
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
    resource_count: usize,
    json: Result<Value, JavaThrowable>,
}

static PACK_ELEMENTS: Mutex<Vec<PackElement>> = Mutex::new(Vec::new());

thread_local! {
    static UNBOUND: RefCell<Option<Vec<(String, String)>>> = const { RefCell::new(None) };
    static UNBOUND_TAGS: RefCell<Option<Vec<(String, String)>>> = const { RefCell::new(None) };
}

pub fn unknown_reference(registry: &str, key: &str) -> String {
    UNBOUND.with(|unbound| {
        if let Some(unbound) = unbound.borrow_mut().as_mut() {
            unbound.push((registry.to_string(), key.to_string()));
        }
    });
    unknown_reference_message(registry, key)
}

pub fn missing_tag(registry: &str, tag: &str) -> Option<String> {
    let loading = registry == "minecraft:dimension" || active().is_some_and(|registries| registries.contains_key(registry));
    let recorded = loading
        && UNBOUND_TAGS.with(|unbound| match unbound.borrow_mut().as_mut() {
            Some(unbound) => {
                unbound.push((registry.to_string(), tag.to_string()));
                true
            }
            None => false,
        });
    if recorded { None } else { Some(format!("Missing tag: '{tag}' in '{registry}'")) }
}

fn unknown_reference_message(registry: &str, key: &str) -> String {
    format!("Unknown registry key in ResourceKey[minecraft:root / {registry}]: {key}")
}

fn read_element(registry: &'static str, id: &str, resource: &crate::server::packs::Resource, resource_count: usize, pack_elements: &mut Vec<PackElement>) -> Option<Value> {
    let json = match resource.supplier.read() {
        Ok(bytes) => crate::util::gson::parse_strict_bytes_throwable(&bytes),
        Err(error) => Err(JavaThrowable::new(format!("java.io.IOException: {error}"), Vec::new())),
    };
    let parsed = json.as_ref().ok().cloned();
    if resource.source_pack_id != VANILLA_ID {
        pack_elements.push(PackElement { registry, id: id.to_string(), pack: resource.source_pack_id.clone(), resource_count, json });
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
        let Some(json) = read_element(name, &id, resource, resources.len(), pack_elements) else { continue };
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
    let network = (0..entries.len()).map(|_| OnceLock::new()).collect();
    (DataRegistry { name, to_network, from_network, contents, network, elements, json: Box::leak(json.into_boxed_slice()) }, Box::leak(entries.into_boxed_slice()))
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
    let dimension_resources = resource_manager.list_resources("dimension", ".json");
    for (location, resource) in &dimension_resources {
        let (namespace, path) = location.split_once(':').unwrap();
        let id = format!("{namespace}:{}", &path["dimension".len() + 1..path.len() - ".json".len()]);
        if let Some(json) = read_element("minecraft:dimension", &id, resource, dimension_resources.len(), &mut pack_elements) {
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

#[cfg(test)]
pub fn with_loading<R>(f: impl FnOnce() -> R) -> R {
    let previous = UNBOUND.with(|unbound| unbound.replace(Some(Vec::new())));
    let result = f();
    UNBOUND.with(|unbound| unbound.replace(previous));
    result
}

pub fn is_loading() -> bool {
    UNBOUND.with(|unbound| unbound.borrow().is_some())
}

pub fn group_unbound(unbound: Vec<(String, String)>) -> BTreeMap<String, Vec<String>> {
    let mut by_registry: BTreeMap<String, Vec<(String, ())>> = BTreeMap::new();
    for (registry, key) in unbound {
        let keys = by_registry.entry(registry).or_default();
        if !keys.iter().any(|(k, _)| *k == key) {
            keys.push((key, ()));
        }
    }
    by_registry
        .into_iter()
        .map(|(registry, mut keys)| {
            crate::tags::identifier_order(&mut keys);
            (registry, keys.into_iter().map(|(key, _)| key).collect())
        })
        .collect()
}

fn validate_timelines(errors: &mut LoadingErrors) {
    const REGISTRY: &str = "minecraft:timeline";
    let Some(registry) = get(REGISTRY) else { return };
    let mut timelines: Vec<&(&str, &str)> = registry.json_entries().iter().collect();
    timelines.sort_by_key(|(id, _)| file_key(id));
    let mut defined: Vec<(String, String)> = Vec::new();
    for (id, json) in timelines {
        let Ok(value) = serde_json::from_str::<Value>(json) else { continue };
        let Some(clock) = value["clock"].as_str().and_then(|clock| crate::resources::identifier::read(clock).ok()) else { continue };
        let Some(markers) = value["time_markers"].as_object() else { continue };
        for marker in markers.keys() {
            let Ok(marker) = crate::resources::identifier::read(marker) else { continue };
            let key = (clock.clone(), marker);
            if defined.contains(&key) {
                errors.0.insert(
                    (REGISTRY.to_string(), id.to_string()),
                    registry_load_task::timeline_marker_defined_multiple_times(format!("ResourceKey[minecraft:clock_time_marker / {}] was defined multiple times in {}", key.1, key.0)),
                );
            } else {
                defined.push(key);
            }
        }
    }
}

pub fn validate() -> Result<(), JavaThrowable> {
    let pack_elements = std::mem::take(&mut *PACK_ELEMENTS.lock().unwrap());
    let mut errors = LoadingErrors(Default::default());
    UNBOUND.with(|unbound| unbound.replace(Some(Vec::new())));
    UNBOUND_TAGS.with(|unbound| unbound.replace(Some(Vec::new())));
    for element in &pack_elements {
        let decoded = match &element.json {
            Ok(json) => crate::nbt::with_json_source(|| decode(element.registry, json)).map_err(ElementFailure::Decode),
            Err(throwable) => Err(ElementFailure::Parse(throwable.clone())),
        };
        let unbound = UNBOUND.with(|unbound| unbound.borrow().as_ref().and_then(|unbound| unbound.last().map(|(registry, key)| unknown_reference_message(registry, key))));
        match decoded {
            Err(ElementFailure::Decode(cause)) if unbound.as_ref() == Some(&cause) => {}
            Err(failure) => errors.parse_error(element.registry, &element.id, &element.pack, element.resource_count, failure),
            Ok(()) => {}
        }
    }
    let unbound_values = group_unbound(UNBOUND.with(|unbound| unbound.take()).unwrap_or_default());
    let unbound_tags = group_unbound(UNBOUND_TAGS.with(|unbound| unbound.take()).unwrap_or_default());
    for (registry, keys) in &unbound_tags {
        if !unbound_values.contains_key(registry) {
            errors.0.insert(("minecraft:root".to_string(), registry.clone()), registry_load_task::unbound_tags(&format!("ResourceKey[minecraft:root / {registry}]"), keys));
        }
    }
    for (registry, keys) in unbound_values {
        errors.0.insert(("minecraft:root".to_string(), registry.clone()), registry_load_task::unbound_values(&format!("ResourceKey[minecraft:root / {registry}]"), &keys));
    }
    let mut validation_stage = false;
    if errors.0.is_empty() {
        validation_stage = true;
        validate_timelines(&mut errors);
    }
    if errors.0.is_empty() {
        return Ok(());
    }
    error!("{}", errors.report());
    Err(registry_load_task::registry_loading_failed(validation_stage))
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
        "minecraft:damage_type" => decode_with(crate::world::damagesource::damage_type::direct_codec(), json),
        "minecraft:banner_pattern" => decode_with(crate::world::level::block::entity::banner_pattern::direct_codec(), json),
        "minecraft:decorated_pot_pattern" => decode_with(crate::world::level::block::entity::decorated_pot_pattern::codec(), json),
        "minecraft:trim_pattern" => decode_with(crate::world::item::equipment::trim::trim_pattern::direct_codec(), json),
        "minecraft:trim_material" => decode_with(crate::world::item::equipment::trim::trim_material::direct_codec(), json),
        "minecraft:jukebox_song" => decode_with(crate::world::item::jukebox_song::direct_codec(), json),
        "minecraft:instrument" => decode_with(crate::world::item::instrument::direct_codec(), json),
        "minecraft:chat_type" => decode_with(crate::network::chat_type::direct_codec(), json),
        "minecraft:wolf_variant" => decode_with(crate::world::entity::animal::wolf::wolf_variant::direct_codec(), json),
        "minecraft:wolf_sound_variant" => decode_with(crate::world::entity::animal::wolf::wolf_sound_variant::direct_codec(), json),
        "minecraft:pig_variant" => decode_with(crate::world::entity::animal::pig::pig_variant::direct_codec(), json),
        "minecraft:pig_sound_variant" => decode_with(crate::world::entity::animal::pig::pig_sound_variant::direct_codec(), json),
        "minecraft:cow_variant" => decode_with(crate::world::entity::animal::cow::cow_variant::direct_codec(), json),
        "minecraft:cow_sound_variant" => decode_with(crate::world::entity::animal::cow::cow_sound_variant::direct_codec(), json),
        "minecraft:chicken_variant" => decode_with(crate::world::entity::animal::chicken_variant::direct_codec(), json),
        "minecraft:chicken_sound_variant" => decode_with(crate::world::entity::animal::chicken_sound_variant::direct_codec(), json),
        "minecraft:frog_variant" => decode_with(crate::world::entity::animal::frog::frog_variant::direct_codec(), json),
        "minecraft:cat_variant" => decode_with(crate::world::entity::animal::feline::cat_variant::direct_codec(), json),
        "minecraft:cat_sound_variant" => decode_with(crate::world::entity::animal::feline::cat_sound_variant::direct_codec(), json),
        "minecraft:zombie_nautilus_variant" => decode_with(crate::world::entity::animal::nautilus::zombie_nautilus_variant::direct_codec(), json),
        "minecraft:world_clock" => decode_with(crate::world::item::component::types::record(Vec::new()), json),
        "minecraft:sulfur_cube_archetype" => decode_with(crate::world::entity::sulfur_cube_archetype::direct_codec(), json),
        "minecraft:timeline" => crate::world::clock::Timeline::parse("", json).and_then(|_| crate::world::clock::encode_network_timeline(json)).map(drop),
        "minecraft:test_environment" => decode_with(crate::gametest::framework::test_environment_definition::direct_codec(), json),
        "minecraft:test_instance" => decode_with(crate::gametest::framework::game_test_instance::direct_codec(), json),
        "minecraft:block_transformer" => decode_with(crate::core::component::block_transformer::direct_codec(), json),
        "minecraft:worldgen/block_state_provider" => decode_with(crate::world::level::levelgen::feature::stateproviders::block_state_provider::direct_codec(), json),
        "minecraft:dialog" => crate::server::dialog::validate(&json_to_nbt(json)),
        "minecraft:dimension_type" => {
            use crate::world::level::dimension::dimension_type;
            let tag = dimension_type::direct_codec().0.decode(&crate::world::item::component::json_to_nbt(json))?;
            dimension_type::check(&tag).map_err(|e| format!("Caught exception decoding {json}: {e}"))?;
            dimension_type::encode_network(json).map(drop)
        }
        _ => Ok(()),
    }
}

fn network_codec(registry: &str) -> Option<crate::world::item::component::types::Pair> {
    Some(match registry {
        "minecraft:damage_type" => crate::world::damagesource::damage_type::direct_codec(),
        "minecraft:banner_pattern" => crate::world::level::block::entity::banner_pattern::direct_codec(),
        "minecraft:decorated_pot_pattern" => crate::world::level::block::entity::decorated_pot_pattern::codec(),
        "minecraft:trim_pattern" => crate::world::item::equipment::trim::trim_pattern::direct_codec(),
        "minecraft:trim_material" => crate::world::item::equipment::trim::trim_material::direct_codec(),
        "minecraft:jukebox_song" => crate::world::item::jukebox_song::direct_codec(),
        "minecraft:instrument" => crate::world::item::instrument::direct_codec(),
        "minecraft:chat_type" => crate::network::chat_type::direct_codec(),
        "minecraft:painting_variant" => crate::world::entity::decoration::painting::painting_variant::direct_codec(),
        "minecraft:wolf_variant" => crate::world::entity::animal::wolf::wolf_variant::network_codec(),
        "minecraft:wolf_sound_variant" => crate::world::entity::animal::wolf::wolf_sound_variant::direct_codec(),
        "minecraft:pig_variant" => crate::world::entity::animal::pig::pig_variant::network_codec(),
        "minecraft:pig_sound_variant" => crate::world::entity::animal::pig::pig_sound_variant::direct_codec(),
        "minecraft:cow_variant" => crate::world::entity::animal::cow::cow_variant::network_codec(),
        "minecraft:cow_sound_variant" => crate::world::entity::animal::cow::cow_sound_variant::direct_codec(),
        "minecraft:chicken_variant" => crate::world::entity::animal::chicken_variant::network_codec(),
        "minecraft:chicken_sound_variant" => crate::world::entity::animal::chicken_sound_variant::direct_codec(),
        "minecraft:frog_variant" => crate::world::entity::animal::frog::frog_variant::network_codec(),
        "minecraft:cat_variant" => crate::world::entity::animal::feline::cat_variant::network_codec(),
        "minecraft:cat_sound_variant" => crate::world::entity::animal::feline::cat_sound_variant::direct_codec(),
        "minecraft:zombie_nautilus_variant" => crate::world::entity::animal::nautilus::zombie_nautilus_variant::network_codec(),
        "minecraft:world_clock" => crate::world::item::component::types::record(Vec::new()),
        "minecraft:sulfur_cube_archetype" => crate::world::entity::sulfur_cube_archetype::direct_codec(),
        "minecraft:enchantment" => crate::world::item::enchantment::enchantment::direct_codec(),
        "minecraft:test_environment" => crate::gametest::framework::test_environment_definition::direct_codec(),
        "minecraft:test_instance" => crate::gametest::framework::game_test_instance::direct_codec(),
        "minecraft:block_transformer" => crate::core::component::block_transformer::direct_codec(),
        "minecraft:worldgen/block_state_provider" => crate::world::level::levelgen::feature::stateproviders::block_state_provider::direct_codec(),
        _ => return None,
    })
}

fn try_encode_network(registry: &str, json: &Value) -> Result<Nbt, String> {
    let tag = crate::world::item::component::json_to_nbt(json);
    crate::nbt::with_json_source(|| match registry {
        "minecraft:dialog" => crate::server::dialog::dialog::decode_direct(&tag),
        "minecraft:worldgen/biome" => crate::world::level::biome::encode_network(json),
        "minecraft:dimension_type" => crate::world::level::dimension::dimension_type::encode_network(json),
        "minecraft:timeline" => crate::world::clock::encode_network_timeline(json),
        _ => match network_codec(registry) {
            Some(codec) => codec.0.decode(&tag),
            None => Ok(tag.clone()),
        },
    })
}

fn encode_network(registry: &str, json: &Value) -> Nbt {
    try_encode_network(registry, json).unwrap_or_else(|_| crate::world::item::component::json_to_nbt(json))
}

fn decode_with(codec: crate::world::item::component::types::Pair, json: &Value) -> Result<(), String> {
    codec.0.decode(&crate::world::item::component::json_to_nbt(json)).map(drop)
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

    #[test]
    fn network_contents_use_the_network_codec() {
        let json = serde_json::json!({
            "assets": {"wild": "a", "tame": "b", "angry": "c"},
            "baby_assets": {"wild": "a", "tame": "b", "angry": "c"},
            "spawn_conditions": [{"priority": 0}]
        });
        let wolf = super::encode_network("minecraft:wolf_variant", &json);
        assert!(wolf.get("spawn_conditions").is_none());
        assert_eq!(wolf.get("assets").and_then(|assets| assets.get("wild")).and_then(|wild| wild.as_str()), Some("minecraft:a"));
        let damage = serde_json::json!({"exhaustion": 1, "message_id": "m", "scaling": "always"});
        let damage = super::encode_network("minecraft:damage_type", &damage);
        assert_eq!(damage.get("exhaustion"), Some(&wafer_protocol::nbt::Nbt::Float(1.0)));
    }

    #[test]
    fn every_vanilla_entry_encodes_for_the_network() {
        for builtin in wafer_data::builtin_synced_registries() {
            for &(id, json) in super::json_entries(builtin.name) {
                let json: serde_json::Value = serde_json::from_str(json).unwrap();
                if let Err(error) = super::try_encode_network(builtin.name, &json) {
                    panic!("{} {id}: {error}", builtin.name);
                }
            }
        }
    }

    #[test]
    fn biomes_are_encoded_with_their_network_codec() {
        use wafer_protocol::nbt::Nbt;
        let json = serde_json::json!({
            "has_precipitation": true,
            "temperature": 1,
            "downfall": 0,
            "effects": {"water_color": 4159204, "grass_color": [0.5, 0.25, 0.125], "foliage_color": "#ABCDEF"},
            "features": [],
            "carvers": [],
            "attributes": {
                "minecraft:visual/fog_color": [0.1, 0.2, 0.3],
                "minecraft:visual/fog_end_distance": 100,
                "minecraft:gameplay/bed_rule": {"can_sleep": "always", "can_set_spawn": "never"},
                "minecraft:visual/sun_angle": {"modifier": "add", "argument": 3}
            }
        });
        let tag = super::encode_network("minecraft:worldgen/biome", &json);
        assert_eq!(tag.get("temperature"), Some(&Nbt::Float(1.0)));
        assert!(tag.get("features").is_none());
        let effects = tag.get("effects").unwrap();
        assert_eq!(effects.get("water_color").and_then(Nbt::as_str), Some("#3f76e4"));
        assert_eq!(effects.get("grass_color").and_then(Nbt::as_str), Some("#7f3f1f"));
        assert_eq!(effects.get("foliage_color").and_then(Nbt::as_str), Some("#abcdef"));
        let attributes = tag.get("attributes").unwrap();
        assert_eq!(attributes.get("minecraft:visual/fog_color").and_then(Nbt::as_str), Some("#19334c"));
        assert_eq!(attributes.get("minecraft:visual/fog_end_distance"), Some(&Nbt::Float(100.0)));
        assert!(attributes.get("minecraft:gameplay/bed_rule").is_none());
        let sun_angle = attributes.get("minecraft:visual/sun_angle").unwrap();
        assert_eq!(sun_angle.get("modifier").and_then(Nbt::as_str), Some("add"));
        assert_eq!(sun_angle.get("argument"), Some(&Nbt::Float(3.0)));
    }

    #[test]
    fn dimension_types_are_encoded_with_their_network_codec() {
        use wafer_protocol::nbt::Nbt;
        let json = serde_json::json!({
            "ambient_light": 0,
            "coordinate_scale": 1,
            "has_ceiling": false,
            "has_ender_dragon_fight": false,
            "has_skylight": true,
            "height": 384,
            "infiniburn": "#minecraft:infiniburn_overworld",
            "logical_height": 384,
            "min_y": -64,
            "monster_spawn_block_light_limit": 0,
            "monster_spawn_light_level": {"type": "minecraft:constant", "value": 7},
            "timelines": [],
            "attributes": {"minecraft:visual/cloud_height": 192, "minecraft:gameplay/respawn_anchor_works": true}
        });
        let tag = super::encode_network("minecraft:dimension_type", &json);
        assert_eq!(tag.get("coordinate_scale"), Some(&Nbt::Double(1.0)));
        assert_eq!(tag.get("ambient_light"), Some(&Nbt::Float(0.0)));
        assert_eq!(tag.get("monster_spawn_light_level"), Some(&Nbt::Int(7)));
        assert!(tag.get("timelines").is_none());
        let attributes = tag.get("attributes").unwrap();
        assert_eq!(attributes.get("minecraft:visual/cloud_height"), Some(&Nbt::Float(192.0)));
        assert!(attributes.get("minecraft:gameplay/respawn_anchor_works").is_none());
    }

    #[test]
    fn timelines_are_encoded_with_their_network_codec() {
        use wafer_protocol::nbt::Nbt;
        let json = serde_json::json!({
            "clock": "overworld",
            "period_ticks": 24000,
            "time_markers": {"ta:a": 5, "b": {"ticks": 6}, "ta:c": {"ticks": 7, "show_in_commands": true}},
            "tracks": {
                "minecraft:visual/fog_start_distance": {"ease": "linear", "keyframes": [{"ticks": 0, "value": 1}]},
                "minecraft:gameplay/villager_activity": {"keyframes": [{"ticks": 0, "value": "minecraft:work"}]},
                "minecraft:visual/sky_color": {"modifier": "override", "ease": {"cubic_bezier": [0, 0.5, 1, 2]}, "keyframes": [{"ticks": 0, "value": [0.5, 0.5, 0.5]}]}
            }
        });
        let tag = super::encode_network("minecraft:timeline", &json);
        assert_eq!(tag.get("clock").and_then(Nbt::as_str), Some("minecraft:overworld"));
        assert_eq!(tag.get("period_ticks"), Some(&Nbt::Int(24000)));
        let markers = tag.get("time_markers").unwrap();
        assert_eq!(markers.get("minecraft:b"), Some(&Nbt::Int(6)));
        assert_eq!(markers.get("ta:c").and_then(|marker| marker.get("show_in_commands")), Some(&Nbt::Byte(1)));
        let tracks = tag.get("tracks").unwrap();
        let fog = tracks.get("minecraft:visual/fog_start_distance").unwrap();
        assert!(fog.get("ease").is_none());
        assert_eq!(fog.get("keyframes").unwrap().as_list()[0].get("value"), Some(&Nbt::Float(1.0)));
        assert!(tracks.get("minecraft:gameplay/villager_activity").is_none());
        let sky = tracks.get("minecraft:visual/sky_color").unwrap();
        assert!(sky.get("modifier").is_none());
        assert_eq!(sky.get("keyframes").unwrap().as_list()[0].get("value").and_then(Nbt::as_str), Some("#7f7f7f"));
        assert!(sky.get("ease").unwrap().get("cubic_bezier").is_some());
    }

    #[test]
    fn block_state_providers_and_transformers_are_encoded_with_their_network_codecs() {
        use wafer_protocol::nbt::Nbt;
        let simple = super::encode_network("minecraft:worldgen/block_state_provider", &serde_json::json!({"type": "simple", "state": "minecraft:oak_stairs"}));
        assert_eq!(simple.get("id").and_then(Nbt::as_str), Some("minecraft:oak_stairs"));
        assert!(simple.get("properties").is_some());
        assert!(simple.get("type").is_none());
        let weighted = super::encode_network(
            "minecraft:worldgen/block_state_provider",
            &serde_json::json!({"type": "weighted", "entries": [{"data": {"id": "minecraft:stone"}, "weight": 1}]}),
        );
        assert_eq!(weighted.get("entries").unwrap().as_list()[0].get("data").and_then(Nbt::as_str), Some("minecraft:stone"));
        let transformer = super::encode_network("minecraft:block_transformer", &serde_json::json!([{"block_state_provider": {"id": "minecraft:dirt"}, "particle": "none", "consume_on_use": true}]));
        let data = &transformer.as_list()[0];
        assert!(data.get("particle").is_none());
        assert!(data.get("consume_on_use").is_none());
        assert!(data.get("block_state_provider").is_some());
    }

    #[test]
    fn test_registries_are_encoded_with_their_network_codecs() {
        use wafer_protocol::nbt::Nbt;
        let environment = super::encode_network(
            "minecraft:test_environment",
            &serde_json::json!({"type": "all_of", "definitions": [{"type": "game_rules", "rules": {"keep_inventory": true, "random_tick_speed": 5.0}}, "minecraft:default"]}),
        );
        let definitions = environment.get("definitions").unwrap().as_list();
        assert_eq!(definitions[0].get("rules").unwrap().get("minecraft:random_tick_speed"), Some(&Nbt::Int(5)));
        assert_eq!(definitions[1].as_str(), Some("minecraft:default"));
        let instance = super::encode_network(
            "minecraft:test_instance",
            &serde_json::json!({"type": "function", "function": "ta:f", "environment": {"type": "weather", "weather": "clear"}, "structure": "s", "max_ticks": 5, "padding": 0, "rotation": "none"}),
        );
        assert_eq!(instance.get("structure").and_then(Nbt::as_str), Some("minecraft:s"));
        assert!(instance.get("padding").is_none());
        assert!(instance.get("rotation").is_none());
    }

    #[test]
    fn enchantments_are_encoded_with_their_network_codec() {
        use wafer_protocol::nbt::Nbt;
        let json = serde_json::json!({
            "anvil_cost": 1,
            "description": "Plain",
            "effects": {
                "minecraft:damage": [{
                    "effect": {"type": "minecraft:add", "value": {"type": "minecraft:linear", "base": 1, "per_level_above_first": 2}},
                    "requirements": {
                        "type": "minecraft:entity_properties",
                        "entity": "attacker",
                        "predicate": {"minecraft:distance": {"x": {"min": 1, "max": 2}, "horizontal": 3}, "minecraft:entity_type": ["zombie"]}
                    }
                }],
                "minecraft:prevent_equipment_drop": {}
            },
            "max_cost": {"base": 3, "per_level_above_first": 4},
            "max_level": 3,
            "min_cost": {"base": 1, "per_level_above_first": 2},
            "slots": ["mainhand"],
            "supported_items": ["stick"],
            "weight": 2
        });
        let tag = super::try_encode_network("minecraft:enchantment", &json).unwrap();
        assert_eq!(tag.get("description").and_then(Nbt::as_str), Some("Plain"));
        assert_eq!(tag.get("supported_items").and_then(Nbt::as_str), Some("minecraft:stick"));
        assert!(tag.get("exclusive_set").is_none());
        let damage = &tag.get("effects").unwrap().get("minecraft:damage").unwrap().as_list()[0];
        let value = damage.get("effect").unwrap().get("value").unwrap();
        assert_eq!(value.get("base"), Some(&Nbt::Float(1.0)));
        let predicate = damage.get("requirements").unwrap().get("predicate").unwrap();
        assert_eq!(predicate.get("minecraft:entity_type").and_then(Nbt::as_str), Some("minecraft:zombie"));
        let distance = predicate.get("minecraft:distance").unwrap();
        assert_eq!(distance.get("horizontal"), Some(&Nbt::Double(3.0)));
        assert_eq!(distance.get("x").unwrap().get("min"), Some(&Nbt::Double(1.0)));
        assert_eq!(tag.get("effects").unwrap().get("minecraft:prevent_equipment_drop"), Some(&Nbt::Compound(Vec::new())));
    }

    #[test]
    fn sulfur_cube_archetypes_are_encoded_with_their_network_codec() {
        use wafer_protocol::nbt::Nbt;
        let tag = super::encode_network(
            "minecraft:sulfur_cube_archetype",
            &serde_json::json!({
                "items": ["stone"],
                "attribute_modifiers": [{"attribute": "knockback_resistance", "id": "ta:x", "amount": 1, "operation": "add_value"}],
                "contact_damage": {"damage_type": "cactus", "amount": 4, "attribute_to_source": true},
                "knockback_modifiers": {"horizontal_power": 1, "vertical_power": 2},
                "sound_settings": {"hit_sound": "entity.sulfur_cube.regular.hit", "push_sound": "entity.sulfur_cube.regular.push", "push_sound_impulse_threshold": 1, "push_sound_cooldown": 2}
            }),
        );
        assert_eq!(tag.get("items").and_then(Nbt::as_str), Some("minecraft:stone"));
        assert!(tag.get("buoyant").is_none());
        assert_eq!(tag.get("contact_damage").unwrap().get("amount"), Some(&Nbt::Float(4.0)));
        assert_eq!(tag.get("knockback_modifiers").unwrap().get("vertical_power"), Some(&Nbt::Float(2.0)));
    }

    #[test]
    fn vanilla_entries_decode() {
        let mut failures = Vec::new();
        let names: Vec<&str> = wafer_data::builtin_synced_registries().iter().map(|registry| registry.name).chain(super::UNSYNCHRONIZED.iter().map(|&(name, _)| name)).collect();
        for name in names {
            for &(id, json) in super::json_entries(name) {
                let json: serde_json::Value = serde_json::from_str(json).unwrap();
                if let Err(e) = crate::nbt::with_json_source(|| super::decode(name, &json)) {
                    failures.push(format!("{name} {id}: {e}"));
                }
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }
}
