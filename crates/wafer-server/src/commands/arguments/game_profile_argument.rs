use super::ArgumentValue;
use super::entity_argument::{ERROR_ONLY_PLAYERS_ALLOWED, NO_PLAYERS_FOUND};
use super::selector::entity_selector::EntitySelector;
use super::selector::entity_selector_parser::EntitySelectorParser;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType, StringReader};
use crate::commands::source::CommandSourceStack;
use crate::server::Server;
use crate::server::command_frames::command_frames;

pub use crate::server::players::NameAndId;

const PARSE_RESULT: &str = "net.minecraft.commands.arguments.GameProfileArgument.lambda$parse$0(GameProfileArgument.java:66)";
const GET_GAME_PROFILES: &str = "net.minecraft.commands.arguments.GameProfileArgument.getGameProfiles(GameProfileArgument.java:32)";

static ERROR_UNKNOWN_PLAYER: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.player.unknown");

#[derive(Clone, PartialEq, Debug)]
pub enum GameProfileResult {
    Selector(Box<EntitySelector>),
    Name(String),
}

pub fn parse(reader: &mut StringReader, allow_selectors: bool) -> Result<GameProfileResult, CommandSyntaxException> {
    if reader.at('@') {
        let selector = EntitySelectorParser::new(reader, allow_selectors).parse()?;
        if selector.includes_entities {
            return Err(ERROR_ONLY_PLAYERS_ALLOWED.create_with_context(reader));
        }
        return Ok(GameProfileResult::Selector(Box::new(selector)));
    }
    let start = reader.cursor;
    while reader.can_read() && reader.peek() != b' ' as u16 {
        reader.skip();
    }
    Ok(GameProfileResult::Name(reader.substring(start, reader.cursor)))
}

impl GameProfileResult {
    fn names(&self, server: &Server, source: &CommandSourceStack, command_frame: &'static str) -> Result<Vec<NameAndId>, CommandSyntaxException> {
        match self {
            GameProfileResult::Selector(selector) => {
                let players = selector.find_players(server, source)?;
                if players.is_empty() {
                    return Err(NO_PLAYERS_FOUND.create());
                }
                Ok(players.into_iter().map(|id| name_and_id(server, id)).collect())
            }
            GameProfileResult::Name(name) => Ok(vec![name_to_id(server, name, command_frame)?]),
        }
    }
}

fn name_and_id(server: &Server, id: crate::world::entity::EntityId) -> NameAndId {
    let profile = &server.player(id).profile;
    NameAndId { id: profile.uuid, name: profile.name.clone() }
}

fn name_to_id(server: &Server, name: &str, command_frame: &'static str) -> Result<NameAndId, CommandSyntaxException> {
    let mut frames = vec![PARSE_RESULT, GET_GAME_PROFILES, command_frame];
    frames.extend(command_frames(server.command_origin));
    server.name_to_id_cache.get(&server.services, name, &frames).ok_or_else(|| ERROR_UNKNOWN_PLAYER.create())
}

impl CommandContext {
    pub fn get_game_profiles(&self, server: &Server, source: &CommandSourceStack, name: &str, command_frame: &'static str) -> Result<Vec<NameAndId>, CommandSyntaxException> {
        match self.argument(name) {
            ArgumentValue::GameProfile(result) => result.names(server, source, command_frame),
            other => panic!("{name} is {other:?}"),
        }
    }
}
