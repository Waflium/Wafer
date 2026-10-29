use serde_json::Value;

use super::spawn_context::SpawnContext;
use crate::world::level::levelgen::structure::structure::StructureHolder;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

pub enum StructureCheck {
    Tag(String),
    Direct(Vec<String>),
}

impl StructureCheck {
    pub fn from_json(condition: &Value) -> StructureCheck {
        match &condition["structures"] {
            Value::String(structures) => match structures.strip_prefix('#') {
                Some(tag) => StructureCheck::Tag(tag.to_string()),
                None => StructureCheck::Direct(vec![structures.clone()]),
            },
            Value::Array(structures) => StructureCheck::Direct(structures.iter().filter_map(Value::as_str).map(str::to_string).collect()),
            _ => StructureCheck::Direct(Vec::new()),
        }
    }

    fn contains(&self, structure: StructureHolder) -> bool {
        match self {
            StructureCheck::Tag(tag) => wafer_data::tag("minecraft:worldgen/structure", &crate::world::level::levelgen::density_function::resource_name(tag)).contains(&(structure.0 as i32)),
            StructureCheck::Direct(structures) => structures.iter().any(|name| StructureHolder::by_name(name) == Some(structure)),
        }
    }

    pub fn test<L: ServerLevelAccessor + ?Sized>(&self, context: &mut SpawnContext<L>) -> bool {
        context.level.get_structure_with_piece_at(context.pos, &|structure| self.contains(structure)).is_some()
    }
}
