use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use wafer_protocol::nbt::Nbt;

use crate::datafixers::dynamic::Dynamic;
use crate::util::datafix::fixes::block_state_data_table::{MAP, OLD_NAMES, OLD_TAGS, STATES};

pub const FILTER_ME: &str = "%%FILTER_ME%%";

type Properties = Option<&'static [(&'static str, &'static str)]>;

static TAGS: LazyLock<Vec<Arc<Dynamic>>> = LazyLock::new(|| STATES.iter().map(|(name, properties)| Arc::new(create(name, *properties))).collect());

static ID_BY_OLD: LazyLock<HashMap<&'static str, Vec<(Properties, u16)>>> = LazyLock::new(|| {
    let mut map: HashMap<&'static str, Vec<(Properties, u16)>> = HashMap::new();
    for (name, properties, id) in OLD_TAGS.iter() {
        map.entry(name).or_default().push((*properties, *id));
    }
    map
});

static ID_BY_OLD_NAME: LazyLock<HashMap<&'static str, u16>> = LazyLock::new(|| OLD_NAMES.iter().copied().collect());

const NO_TAG: u16 = u16::MAX;

fn create(name: &str, properties: Properties) -> Dynamic {
    let mut entries = vec![("Name".to_string(), Nbt::String(name.to_string()))];
    if let Some(properties) = properties {
        entries.push(("Properties".to_string(), Nbt::Compound(properties.iter().map(|(key, value)| (key.to_string(), Nbt::String(value.to_string()))).collect())));
    }
    Dynamic::new(Nbt::Compound(entries))
}

fn map_tag(id: i32) -> Option<&'static Arc<Dynamic>> {
    if !(0..MAP.len() as i32).contains(&id) {
        return None;
    }
    match MAP[id as usize] {
        NO_TAG => None,
        index => Some(&TAGS[index as usize]),
    }
}

fn find_old_id(old_tag: &Dynamic) -> i32 {
    let Some(Nbt::Compound(entries)) = &old_tag.value else { return -1 };
    let mut name = None;
    let mut properties: Option<&Vec<(String, Nbt)>> = None;
    for (key, value) in entries {
        match (key.as_str(), value) {
            ("Name", Nbt::String(value)) => name = Some(value.as_str()),
            ("Properties", Nbt::Compound(value)) => properties = Some(value),
            _ => return -1,
        }
    }
    let Some(candidates) = name.and_then(|name| ID_BY_OLD.get(name)) else { return -1 };
    for (candidate, id) in candidates {
        let matches = match (candidate, properties) {
            (None, None) => true,
            (Some(candidate), Some(properties)) => {
                candidate.len() == properties.len() && candidate.iter().all(|(key, value)| properties.iter().any(|(property_key, property_value)| property_key == key && matches!(property_value, Nbt::String(property_value) if property_value == value)))
            }
            _ => false,
        };
        if matches {
            return *id as i32;
        }
    }
    -1
}

pub fn upgrade_block_state_tag(old_tag: Dynamic) -> Dynamic {
    let id = find_old_id(&old_tag);
    match map_tag(id) {
        Some(tag) => (**tag).clone(),
        None => old_tag,
    }
}

pub fn upgrade_block(old_name: &str) -> String {
    match ID_BY_OLD_NAME.get(old_name) {
        Some(id) => match map_tag(*id as i32) {
            Some(tag) => tag.get("Name").as_string_or(""),
            None => old_name.to_string(),
        },
        None => old_name.to_string(),
    }
}

pub fn upgrade_block_id(id: i32) -> String {
    match map_tag(id) {
        Some(tag) => tag.get("Name").as_string_or(""),
        None => "minecraft:air".to_string(),
    }
}

pub fn get_tag(id: i32) -> Arc<Dynamic> {
    match map_tag(id) {
        Some(tag) => tag.clone(),
        None => map_tag(0).expect("the air block state").clone(),
    }
}
