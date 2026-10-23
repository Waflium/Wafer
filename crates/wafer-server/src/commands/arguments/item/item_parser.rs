use wafer_data::{Item, registry};

use super::ItemInput;
use crate::commands::arguments::read_identifier;
use crate::commands::brigadier::{CommandSyntaxException, Dynamic2CommandExceptionType, DynamicCommandExceptionType, SimpleCommandExceptionType, StringReader, tr};
use crate::commands::shared_suggestion_provider;
use crate::commands::suggestion::{Suggestions, SuggestionsBuilder};
use crate::nbt;
use crate::world::item::component::{DataComponentPatch, DataComponentType};

pub static ERROR_UNKNOWN_ITEM: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("argument.item.id.invalid", vec![id]));
pub static ERROR_UNKNOWN_COMPONENT: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("arguments.item.component.unknown", vec![id]));
pub static ERROR_MALFORMED_COMPONENT: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|ty, message| tr("arguments.item.component.malformed", vec![ty, message]));
pub static ERROR_EXPECTED_COMPONENT: SimpleCommandExceptionType = SimpleCommandExceptionType("arguments.item.component.expected");
pub static ERROR_REPEATED_COMPONENT: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("arguments.item.component.repeated", vec![id]));

enum Suggest {
    Nothing,
    Item,
    StartComponents,
    ComponentAssignmentOrRemoval,
    Component,
    Assignment,
    NextOrEndComponents,
}

pub fn parse(reader: &mut StringReader) -> Result<ItemInput, CommandSyntaxException> {
    let cursor = reader.cursor;
    let result = parse_state(reader, &mut Suggest::Nothing);
    if result.is_err() {
        reader.cursor = cursor;
    }
    result
}

pub fn fill_suggestions(builder: &SuggestionsBuilder) -> Suggestions {
    let input = builder.input().to_vec();
    let mut reader = StringReader::new(&input);
    reader.cursor = builder.start();
    let mut suggestions = Suggest::Nothing;
    let _ = parse_state(&mut reader, &mut suggestions);
    let mut builder = builder.create_offset(reader.cursor);
    match suggestions {
        Suggest::Nothing => {}
        Suggest::Item => shared_suggestion_provider::suggest_resource(registry::ITEM.iter(), &mut builder),
        Suggest::StartComponents => {
            if builder.remaining().is_empty() {
                builder.suggest("[");
            }
        }
        Suggest::NextOrEndComponents => {
            if builder.remaining().is_empty() {
                builder.suggest(",");
                builder.suggest("]");
            }
        }
        Suggest::Assignment => {
            if builder.remaining().is_empty() {
                builder.suggest("=");
            }
        }
        Suggest::ComponentAssignmentOrRemoval => {
            builder.suggest("!");
            suggest_component(&mut builder, "=");
        }
        Suggest::Component => suggest_component(&mut builder, ""),
    }
    builder.build()
}

fn suggest_component(builder: &mut SuggestionsBuilder, suffix: &str) {
    let contents = builder.remaining().to_lowercase();
    shared_suggestion_provider::filter_resources(
        registry::DATA_COMPONENT_TYPE.iter().filter(|id| DataComponentType::by_name(id).is_some_and(|ty| !ty.is_transient())),
        &contents,
        |id| builder.suggest(format!("{id}{suffix}")),
    );
}

fn parse_state(reader: &mut StringReader, suggestions: &mut Suggest) -> Result<ItemInput, CommandSyntaxException> {
    *suggestions = Suggest::Item;
    let item = read_item(reader)?;
    *suggestions = Suggest::StartComponents;
    let mut components = DataComponentPatch::default();
    if reader.at('[') {
        *suggestions = Suggest::Nothing;
        read_components(reader, &mut components, suggestions)?;
    }
    Ok(ItemInput { item, components })
}

fn read_item(reader: &mut StringReader) -> Result<Item, CommandSyntaxException> {
    let cursor = reader.cursor;
    let id = read_identifier(reader)?;
    Item::by_name(&id).ok_or_else(|| {
        reader.cursor = cursor;
        ERROR_UNKNOWN_ITEM.create_with_context(reader, id)
    })
}

fn read_components(reader: &mut StringReader, components: &mut DataComponentPatch, suggestions: &mut Suggest) -> Result<(), CommandSyntaxException> {
    reader.expect('[')?;
    *suggestions = Suggest::ComponentAssignmentOrRemoval;
    let mut visited: Vec<DataComponentType> = Vec::new();
    while reader.can_read() && !reader.at(']') {
        reader.skip_whitespace();
        if reader.at('!') {
            reader.skip();
            *suggestions = Suggest::Component;
            let component = read_component_type(reader)?;
            if visited.contains(&component) {
                return Err(ERROR_REPEATED_COMPONENT.create(component.name()));
            }
            visited.push(component);
            components.put(component, None);
            *suggestions = Suggest::Nothing;
            reader.skip_whitespace();
        } else {
            let component = read_component_type(reader)?;
            if visited.contains(&component) {
                return Err(ERROR_REPEATED_COMPONENT.create(component.name()));
            }
            visited.push(component);
            *suggestions = Suggest::Assignment;
            reader.skip_whitespace();
            reader.expect('=')?;
            *suggestions = Suggest::Nothing;
            reader.skip_whitespace();
            let value = read_component(reader, component)?;
            components.put(component, Some(value));
            reader.skip_whitespace();
        }
        *suggestions = Suggest::NextOrEndComponents;
        if !reader.at(',') {
            break;
        }
        reader.skip();
        reader.skip_whitespace();
        *suggestions = Suggest::ComponentAssignmentOrRemoval;
        if !reader.can_read() {
            return Err(ERROR_EXPECTED_COMPONENT.create_with_context(reader));
        }
    }
    reader.expect(']')?;
    *suggestions = Suggest::Nothing;
    Ok(())
}

pub fn read_component_type(reader: &mut StringReader) -> Result<DataComponentType, CommandSyntaxException> {
    if !reader.can_read() {
        return Err(ERROR_EXPECTED_COMPONENT.create_with_context(reader));
    }
    let cursor = reader.cursor;
    let id = read_identifier(reader)?;
    match DataComponentType::by_name(&id) {
        Some(component) if !component.is_transient() => Ok(component),
        _ => {
            reader.cursor = cursor;
            Err(ERROR_UNKNOWN_COMPONENT.create_with_context(reader, id))
        }
    }
}

fn read_component(reader: &mut StringReader, component: DataComponentType) -> Result<wafer_protocol::nbt::Nbt, CommandSyntaxException> {
    let cursor = reader.cursor;
    let tag = nbt::parse_as_argument(reader)?;
    component.decode(&tag).map_err(|message| {
        reader.cursor = cursor;
        ERROR_MALFORMED_COMPONENT.create_with_context(reader, component.name(), message)
    })
}
