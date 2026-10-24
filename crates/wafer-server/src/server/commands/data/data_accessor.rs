use wafer_core::BlockPos;
use wafer_data::commands::CommandNode;
use wafer_protocol::nbt::Nbt;

use crate::commands::arguments::coordinates::get_loaded_block_pos;
use crate::commands::arguments::nbt_path_argument::NbtPath;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::unexpected_error;
use crate::commands::source::CommandSourceStack;
use crate::nbt::text_component_tag_visitor::to_pretty_component;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::util::java_format_fixed;
use crate::util::problem_reporter::PathElement;
use crate::world::entity::EntityId;
use crate::world::entity::entity_save::LOADING_ENTITY_NBT;
use crate::world::level::storage::tag_value_input;

const ENTITY_DATA_ACCESSOR_LOGGER: &str = "net.minecraft.server.commands.data.EntityDataAccessor";
const BLOCK_DATA_ACCESSOR_LOGGER: &str = "net.minecraft.server.commands.data.BlockDataAccessor";

static ERROR_NOT_A_BLOCK_ENTITY: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.data.block.invalid");

static ERROR_NO_PLAYERS: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.data.entity.invalid");

pub enum DataAccessor {
    Entity(EntityId),
    Block(usize, BlockPos),
    Storage(String),
}

pub const KINDS: &[&str] = &["entity", "block", "storage"];

impl DataAccessor {
    pub fn access(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, literal: usize, arg: &str) -> Result<DataAccessor, CommandSyntaxException> {
        Ok(match CommandNode::get(context.nodes[literal].node).name {
            "entity" => DataAccessor::Entity(context.get_entity(server, source, arg)?),
            "storage" => DataAccessor::Storage(context.get_identifier(arg).to_string()),
            "block" => {
                let pos = get_loaded_block_pos(server, context, source, arg)?;
                if server.levels[source.level].get_block_entity(pos).is_none() {
                    return Err(ERROR_NOT_A_BLOCK_ENTITY.create());
                }
                DataAccessor::Block(source.level, pos)
            }
            other => unreachable!("no {other} data accessor"),
        })
    }

    pub fn get_data(&self, server: &mut Server) -> Result<Nbt, CommandSyntaxException> {
        Ok(match self {
            DataAccessor::Entity(entity) => server.entity_level(*entity).entity_tag_to_compare(*entity),
            DataAccessor::Storage(id) => Nbt::Compound(server.command_storage.borrow_mut().get(id)),
            DataAccessor::Block(level, pos) => server.levels[*level].block_entity_data(*pos).ok_or_else(|| ERROR_NOT_A_BLOCK_ENTITY.create())?,
        })
    }

    pub fn set_data(&self, server: &mut Server, tag: Nbt) -> Result<(), CommandSyntaxException> {
        match self {
            DataAccessor::Entity(id) => {
                let level = server.entity_level_mut(*id);
                let scoreboard = level.scoreboard.clone();
                if let Some(part) = level.dragon_part_entity_mut(*id) {
                    let uuid = part.uuid;
                    if part.load(&tag).is_err() {
                        return Err(unexpected_error(LOADING_ENTITY_NBT));
                    }
                    part.load_team(&tag, &scoreboard);
                    part.uuid = uuid;
                    return Ok(());
                }
                if level.is_thrown_enderpearl(*id) {
                    level.thrown_enderpearl_deregister_from_current_owner(*id);
                }
                let previous_cube_size = level.mob(*id).filter(|mob| mob.is_cube_mob()).map(crate::world::entity::mob::Mob::get_size);
                if level.is_leashable(*id) && level.get_leash_data(*id).is_some() && crate::world::entity::leashable::LeashData::read(&tag).is_none() {
                    level.remove_leash(*id);
                }
                let previous_framed_item = level.item_frame_item(*id);
                let mut collector = tag_value_input::ScopedCollector::new_empty(ENTITY_DATA_ACCESSOR_LOGGER);
                let Some(entity) = level.entities.get(id) else { return Err(ERROR_NO_PLAYERS.create()) };
                let converted = level.with_old_owner_converted(entity, &tag);
                let entity = level.entities.get_mut(id).expect("the entity");
                let uuid = entity.entity().uuid;
                let previous_dimensions = entity.entity().dimensions;
                if entity.load_in_level(&converted, &level.scoreboard).is_err() {
                    return Err(unexpected_error(LOADING_ENTITY_NBT));
                }
                entity.entity_mut().uuid = uuid;
                level.item_frame_loaded(*id, previous_framed_item);
                if level.mob(*id).is_some_and(crate::world::entity::mob::Mob::is_abstract_skeleton) {
                    level.reassess_weapon_goal(*id);
                }
                level.on_dimensions_loaded(*id, previous_dimensions, previous_cube_size);
                level.read_level_dependent_save_data(*id, &tag);
                collector.set_root(PathElement::Text(level.entity_to_string(*id)));
                level.entity_moved(*id);
                let level = level.index;
                server.resolve_display_text(level, *id);
            }
            DataAccessor::Storage(id) => server.command_storage.borrow_mut().set(id, into_fields(tag)),
            DataAccessor::Block(level, pos) => {
                let level = &mut server.levels[*level];
                if level.get_block_entity(*pos).is_none() {
                    return Err(ERROR_NOT_A_BLOCK_ENTITY.create());
                }
                let path = level.get_block_entity(*pos).map(|entity| entity.problem_path()).unwrap_or(PathElement::Empty);
                let _collector = tag_value_input::ScopedCollector::new(path, BLOCK_DATA_ACCESSOR_LOGGER);
                level.block_entity_load_with_components(*pos, &tag);
                level.block_entity_set_changed(*pos);
                let state = crate::world::level::BlockGetter::block_state(level, *pos);
                level.send_block_updated(*pos, state, state);
            }
        }
        Ok(())
    }

