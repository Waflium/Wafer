use wafer_core::java_string::JavaStr;
use std::any::Any;
use std::cell::{Cell, RefCell};

use crate::util::dfu_frames;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::{Mutex, OnceLock};

use serde_json::Value;
use wafer_data::SyncedRegistry;
use wafer_protocol::nbt::Nbt;

use crate::server::packs::repository::{VANILLA_ID, has_vanilla_known_pack_info};
use crate::util::lifecycle::Lifecycle;
use crate::world::item::component::json_to_nbt;
use crate::world::level::dimension::level_stem::LevelStem;
use crate::world::level::levelgen::density_function::resource_name;
use crate::world::level::levelgen::presets::world_preset::WorldPreset;
use crate::server::packs::resources::MultiPackResourceManager;
use crate::resources::registry_load_task::{self, ElementFailure};
use crate::server::reloadable_server_registries::LoadingErrors;
use crate::util::gson::JsonElement;
use crate::util::java_throwable::JavaThrowable;

pub struct DataRegistry {
    name: &'static str,
    to_network: Vec<i32>,
    from_network: Vec<i32>,
    contents: Vec<Option<Value>>,
    network: Vec<OnceLock<Nbt>>,
    elements: Vec<Value>,
    json: &'static [(&'static str, &'static str)],
    blocked: Vec<bool>,
    decoded: OnceLock<Box<dyn Any + Send + Sync>>,
}

impl DataRegistry {
    pub fn is_blocked(&self, id: i32) -> bool {
        self.blocked.get(id as usize).copied().unwrap_or(false)
    }

