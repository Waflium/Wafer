use wafer_data::SyncedRegistry;
use wafer_protocol::Writer;

use crate::commands::source::CommandSourceStack;
use crate::network::chat::player_chat_message::PlayerChatMessage;
use crate::network::chat::Component;
use crate::network::chat_type_decoration::ChatTypeDecoration;
use crate::server::Server;
use crate::world::entity::EntityId;
use crate::world::entity::player::profile_public_key::now_millis;
use crate::world::item::component::types::{self, Pair, record, req, def};
use crate::world::item::component::text;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChatType {
    Chat,
    SayCommand,
    MsgCommandIncoming,
    MsgCommandOutgoing,
    TeamMsgCommandIncoming,
    TeamMsgCommandOutgoing,
    EmoteCommand,
}

impl ChatType {
    const ALL: [ChatType; 7] = [
        ChatType::Chat,
        ChatType::SayCommand,
        ChatType::MsgCommandIncoming,
        ChatType::MsgCommandOutgoing,
        ChatType::TeamMsgCommandIncoming,
        ChatType::TeamMsgCommandOutgoing,
        ChatType::EmoteCommand,
    ];

    fn name(self) -> &'static str {
        match self {
            ChatType::Chat => "minecraft:chat",
            ChatType::SayCommand => "minecraft:say_command",
            ChatType::MsgCommandIncoming => "minecraft:msg_command_incoming",
            ChatType::MsgCommandOutgoing => "minecraft:msg_command_outgoing",
            ChatType::TeamMsgCommandIncoming => "minecraft:team_msg_command_incoming",
            ChatType::TeamMsgCommandOutgoing => "minecraft:team_msg_command_outgoing",
            ChatType::EmoteCommand => "minecraft:emote_command",
        }
    }

    fn chat(self) -> &'static ChatTypeDecoration {
        &crate::core::data_registries::decoded("minecraft:chat_type", |entries| -> Vec<ChatTypeDecoration> {
            ChatType::ALL
                .iter()
                .map(|chat_type| {
                    let (_, json) = entries.iter().find(|(name, _)| *name == chat_type.name()).expect("a chat type");
                    ChatTypeDecoration::parse(&serde_json::from_str::<serde_json::Value>(json).expect("chat type JSON")["chat"])
                })
                .collect()
        })[self as usize]
    }

    pub fn bind(self, name: Component) -> Bound {
        Bound { chat_type: self, name, target_name: None }
    }
}

#[derive(Clone, Debug)]
pub struct Bound {
    pub chat_type: ChatType,
    pub name: Component,
    pub target_name: Option<Component>,
}

impl Bound {
    pub fn with_target_name(&self, target_name: Component) -> Bound {
        Bound { target_name: Some(target_name), ..self.clone() }
    }

    pub fn decorate(&self, content: Component) -> Component {
        self.chat_type.chat().decorate(content, self)
    }

    pub fn write(&self, w: &mut Writer) {
        let id = SyncedRegistry::get("minecraft:chat_type").id_of(self.chat_type.name()).expect("a chat type");
        w.varint(crate::core::data_registries::network_id("minecraft:chat_type", id) + 1);
        self.name.write(w);
        w.bool(self.target_name.is_some());
        if let Some(target) = &self.target_name {
            target.write(w);
        }
    }
}

pub fn broadcast_chat_message(server: &mut Server, message: &PlayerChatMessage, _sender: Option<EntityId>, chat_type: &Bound) {
    let trusted = message.has_signature() && !message.has_expired_server(now_millis());
    let decorated = chat_type.decorate(message.decorated_content()).get_string();
    if trusted {
        info!("{decorated}");
    } else {
        info!("[Not Secure] {decorated}");
    }
    for player in server.player_ids() {
        server.player_mut(player).send_chat_message(message, chat_type);
    }
}

pub fn source_send_chat_message(server: &mut Server, source: &CommandSourceStack, message: &PlayerChatMessage, chat_type: &Bound) {
    if source.silent {
        return;
    }
    match source.player_id(server) {
        Some(player) => server.player_mut(player).send_chat_message(message, chat_type),
        None => source.send_system_message(server, &chat_type.decorate(message.decorated_content())),
    }
}

fn decoration_codec() -> Pair {
    record(vec![
        req("translation_key", types::string()),
        req("parameters", types::list(types::enumeration(&["sender", "target", "content"]))),
        def("style", text::style(), types::compound(&[])),
    ])
}

pub fn direct_codec() -> Pair {
    record(vec![req("chat", decoration_codec()), req("narration", decoration_codec())])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::chat::{ChatFormatting, Contents};

    #[test]
    fn the_vanilla_decorations_are_read_from_the_registry() {
        let named = |chat_type: ChatType| chat_type.bind(Component::literal("n")).with_target_name(Component::literal("t")).decorate(Component::literal("c")).get_string();
        assert_eq!(named(ChatType::Chat), "<n> c");
        assert_eq!(named(ChatType::SayCommand), "[n] c");
        assert_eq!(named(ChatType::EmoteCommand), "* n c");
        assert_eq!(named(ChatType::MsgCommandIncoming), "n whispers to you: c");
        assert_eq!(named(ChatType::MsgCommandOutgoing), "You whisper to t: c");
        assert_eq!(named(ChatType::TeamMsgCommandIncoming), "t <n> c");
    }

    #[test]
    fn a_decoration_orders_its_parameters_and_applies_its_style() {
        let decoration = ChatTypeDecoration::parse(&serde_json::json!({"translation_key": "k", "parameters": ["content", "target", "sender"], "style": {"color": "red", "italic": true}}));
        let bound = ChatType::Chat.bind(Component::literal("n")).with_target_name(Component::literal("t"));
        let component = decoration.decorate(Component::literal("c"), &bound);
        let Contents::Translatable { key, args, .. } = &component.contents else { panic!("a translation") };
        assert_eq!(key, "k");
        assert_eq!(args.len(), 3);
        assert_eq!(component.style.italic, Some(true));
        assert_eq!(component.style.color, Some(ChatFormatting::Red.color()));
    }
}
