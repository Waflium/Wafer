pub mod timer_queue;

use wafer_protocol::nbt::Nbt;

use crate::server::Server;
use crate::server::server_function_manager::ServerFunctionManager;
use crate::util::codec::{self, DataResult};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TimerCallback {
    Function(String),
    FunctionTag(String),
}

impl TimerCallback {
    pub fn handle(&self, server: &mut Server) {
        match self {
            TimerCallback::Function(function_id) => {
                if let Some(function) = server.functions.get(function_id) {
                    let sender = ServerFunctionManager::game_loop_sender(server);
                    ServerFunctionManager::execute(server, &function, &sender);
                }
            }
            TimerCallback::FunctionTag(tag_id) => {
                for function in server.functions.get_tag(tag_id).to_vec() {
                    let sender = ServerFunctionManager::game_loop_sender(server);
                    ServerFunctionManager::execute(server, &function, &sender);
                }
            }
        }
    }

    fn decode(nbt: &Nbt) -> DataResult<TimerCallback> {
        let kind = codec::identifier(codec::field(nbt, "type")?)?;
        match kind.as_str() {
            "minecraft:function" => Ok(TimerCallback::Function(codec::identifier(codec::field(nbt, "id")?)?)),
            "minecraft:function_tag" => Ok(TimerCallback::FunctionTag(codec::identifier(codec::field(nbt, "id")?)?)),
            _ => Err(format!("Unknown element id: {kind}")),
        }
    }

    fn to_nbt(&self) -> Nbt {
        let (kind, id) = match self {
            TimerCallback::Function(id) => ("minecraft:function", id),
            TimerCallback::FunctionTag(id) => ("minecraft:function_tag", id),
        };
        Nbt::Compound(vec![("id".into(), Nbt::String(id.clone())), ("type".into(), Nbt::String(kind.into()))])
    }
}
