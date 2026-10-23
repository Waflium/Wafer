use std::sync::LazyLock;

use serde_json::Value;

use super::placement::structure_placement::StructurePlacement;
use super::structure::StructureHolder;
use crate::world::level::levelgen::blockpredicates::block_predicate::resource;

pub struct StructureSelectionEntry {
    pub structure: StructureHolder,
    pub weight: i32,
}

pub struct StructureSet {
    pub structures: Vec<StructureSelectionEntry>,
    pub placement: StructurePlacement,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct StructureSetHolder(pub u16);

impl StructureSetHolder {
    pub fn by_name(name: &str) -> Option<StructureSetHolder> {
        let name = resource(name);
        crate::core::data_registries::json_entries("minecraft:worldgen/structure_set").iter().position(|(n, _)| *n == name).map(|i| StructureSetHolder(i as u16))
    }

    pub fn key(self) -> &'static str {
        crate::core::data_registries::json_entries("minecraft:worldgen/structure_set")[self.0 as usize].0
    }

    pub fn value(self) -> &'static StructureSet {
        &STRUCTURE_SETS[self.0 as usize]
    }

    pub fn all() -> impl Iterator<Item = StructureSetHolder> {
        (0..crate::core::data_registries::json_entries("minecraft:worldgen/structure_set").len()).map(|i| StructureSetHolder(i as u16))
    }
}

static STRUCTURE_SETS: LazyLock<Vec<StructureSet>> = LazyLock::new(|| {
    crate::core::data_registries::json_entries("minecraft:worldgen/structure_set")
        .iter()
        .map(|(name, source)| {
            let json: Value = serde_json::from_str(source).expect("structure set json");
            StructureSet::parse(&json).unwrap_or_else(|e| panic!("structure set {name}: {e}"))
        })
        .collect()
});

impl StructureSet {
    pub fn parse(json: &Value) -> Result<StructureSet, String> {
        let structures = json
            .get("structures")
            .and_then(Value::as_array)
            .ok_or("missing structures")?
            .iter()
            .map(|entry| {
                let name = entry.get("structure").and_then(Value::as_str).ok_or("missing structure")?;
                Ok(StructureSelectionEntry {
                    structure: StructureHolder::by_name(name).ok_or_else(|| format!("unknown structure {name}"))?,
                    weight: entry.get("weight").and_then(Value::as_i64).ok_or("missing weight")? as i32,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(StructureSet { structures, placement: StructurePlacement::parse(json.get("placement").ok_or("missing placement")?)? })
    }
}
