use std::collections::HashMap;

use wafer_core::BlockPos;
use wafer_core::random::RandomSource;
use wafer_data::SyncedRegistry;
use serde_json::Value;
use wafer_protocol::nbt::Nbt;

use super::priority_provider::{self, Selector};
use super::spawn_condition::SpawnCondition;
use super::spawn_context::SpawnContext;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::storage::tag_value_input;

pub struct SpawnPrioritySelectors {
    selectors: Vec<(i32, Selector)>,
}

impl SpawnPrioritySelectors {
    pub fn load(registry: &str, variants: &[(&str, &str)]) -> SpawnPrioritySelectors {
        let json: HashMap<&str, Value> = variants.iter().map(|&(id, json)| (id, serde_json::from_str(json).expect("vanilla JSON"))).collect();
        let mut selectors = Vec::new();
        for (id, name) in SyncedRegistry::get(registry).entries.iter().enumerate() {
            for selector in json[name]["spawn_conditions"].as_array().into_iter().flatten() {
                let condition = selector.get("condition").map(SpawnCondition::from_json);
                selectors.push((id as i32, Selector { condition, priority: selector["priority"].as_i64().unwrap_or(0) as i32 }));
            }
        }
        SpawnPrioritySelectors { selectors }
    }
}

pub fn select_variant_to_spawn(level: &mut (impl ServerLevelAccessor + ?Sized), pos: BlockPos, variants: &SpawnPrioritySelectors) -> Option<i32> {
    let mut context = SpawnContext::create(level, pos);
    let selected = priority_provider::select(&variants.selectors, |condition| condition.test(&mut context));
    crate::util::get_random_safe(&selected, level.get_random())
}

pub fn pick_random_sound_variant(registry: &str, random: &mut (impl RandomSource + ?Sized)) -> i32 {
    random.next_int_bounded(SyncedRegistry::get(registry).entries.len() as i32)
}

pub fn read_variant(registry: &str, name: &str) -> Option<i32> {
    let name = if name.contains(':') { name.to_string() } else { format!("minecraft:{name}") };
    SyncedRegistry::get(registry).id_of(&name)
}

pub fn read_variant_field(input: &Nbt, key: &str, registry: &str) -> Option<i32> {
    let name = tag_value_input::read_or_error(input, key, crate::util::codec::identifier)?;
    SyncedRegistry::get(registry).id_of(&name)
}

pub fn variant_name(registry: &str, id: i32) -> &'static str {
    SyncedRegistry::get(registry).entries[id as usize]
}
