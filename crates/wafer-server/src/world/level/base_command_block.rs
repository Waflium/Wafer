use std::sync::atomic::{AtomicU64, Ordering};

use wafer_core::BlockPos;
use wafer_data::game_rule;
use wafer_protocol::nbt::Nbt;

use crate::commands::source::{CommandSource, CommandSourceStack};
use crate::network::chat::Component;
use crate::server::Server;
use crate::world::entity::EntityId;
use crate::network::chat::component_serialization::parse_custom_name_safe;

const NO_LAST_EXECUTION: i64 = -1;

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CommandBlockOwner {
    Block(usize, BlockPos),
    Minecart(usize, EntityId),
}

impl CommandBlockOwner {
    pub fn level(self) -> usize {
        match self {
            CommandBlockOwner::Block(level, _) | CommandBlockOwner::Minecart(level, _) => level,
        }
    }
}

#[derive(Clone)]
pub struct BaseCommandBlock {
    last_execution: i64,
    update_last_execution: bool,
    success_count: i32,
    track_output: bool,
    last_output: Option<Nbt>,
    command: String,
    custom_name: Option<Nbt>,
}

impl Default for BaseCommandBlock {
    fn default() -> BaseCommandBlock {
        BaseCommandBlock {
            last_execution: NO_LAST_EXECUTION,
            update_last_execution: true,
            success_count: 0,
            track_output: true,
            last_output: None,
            command: String::new(),
            custom_name: None,
        }
    }
}

impl BaseCommandBlock {
    pub fn get_success_count(&self) -> i32 {
        self.success_count
    }

    pub fn set_success_count(&mut self, success_count: i32) {
        self.success_count = success_count;
    }

    pub fn get_last_output(&self) -> Nbt {
        self.last_output.clone().unwrap_or_else(|| Component::empty().to_nbt())
    }

    pub fn save(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("Command".into(), Nbt::String(self.command.clone())));
        output.push(("SuccessCount".into(), Nbt::Int(self.success_count)));
        if let Some(custom_name) = &self.custom_name {
            output.push(("CustomName".into(), custom_name.clone()));
        }
        output.push(("TrackOutput".into(), Nbt::Byte(self.track_output as i8)));
        if self.track_output
            && let Some(last_output) = &self.last_output
        {
            output.push(("LastOutput".into(), last_output.clone()));
        }
        output.push(("UpdateLastExecution".into(), Nbt::Byte(self.update_last_execution as i8)));
        if self.update_last_execution && self.last_execution != NO_LAST_EXECUTION {
            output.push(("LastExecution".into(), Nbt::Long(self.last_execution)));
        }
    }

    pub fn load(&mut self, input: &Nbt) {
        let boolean = |key: &str, default: bool| input.get(key).and_then(Nbt::as_i64).map_or(default, |v| v != 0);
        self.command = input.get("Command").and_then(Nbt::as_str).unwrap_or_default().to_string();
        self.success_count = input.get("SuccessCount").and_then(Nbt::as_i64).map_or(0, |v| v as i32);
        self.set_custom_name(parse_custom_name_safe(input, "CustomName"));
        self.track_output = boolean("TrackOutput", true);
        self.last_output = if self.track_output { parse_custom_name_safe(input, "LastOutput") } else { None };
        self.update_last_execution = boolean("UpdateLastExecution", true);
        self.last_execution = if self.update_last_execution { input.get("LastExecution").and_then(Nbt::as_i64).unwrap_or(NO_LAST_EXECUTION) } else { NO_LAST_EXECUTION };
    }

    pub fn set_command(&mut self, command: &str) {
        self.command = command.to_string();
        self.success_count = 0;
    }

    pub fn get_command(&self) -> &str {
        &self.command
    }

    pub fn get_name(&self) -> Component {
        match &self.custom_name {
            Some(custom_name) => Component::from_nbt(custom_name),
            None => Component::literal("@"),
        }
    }

    pub fn get_custom_name(&self) -> Option<&Nbt> {
        self.custom_name.as_ref()
    }

    pub fn set_custom_name(&mut self, name: Option<Nbt>) {
        self.custom_name = name;
    }

    pub fn set_last_output(&mut self, last_output: Option<Nbt>) {
        self.last_output = last_output;
    }

    pub fn set_track_output(&mut self, track_output: bool) {
        self.track_output = track_output;
    }

    pub fn is_track_output(&self) -> bool {
        self.track_output
    }
}

