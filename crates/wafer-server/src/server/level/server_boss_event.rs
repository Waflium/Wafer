use wafer_data::packets::play::clientbound;
use wafer_protocol::Writer;

use super::ServerLevel;
use crate::network::chat::{ChatFormatting, Component};
use crate::network::connection::Outbound;
use crate::world::entity::EntityId;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BossBarColor {
    Pink,
    Blue,
    Red,
    Green,
    Yellow,
    Purple,
    White,
}

impl BossBarColor {
    pub const VALUES: [BossBarColor; 7] =
        [BossBarColor::Pink, BossBarColor::Blue, BossBarColor::Red, BossBarColor::Green, BossBarColor::Yellow, BossBarColor::Purple, BossBarColor::White];

    pub fn name(self) -> &'static str {
        match self {
            BossBarColor::Pink => "pink",
            BossBarColor::Blue => "blue",
            BossBarColor::Red => "red",
            BossBarColor::Green => "green",
            BossBarColor::Yellow => "yellow",
            BossBarColor::Purple => "purple",
            BossBarColor::White => "white",
        }
    }

    pub fn by_name(name: &str) -> Option<BossBarColor> {
        BossBarColor::VALUES.into_iter().find(|c| c.name() == name)
    }

    pub fn formatting(self) -> ChatFormatting {
        match self {
            BossBarColor::Pink => ChatFormatting::Red,
            BossBarColor::Blue => ChatFormatting::Blue,
            BossBarColor::Red => ChatFormatting::DarkRed,
            BossBarColor::Green => ChatFormatting::Green,
            BossBarColor::Yellow => ChatFormatting::Yellow,
            BossBarColor::Purple => ChatFormatting::DarkBlue,
            BossBarColor::White => ChatFormatting::White,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BossBarOverlay {
    Progress,
    Notched6,
    Notched10,
    Notched12,
    Notched20,
}

impl BossBarOverlay {
    pub const VALUES: [BossBarOverlay; 5] =
        [BossBarOverlay::Progress, BossBarOverlay::Notched6, BossBarOverlay::Notched10, BossBarOverlay::Notched12, BossBarOverlay::Notched20];

    pub fn name(self) -> &'static str {
        match self {
            BossBarOverlay::Progress => "progress",
            BossBarOverlay::Notched6 => "notched_6",
            BossBarOverlay::Notched10 => "notched_10",
            BossBarOverlay::Notched12 => "notched_12",
            BossBarOverlay::Notched20 => "notched_20",
        }
    }

    pub fn by_name(name: &str) -> Option<BossBarOverlay> {
        BossBarOverlay::VALUES.into_iter().find(|o| o.name() == name)
    }
}

#[derive(Clone, Copy)]
enum Operation {
    Add,
    Remove,
    UpdateProgress,
    UpdateName,
    UpdateStyle,
    UpdateProperties,
}

pub struct ServerBossEvent {
    pub id: u128,
    pub name: Component,
    pub progress: f32,
    pub color: BossBarColor,
    pub overlay: BossBarOverlay,
    pub darken_screen: bool,
    pub play_boss_music: bool,
    pub create_world_fog: bool,
    pub players: Vec<EntityId>,
    connections: Vec<Outbound>,
    pub visible: bool,
}

impl ServerBossEvent {
    pub fn new(id: u128, name: Component, color: BossBarColor, overlay: BossBarOverlay) -> ServerBossEvent {
        ServerBossEvent {
            id,
            name,
            progress: 1.0,
            color,
            overlay,
            darken_screen: false,
            play_boss_music: false,
            create_world_fog: false,
            players: Vec::new(),
            connections: Vec::new(),
            visible: true,
        }
    }

    fn write(&self, w: &mut Writer, operation: Operation) {
        w.uuid(self.id);
        w.varint(operation as i32);
        let properties = |w: &mut Writer| w.u8(self.darken_screen as u8 | (self.play_boss_music as u8) << 1 | (self.create_world_fog as u8) << 2);
        match operation {
            Operation::Add => {
                self.name.write(w);
                w.f32(self.progress);
                w.varint(self.color as i32);
                w.varint(self.overlay as i32);
                properties(w);
            }
            Operation::Remove => {}
            Operation::UpdateProgress => w.f32(self.progress),
            Operation::UpdateName => self.name.write(w),
            Operation::UpdateStyle => {
                w.varint(self.color as i32);
                w.varint(self.overlay as i32);
            }
            Operation::UpdateProperties => properties(w),
        }
    }

    fn send(&self, connection: &Outbound, operation: Operation) {
        connection.send(clientbound::BOSS_EVENT, |w| self.write(w, operation));
    }

    fn broadcast(&self, operation: Operation) {
        if self.visible {
            for connection in &self.connections {
                self.send(connection, operation);
            }
        }
    }

    pub fn set_progress(&mut self, progress: f32) -> bool {
        if progress == self.progress {
            return false;
        }
        self.progress = progress;
        self.broadcast(Operation::UpdateProgress);
        true
    }

    pub fn set_color(&mut self, color: BossBarColor) -> bool {
        if color == self.color {
            return false;
        }
        self.color = color;
        self.broadcast(Operation::UpdateStyle);
        true
    }

    pub fn set_overlay(&mut self, overlay: BossBarOverlay) -> bool {
        if overlay == self.overlay {
            return false;
        }
        self.overlay = overlay;
        self.broadcast(Operation::UpdateStyle);
        true
    }

    pub fn set_darken_screen(&mut self, darken_screen: bool) -> bool {
        if darken_screen == self.darken_screen {
            return false;
        }
        self.darken_screen = darken_screen;
        self.broadcast(Operation::UpdateProperties);
        true
    }

    pub fn set_play_boss_music(&mut self, play_boss_music: bool) -> bool {
        if play_boss_music == self.play_boss_music {
            return false;
        }
        self.play_boss_music = play_boss_music;
        self.broadcast(Operation::UpdateProperties);
        true
    }

    pub fn set_create_world_fog(&mut self, create_world_fog: bool) -> bool {
        if create_world_fog == self.create_world_fog {
            return false;
        }
        self.create_world_fog = create_world_fog;
        self.broadcast(Operation::UpdateProperties);
        true
    }

    pub fn set_name(&mut self, name: Component) -> bool {
        if name == self.name {
            return false;
        }
        self.name = name;
        self.broadcast(Operation::UpdateName);
        true
    }

    pub fn add_player(&mut self, levels: &[ServerLevel], player: EntityId) {
        if !self.players.contains(&player) {
            let Some(connection) = levels.iter().find_map(|level| level.players.get(&player)).map(|p| p.connection.clone()) else { return };
            if self.visible {
                self.send(&connection, Operation::Add);
            }
            self.players.push(player);
            self.connections.push(connection);
        }
    }

    pub fn remove_player(&mut self, player: EntityId) {
        if let Some(i) = self.players.iter().position(|&p| p == player) {
            self.players.remove(i);
            let connection = self.connections.remove(i);
            if self.visible {
                self.send(&connection, Operation::Remove);
            }
        }
    }

    pub fn set_visible(&mut self, visible: bool) -> bool {
        if visible == self.visible {
            return false;
        }
        self.visible = visible;
        for connection in &self.connections {
            self.send(connection, if visible { Operation::Add } else { Operation::Remove });
        }
        true
    }
}
