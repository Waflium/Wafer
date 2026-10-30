pub mod allowlist_service;
pub mod banlist_service;
pub mod discovery_service;
pub mod game_rules_service;
pub mod ip_banlist_service;
pub mod operator_service;
pub mod player_service;
pub mod server_settings_service;
pub mod server_state_service;

use std::fmt;

use serde_json::Value;

use super::codec::{self, DecodeResult, Fields};
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::server::players::NameAndId;

#[derive(Clone, Copy, Debug)]
pub struct ClientInfo {
    pub connection_id: i32,
}

pub fn log(client: ClientInfo, message: fmt::Arguments) {
    info!("RPC Connection #{}: {message}", client.connection_id);
}

const FETCH_USER_BY_NAME: &str = "net.minecraft.server.jsonrpc.internalapi.MinecraftPlayerListServiceImpl.fetchUserByName(MinecraftPlayerListServiceImpl.java:42)";
const GET_USER_BY_NAME: &str = "net.minecraft.server.jsonrpc.internalapi.MinecraftPlayerListService.lambda$getUser$1(MinecraftPlayerListService.java:28)";
const FETCH_USER_BY_ID: &str = "net.minecraft.server.jsonrpc.internalapi.MinecraftPlayerListServiceImpl.fetchUserById(MinecraftPlayerListServiceImpl.java:47)";
const GET_USER_BY_ID: &str = "net.minecraft.server.jsonrpc.internalapi.MinecraftPlayerListService.lambda$getUser$0(MinecraftPlayerListService.java:26)";

enum PendingUser {
    Ready(Option<NameAndId>),
    Fetching(std::sync::mpsc::Receiver<Option<NameAndId>>),
}

fn start_get_user(server: &Server, id: Option<u128>, name: Option<&str>) -> PendingUser {
    let (services, name_to_id_cache) = (server.services.clone(), server.name_to_id_cache.clone());
    let lookup: Box<dyn FnOnce() -> Option<NameAndId> + Send> = match (id, name) {
        (Some(id), _) => {
            if let Some(cached) = name_to_id_cache.get_by_id(id) {
                return PendingUser::Ready(Some(cached));
            }
            Box::new(move || services.fetch_game_profile(id, &async_supply_frames(FETCH_USER_BY_ID, GET_USER_BY_ID)).map(|profile| NameAndId { id: profile.uuid, name: profile.name }))
        }
        (None, Some(name)) => {
            let name = name.to_string();
            Box::new(move || name_to_id_cache.get(&services, &name, &async_supply_frames(FETCH_USER_BY_NAME, GET_USER_BY_NAME)))
        }
        (None, None) => return PendingUser::Ready(None),
    };
    let (sender, receiver) = std::sync::mpsc::channel();
    crate::server::commands::fetch_profile_command::DOWNLOAD_POOL.execute(move || {
        let _ = sender.send(lookup());
    });
    PendingUser::Fetching(receiver)
}

pub fn get_users(server: &Server, users: &[(Option<u128>, Option<&str>)]) -> Vec<Option<NameAndId>> {
    let pending: Vec<PendingUser> = users.iter().map(|&(id, name)| start_get_user(server, id, name)).collect();
    pending
        .into_iter()
        .map(|user| match user {
            PendingUser::Ready(user) => user,
            PendingUser::Fetching(receiver) => receiver.recv().ok().flatten(),
        })
        .collect()
}

fn async_supply_frames(fetch: &'static str, supplier: &'static str) -> [&'static str; 6] {
    [
        fetch,
        supplier,
        "java.base/java.util.concurrent.CompletableFuture$AsyncSupply.run(CompletableFuture.java:1789)",
        crate::server::commands::fetch_profile_command::POOL_RUN_WORKER,
        crate::server::commands::fetch_profile_command::POOL_WORKER_RUN,
        crate::server::commands::fetch_profile_command::THREAD_RUN,
    ]
}

pub struct Message {
    pub literal: Option<String>,
    pub translatable: Option<String>,
    pub translatable_params: Option<Vec<String>>,
}

impl Message {
    pub fn decode(value: &Value) -> DecodeResult<Message> {
        let mut fields = Fields::of(value)?;
        let literal = fields.optional("literal", codec::string);
        let translatable = fields.optional("translatable", codec::string);
        let translatable_params = fields.lenient("translatableParams", |v| codec::list(v, codec::string));
        fields.finish(|| Message { literal, translatable, translatable_params })
    }

    pub fn as_component(&self) -> Option<Component> {
        match (&self.translatable, &self.translatable_params) {
            (Some(key), Some(params)) => Some(Component::translatable_with(key, params.iter().map(|p| Arg::String(p.clone())).collect())),
            (Some(key), None) => Some(Component::translatable(key)),
            (None, _) => self.literal.as_deref().map(Component::literal),
        }
    }
}

pub fn decode_params<T>(value: &Value, decode: impl Fn(&Value) -> DecodeResult<T>) -> Result<T, String> {
    decode(value).map_err(codec::message)
}