impl Server {
    pub fn base_command_block(&mut self, owner: CommandBlockOwner) -> Option<&mut BaseCommandBlock> {
        let level = &mut self.levels[owner.level()];
        match owner {
            CommandBlockOwner::Block(_, pos) => level.command_block_entity(pos).map(|entity| &mut entity.command_block),
            CommandBlockOwner::Minecart(_, entity) => level.minecart_command_block(entity),
        }
    }

    pub fn base_command_block_on_updated(&mut self, owner: CommandBlockOwner) {
        let level = &mut self.levels[owner.level()];
        match owner {
            CommandBlockOwner::Block(_, pos) => level.command_block_entity_on_updated(pos),
            CommandBlockOwner::Minecart(_, entity) => level.minecart_command_block_on_updated(entity),
        }
    }

    fn command_block_create_command_source_stack(&mut self, owner: CommandBlockOwner, source: CommandSource) -> Option<CommandSourceStack> {
        let name = self.base_command_block(owner)?.get_name();
        let level = owner.level();
        let (position, rotation) = match owner {
            CommandBlockOwner::Block(_, pos) => {
                let facing = self.levels[level].command_block_facing(pos);
                (wafer_core::Vec3::at_center_of(pos), (0.0, facing.to_y_rot()))
            }
            CommandBlockOwner::Minecart(_, entity) => {
                let e = self.levels[level].entity(entity);
                (e.position(), (e.x_rot, e.y_rot))
            }
        };
        let entity = match owner {
            CommandBlockOwner::Minecart(_, entity) => Some(entity),
            CommandBlockOwner::Block(..) => None,
        };
        let mut stack = CommandSourceStack::server(self);
        stack.source = source;
        stack.position = position;
        stack.rotation = rotation;
        stack.level = level;
        stack.permission = 2;
        stack.names = if entity.is_some() { crate::commands::source::Names::ForEntity } else { crate::commands::source::Names::Constant(name) };
        stack.entity = entity;
        Some(stack)
    }

    pub fn base_command_block_perform_command(&mut self, owner: CommandBlockOwner) -> bool {
        let game_time = self.levels[owner.level()].game_time;
        let command_blocks_work = self.levels[owner.level()].game_rules.get_bool(game_rule::COMMAND_BLOCKS_WORK);
        let Some(command_block) = self.base_command_block(owner) else { return false };
        if game_time == command_block.last_execution {
            return false;
        }
        if command_block.command.eq_ignore_ascii_case("Searge") {
            command_block.last_output = Some(Component::literal("#itzlipofutzli").to_nbt());
            command_block.success_count = 1;
            return true;
        }
        command_block.success_count = 0;
        if command_blocks_work && !command_block.command.is_empty() {
            command_block.last_output = None;
            let command = command_block.command.clone();
            let session = command_block.track_output.then(|| NEXT_SESSION.fetch_add(1, Ordering::Relaxed));
            let source = session.map_or(CommandSource::Null, CommandSource::CommandBlock);
            if let Some(stack) = self.command_block_create_command_source_stack(owner, source) {
                let stack = stack.with_callback(crate::commands::source::CommandResultCallback::new(move |server, success, _| {
                    if success && let Some(command_block) = server.base_command_block(owner) {
                        command_block.success_count += 1;
                    }
                }));
                crate::commands::perform_prefixed_command(self, &stack, &command);
            }
            if let Some(session) = session {
                let messages: Vec<Component> = self.take_command_block_messages(session);
                for message in messages {
                    let time = chrono::Local::now().format("%H:%M:%S").to_string();
                    let last_output = Component::literal(format!("[{time}] ")).append(message).to_nbt();
                    if let Some(command_block) = self.base_command_block(owner) {
                        command_block.last_output = Some(last_output);
                    }
                    self.base_command_block_on_updated(owner);
                }
            }
        }
        let Some(command_block) = self.base_command_block(owner) else { return true };
        command_block.last_execution = if command_block.update_last_execution { game_time } else { NO_LAST_EXECUTION };
        true
    }

    fn take_command_block_messages(&mut self, session: u64) -> Vec<Component> {
        let mut messages = self.command_block_messages.borrow_mut();
        let (taken, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut *messages).into_iter().partition(|(s, _)| *s == session);
        *messages = kept;
        taken.into_iter().map(|(_, message)| message).collect()
    }
}
