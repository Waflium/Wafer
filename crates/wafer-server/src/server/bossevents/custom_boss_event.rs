use wafer_core::mth;
use wafer_protocol::nbt::Nbt;

use crate::network::chat::{Component, HoverEvent, component_serialization};
use crate::server::level::ServerLevel;
use crate::server::level::server_boss_event::{BossBarColor, BossBarOverlay, ServerBossEvent};
use crate::util::codec::{self, DecodeResult, Record};
use crate::util::extra_codecs;
use crate::world::entity::EntityId;
use crate::world::level::storage::uuid_ints;

const DEFAULT_MAX: i32 = 100;

fn player_in(levels: &[ServerLevel], player: EntityId) -> &crate::server::level::server_player::ServerPlayer {
    levels.iter().find_map(|level| level.find_player(player)).expect("player on this server")
}

pub struct CustomBossEvent {
    pub event: ServerBossEvent,
    pub custom_id: String,
    pub players: Vec<u128>,
    pub value: i32,
    pub max: i32,
    pub dirty: bool,
}

impl CustomBossEvent {
    pub fn new(id: u128, custom_id: String, name: Component) -> CustomBossEvent {
        let mut event = ServerBossEvent::new(id, name, BossBarColor::White, BossBarOverlay::Progress);
        event.progress = 0.0;
        CustomBossEvent { event, custom_id, players: Vec::new(), value: 0, max: DEFAULT_MAX, dirty: false }
    }

    pub fn add_player(&mut self, levels: &[ServerLevel], player: EntityId) {
        self.event.add_player(levels, player);
        let uuid = player_in(levels, player).profile.uuid;
        if !self.players.contains(&uuid) {
            self.players.push(uuid);
            self.dirty = true;
        }
    }

    pub fn remove_player(&mut self, levels: &[ServerLevel], player: EntityId) {
        self.event.remove_player(player);
        let uuid = player_in(levels, player).profile.uuid;
        if let Some(i) = self.players.iter().position(|&p| p == uuid) {
            self.players.remove(i);
            self.dirty = true;
        }
    }

    pub fn remove_all_players(&mut self) {
        for player in self.event.players.clone() {
            self.event.remove_player(player);
        }
        if !self.players.is_empty() {
            self.players.clear();
            self.dirty = true;
        }
    }

    pub fn set_value(&mut self, value: i32) {
        self.value = value;
        self.event.set_progress(mth::clamp_f(value as f32 / self.max as f32, 0.0, 1.0));
        self.dirty = true;
    }

    pub fn set_max(&mut self, max: i32) {
        self.max = max;
        self.event.set_progress(mth::clamp_f(self.value as f32 / max as f32, 0.0, 1.0));
        self.dirty = true;
    }

    pub fn display_name(&self) -> Component {
        Component::translatable_with("chat.square_brackets", vec![self.event.name.clone().into()])
            .with_color(self.event.color.formatting())
            .with_hover(HoverEvent::ShowText(Box::new(Component::literal(self.custom_id.clone()))))
            .with_insertion(&self.custom_id)
    }

    pub fn set_players(&mut self, levels: &[ServerLevel], players: &[EntityId]) -> bool {
        let uuids: Vec<(EntityId, u128)> = players.iter().map(|&p| (p, player_in(levels, p).profile.uuid)).collect();
        let to_remove: Vec<u128> = self.players.iter().copied().filter(|uuid| !uuids.iter().any(|(_, u)| u == uuid)).collect();
        let to_add: Vec<EntityId> = uuids.iter().filter(|(_, u)| !self.players.contains(u)).map(|&(p, _)| p).collect();
        for &uuid in &to_remove {
            if let Some(&player) = self.event.players.iter().find(|&&p| player_in(levels, p).profile.uuid == uuid) {
                self.remove_player(levels, player);
            }
            self.players.retain(|&p| p != uuid);
        }
        for &player in &to_add {
            self.add_player(levels, player);
        }
        let changed = !to_remove.is_empty() || !to_add.is_empty();
        if changed {
            self.dirty = true;
        }
        changed
    }

