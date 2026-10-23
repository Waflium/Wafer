use wafer_protocol::nbt::Nbt;

use super::resolution_context::{RESOLVED_COMPONENT_LIMIT, ResolutionContext};
use super::{Component, component_serialization, component_utils};
use crate::commands::arguments::coordinates::Coordinates;
use crate::commands::arguments::nbt_path_argument::NbtPath;
use crate::commands::arguments::selector::entity_selector::EntitySelector;
use crate::commands::brigadier::CommandSyntaxException;
use crate::nbt::text_component_tag_visitor::to_component;
use crate::util::compilable_string::CompilableString;

#[derive(Clone, Debug, PartialEq)]
pub enum DataSource {
    Entity(CompilableString<EntitySelector>),
    Block(CompilableString<Coordinates>),
    Storage(String),
}

impl DataSource {
    pub fn to_field(&self) -> (&'static str, Nbt) {
        match self {
            DataSource::Entity(selector) => ("entity", Nbt::String(selector.source().into())),
            DataSource::Block(coordinates) => ("block", Nbt::String(coordinates.source().into())),
            DataSource::Storage(id) => ("storage", Nbt::String(id.clone())),
        }
    }

    pub fn get_data(&self, context: &mut ResolutionContext) -> Result<Vec<Nbt>, CommandSyntaxException> {
        Ok(match self {
            DataSource::Entity(selector) => {
                let world = context.world;
                selector
                    .compiled()
                    .find_entities(world, context.source)?
                    .into_iter()
                    .map(|entity| world.level_of_entity(entity).expect("entity in a loaded level").entity_tag_to_compare(entity))
                    .collect()
            }
            DataSource::Block(coordinates) => {
                let pos = coordinates.compiled().block_pos(context.world, context.source);
                match context.world.level(context.source.level).loaded_block_entity_data(pos) {
                    Some(data) => vec![data],
                    None => Vec::new(),
                }
            }
            DataSource::Storage(id) => vec![Nbt::Compound(context.world.command_storage().borrow_mut().get(id))],
        })
    }
}

fn format_parameter(tag: &Nbt) -> String {
    match tag {
        Nbt::List(items) => format!("[{}]", items.iter().map(format_parameter).collect::<Vec<_>>().join(", ")),
        other => crate::nbt::to_string(other),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NbtContents {
    pub nbt_path: CompilableString<NbtPath>,
    pub interpreting: bool,
    pub plain: bool,
    pub separator: Option<Box<Component>>,
    pub data_source: DataSource,
}

impl NbtContents {
    pub fn resolve(&self, context: &mut ResolutionContext, recursion_depth: i32) -> Result<Component, CommandSyntaxException> {
        if context.resolved_component_count > RESOLVED_COMPONENT_LIMIT {
            return Ok(Component::empty());
        }
        context.resolved_component_count += 1;
        let data = self.data_source.get_data(context)?;
        let elements: Vec<Nbt> = data.iter().flat_map(|tag| self.nbt_path.compiled().get(tag).unwrap_or_default()).collect();
        let resolved_separator = match &self.separator {
            Some(separator) => component_utils::resolve_depth(context, separator, recursion_depth)?,
            None => Component::literal(", "),
        };
        let mut result: Option<Component> = None;
        for tag in &elements {
            let component = if self.interpreting {
                match component_serialization::decode(tag).map_err(|_| ()).and_then(|component| component_utils::resolve_depth(context, &component, recursion_depth).map_err(|_| ())) {
                    Ok(component) => component,
                    Err(()) => {
                        warn!("Failed to parse component: {}", format_parameter(tag));
                        continue;
                    }
                }
            } else {
                to_component(tag, self.plain)
            };
            result = Some(match result {
                None => component,
                Some(left) => left.append(resolved_separator.clone()).append(component),
            });
        }
        Ok(result.unwrap_or_else(Component::empty))
    }
}
