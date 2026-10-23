use std::collections::HashMap;
use std::sync::atomic::{AtomicPtr, Ordering};

use serde_json::Value;
use wafer_data::SyncedRegistry;

use crate::server::packs::repository::VANILLA_ID;
use crate::server::packs::resources::MultiPackResourceManager;

pub struct DataRegistry {
    pub name: &'static str,
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

    pub fn element(&self, id: i32) -> Option<&Value> {
        self.elements.get(id as usize)
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

pub fn element(registry: &str, id: i32) -> Option<&'static Value> {
    get(registry)?.element(id)
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

pub fn load(resource_manager: &MultiPackResourceManager) {
    let mut synced = Vec::new();
    let mut registries = HashMap::new();
    for builtin in wafer_data::builtin_synced_registries() {
        let directory = builtin.name.strip_prefix("minecraft:").unwrap_or(builtin.name);
        let resources = resource_manager.list_resources(directory, ".json");
        let mut entries: Vec<&'static str> = builtin.entries.to_vec();
        let mut contents: Vec<Option<Value>> = vec![None; entries.len()];
        let mut elements: Vec<Value> = vec![Value::Null; entries.len()];
        for (location, resource) in &resources {
            let (namespace, path) = location.split_once(':').unwrap();
            let id = format!("{namespace}:{}", &path[directory.len() + 1..path.len() - ".json".len()]);
            let json = match resource.supplier.read().map_err(|e| e.to_string()).and_then(|bytes| serde_json::from_slice::<Value>(&bytes).map_err(|e| e.to_string())) {
                Ok(json) => json,
                Err(e) => {
                    error!("Failed to parse {location} from pack {}: {e}", resource.source_pack_id);
                    continue;
                }
            };
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
        synced.push(SyncedRegistry { name: builtin.name, entries: Box::leak(entries.into_boxed_slice()) });
        registries.insert(builtin.name, DataRegistry { name: builtin.name, to_network, from_network, contents, elements, json: Box::leak(json.into_boxed_slice()) });
    }
    wafer_data::set_synced_registries(synced);
    let dimension_registry = &registries["minecraft:dimension_type"];
    let dimension_entries = wafer_data::SyncedRegistry::get("minecraft:dimension_type").entries;
    wafer_data::set_dimension_types(dimension_entries.iter().zip(&dimension_registry.elements).map(|(&name, json)| dimension_type(name, json)).collect());
    let registries: &'static HashMap<&'static str, DataRegistry> = Box::leak(Box::new(registries));
    BIOMES.store((&registries["minecraft:worldgen/biome"] as *const DataRegistry).cast_mut(), Ordering::Release);
    ACTIVE.store((registries as *const HashMap<&'static str, DataRegistry>).cast_mut(), Ordering::Release);
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
                assert!(registry.element(id).is_some_and(|element| !element.is_null()), "{} {}", builtin.name, builtin.entries[id as usize]);
            }
        }
    }
}