    pub fn on_player_connect(&mut self, levels: &[ServerLevel], player: EntityId) {
        if self.players.contains(&player_in(levels, player).profile.uuid) {
            self.add_player(levels, player);
        }
    }

    pub fn on_player_disconnect(&mut self, player: EntityId) {
        self.event.remove_player(player);
    }

    pub fn load(id: u128, custom_id: String, packed: &Nbt) -> DecodeResult<CustomBossEvent> {
        if let Err(error) = codec::map(packed) {
            return DecodeResult::error(error);
        }
        let enum_name = |tag: &Nbt| codec::string(tag).map(str::to_string);
        let mut record = Record::default();
        let name = record.required(packed, "Name", component_serialization::decode);
        let visible = record.with_default(packed, "Visible", false, codec::bool_value);
        let value = record.with_default(packed, "Value", 0, codec::int);
        let max = record.with_default(packed, "Max", DEFAULT_MAX, codec::int);
        let color = record.with_default(packed, "Color", BossBarColor::White, |tag| {
            enum_name(tag).and_then(|name| BossBarColor::by_name(&name).ok_or_else(|| format!("Unknown element name:{name}")))
        });
        let overlay = record.with_default(packed, "Overlay", BossBarOverlay::Progress, |tag| {
            enum_name(tag).and_then(|name| BossBarOverlay::by_name(&name).ok_or_else(|| format!("Unknown element name:{name}")))
        });
        let darken_screen = record.with_default(packed, "DarkenScreen", false, codec::bool_value);
        let play_boss_music = record.with_default(packed, "PlayBossMusic", false, codec::bool_value);
        let create_world_fog = record.with_default(packed, "CreateWorldFog", false, codec::bool_value);
        let players = record.with_default(packed, "Players", Vec::new(), |tag| codec::list_partial(tag, extra_codecs::uuid));
        record.build(|| {
            let mut custom = CustomBossEvent::new(id, custom_id, name?);
            custom.value = value;
            custom.max = max;
            let event = &mut custom.event;
            event.visible = visible;
            event.progress = mth::clamp_f(custom.value as f32 / custom.max as f32, 0.0, 1.0);
            event.color = color;
            event.overlay = overlay;
            event.darken_screen = darken_screen;
            event.play_boss_music = play_boss_music;
            event.create_world_fog = create_world_fog;
            for uuid in players {
                if !custom.players.contains(&uuid) {
                    custom.players.push(uuid);
                }
            }
            Some(custom)
        })
    }

    pub fn pack(&self) -> Nbt {
        let e = &self.event;
        let mut fields = vec![("Name".to_string(), e.name.to_nbt())];
        let defaults = [
            ("Visible", e.visible.then_some(Nbt::Byte(1))),
            ("Value", (self.value != 0).then_some(Nbt::Int(self.value))),
            ("Max", (self.max != DEFAULT_MAX).then_some(Nbt::Int(self.max))),
            ("Color", (e.color != BossBarColor::White).then(|| Nbt::String(e.color.name().into()))),
            ("Overlay", (e.overlay != BossBarOverlay::Progress).then(|| Nbt::String(e.overlay.name().into()))),
            ("DarkenScreen", e.darken_screen.then_some(Nbt::Byte(1))),
            ("PlayBossMusic", e.play_boss_music.then_some(Nbt::Byte(1))),
            ("CreateWorldFog", e.create_world_fog.then_some(Nbt::Byte(1))),
            ("Players", (!self.players.is_empty()).then(|| Nbt::List(self.players.iter().map(|&u| uuid_ints(u)).collect()))),
        ];
        fields.extend(defaults.into_iter().filter_map(|(key, value)| Some((key.to_string(), value?))));
        Nbt::Compound(fields.into())
    }
}