    pub fn write_back(&self, server: &mut Server, tag: &Nbt) {
        match self {
            DataAccessor::Storage(id) => server.command_storage.borrow_mut().write_back(id, into_fields(tag.clone())),
            DataAccessor::Entity(_) | DataAccessor::Block(..) => {}
        }
    }

    fn position_args(pos: BlockPos) -> Vec<Arg> {
        vec![Arg::Int(pos.x), Arg::Int(pos.y), Arg::Int(pos.z)]
    }

    pub fn modified_success(&self, server: &Server) -> Component {
        match self {
            DataAccessor::Entity(entity) => Component::translatable_with("commands.data.entity.modified", vec![server.entity_display_name(*entity).into()]),
            DataAccessor::Storage(id) => Component::translatable_with("commands.data.storage.modified", vec![Arg::Component(Component::literal(id))]),
            DataAccessor::Block(_, pos) => Component::translatable_with("commands.data.block.modified", Self::position_args(*pos)),
        }
    }

    pub fn print_success(&self, server: &Server, data: &Nbt) -> Component {
        match self {
            DataAccessor::Entity(entity) => Component::translatable_with(
                "commands.data.entity.query",
                vec![server.entity_display_name(*entity).into(), Arg::Component(to_pretty_component(data))],
            ),
            DataAccessor::Storage(id) => {
                Component::translatable_with("commands.data.storage.query", vec![Arg::Component(Component::literal(id)), Arg::Component(to_pretty_component(data))])
            }
            DataAccessor::Block(_, pos) => {
                let mut args = Self::position_args(*pos);
                args.push(Arg::Component(to_pretty_component(data)));
                Component::translatable_with("commands.data.block.query", args)
            }
        }
    }

    pub fn print_scaled_success(&self, server: &Server, path: &NbtPath, scale: f64, value: i32) -> Component {
        match self {
            DataAccessor::Entity(entity) => Component::translatable_with(
                "commands.data.entity.get",
                vec![path.original.as_str().into(), server.entity_display_name(*entity).into(), java_format_fixed(scale, 2).into(), value.into()],
            ),
            DataAccessor::Storage(id) => Component::translatable_with(
                "commands.data.storage.get",
                vec![path.original.as_str().into(), Arg::Component(Component::literal(id)), java_format_fixed(scale, 2).into(), value.into()],
            ),
            DataAccessor::Block(_, pos) => {
                let mut args = vec![path.original.as_str().into()];
                args.extend(Self::position_args(*pos));
                args.push(java_format_fixed(scale, 2).into());
                args.push(value.into());
                Component::translatable_with("commands.data.block.get", args)
            }
        }
    }
}

fn into_fields(tag: Nbt) -> Vec<(String, Nbt)> {
    match tag {
        Nbt::Compound(fields) => fields,
        _ => unreachable!("data roots are compounds"),
    }
}

pub fn suggest_storage(
    server: &Server,
    _context: &CommandContext,
    _source: &CommandSourceStack,
    mut builder: crate::commands::suggestion::SuggestionsBuilder,
) -> Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    crate::commands::shared_suggestion_provider::suggest_resource(server.command_storage.borrow_mut().keys(), &mut builder);
    Ok(builder.build())
}
