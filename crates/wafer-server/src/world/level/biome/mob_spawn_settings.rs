use serde_json::Value;

use crate::util::random::WeightedList;
use crate::util::valueproviders::{IntProvider, int_provider_codec};
use crate::world::entity::entity_type::EntityType;
use crate::world::item::component::codec::Field;
use crate::world::item::component::types::{self, req};
use crate::world::entity::entity_types;
use crate::world::entity::mob_category::MobCategory;
use crate::world::level::levelgen::density_function::resource_name;

#[derive(Clone, PartialEq, Debug)]
pub struct SpawnerData {
    pub entity_type: i32,
    pub count: IntProvider,
}

pub fn spawner_data_fields() -> Vec<Field> {
    vec![req("type", types::by_name("minecraft:entity_type")), req("count", int_provider_codec())]
}

impl SpawnerData {
    pub fn new(entity_type: i32, count: IntProvider) -> SpawnerData {
        let entity_type = if EntityType::by_id(entity_type).get_category() == MobCategory::Misc { entity_types::PIG.id } else { entity_type };
        SpawnerData { entity_type, count }
    }

    pub fn type_(&self) -> &'static EntityType {
        EntityType::by_id(self.entity_type)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MobSpawnCost {
    pub energy_budget: f64,
    pub charge: f64,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct MobSpawnSettings {
    pub spawns_by_category: [Option<WeightedList<SpawnerData>>; 8],
    pub mob_spawn_costs: Vec<(i32, MobSpawnCost)>,
}

fn entity_type(json: &Value) -> Result<i32, String> {
    let name = resource_name(json.as_str().ok_or_else(|| format!("not an entity type: {json}"))?);
    wafer_data::registry::ENTITY_TYPE.iter().position(|&e| e == name).map(|i| i as i32).ok_or_else(|| format!("unknown entity type {name}"))
}

fn parse_spawner(entry: &Value) -> Result<(SpawnerData, i32), String> {
    let weight = entry.get("weight").and_then(Value::as_i64).filter(|w| *w >= 0).ok_or("spawn weight must be non-negative")? as i32;
    let count = IntProvider::parse(entry.get("count").ok_or("missing spawn count")?)?;
    Ok((SpawnerData::new(entity_type(&entry["type"])?, count), weight))
}

pub fn parse_spawner_list(json: &Value) -> Result<WeightedList<SpawnerData>, String> {
    let mut entries = Vec::new();
    for entry in json.as_array().ok_or("spawns must be a list")? {
        entries.push(parse_spawner(entry)?);
    }
    Ok(WeightedList::new(entries))
}

fn parse_spawner_list_leniently(json: &Value) -> Result<WeightedList<SpawnerData>, String> {
    let entries = json.as_array().ok_or("spawns must be a list")?.iter().filter_map(|entry| parse_spawner(entry).ok()).collect();
    Ok(WeightedList::new(entries))
}

impl MobSpawnSettings {
    pub fn get_mobs_in_category(&self, category: MobCategory) -> Option<&WeightedList<SpawnerData>> {
        self.spawns_by_category[category as usize].as_ref()
    }

    pub fn get_mobs_to_spawn(&self, category: MobCategory) -> &WeightedList<SpawnerData> {
        static EMPTY_MOB_LIST: WeightedList<SpawnerData> = WeightedList::empty();
        self.get_mobs_in_category(category).unwrap_or(&EMPTY_MOB_LIST)
    }

    pub fn get_mob_spawn_cost(&self, entity_type: i32) -> Option<MobSpawnCost> {
        self.mob_spawn_costs.iter().find(|(e, _)| *e == entity_type).map(|&(_, c)| c)
    }

    pub fn is_empty(&self) -> bool {
        self.spawns_by_category.iter().all(Option::is_none) && self.mob_spawn_costs.is_empty()
    }

    pub fn put_cost(&mut self, entity_type: i32, cost: MobSpawnCost) {
        match self.mob_spawn_costs.iter_mut().find(|(e, _)| *e == entity_type) {
            Some(entry) => entry.1 = cost,
            None => self.mob_spawn_costs.push((entity_type, cost)),
        }
    }

    pub fn parse(json: &Value) -> Result<MobSpawnSettings, String> {
        let mut settings = MobSpawnSettings::default();
        let spawns = json.get("spawns_by_category").and_then(Value::as_object).ok_or("missing spawns_by_category")?;
        for (name, list) in spawns {
            let category = MobCategory::by_name(name).ok_or_else(|| format!("unknown mob category {name}"))?;
            settings.spawns_by_category[category as usize] = Some(parse_spawner_list_leniently(list)?);
        }
        let costs = json.get("spawn_costs").and_then(Value::as_object).ok_or("missing spawn_costs")?;
        for (name, cost) in costs {
            let field = |key: &str| cost.get(key).and_then(Value::as_f64).ok_or_else(|| crate::util::codec::no_key(key, cost));
            settings.put_cost(entity_type(&Value::String(name.clone()))?, MobSpawnCost { energy_budget: field("energy_budget")?, charge: field("charge")? });
        }
        Ok(settings)
    }
}