    pub fn size(&self) -> usize {
        self.blocked.iter().filter(|&&blocked| !blocked).count()
    }

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

static REGISTRIES_LIFECYCLE: Mutex<Lifecycle> = Mutex::new(Lifecycle::Stable);

pub fn registries_lifecycle() -> Lifecycle {
    *REGISTRIES_LIFECYCLE.lock().unwrap()
}

static DECODED_DIMENSIONS: Mutex<Vec<(String, LevelStem)>> = Mutex::new(Vec::new());

fn decode_dimension(id: &str, element: &JsonElement) -> Result<(), String> {
    let decoded = LevelStem::decode(&crate::world::item::component::element_to_nbt(element));
    if let Some(error) = decoded.error {
        return Err(error);
    }
    if let Some(stem) = decoded.value {
        let mut dimensions = DECODED_DIMENSIONS.lock().unwrap();
        dimensions.retain(|(decoded_id, _)| decoded_id != id);
        dimensions.push((id.to_string(), stem));
    }
    Ok(())
}

static DECODED_WORLD_PRESETS: Mutex<Vec<(String, WorldPreset)>> = Mutex::new(Vec::new());

fn decode_world_preset(id: &str, element: &JsonElement) -> Result<(), String> {
    let decoded = unwinding(|| vec![dfu_frames::DECODER_PARSE], || WorldPreset::decode(&crate::world::item::component::element_to_nbt(element)));
    if let Some(error) = decoded.error {
        return Err(error);
    }
    if let Some(preset) = decoded.value {
        let mut presets = DECODED_WORLD_PRESETS.lock().unwrap();
        presets.retain(|(decoded_id, _)| decoded_id != id);
        presets.push((id.to_string(), preset));
    }
    Ok(())
}

pub fn decoded_world_preset(id: &str) -> Option<WorldPreset> {
    DECODED_WORLD_PRESETS.lock().unwrap().iter().find(|(decoded_id, _)| decoded_id == id).map(|(_, preset)| preset.clone())
}

pub fn datapack_dimensions() -> Vec<(String, LevelStem)> {
    let decoded = DECODED_DIMENSIONS.lock().unwrap();
    level_stems()
        .iter()
        .filter_map(|(id, json)| match decoded.iter().find(|(decoded_id, _)| decoded_id == id) {
            Some((_, stem)) => Some((id.to_string(), stem.clone())),
            None => crate::nbt::with_json_source(|| LevelStem::decode(&json_to_nbt(json))).value.map(|stem| (id.to_string(), stem)),
        })
        .collect()
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

pub fn ids_in_registry_order(registry: &str) -> Vec<i32> {
    match get(registry) {
        Some(data) => data.network_order().collect(),
        None => (0..crate::world::item::component::codec::registry(registry).len() as i32).collect(),
    }
}

pub fn names_in_registry_order(registry: &str) -> Vec<&'static str> {
    let names = crate::world::item::component::codec::registry(registry);
    ids_in_registry_order(registry).into_iter().map(|id| names[id as usize]).collect()
}

pub fn random_id(registry: &str, random: &mut (impl wafer_core::random::RandomSource + ?Sized)) -> i32 {
    let size = crate::world::item::component::codec::registry(registry).len() as i32;
    from_network_id(registry, random.next_int_bounded(size))
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

static ANY_BLOCKED: AtomicBool = AtomicBool::new(false);

pub fn any_blocked() -> bool {
    ANY_BLOCKED.load(Ordering::Acquire)
}

pub fn is_entry_blocked(registry: &str, id: i32) -> bool {
    ANY_BLOCKED.load(Ordering::Acquire) && get(registry).is_some_and(|data| data.is_blocked(id))
}

pub fn present_json_entries(registry: &str) -> impl Iterator<Item = &'static (&'static str, &'static str)> {
    let entries = json_entries(registry);
    let data = get(registry);
    entries.iter().enumerate().filter(move |&(id, _)| !data.is_some_and(|data| data.is_blocked(id as i32))).map(|(_, entry)| entry)
}

pub fn decoded<T: Any + Send + Sync>(registry: &str, decode: impl FnOnce(&'static [(&'static str, &'static str)]) -> T) -> &'static T {
    let entries = json_entries(registry);
    let data = get(registry).unwrap_or_else(|| panic!("{registry} is not a data registry"));
    data.decoded.get_or_init(|| Box::new(decode(entries))).downcast_ref::<T>().expect("a registry is always decoded to the same type")
}

fn leak(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn dimension_type(name: &'static str, v: &Value) -> wafer_data::DimensionType {
    let int = |key: &str| v[key].as_i64().unwrap_or(0);
    let infiniburn_blocks = |names: Vec<&str>| {
        let ids: Vec<i32> = names.into_iter().filter_map(|name| wafer_data::Block::by_name(&resource_name(name))).map(|block| block.0 as i32).collect();
        wafer_data::direct_tag("minecraft:block", &ids)
    };
    let infiniburn = match &v["infiniburn"] {
        Value::String(name) => match name.strip_prefix('#') {
            Some(tag) => wafer_data::dynamic_tag("minecraft:block", &resource_name(tag)),
            None => infiniburn_blocks(vec![name.as_str()]),
        },
        Value::Array(names) => infiniburn_blocks(names.iter().filter_map(Value::as_str).collect()),
        _ => infiniburn_blocks(Vec::new()),
    };
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
        infiniburn,
        default_clock: v["default_clock"].as_str().map(|clock| leak(clock.to_owned())),
        attributes: leak(v.get("attributes").map_or("{}".to_owned(), Value::to_string)),
        timelines: leak(v["timelines"].to_string()),
        monster_spawn_light_level: leak(v["monster_spawn_light_level"].to_string()),
        monster_spawn_block_light_limit: int("monster_spawn_block_light_limit") as i32,
    }
}

fn damage_type_info(name: &'static str, v: &Value) -> wafer_data::DamageTypeInfo {
    use wafer_data::{DamageEffects, DamageScaling, DeathMessageType};
    wafer_data::DamageTypeInfo {
        name,
        msg_id: leak(v["message_id"].as_str().unwrap_or_default().to_owned()),
        scaling: match v["scaling"].as_str() {
            Some("never") => DamageScaling::Never,
            Some("always") => DamageScaling::Always,
            _ => DamageScaling::WhenCausedByLivingNonPlayer,
        },
        exhaustion: crate::world::level::levelgen::density_function::float(&v["exhaustion"]).unwrap_or(0.0),
        effects: match v["effects"].as_str() {
            Some("thorns") => DamageEffects::Thorns,
            Some("drowning") => DamageEffects::Drowning,
            Some("burning") => DamageEffects::Burning,
            Some("poking") => DamageEffects::Poking,
            Some("freezing") => DamageEffects::Freezing,
            _ => DamageEffects::Hurt,
        },
        death_message_type: match v["death_message_type"].as_str() {
            Some("fall_variants") => DeathMessageType::FallVariants,
            Some("intentional_game_design") => DeathMessageType::IntentionalGameDesign,
            _ => DeathMessageType::Default,
        },
    }
}

pub fn inline_dimension_type(name: &'static str, tag: &Nbt) -> &'static wafer_data::DimensionType {
    let json = crate::nbt::with_json_source(|| crate::util::codec::to_json(tag));
    Box::leak(Box::new(dimension_type(name, &json)))
}

fn file_key(id: &str) -> (Vec<u16>, Vec<u16>) {
    let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
    (format!("{path}.json").java_utf16().collect(), namespace.java_utf16().collect())
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
    ("minecraft:worldgen/flat_level_generator_preset", &[]),
];

pub const DIMENSION_REGISTRY: &str = "minecraft:dimension";

pub fn is_dimension_registry(registry: &str) -> bool {
    registry == DIMENSION_REGISTRY
}

pub fn dynamic_registry_names() -> impl Iterator<Item = &'static str> {
    UNSYNCHRONIZED.iter().map(|&(name, _)| name).chain(std::iter::once(DIMENSION_REGISTRY))
}

pub fn is_dynamic_registry(registry: &str) -> bool {
    registry == DIMENSION_REGISTRY || get(registry).is_some()
}

static TAG_UNBOUND: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

pub fn record_unbound_tag_value(registry: &str, key: &str) {
    TAG_UNBOUND.lock().unwrap().push((registry.to_string(), key.to_string()));
}

static TAG_REFERENCES: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

pub fn record_tag_reference(registry: &str, key: &str) {
    TAG_REFERENCES.lock().unwrap().push((registry.to_string(), key.to_string()));
}

struct PackElement {
    registry: &'static str,
    id: String,
    pack: String,
    resource_count: usize,
    json: Result<JsonElement, JavaThrowable>,
}

static PACK_ELEMENTS: Mutex<Vec<PackElement>> = Mutex::new(Vec::new());

thread_local! {
    static UNBOUND: RefCell<Option<Vec<(String, String)>>> = const { RefCell::new(None) };
    static UNBOUND_TAGS: RefCell<Option<Vec<(String, String)>>> = const { RefCell::new(None) };
    static THROWN: RefCell<Option<JavaThrowable>> = const { RefCell::new(None) };
    static RECORDING: Cell<bool> = const { Cell::new(false) };
    static DEPTH: Cell<usize> = const { Cell::new(0) };
    static UNWOUND_TO: Cell<usize> = const { Cell::new(0) };
    static REFERENCED: RefCell<Option<Vec<(String, String)>>> = const { RefCell::new(None) };
}

pub fn known_reference(registry: &str, key: &str) {
    if !has_thrown() {
        crate::server::reloadable_server_registries::record_reference(registry, key);
    }
    REFERENCED.with(|referenced| {
        if !has_thrown()
            && let Some(referenced) = referenced.borrow_mut().as_mut()
        {
            referenced.push((registry.to_string(), key.to_string()));
        }
    });
}

pub fn throw(exception: &str, message: String, frames: &[&'static str]) {
    throw_at(exception, message, frames, 0);
}

pub fn throw_unwinding(exception: &str, message: String, frames: &[&'static str]) {
    throw_at(exception, message, frames, DEPTH.with(Cell::get));
}

fn throw_at(exception: &str, message: String, frames: &[&'static str], unwind_from: usize) {
    THROWN.with(|thrown| {
        let mut thrown = thrown.borrow_mut();
        if thrown.is_none() {
            *thrown = Some(JavaThrowable::new(format!("{exception}: {message}"), frames.to_vec()));
            UNWOUND_TO.with(|unwound_to| unwound_to.set(unwind_from));
        }
    });
}

pub fn recording<R>(decode: impl FnOnce() -> R) -> R {
    let previous = RECORDING.with(|recording| recording.replace(true));
    DEPTH.with(|depth| depth.set(0));
    let result = decode();
    RECORDING.with(|recording| recording.set(previous));
    result
}

pub fn unwinding<R>(frames: impl FnOnce() -> Vec<&'static str>, decode: impl FnOnce() -> R) -> R {
    if !RECORDING.with(Cell::get) {
        return decode();
    }
    let depth = DEPTH.with(|depth| depth.replace(depth.get() + 1));
    let result = decode();
    DEPTH.with(|current| current.set(depth));
    if depth < UNWOUND_TO.with(Cell::get) {
        THROWN.with(|thrown| {
            if let Some(thrown) = thrown.borrow_mut().as_mut() {
                thrown.push_frames(&frames());
                UNWOUND_TO.with(|unwound_to| unwound_to.set(depth));
            }
        });
    }
    result
}

pub fn unwinding_deferred<R>(decode: impl FnOnce() -> R) -> (R, Option<usize>) {
    if !RECORDING.with(Cell::get) {
        return (decode(), None);
    }
    let depth = DEPTH.with(|depth| depth.replace(depth.get() + 1));
    let result = decode();
    DEPTH.with(|current| current.set(depth));
    if depth < UNWOUND_TO.with(Cell::get) {
        let position = THROWN.with(|thrown| thrown.borrow().as_ref().map(|thrown| thrown.stack_trace.len()));
        if position.is_some() {
            UNWOUND_TO.with(|unwound_to| unwound_to.set(depth));
        }
        return (result, position);
    }
    (result, None)
}

pub fn splice_frames(position: usize, frames: &[&'static str]) {
    THROWN.with(|thrown| {
        if let Some(thrown) = thrown.borrow_mut().as_mut() {
            thrown.insert_frames(position, frames);
        }
    });
}

pub fn take_thrown() -> Option<JavaThrowable> {
    THROWN.with(|thrown| thrown.borrow_mut().take())
}

fn has_thrown() -> bool {
    THROWN.with(|thrown| thrown.borrow().is_some())
}

pub fn unknown_reference(registry: &str, key: &str) -> String {
    UNBOUND.with(|unbound| {
        if !has_thrown()
            && let Some(unbound) = unbound.borrow_mut().as_mut()
        {
            unbound.push((registry.to_string(), key.to_string()));
        }
    });
    unknown_reference_message(registry, key)
}

pub fn missing_tag(registry: &str, tag: &str) -> Option<String> {
    let loading = registry == "minecraft:dimension" || active().is_some_and(|registries| registries.contains_key(registry));
    let recorded = loading
        && !has_thrown()
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

static HIDES_VANILLA: AtomicBool = AtomicBool::new(false);
static TAG_LOAD_FAILED: AtomicBool = AtomicBool::new(false);

pub fn note_tag_load_failure() {
    TAG_LOAD_FAILED.store(true, Ordering::Release);
}

fn read_element(registry: &'static str, id: &str, resource: &crate::server::packs::Resource, resource_count: usize, pack_elements: &mut Vec<PackElement>) -> Option<Value> {
    let json = match resource.supplier.read() {
        Ok(bytes) => crate::util::gson::parse_strict_bytes_element_throwable(&bytes),
        Err(error) => Err(JavaThrowable::new(format!("java.io.IOException: {error}"), Vec::new())),
    };
    let parsed = json.as_ref().ok().cloned().map(crate::util::gson::to_value);
    if resource.source_pack_id != VANILLA_ID {
        pack_elements.push(PackElement { registry, id: id.to_string(), pack: resource.source_pack_id.clone(), resource_count, json });
    }
    parsed
}

fn vanilla_elements(resource_manager: &MultiPackResourceManager) -> Vec<PackElement> {
    let mut elements = Vec::new();
    let registries = wafer_data::builtin_synced_registries().iter().map(|builtin| builtin.name).chain(UNSYNCHRONIZED.iter().map(|&(name, _)| name)).chain(std::iter::once(DIMENSION_REGISTRY));
    for name in registries {
        let directory = name.strip_prefix("minecraft:").unwrap_or(name);
        let resources = resource_manager.list_resources(directory, ".json");
        for (location, resource) in resources.iter().filter(|(_, resource)| resource.source_pack_id == VANILLA_ID) {
            let (namespace, path) = location.split_once(':').unwrap();
            let id = format!("{namespace}:{}", &path[directory.len() + 1..path.len() - ".json".len()]);
            let json = match resource.supplier.read() {
                Ok(bytes) => crate::util::gson::parse_strict_bytes_element_throwable(&bytes),
                Err(error) => Err(JavaThrowable::new(format!("java.io.IOException: {error}"), Vec::new())),
            };
            elements.push(PackElement { registry: name, id, pack: VANILLA_ID.to_string(), resource_count: resources.len(), json });
        }
    }
    elements
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
    let mut blocked = vec![true; entries.len()];
    for (location, resource) in &resources {
        let (namespace, path) = location.split_once(':').unwrap();
        let id = format!("{namespace}:{}", &path[directory.len() + 1..path.len() - ".json".len()]);
        let existing = entries.iter().position(|&entry| entry == id);
        if let Some(index) = existing {
            blocked[index] = false;
        }
        let Some(json) = read_element(name, &id, resource, resources.len(), pack_elements) else { continue };
        let index = match existing {
            Some(index) => index,
            None => {
                entries.push(Box::leak(id.into_boxed_str()));
                contents.push(None);
                elements.push(Value::Null);
                blocked.push(false);
                entries.len() - 1
            }
        };
        if resource.source_pack_id != VANILLA_ID {
            contents[index] = Some(json.clone());
        }
        elements[index] = json;
    }
    let mut from_network: Vec<i32> = (0..entries.len() as i32).filter(|&id| !blocked[id as usize]).collect();
    from_network.sort_by_key(|&id| file_key(entries[id as usize]));
    let mut to_network = vec![0; entries.len()];
    for (network_id, &id) in from_network.iter().enumerate() {
        to_network[id as usize] = network_id as i32;
    }
    let json: Vec<(&'static str, &'static str)> = entries.iter().zip(&elements).map(|(&name, element)| (name, &*Box::leak(element.to_string().into_boxed_str()))).collect();
    let network = (0..entries.len()).map(|_| OnceLock::new()).collect();
    (DataRegistry { name, to_network, from_network, contents, network, elements, json: Box::leak(json.into_boxed_slice()), blocked, decoded: OnceLock::new() }, Box::leak(entries.into_boxed_slice()))
}

pub fn load(resource_manager: &MultiPackResourceManager) {
    DECODED_DIMENSIONS.lock().unwrap().clear();
    DECODED_WORLD_PRESETS.lock().unwrap().clear();
    let mut pack_elements = Vec::new();
    let mut synced = Vec::new();
    let mut registries = HashMap::new();
    HIDES_VANILLA.store(resource_manager.hides_vanilla_resources(), Ordering::Release);
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
    let lifecycle = if pack_elements.iter().any(|element| !has_vanilla_known_pack_info(&element.pack)) { Lifecycle::Experimental } else { Lifecycle::Stable };
    *REGISTRIES_LIFECYCLE.lock().unwrap() = lifecycle;
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
    unsynchronized.push((DIMENSION_REGISTRY, &*Box::leak(stems.iter().map(|&(id, _)| id).collect::<Vec<_>>().into_boxed_slice())));
    wafer_data::set_data_registry_entries(unsynchronized);
    LEVEL_STEMS.store(Box::into_raw(Box::new(stems)), Ordering::Release);
    let dimension_registry = &registries["minecraft:dimension_type"];
    let dimension_entries = wafer_data::SyncedRegistry::get("minecraft:dimension_type").entries;
    wafer_data::set_dimension_types(dimension_entries.iter().zip(&dimension_registry.elements).map(|(&name, json)| dimension_type(name, json)).collect());
    let damage_type_entries = wafer_data::SyncedRegistry::get("minecraft:damage_type").entries;
    wafer_data::set_damage_types(damage_type_entries.iter().zip(&registries["minecraft:damage_type"].elements).map(|(&name, json)| damage_type_info(name, json)).collect());
    let registries: &'static HashMap<&'static str, DataRegistry> = Box::leak(Box::new(registries));
    ANY_BLOCKED.store(registries.values().any(|registry| registry.blocked.contains(&true)), Ordering::Release);
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

pub(crate) fn with_failed_references(mut unbound: Vec<(String, String)>, referenced: Vec<(String, String)>, failed: &[(&str, &str)]) -> Vec<(String, String)> {
    unbound.extend(referenced.into_iter().filter(|(registry, key)| failed.iter().any(|(failed_registry, failed_key)| failed_registry == registry && failed_key == key)));
    unbound
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

const NON_EMPTY_REGISTRIES: [&str; 13] = [
    "minecraft:wolf_variant",
    "minecraft:wolf_sound_variant",
    "minecraft:pig_variant",
    "minecraft:pig_sound_variant",
    "minecraft:frog_variant",
    "minecraft:cat_variant",
    "minecraft:cat_sound_variant",
    "minecraft:cow_variant",
    "minecraft:cow_sound_variant",
    "minecraft:chicken_variant",
    "minecraft:chicken_sound_variant",
    "minecraft:zombie_nautilus_variant",
    "minecraft:painting_variant",
];

fn validate_non_empty(errors: &mut LoadingErrors) {
    for registry in NON_EMPTY_REGISTRIES {
        if get(registry).is_none_or(|data| data.size() == 0) {
            errors.0.insert(("minecraft:root".to_string(), registry.to_string()), registry_load_task::registry_must_be_non_empty(registry));
        }
    }
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

pub fn validate(resource_manager: &MultiPackResourceManager) -> Result<(), JavaThrowable> {
    let mut pack_elements = std::mem::take(&mut *PACK_ELEMENTS.lock().unwrap());
    let tag_load_failed = TAG_LOAD_FAILED.swap(false, Ordering::AcqRel);
    if HIDES_VANILLA.load(Ordering::Acquire) || !pack_elements.is_empty() || tag_load_failed {
        pack_elements.extend(vanilla_elements(resource_manager));
    }
    let (dimensions, worldgen): (Vec<&PackElement>, Vec<&PackElement>) = pack_elements.iter().partition(|element| element.registry == DIMENSION_REGISTRY);
    validate_registries(&worldgen, true, std::mem::take(&mut *TAG_UNBOUND.lock().unwrap()), std::mem::take(&mut *TAG_REFERENCES.lock().unwrap()))?;
    crate::tags::load_tags_for_dimension_registry(resource_manager);
    validate_registries(&dimensions, false, std::mem::take(&mut *TAG_UNBOUND.lock().unwrap()), std::mem::take(&mut *TAG_REFERENCES.lock().unwrap()))
}

fn validate_registries(pack_elements: &[&PackElement], worldgen_stage: bool, tag_unbound: Vec<(String, String)>, tag_references: Vec<(String, String)>) -> Result<(), JavaThrowable> {
    let mut errors = LoadingErrors(Default::default());
    UNBOUND.with(|unbound| unbound.replace(worldgen_stage.then(Vec::new)));
    UNBOUND_TAGS.with(|unbound| unbound.replace(worldgen_stage.then(Vec::new)));
    REFERENCED.with(|referenced| referenced.replace(worldgen_stage.then(Vec::new)));
    let mut failed: Vec<(&str, &str)> = Vec::new();
    let mut duplicate_values = DuplicateValues::default();
    for element in pack_elements {
        take_thrown();
        let decoded = match &element.json {
            Ok(json) if element.registry == DIMENSION_REGISTRY => crate::nbt::with_json_source(|| decode_dimension(&element.id, json)).map_err(ElementFailure::Decode),
            Ok(json) if element.registry == "minecraft:worldgen/world_preset" => crate::nbt::with_json_source(|| recording(|| decode_world_preset(&element.id, json))).map_err(ElementFailure::Decode),
            Ok(json) => crate::nbt::with_json_source(|| recording(|| decode(element.registry, json))).map_err(ElementFailure::Decode),
            Err(throwable) => Err(ElementFailure::Parse(throwable.clone())),
        };
        let decoded = match take_thrown() {
            Some(thrown) => Err(ElementFailure::Throw(thrown)),
            None => decoded,
        };
        let unbound = UNBOUND.with(|unbound| unbound.borrow().as_ref().and_then(|unbound| unbound.last().map(|(registry, key)| unknown_reference_message(registry, key))));
        match decoded {
            Err(ElementFailure::Decode(cause)) if unbound.as_ref() == Some(&cause) => {}
            Err(failure) => {
                failed.push((element.registry, element.id.as_str()));
                errors.parse_error(element.registry, &element.id, &element.pack, element.resource_count, failure);
                continue;
            }
            Ok(()) => {}
        }
        if let Ok(json) = &element.json {
            duplicate_values.register(element.registry, json);
        }
    }
    if let Some(duplicate) = duplicate_values.duplicate {
        return Err(duplicate);
    }
    let unbound_references = UNBOUND.with(|unbound| unbound.take()).unwrap_or_default();
    let referenced = REFERENCED.with(|referenced| referenced.take()).unwrap_or_default();
    let mut unbound_values = with_failed_references(unbound_references, referenced, &failed);
    unbound_values.extend(tag_unbound);
    unbound_values.extend(tag_references.into_iter().filter(|reference| errors.0.contains_key(reference)));
    let unbound_values = group_unbound(unbound_values);
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
    if errors.0.is_empty() && worldgen_stage {
        validation_stage = true;
        validate_non_empty(&mut errors);
        validate_timelines(&mut errors);
    }
    if errors.0.is_empty() {
        return Ok(());
    }
    error!("{}", errors.report());
    Err(registry_load_task::registry_loading_failed(validation_stage))
}

pub fn singleton_value(registry: &str, tag: &Nbt) -> Option<(&'static str, String)> {
    let name = |key: &str| {
        let name = tag.get(key)?.as_str()?;
        name.strip_prefix("minecraft:").or_else(|| (!name.contains(':')).then_some(name))
    };
    let object_hash = |class: &str| {
        let hash = class.java_utf16().fold(0i32, |hash, unit| hash.wrapping_mul(31).wrapping_add(unit as i32));
        format!("{class}@{hash:x}")
    };
    match registry {
        "minecraft:worldgen/density_function" => match name("type")? {
            "blend_alpha" => Some(("blend_alpha", "BLEND_ALPHA".to_string())),
            "blend_offset" => Some(("blend_offset", "BLEND_OFFSET".to_string())),
            "beardifier" => Some(("beardifier", "BEARDIFIER".to_string())),
            "end_outer_islands" => Some(("end_outer_islands", "EndIslandFunction[]".to_string())),
            _ => None,
        },
        "minecraft:worldgen/material_rule" => (name("type")? == "bandlands").then(|| ("bandlands", "INSTANCE".to_string())),
        "minecraft:worldgen/material_condition" => match name("type")? {
            "hole" => Some(("hole", "INSTANCE".to_string())),
            "steep" => Some(("steep", "INSTANCE".to_string())),
            "temperature" => Some(("temperature", "INSTANCE".to_string())),
            "above_preliminary_surface" => Some(("above_preliminary_surface", "INSTANCE".to_string())),
            _ => None,
        },
        "minecraft:predicate" => match name("type")? {
            "killed_by_player" => Some(("killed_by_player", object_hash("net.minecraft.world.level.storage.loot.predicates.LootItemKilledByPlayerCondition"))),
            "survives_explosion" => Some(("survives_explosion", object_hash("net.minecraft.world.level.storage.loot.predicates.ExplosionCondition"))),
            _ => None,
        },
        "minecraft:slot_source" => (name("type")? == "empty").then(|| ("empty", "EmptySlotSource[]".to_string())),
        "minecraft:recipe" => (name("type")? == "crafting_special_repairitem").then(|| ("crafting_special_repairitem", object_hash("net.minecraft.world.item.crafting.RepairItemRecipe"))),
        _ => None,
    }
}

#[derive(Default)]
pub struct DuplicateValues {
    registered: Vec<(&'static str, &'static str)>,
    pub duplicate: Option<JavaThrowable>,
}

impl DuplicateValues {
    pub fn register(&mut self, registry: &'static str, element: &JsonElement) {
        if self.duplicate.is_some()
            || !matches!(
                registry,
                "minecraft:worldgen/density_function"
                    | "minecraft:worldgen/material_rule"
                    | "minecraft:worldgen/material_condition"
                    | "minecraft:predicate"
                    | "minecraft:slot_source"
                    | "minecraft:recipe"
            )
        {
            return;
        }
        let Some((key, text)) = singleton_value(registry, &crate::world::item::component::element_to_nbt(element)) else { return };
        if self.registered.contains(&(registry, key)) {
            self.duplicate = Some(registry_load_task::duplicate_value(&text));
        } else {
            self.registered.push((registry, key));
        }
    }
}

fn decode(registry: &str, element: &JsonElement) -> Result<(), String> {
    use crate::world::level::levelgen;
    let tag = crate::world::item::component::element_to_nbt(element);
    let json = || crate::util::gson::to_value(element.clone());
    match registry {
        "minecraft:worldgen/material_rule" => decode_with(levelgen::material::rule::material_rule::direct_codec(), &tag),
        "minecraft:worldgen/material_condition" => decode_with(levelgen::material::condition::material_condition::direct_codec(), &tag),
        "minecraft:worldgen/noise" => decode_with(crate::world::level::levelgen::synth::parameters_codec(), &tag),
        "minecraft:worldgen/density_function" => decode_with(levelgen::density_function::direct_codec(), &tag),
        "minecraft:worldgen/noise_settings" => decode_with(levelgen::noise_settings_direct_codec(), &tag),
        "minecraft:worldgen/feature" => decode_with(levelgen::feature::feature::direct_codec(), &tag),
        "minecraft:worldgen/placed_feature" => decode_with(levelgen::placement::placed_feature::direct_codec(), &tag),
        "minecraft:worldgen/structure" => decode_with(levelgen::structure::structure::direct_codec(), &tag),
        "minecraft:worldgen/structure_set" => decode_with(levelgen::structure::structure_set::direct_codec(), &tag),
        "minecraft:worldgen/template_pool" => decode_with(levelgen::structure::pools::structure_template_pool::direct_codec(), &tag),
        "minecraft:worldgen/processor_list" => decode_with(levelgen::structure::templatesystem::structure_processor::list_direct_codec(), &tag),
        "minecraft:worldgen/configured_carver" | "minecraft:worldgen/carver" => decode_with(levelgen::carver::direct_codec(), &tag),
        "minecraft:worldgen/world_preset" => parsing(|| levelgen::presets::world_preset::WorldPreset::validate(&tag)),
        "minecraft:worldgen/flat_level_generator_preset" => parsing(|| levelgen::flat::decode_preset(&tag).into_result()).map(drop),
        "minecraft:worldgen/multi_noise_biome_source_parameter_list" => crate::world::level::biome::decode_parameter_list(&tag),
        "minecraft:worldgen/biome" => crate::world::level::biome::validate(&tag),
        "minecraft:dimension" => LevelStem::validate(&tag),
        "minecraft:enchantment" => {
            decode_with(crate::world::item::enchantment::enchantment::direct_codec(), &tag)?;
            crate::world::item::enchantment::enchantment::Enchantment::decode(&tag).map(drop)
        }
        "minecraft:enchantment_provider" => {
            use crate::world::item::enchantment::providers::enchantment_provider;
            decode_with(enchantment_provider::direct_codec(), &tag)?;
            enchantment_provider::EnchantmentProvider::parse(&json()).map(drop)
        }
        "minecraft:villager_trade" => crate::world::item::trading::villager_trade::VillagerTrade::decode(&tag).map(drop),
        "minecraft:trade_set" => crate::world::item::trading::trade_set::TradeSet::decode(&tag).map(drop),
        "minecraft:trial_spawner" => crate::world::level::block::entity::trialspawner::trial_spawner_config::TrialSpawnerConfig::decode_direct(&tag).map(drop),
        "minecraft:painting_variant" => decode_with(crate::world::entity::decoration::painting::painting_variant::direct_codec(), &tag),
        "minecraft:damage_type" => decode_with(crate::world::damagesource::damage_type::direct_codec(), &tag),
        "minecraft:banner_pattern" => decode_with(crate::world::level::block::entity::banner_pattern::direct_codec(), &tag),
        "minecraft:decorated_pot_pattern" => decode_with(crate::world::level::block::entity::decorated_pot_pattern::codec(), &tag),
        "minecraft:trim_pattern" => decode_with(crate::world::item::equipment::trim::trim_pattern::direct_codec(), &tag),
        "minecraft:trim_material" => decode_with(crate::world::item::equipment::trim::trim_material::direct_codec(), &tag),
        "minecraft:jukebox_song" => decode_with(crate::world::item::jukebox_song::direct_codec(), &tag),
        "minecraft:instrument" => decode_with(crate::world::item::instrument::direct_codec(), &tag),
        "minecraft:chat_type" => decode_with(crate::network::chat_type::direct_codec(), &tag),
        "minecraft:wolf_variant" => decode_with(crate::world::entity::animal::wolf::wolf_variant::direct_codec(), &tag),
        "minecraft:wolf_sound_variant" => decode_with(crate::world::entity::animal::wolf::wolf_sound_variant::direct_codec(), &tag),
        "minecraft:pig_variant" => decode_with(crate::world::entity::animal::pig::pig_variant::direct_codec(), &tag),
        "minecraft:pig_sound_variant" => decode_with(crate::world::entity::animal::pig::pig_sound_variant::direct_codec(), &tag),
        "minecraft:cow_variant" => decode_with(crate::world::entity::animal::cow::cow_variant::direct_codec(), &tag),
        "minecraft:cow_sound_variant" => decode_with(crate::world::entity::animal::cow::cow_sound_variant::direct_codec(), &tag),
        "minecraft:chicken_variant" => decode_with(crate::world::entity::animal::chicken_variant::direct_codec(), &tag),
        "minecraft:chicken_sound_variant" => decode_with(crate::world::entity::animal::chicken_sound_variant::direct_codec(), &tag),
        "minecraft:frog_variant" => decode_with(crate::world::entity::animal::frog::frog_variant::direct_codec(), &tag),
        "minecraft:cat_variant" => decode_with(crate::world::entity::animal::feline::cat_variant::direct_codec(), &tag),
        "minecraft:cat_sound_variant" => decode_with(crate::world::entity::animal::feline::cat_sound_variant::direct_codec(), &tag),
        "minecraft:zombie_nautilus_variant" => decode_with(crate::world::entity::animal::nautilus::zombie_nautilus_variant::direct_codec(), &tag),
        "minecraft:world_clock" => decode_with(crate::world::item::component::types::record(Vec::new()), &tag),
        "minecraft:sulfur_cube_archetype" => decode_with(crate::world::entity::sulfur_cube_archetype::direct_codec(), &tag),
        "minecraft:timeline" => decode_with(crate::world::clock::direct_codec(), &tag),
        "minecraft:test_environment" => decode_with(crate::gametest::framework::test_environment_definition::direct_codec(), &tag),
        "minecraft:test_instance" => decode_with(crate::gametest::framework::game_test_instance::direct_codec(), &tag),
        "minecraft:block_transformer" => decode_with(crate::core::component::block_transformer::direct_codec(), &tag),
        "minecraft:worldgen/block_state_provider" => decode_with(crate::world::level::levelgen::feature::stateproviders::block_state_provider::direct_codec(), &tag),
        "minecraft:dialog" => crate::server::dialog::validate(&tag),
        "minecraft:dimension_type" => {
            use crate::world::level::dimension::dimension_type;
            let caught = |message: &str| format!("Caught exception decoding {}: {message}", crate::util::gson::to_json_string(element));
            let decoded = match dimension_type::direct_codec().0.decode(&tag) {
                Ok(decoded) => decoded,
                Err(error) => match take_thrown() {
                    Some(thrown) => return Err(caught(thrown.description.split_once(": ").map_or(thrown.description.as_str(), |(_, message)| message))),
                    None => return Err(error),
                },
            };
            if let Some(thrown) = take_thrown() {
                return Err(caught(thrown.description.split_once(": ").map_or(thrown.description.as_str(), |(_, message)| message)));
            }
            dimension_type::check(&decoded).map_err(|e| caught(&e))?;
            dimension_type::encode_network(&json()).map(drop)
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

fn decode_with(codec: crate::world::item::component::types::Pair, tag: &Nbt) -> Result<(), String> {
    parsing(|| codec.0.decode_partial(tag).into_result()).map(drop)
}

pub fn parsing<T>(decode: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let result = unwinding(|| vec![dfu_frames::DECODER_PARSE], decode);
    match THROWN.with(|thrown| thrown.borrow().as_ref().map(|thrown| thrown.description.clone())) {
        Some(description) => Err(description),
        None => result,
    }
}

#[cfg(test)]
mod tests {
    use crate::server::packs::repository::{PackRepository, VANILLA_ID};
    use crate::server::packs::resources::MultiPackResourceManager;
    use crate::util::gson::JsonElement;

    fn element(json: &serde_json::Value) -> JsonElement {
        crate::util::gson::parse_lenient(&json.to_string()).expect("valid JSON")
    }

    #[test]
    fn references_to_entries_that_failed_to_decode_stay_unbound() {
        let pair = |registry: &str, key: &str| (registry.to_string(), key.to_string());
        let unbound = vec![pair("minecraft:worldgen/feature", "ta:missing")];
        let referenced = vec![pair("minecraft:worldgen/feature", "ta:broken"), pair("minecraft:worldgen/feature", "ta:fine"), pair("minecraft:worldgen/noise", "ta:broken")];
        let failed = [("minecraft:worldgen/feature", "ta:broken"), ("minecraft:worldgen/feature", "ta:unreferenced")];
        let grouped = super::group_unbound(super::with_failed_references(unbound, referenced, &failed));
        assert_eq!(grouped.get("minecraft:worldgen/feature").unwrap(), &vec!["ta:broken".to_string(), "ta:missing".to_string()]);
        assert!(grouped.get("minecraft:worldgen/noise").is_none());
    }

    #[test]
    fn vanilla_worldgen_entries_decode_with_their_codecs() {
        let mut failures = Vec::new();
        for registry in [
            "minecraft:worldgen/noise",
            "minecraft:worldgen/density_function",
            "minecraft:worldgen/noise_settings",
            "minecraft:worldgen/material_rule",
            "minecraft:worldgen/material_condition",
            "minecraft:worldgen/feature",
            "minecraft:worldgen/placed_feature",
            "minecraft:worldgen/structure",
            "minecraft:worldgen/structure_set",
            "minecraft:worldgen/template_pool",
            "minecraft:worldgen/processor_list",
            "minecraft:worldgen/carver",
        ] {
            for (name, source) in super::json_entries(registry) {
                let json: serde_json::Value = serde_json::from_str(source).unwrap();
                if let Err(error) = crate::nbt::with_json_source(|| super::decode(registry, &element(&json))) {
                    failures.push(format!("{registry} {name}: {error}"));
                }
            }
        }
        assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.join("\n"));
    }

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
    fn registry_order_is_a_permutation_sorted_by_path_then_namespace() {
        for registry in ["minecraft:enchantment", "minecraft:wolf_variant", "minecraft:cat_sound_variant"] {
            super::json_entries(registry);
            let ids = super::ids_in_registry_order(registry);
            let mut sorted_ids = ids.clone();
            sorted_ids.sort_unstable();
            assert_eq!(sorted_ids, (0..ids.len() as i32).collect::<Vec<_>>(), "{registry}");
            let names = super::names_in_registry_order(registry);
            let mut expected = names.clone();
            expected.sort_by(|a, b| {
                let (a_namespace, a_path) = a.split_once(':').unwrap();
                let (b_namespace, b_path) = b.split_once(':').unwrap();
                a_path.cmp(b_path).then(a_namespace.cmp(b_namespace))
            });
            assert_eq!(names, expected, "{registry}");
        }
    }

    #[test]
    fn a_random_id_follows_the_registry_order() {
        super::json_entries("minecraft:enchantment");
        let order = super::ids_in_registry_order("minecraft:enchantment");
        let mut random = wafer_core::random::LegacyRandomSource::new(7);
        let mut mirror = wafer_core::random::LegacyRandomSource::new(7);
        for _ in 0..64 {
            let index = wafer_core::random::RandomSource::next_int_bounded(&mut mirror, order.len() as i32);
            assert_eq!(super::random_id("minecraft:enchantment", &mut random), order[index as usize]);
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
        assert_eq!(tag.get("effects").unwrap().get("minecraft:prevent_equipment_drop"), Some(&Nbt::Compound(Vec::new().into())));
    }

    fn decode_error(registry: &str, json: serde_json::Value) -> String {
        crate::nbt::with_json_source(|| super::decode(registry, &element(&json))).expect_err("a decode error")
    }

    #[test]
    fn decode_errors_quote_number_literals_as_written() {
        for literal in ["1e-9", "0.50", "2.9802322e-8", "100000000000000000000", "18446744073709551615", "-0", "1E5"] {
            let json = format!(r#"{{"type":"minecraft:constant","argument":{literal}}}"#);
            let element = crate::util::gson::parse_strict(&json).unwrap();
            let error = crate::nbt::with_json_source(|| super::decode("minecraft:worldgen/density_function", &element)).expect_err("a decode error");
            assert_eq!(error, format!("Failed to parse either. First: Not a number: {json}; Second: No key value in MapLike[{json}]"));
        }
    }

    #[test]
    fn a_second_value_decoding_to_the_same_instance_is_a_duplicate() {
        let element = |json: &str| crate::util::gson::parse_strict(json).unwrap();
        let mut values = super::DuplicateValues::default();
        values.register("minecraft:worldgen/density_function", &element(r#"{"type":"minecraft:blend_alpha"}"#));
        values.register("minecraft:worldgen/density_function", &element(r#"{"type":"minecraft:blend_offset"}"#));
        values.register("minecraft:worldgen/material_rule", &element(r#"{"type":"minecraft:bandlands"}"#));
        values.register("minecraft:worldgen/noise", &element(r#"{"type":"minecraft:bandlands"}"#));
        values.register("minecraft:worldgen/noise", &element(r#"{"type":"minecraft:bandlands"}"#));
        assert!(values.duplicate.is_none());
        values.register("minecraft:worldgen/density_function", &element(r#"{"type":"blend_alpha","ignored":1}"#));
        assert_eq!(values.duplicate.unwrap().description, "java.lang.IllegalStateException: Adding duplicate value 'BLEND_ALPHA' to registry");
        let mut values = super::DuplicateValues::default();
        values.register("minecraft:worldgen/material_condition", &element(r#"{"type":"minecraft:hole"}"#));
        values.register("minecraft:worldgen/material_condition", &element(r#"{"type":"minecraft:hole"}"#));
        assert_eq!(values.duplicate.unwrap().description, "java.lang.IllegalStateException: Adding duplicate value 'INSTANCE' to registry");
    }

    #[test]
    fn a_null_inside_a_translation_argument_throws_like_json_ops() {
        let message = "java.lang.NullPointerException: Cannot invoke \"com.google.gson.JsonElement.getAsJsonPrimitive()\" because \"input\" is null";
        for arguments in [r#"[[null]]"#, r#"[{"text":null}]"#, r#"[{"a":{"b":[null]}}]"#] {
            let json = format!(r#"{{"sound_event":"minecraft:music_disc.13","description":{{"translate":"a","with":{arguments}}},"length_in_seconds":10,"comparator_output":1}}"#);
            let element = crate::util::gson::parse_strict(&json).unwrap();
            super::take_thrown();
            let result = crate::nbt::with_json_source(|| super::decode("minecraft:jukebox_song", &element));
            assert!(result.is_err());
            assert_eq!(super::take_thrown().unwrap().description, message);
        }
        let json = r#"{"sound_event":"minecraft:music_disc.13","description":{"translate":"a","with":[null]},"length_in_seconds":10,"comparator_output":1}"#;
        let element = crate::util::gson::parse_strict(json).unwrap();
        super::take_thrown();
        crate::nbt::with_json_source(|| super::decode("minecraft:jukebox_song", &element)).ok();
        assert!(super::take_thrown().is_none());
    }

    fn thrown_while_decoding(registry: &str, json: &str) -> String {
        let element = crate::util::gson::parse_strict(json).unwrap();
        super::take_thrown();
        let decoded = crate::nbt::with_json_source(|| super::recording(|| super::decode(registry, &element)));
        assert!(decoded.is_err());
        super::take_thrown().expect("the decoder throws").print_stack_trace()
    }

    #[test]
    fn number_format_exceptions_carry_the_codec_path_up_to_the_registry_root() {
        let jukebox = r#"{"sound_event":"minecraft:music_disc.13","description":"x","length_in_seconds":10,"comparator_output":1E+10000}"#;
        assert_eq!(thrown_while_decoding("minecraft:jukebox_song", jukebox), include_str!("dfu_traces/jukebox_comparator_number_format.txt"));
        let structure = r#"{"type":"minecraft:buried_treasure","biomes":"minecraft:plains","step":"surface_structures","spawn_overrides":{"monster":{"bounding_box":"full","spawns":[{"type":"minecraft:pig","count":1,"weight":1E+10000}]}}}"#;
        assert_eq!(thrown_while_decoding("minecraft:worldgen/structure", structure), include_str!("dfu_traces/structure_spawn_weight_number_format.txt"));
    }

    #[test]
    fn an_overflowing_sum_of_weights_carries_the_codec_path_up_to_the_registry_root() {
        let structure = r#"{"type":"minecraft:buried_treasure","biomes":"minecraft:plains","step":"surface_structures","spawn_overrides":{"monster":{"bounding_box":"full","spawns":[{"type":"minecraft:pig","count":1,"weight":2147483647},{"type":"minecraft:pig","count":1,"weight":2147483647}]}}}"#;
        assert_eq!(thrown_while_decoding("minecraft:worldgen/structure", structure), include_str!("dfu_traces/structure_spawn_weights_overflow.txt"));
    }

    #[test]
    fn validate_and_xmap_wrappers_and_pair_map_codecs_add_their_frames() {
        let jigsaw = r#"{"type":"minecraft:jigsaw","biomes":"minecraft:plains","step":"surface_structures","spawn_overrides":{},"start_pool":"minecraft:village/plains/town_centers","size":1E+10000,"start_height":{"absolute":0},"use_expansion_hack":false,"max_distance_from_center":80}"#;
        assert_eq!(thrown_while_decoding("minecraft:worldgen/structure", jigsaw), include_str!("dfu_traces/structure_jigsaw_size_number_format.txt"));
        let spread = r#"{"structures":[{"structure":"minecraft:village_plains","weight":1}],"placement":{"type":"minecraft:random_spread","spacing":1E+10000,"separation":8,"salt":1}}"#;
        assert_eq!(thrown_while_decoding("minecraft:worldgen/structure_set", spread), include_str!("dfu_traces/structure_set_random_spread_spacing_number_format.txt"));
        let pool = r#"{"fallback":"minecraft:empty","elements":[{"element":{"element_type":"minecraft:empty_pool_element"},"weight":1E+10000}]}"#;
        assert_eq!(thrown_while_decoding("minecraft:worldgen/template_pool", pool), include_str!("dfu_traces/template_pool_weight_number_format.txt"));
        let placed = r#"{"feature":"minecraft:ore_coal","placement":[{"type":"minecraft:count","count":1E+10000}]}"#;
        assert_eq!(thrown_while_decoding("minecraft:worldgen/placed_feature", placed), include_str!("dfu_traces/placed_feature_count_number_format.txt"));
        let enchantment = r##"{"anvil_cost":1,"description":"Plain","effects":{},"max_cost":{"base":3,"per_level_above_first":4},"max_level":1E+10000,"min_cost":{"base":1,"per_level_above_first":2},"slots":["mainhand"],"supported_items":"#minecraft:swords","weight":5}"##;
        assert_eq!(thrown_while_decoding("minecraft:enchantment", enchantment), include_str!("dfu_traces/enchantment_max_level_number_format.txt"));
    }

    #[test]
    fn a_null_inside_translation_arguments_carries_the_json_ops_and_component_frames() {
        let jukebox = |description: &str| format!(r#"{{"sound_event":"minecraft:music_disc.13","description":{description},"length_in_seconds":10,"comparator_output":1}}"#);
        let cases = [
            (r#"{"translate":"a","with":[[[null]]]}"#, include_str!("dfu_traces/translate_with_nested_null.txt")),
            (r#"{"translate":"a","with":[[{"a":null}]]}"#, include_str!("dfu_traces/translate_with_list_map_null.txt")),
            (r#"{"text":"x","extra":[{"translate":"a","with":[[null]]}]}"#, include_str!("dfu_traces/translate_with_extra_null.txt")),
            (r#"{"type":"translatable","translate":"a","with":[[null]]}"#, include_str!("dfu_traces/translate_with_typed_null.txt")),
            (r#"[{"translate":"a","with":[[null]]}]"#, include_str!("dfu_traces/translate_with_list_root_null.txt")),
        ];
        for (description, expected) in cases {
            assert_eq!(thrown_while_decoding("minecraft:jukebox_song", &jukebox(description)), expected, "{description}");
        }
    }

    fn thrown_while_decoding_loot(decode: impl FnOnce(&wafer_protocol::nbt::Nbt), json: &str) -> String {
        let element = crate::util::gson::parse_strict(json).unwrap();
        super::take_thrown();
        crate::nbt::with_json_source(|| {
            super::recording(|| super::unwinding(|| vec![crate::util::dfu_frames::DECODER_PARSE], || decode(&crate::world::item::component::element_to_nbt(&element))))
        });
        super::take_thrown().expect("the decoder throws").print_stack_trace()
    }

    #[test]
    fn hand_written_loot_decoders_carry_the_codec_path_up_to_the_registry_root() {
        use crate::world::level::storage::loot::loot_table::LootTable;
        use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;
        let table = |tag: &wafer_protocol::nbt::Nbt| drop(LootTable::decode(tag));
        let condition = |tag: &wafer_protocol::nbt::Nbt| drop(LootItemCondition::decode(tag));
        assert_eq!(thrown_while_decoding_loot(table, r#"{"pools":[{"rolls":1E+10000,"entries":[]}]}"#), include_str!("dfu_traces/loot_table_rolls_number_format.txt"));
        assert_eq!(
            thrown_while_decoding_loot(table, r#"{"pools":[{"rolls":1,"entries":[{"type":"minecraft:item","name":"minecraft:stick","weight":1E+10000}]}]}"#),
            include_str!("dfu_traces/loot_table_entry_weight_number_format.txt")
        );
        assert_eq!(
            thrown_while_decoding_loot(table, r#"{"pools":[{"rolls":1,"entries":[],"condition":{"type":"minecraft:time_check","clock":"minecraft:overworld","value":{"min":1},"period":1E+10000}}]}"#),
            include_str!("dfu_traces/loot_table_pool_condition_number_format.txt")
        );
        assert_eq!(
            thrown_while_decoding_loot(condition, r#"{"type":"minecraft:location_check","predicate":{},"offsetX":1E+10000}"#),
            include_str!("dfu_traces/predicate_location_check_offset_number_format.txt")
        );
    }

    #[test]
    fn both_alternatives_message_prints_the_java_strings_of_the_values() {
        let json = r#"{"id":"minecraft:oak_log","properties":{"axis":"x"},"type":"minecraft:simple","state":{"id":"minecraft:oak_log","properties":{"axis":"z"}}}"#;
        let element = crate::util::gson::parse_strict(json).unwrap();
        let error = crate::nbt::with_json_source(|| super::decode("minecraft:worldgen/block_state_provider", &element)).unwrap_err();
        assert_eq!(
            error,
            format!("Both alternatives read successfully, can not pick the correct one; first: (Left[Block{{minecraft:oak_log}}[axis=x]], {json}) second: (Right[SimpleStateProvider[state=Block{{minecraft:oak_log}}[axis=z]]], {json})")
        );
    }

    #[test]
    fn large_integer_literals_keep_their_low_bits() {
        for (literal, int) in [("18446744073709551615", -1), ("123456789012345678901234567890", 1312754386), ("1.2345678901234567E19", -350288040), ("-1e19", 1981284352)] {
            let json = format!(r#"{{"sound_event":"minecraft:music_disc.13","description":"x","length_in_seconds":10,"comparator_output":{literal}}}"#);
            let element = crate::util::gson::parse_strict(&json).unwrap();
            let result = crate::nbt::with_json_source(|| super::decode("minecraft:jukebox_song", &element));
            assert_eq!(result, Err(format!("Value must be within range [0;15]: {int}")));
        }
    }

    #[test]
    fn enchantments_report_the_vanilla_messages_of_their_codecs() {
        let enchantment = |effects: serde_json::Value| {
            serde_json::json!({
                "anvil_cost": 1,
                "description": "Plain",
                "effects": effects,
                "max_cost": {"base": 3, "per_level_above_first": 4},
                "max_level": 3,
                "min_cost": {"base": 1, "per_level_above_first": 2},
                "slots": ["mainhand"],
                "supported_items": ["stick"],
                "weight": 5000
            })
        };
        assert_eq!(decode_error("minecraft:enchantment", enchantment(serde_json::json!({}))), "Value must be within range [1;1024]: 5000");
        let mut json = enchantment(serde_json::json!({"minecraft:tick": [{"effect": {"type": "minecraft:replace_block", "block_state": {"id": "minecraft:stone"}, "offset": [1, 2]}}]}));
        json["weight"] = serde_json::json!(5);
        let error = decode_error("minecraft:enchantment", json);
        assert!(error.starts_with("Input is not a list of 3 ints missed input: {\"minecraft:tick\":"), "{error}");
    }

    #[test]
    fn enchantment_providers_report_the_vanilla_messages_of_their_codecs() {
        assert_eq!(
            decode_error("minecraft:enchantment_provider", serde_json::json!({"type": "minecraft:by_cost"})),
            r#"No key cost in MapLike[{"type":"minecraft:by_cost"}]; No key enchantments in MapLike[{"type":"minecraft:by_cost"}]"#
        );
        assert_eq!(
            decode_error("minecraft:enchantment_provider", serde_json::json!({"type": "minecraft:by_cost_with_difficulty", "enchantments": [], "min_cost": 0, "max_cost_span": 20000})),
            "Value must be within range [0;10000]: 20000; Value must be within range [1;10000]: 0"
        );
        assert_eq!(decode_error("minecraft:enchantment_provider", serde_json::json!({"type": "minecraft:single", "enchantment": 5, "level": 1})), "Not a string: 5");
    }

    #[test]
    fn flat_level_generator_presets_are_loaded_and_leave_unknown_references_unbound() {
        let entries = super::json_entries("minecraft:worldgen/flat_level_generator_preset");
        assert!(super::get("minecraft:worldgen/flat_level_generator_preset").is_some());
        assert!(entries.iter().any(|(name, _)| *name == "minecraft:classic_flat"));
        let preset = serde_json::json!({
            "display": "minecraft:grass_block",
            "settings": {"biome": "test:nobiome", "layers": [{"block": "minecraft:stone", "height": 1}], "structure_overrides": ["test:nostructset"]}
        });
        super::with_loading(|| {
            assert!(crate::nbt::with_json_source(|| super::decode("minecraft:worldgen/flat_level_generator_preset", &element(&preset))).is_ok());
            let unbound = super::UNBOUND.with(|unbound| unbound.borrow().clone().unwrap());
            assert_eq!(unbound, vec![("minecraft:worldgen/biome".to_string(), "test:nobiome".to_string()), ("minecraft:worldgen/structure_set".to_string(), "test:nostructset".to_string())]);
        });
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
    fn damage_types_are_read_from_their_json() {
        let info = super::damage_type_info(
            "ta:custom",
            &serde_json::json!({"exhaustion": 5.5, "message_id": "custom", "scaling": "always", "effects": "burning", "death_message_type": "fall_variants"}),
        );
        assert_eq!(info.name, "ta:custom");
        assert_eq!(info.msg_id, "custom");
        assert_eq!(info.exhaustion, 5.5);
        assert_eq!(info.scaling, wafer_data::DamageScaling::Always);
        assert_eq!(info.effects, wafer_data::DamageEffects::Burning);
        assert_eq!(info.death_message_type, wafer_data::DeathMessageType::FallVariants);
        let plain = super::damage_type_info("ta:plain", &serde_json::json!({"exhaustion": 0, "message_id": "plain", "scaling": "never"}));
        assert_eq!(plain.scaling, wafer_data::DamageScaling::Never);
        assert_eq!(plain.effects, wafer_data::DamageEffects::Hurt);
        assert_eq!(plain.death_message_type, wafer_data::DeathMessageType::Default);
    }

    #[test]
    fn infiniburn_accepts_a_tag_a_block_and_a_list() {
        let dimension = |infiniburn: serde_json::Value| super::dimension_type("ta:dimension", &serde_json::json!({"infiniburn": infiniburn}));
        let stone = wafer_data::blocks::STONE.0 as usize;
        let bedrock = wafer_data::blocks::BEDROCK.0 as usize;
        let netherrack = wafer_data::blocks::NETHERRACK.0 as usize;
        let list = dimension(serde_json::json!(["minecraft:stone", "bedrock"]));
        assert!(list.infiniburn.contains(stone));
        assert!(list.infiniburn.contains(bedrock));
        assert!(!list.infiniburn.contains(netherrack));
        let single = dimension(serde_json::json!("stone"));
        assert!(single.infiniburn.contains(stone));
        assert!(!single.infiniburn.contains(bedrock));
        let tag = dimension(serde_json::json!("#minecraft:infiniburn_overworld"));
        assert!(tag.infiniburn.contains(netherrack));
    }

    #[test]
    fn vanilla_entries_decode() {
        let mut failures = Vec::new();
        let names: Vec<&str> = wafer_data::builtin_synced_registries().iter().map(|registry| registry.name).chain(super::UNSYNCHRONIZED.iter().map(|&(name, _)| name)).collect();
        for name in names {
            for &(id, json) in super::json_entries(name) {
                let json: serde_json::Value = serde_json::from_str(json).unwrap();
                if let Err(e) = crate::nbt::with_json_source(|| super::decode(name, &element(&json))) {
                    failures.push(format!("{name} {id}: {e}"));
                }
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn vanilla_pack_blocks_no_entries() {
        let names: Vec<&str> = wafer_data::builtin_synced_registries().iter().map(|registry| registry.name).chain(super::UNSYNCHRONIZED.iter().map(|&(name, _)| name)).collect();
        for name in names {
            super::json_entries(name);
            let registry = super::get(name).unwrap();
            assert_eq!(registry.size(), registry.json_entries().len(), "{name}");
        }
        assert!(!super::ANY_BLOCKED.load(std::sync::atomic::Ordering::Acquire));
    }

    #[test]
    fn required_registries_are_never_empty_in_the_vanilla_pack() {
        super::json_entries("minecraft:wolf_variant");
        for name in super::NON_EMPTY_REGISTRIES {
            assert!(super::get(name).unwrap().size() > 0, "{name}");
        }
    }
}
