use std::collections::BTreeMap;
use std::str::FromStr;

use anyhow::{Context, bail};
use wafer_data::packets::play;
use wafer_data::{packet_name, registry};
use wafer_protocol::nbt::Nbt;
use wafer_protocol::{Reader, Writer};

use crate::encode::{self, BlockHit, TestInstanceData};

const DEFAULT_TIMEOUT: u32 = 200;

pub struct TrackedEntity {
    pub entity_type: i32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub y_rot: f32,
    pub x_rot: f32,
    pub on_ground: bool,
    pub serial: u64,
}

pub struct Vehicle {
    pub id: i32,
    pub entity_type: i32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub y_rot: f32,
    pub x_rot: f32,
    pub on_ground: bool,
    pub velocity: (f64, f64, f64),
    pub forward: Option<f64>,
    pub controlled: bool,
}

enum Control {
    Auto,
    On,
    Off,
}

enum WaitKind {
    Entity { entity_type: i32, from_serial: Option<u64> },
    Packet(String),
    Chat(String),
    Screen,
    Mounted,
    Dismounted,
    Ack(i32),
}

struct Wait {
    kind: WaitKind,
    description: String,
    remaining: u32,
    done: bool,
}

enum Activity {
    Break { remaining: u32 },
    HoldUse { remaining: u32 },
}

#[derive(Default)]
struct JumpCharge {
    ticks: i32,
    scale: f32,
    was_jumping: bool,
}

pub struct Session {
    pub outbox: Vec<Vec<u8>>,
    pub pos: Option<(f64, f64, f64)>,
    pub sequence: i32,
    pub acked: i32,
    pub open_container: i32,
    pub on_ground: u8,
    pub last_entity: i32,
    pub skip_types: Vec<i32>,
    pub player_id: i32,
    pub y_rot: f32,
    pub x_rot: f32,
    pub aborted: bool,
    entities: BTreeMap<i32, TrackedEntity>,
    state_ids: BTreeMap<i32, i32>,
    serial: u64,
    vehicle: Option<Vehicle>,
    control: Control,
    keys: u8,
    sent_keys: u8,
    timed_keys: Vec<(u32, u8)>,
    jump: JumpCharge,
    wait_ticks: u32,
    wait: Option<Wait>,
    activity: Option<Activity>,
    break_target: Option<((i32, i32, i32), u8)>,
}

fn parse<T: FromStr>(s: &str) -> anyhow::Result<T>
where
    T::Err: std::error::Error + Send + Sync + 'static,
{
    s.parse().with_context(|| format!("cannot parse {s:?}"))
}

fn parse_face(s: &str) -> anyhow::Result<u8> {
    Ok(match s {
        "down" => 0,
        "up" => 1,
        "north" => 2,
        "south" => 3,
        "west" => 4,
        "east" => 5,
        _ => parse(s)?,
    })
}

fn parse_hand(s: &str) -> anyhow::Result<i32> {
    Ok(match s {
        "main" | "main_hand" | "mainhand" => 0,
        "off" | "off_hand" | "offhand" => 1,
        _ => parse(s)?,
    })
}

fn entity_type_id(name: &str) -> anyhow::Result<i32> {
    let full = if name.contains(':') { name.to_owned() } else { format!("minecraft:{name}") };
    match registry::ENTITY_TYPE.iter().position(|e| *e == full) {
        Some(index) => Ok(index as i32),
        None => bail!("unknown entity type {name}"),
    }
}

fn entity_type_name(entity_type: i32) -> &'static str {
    registry::ENTITY_TYPE.get(entity_type as usize).copied().unwrap_or("unknown")
}

fn is_boat(name: &str) -> bool {
    ["_boat", "_raft", "_chest_boat", "_chest_raft"].iter().any(|suffix| name.ends_with(suffix))
}

fn is_jumpable(name: &str) -> bool {
    matches!(
        name,
        "minecraft:horse"
            | "minecraft:donkey"
            | "minecraft:mule"
            | "minecraft:skeleton_horse"
            | "minecraft:zombie_horse"
            | "minecraft:camel"
            | "minecraft:camel_husk"
            | "minecraft:nautilus"
            | "minecraft:zombie_nautilus"
    )
}

fn is_steerable(name: &str) -> bool {
    is_boat(name) || is_jumpable(name) || matches!(name, "minecraft:pig" | "minecraft:strider" | "minecraft:happy_ghast")
}

fn key_mask(names: &[&str]) -> anyhow::Result<u8> {
    let mut mask = 0;
    for name in names {
        mask |= match *name {
            "forward" | "w" => encode::KEY_FORWARD,
            "back" | "backward" | "s" => encode::KEY_BACKWARD,
            "left" | "a" => encode::KEY_LEFT,
            "right" | "d" => encode::KEY_RIGHT,
            "jump" | "space" => encode::KEY_JUMP,
            "sneak" | "shift" => encode::KEY_SHIFT,
            "sprint" => encode::KEY_SPRINT,
            _ => bail!("unknown key {name}"),
        };
    }
    Ok(mask)
}

fn component_text(nbt: &Nbt) -> String {
    match nbt {
        Nbt::String(s) => s.clone(),
        Nbt::List(l) => l.iter().map(component_text).collect(),
        Nbt::Compound(_) => {
            let mut out = String::new();
            if let Some(t) = nbt.get("text").and_then(Nbt::as_str) {
                out.push_str(t);
            }
            if let Some(key) = nbt.get("translate").and_then(Nbt::as_str) {
                let args: Vec<String> = nbt.get("with").map(|w| w.as_list().iter().map(component_text).collect()).unwrap_or_default();
                out.push_str(&format!("{{{key}{}}}", if args.is_empty() { String::new() } else { format!(": {}", args.join(" | ")) }));
            }
            if let Some(extra) = nbt.get("extra") {
                out.push_str(&component_text(extra));
            }
            out
        }
        other => format!("{other:?}"),
    }
}

fn unpack_degrees(packed: i8) -> f32 {
    packed as f32 * 360.0 / 256.0
}

fn read_steps(r: &mut Reader, count: i32) -> anyhow::Result<Vec<(i16, i16, i16)>> {
    if count <= 0 {
        return Ok(vec![(r.i16()?, r.i16()?, r.i16()?)]);
    }
    let mut steps = Vec::with_capacity(count as usize);
    for _ in 0..count {
        r.varint()?;
        steps.push((r.i16()?, r.i16()?, r.i16()?));
    }
    Ok(steps)
}

fn apply_delta(base: f64, delta: i16) -> f64 {
    if delta == 0 {
        return base;
    }
    ((base * 4096.0).round() + delta as f64) / 4096.0
}

fn read_timeout<'a>(words: &[&'a str]) -> anyhow::Result<(Vec<&'a str>, u32)> {
    let mut timeout = DEFAULT_TIMEOUT;
    let mut rest = Vec::new();
    for word in words {
        match word.strip_prefix("timeout=") {
            Some(value) => timeout = parse(value)?,
            None => rest.push(*word),
        }
    }
    Ok((rest, timeout))
}

impl Session {
    pub fn new() -> Self {
        Self {
            outbox: Vec::new(),
            pos: None,
            sequence: 0,
            acked: 0,
            open_container: 0,
            on_ground: 1,
            last_entity: 0,
            skip_types: Vec::new(),
            player_id: 0,
            y_rot: 0.0,
            x_rot: 0.0,
            aborted: false,
            entities: BTreeMap::new(),
            state_ids: BTreeMap::new(),
            serial: 0,
            vehicle: None,
            control: Control::Auto,
            keys: 0,
            sent_keys: 0,
            timed_keys: Vec::new(),
            jump: JumpCharge::default(),
            wait_ticks: 0,
            wait: None,
            activity: None,
            break_target: None,
        }
    }

    fn send(&mut self, id: i32, body: impl FnOnce(&mut Writer)) {
        let mut w = Writer::new();
        w.varint(id);
        body(&mut w);
        self.outbox.push(w.into_inner());
    }

    fn next_sequence(&mut self) -> i32 {
        self.sequence += 1;
        self.sequence
    }

    fn state_id(&self) -> i32 {
        self.state_ids.get(&self.open_container).copied().unwrap_or(0)
    }

    pub fn riding(&self) -> bool {
        self.vehicle.is_some()
    }

    fn eye_position(&self) -> (f64, f64, f64) {
        match &self.vehicle {
            Some(v) => (v.x, v.y, v.z),
            None => self.pos.unwrap_or_default(),
        }
    }

    fn resolve_entity(&self, word: &str) -> anyhow::Result<i32> {
        match word {
            "last" | "target" => return Ok(self.last_entity),
            "self" => return Ok(self.player_id),
            "vehicle" => return self.vehicle.as_ref().map(|v| v.id).context("not riding anything"),
            _ => {}
        }
        if let Ok(id) = word.parse::<i32>() {
            return Ok(id);
        }
        if let Some(name) = word.strip_prefix("last:") {
            let entity_type = entity_type_id(name)?;
            let newest = self.entities.iter().filter(|(_, e)| e.entity_type == entity_type).max_by_key(|(_, e)| e.serial);
            return newest.map(|(id, _)| *id).with_context(|| format!("no {name} known"));
        }
        let entity_type = entity_type_id(word)?;
        self.nearest(entity_type).with_context(|| format!("no {word} known"))
    }

    fn nearest(&self, entity_type: i32) -> Option<i32> {
        let (px, py, pz) = self.eye_position();
        let distance = |e: &TrackedEntity| (e.x - px).powi(2) + (e.y - py).powi(2) + (e.z - pz).powi(2);
        self.entities
            .iter()
            .filter(|(_, e)| e.entity_type == entity_type)
            .min_by(|a, b| distance(a.1).total_cmp(&distance(b.1)))
            .map(|(id, _)| *id)
    }

    pub fn on_packet(&mut self, id: i32, data: &[u8]) -> anyhow::Result<()> {
        let mut r = Reader::new(data);
        match id {
            play::clientbound::LOGIN => self.player_id = r.i32()?,
            play::clientbound::OPEN_SCREEN | play::clientbound::MOUNT_SCREEN_OPEN => self.open_container = r.varint()?,
            play::clientbound::CONTAINER_SET_CONTENT | play::clientbound::CONTAINER_SET_SLOT => {
                let container = r.varint()?;
                let state_id = r.varint()?;
                if container >= 0 {
                    self.state_ids.insert(container, state_id);
                }
            }
            play::clientbound::BLOCK_CHANGED_ACK => self.acked = self.acked.max(r.varint()?),
            play::clientbound::ADD_ENTITY => {
                let entity = r.varint()?;
                r.uuid()?;
                let entity_type = r.varint()?;
                let (x, y, z) = (r.f64()?, r.f64()?, r.f64()?);
                r.lp_vec3()?;
                let x_rot = unpack_degrees(r.u8()? as i8);
                let y_rot = unpack_degrees(r.u8()? as i8);
                self.serial += 1;
                let name = entity_type_name(entity_type);
                let on_ground = !is_boat(name) && name != "minecraft:happy_ghast";
                self.entities.insert(entity, TrackedEntity { entity_type, x, y, z, y_rot, x_rot, on_ground, serial: self.serial });
                if !self.skip_types.contains(&entity_type) {
                    self.last_entity = entity;
                }
            }
            play::clientbound::REMOVE_ENTITIES => {
                for _ in 0..r.varint()? {
                    let entity = r.varint()?;
                    self.entities.remove(&entity);
                    if self.vehicle.as_ref().is_some_and(|v| v.id == entity) {
                        self.dismounted();
                    }
                }
            }
            play::clientbound::MOVE_ENTITY_POS | play::clientbound::MOVE_ENTITY_POS_ROT => {
                let entity = r.varint()?;
                let properties = r.varint()?;
                let steps = read_steps(&mut r, properties >> 1)?;
                let rotation = if id == play::clientbound::MOVE_ENTITY_POS_ROT { Some((r.u8()? as i8, r.u8()? as i8)) } else { None };
                if let Some(e) = self.entities.get_mut(&entity) {
                    for (xa, ya, za) in steps {
                        e.x = apply_delta(e.x, xa);
                        e.y = apply_delta(e.y, ya);
                        e.z = apply_delta(e.z, za);
                    }
                    e.on_ground = properties & 1 != 0;
                    if let Some((y_rot, x_rot)) = rotation {
                        e.y_rot = unpack_degrees(y_rot);
                        e.x_rot = unpack_degrees(x_rot);
                    }
                }
                self.sync_uncontrolled_vehicle(entity);
            }
            play::clientbound::MOVE_ENTITY_ROT => {
                let entity = r.varint()?;
                let on_ground = r.bool()?;
                let (y_rot, x_rot) = (r.u8()? as i8, r.u8()? as i8);
                if let Some(e) = self.entities.get_mut(&entity) {
                    e.on_ground = on_ground;
                    e.y_rot = unpack_degrees(y_rot);
                    e.x_rot = unpack_degrees(x_rot);
                }
                self.sync_uncontrolled_vehicle(entity);
            }
            play::clientbound::ENTITY_POSITION_SYNC => {
                let entity = r.varint()?;
                let position = match r.varint()? {
                    0 => (r.f64()?, r.f64()?, r.f64()?),
                    _ => {
                        let mut last = (0.0, 0.0, 0.0);
                        for _ in 0..r.varint()? {
                            last = (r.f64()?, r.f64()?, r.f64()?);
                            r.varint()?;
                        }
                        last
                    }
                };
                let (y_rot, x_rot, on_ground) = (r.f32()?, r.f32()?, r.bool()?);
                if let Some(e) = self.entities.get_mut(&entity) {
                    (e.x, e.y, e.z) = position;
                    (e.y_rot, e.x_rot, e.on_ground) = (y_rot, x_rot, on_ground);
                }
                self.sync_uncontrolled_vehicle(entity);
            }
            play::clientbound::TELEPORT_ENTITY => {
                let entity = r.varint()?;
                let position = (r.f64()?, r.f64()?, r.f64()?);
                (r.f64()?, r.f64()?, r.f64()?);
                let (y_rot, x_rot) = (r.f32()?, r.f32()?);
                let relatives = r.i32()?;
                let on_ground = r.bool()?;
                if let Some(e) = self.entities.get_mut(&entity) {
                    e.x = if relatives & 1 != 0 { e.x + position.0 } else { position.0 };
                    e.y = if relatives & 2 != 0 { e.y + position.1 } else { position.1 };
                    e.z = if relatives & 4 != 0 { e.z + position.2 } else { position.2 };
                    e.y_rot = if relatives & 8 != 0 { e.y_rot + y_rot } else { y_rot };
                    e.x_rot = if relatives & 16 != 0 { e.x_rot + x_rot } else { x_rot };
                    e.on_ground = on_ground;
                }
                self.sync_uncontrolled_vehicle(entity);
            }
            play::clientbound::SET_PASSENGERS => {
                let vehicle = r.varint()?;
                let mut passengers = Vec::new();
                for _ in 0..r.varint()? {
                    passengers.push(r.varint()?);
                }
                self.set_passengers(vehicle, &passengers);
            }
            play::clientbound::MOVE_VEHICLE => {
                let position = (r.f64()?, r.f64()?, r.f64()?);
                let (y_rot, x_rot) = (r.f32()?, r.f32()?);
                if let Some(v) = self.vehicle.as_mut() {
                    (v.x, v.y, v.z) = position;
                    (v.y_rot, v.x_rot) = (y_rot, x_rot);
                }
            }
            play::clientbound::SYSTEM_CHAT => {
                let text = component_text(&Nbt::read(&mut r)?);
                println!("chat: {text}");
                if let Some(wait) = self.wait.as_mut()
                    && let WaitKind::Chat(expected) = &wait.kind
                    && text.contains(expected.as_str())
                {
                    wait.done = true;
                }
            }
            play::clientbound::DISCONNECT => {
                println!("disconnected: {}", Nbt::read(&mut r).map(|n| component_text(&n)).unwrap_or_default());
            }
            _ => {}
        }
        if let Some(wait) = self.wait.as_mut()
            && let WaitKind::Packet(expected) = &wait.kind
            && packet_name("play", "clientbound", id) == Some(expected.as_str())
        {
            wait.done = true;
        }
        Ok(())
    }

    fn sync_uncontrolled_vehicle(&mut self, entity: i32) {
        let Some(v) = self.vehicle.as_mut() else { return };
        if v.id != entity || v.controlled {
            return;
        }
        if let Some(e) = self.entities.get(&entity) {
            (v.x, v.y, v.z, v.y_rot, v.x_rot, v.on_ground) = (e.x, e.y, e.z, e.y_rot, e.x_rot, e.on_ground);
        }
    }

    fn set_passengers(&mut self, vehicle: i32, passengers: &[i32]) {
        let mine = passengers.contains(&self.player_id);
        match (self.vehicle.as_ref().map(|v| v.id), mine) {
            (Some(current), false) if current == vehicle => self.dismounted(),
            (Some(current), true) if current != vehicle => {
                self.dismounted();
                self.mounted(vehicle, passengers);
            }
            (None, true) => self.mounted(vehicle, passengers),
            _ => {}
        }
    }

    fn mounted(&mut self, vehicle: i32, passengers: &[i32]) {
        let (entity_type, x, y, z, y_rot, x_rot, on_ground) = match self.entities.get(&vehicle) {
            Some(e) => (e.entity_type, e.x, e.y, e.z, e.y_rot, e.x_rot, e.on_ground),
            None => {
                let (x, y, z) = self.pos.unwrap_or_default();
                (-1, x, y, z, self.y_rot, self.x_rot, true)
            }
        };
        let name = entity_type_name(entity_type);
        let controlled = match self.control {
            Control::On => true,
            Control::Off => false,
            Control::Auto => passengers.first() == Some(&self.player_id) && is_steerable(name),
        };
        println!("probe: mounted {vehicle} {name} controlled={controlled}");
        self.vehicle = Some(Vehicle { id: vehicle, entity_type, x, y, z, y_rot, x_rot, on_ground, velocity: (0.0, 0.0, 0.0), forward: None, controlled });
        self.jump = JumpCharge::default();
    }

    fn dismounted(&mut self) {
        let Some(v) = self.vehicle.take() else { return };
        println!("probe: dismounted {}", v.id);
        self.pos = Some((v.x, v.y, v.z));
        self.keys &= !encode::KEY_SHIFT;
        self.timed_keys.retain(|(_, mask)| *mask != encode::KEY_SHIFT);
    }

    pub fn script_blocked(&mut self) -> bool {
        if self.wait_ticks > 0 {
            self.wait_ticks -= 1;
            return true;
        }
        if self.activity.is_some() {
            return true;
        }
        let Some(wait) = self.wait.as_mut() else { return false };
        let satisfied = wait.done
            || match &wait.kind {
                WaitKind::Entity { entity_type, from_serial } => {
                    let matching = |e: &&TrackedEntity| e.entity_type == *entity_type && from_serial.is_none_or(|serial| e.serial > serial);
                    self.entities.values().any(|e| matching(&e))
                }
                WaitKind::Screen => self.open_container != 0,
                WaitKind::Mounted => self.vehicle.is_some(),
                WaitKind::Dismounted => self.vehicle.is_none(),
                WaitKind::Ack(sequence) => self.acked >= *sequence,
                WaitKind::Packet(_) | WaitKind::Chat(_) => false,
            };
        if satisfied {
            let wait = self.wait.take().unwrap();
            if let WaitKind::Entity { entity_type, from_serial } = wait.kind {
                let found = match from_serial {
                    Some(serial) => self.entities.iter().filter(|(_, e)| e.entity_type == entity_type && e.serial > serial).min_by_key(|(_, e)| e.serial).map(|(id, _)| *id),
                    None => self.nearest(entity_type),
                };
                if let Some(found) = found {
                    self.last_entity = found;
                    println!("probe: {} -> entity {found}", wait.description);
                }
            } else {
                println!("probe: {} done", wait.description);
            }
            return false;
        }
        wait.remaining = wait.remaining.saturating_sub(1);
        if wait.remaining == 0 {
            eprintln!("probe: TIMEOUT {}", wait.description);
            self.wait = None;
            self.aborted = true;
        }
        true
    }

    fn start_wait(&mut self, kind: WaitKind, description: String, timeout: u32) {
        self.wait = Some(Wait { kind, description, remaining: timeout, done: false });
    }

    pub fn tick_keys(&mut self) {
        let mut released = 0;
        self.timed_keys.retain_mut(|(ticks, mask)| {
            *ticks -= 1;
            if *ticks == 0 {
                released |= *mask;
            }
            *ticks > 0
        });
        self.keys &= !released;
    }

    pub fn tick_activity(&mut self) {
        match self.activity {
            Some(Activity::Break { remaining }) => {
                if remaining <= 1 {
                    self.activity = None;
                    if let Some((pos, face)) = self.break_target.take() {
                        let sequence = self.next_sequence();
                        self.send(play::serverbound::PLAYER_ACTION, |w| encode::player_action(w, encode::ACTION_STOP_DESTROY_BLOCK, pos, face, sequence));
                    }
                } else {
                    self.activity = Some(Activity::Break { remaining: remaining - 1 });
                }
                self.send(play::serverbound::PUNCH, |_| {});
            }
            Some(Activity::HoldUse { remaining }) => {
                if remaining <= 1 {
                    self.activity = None;
                    self.release_use_item();
                } else {
                    self.activity = Some(Activity::HoldUse { remaining: remaining - 1 });
                }
            }
            None => {}
        }
    }

    fn release_use_item(&mut self) {
        self.send(play::serverbound::PLAYER_ACTION, |w| encode::player_action(w, encode::ACTION_RELEASE_USE_ITEM, (0, 0, 0), encode::FACE_DOWN, 0));
    }

    pub fn send_input_if_changed(&mut self) {
        if self.keys == self.sent_keys {
            return;
        }
        let keys = self.keys;
        let sprint_changed = (keys ^ self.sent_keys) & encode::KEY_SPRINT != 0;
        self.sent_keys = keys;
        self.send(play::serverbound::PLAYER_INPUT, |w| encode::player_input(w, keys));
        if sprint_changed {
            let action = if keys & encode::KEY_SPRINT != 0 { encode::COMMAND_START_SPRINTING } else { encode::COMMAND_STOP_SPRINTING };
            self.player_command(action, 0);
        }
    }

    fn player_command(&mut self, action: i32, data: i32) {
        let entity = self.player_id;
        self.send(play::serverbound::PLAYER_COMMAND, |w| encode::player_command(w, entity, action, data));
    }

    pub fn tick_riding(&mut self) {
        let Some(v) = self.vehicle.as_mut() else { return };
        let name = entity_type_name(v.entity_type);
        let (controlled, boat, jumpable) = (v.controlled, is_boat(name), is_jumpable(name));
        if controlled {
            if !boat {
                v.y_rot = self.y_rot;
                v.x_rot = self.x_rot * 0.5;
            }
            if let Some(speed) = v.forward {
                let yaw = (v.y_rot as f64).to_radians();
                v.velocity = (-yaw.sin() * speed, 0.0, yaw.cos() * speed);
            }
            v.x += v.velocity.0;
            v.y += v.velocity.1;
            v.z += v.velocity.2;
        }
        let keys = self.keys;
        if controlled && boat {
            let forward = keys & encode::KEY_FORWARD != 0;
            let left = keys & encode::KEY_LEFT != 0;
            let right = keys & encode::KEY_RIGHT != 0;
            self.send(play::serverbound::PADDLE_BOAT, |w| encode::paddle_boat(w, right && !left || forward, left && !right || forward));
        }
        self.send_input_if_changed();
        self.tick_jump(controlled && jumpable);
        let (y_rot, x_rot, on_ground) = (self.y_rot, self.x_rot, self.on_ground != 0);
        self.send(play::serverbound::MOVE_PLAYER_ROT, |w| encode::move_player_rot(w, y_rot, x_rot, on_ground, false));
        if controlled && let Some(v) = self.vehicle.as_ref() {
            let (position, y_rot, x_rot, vehicle_on_ground) = ((v.x, v.y, v.z), v.y_rot, v.x_rot, v.on_ground);
            self.send(play::serverbound::MOVE_VEHICLE, |w| encode::move_vehicle(w, position, y_rot, x_rot, vehicle_on_ground));
        }
    }

    fn tick_jump(&mut self, jumpable: bool) {
        let jumping = self.keys & encode::KEY_JUMP != 0;
        if jumpable {
            let mut start_jump = None;
            let jump = &mut self.jump;
            if jump.ticks < 0 {
                jump.ticks += 1;
                if jump.ticks == 0 {
                    jump.scale = 0.0;
                }
            }
            if jump.was_jumping && !jumping {
                jump.ticks = -10;
                start_jump = Some((jump.scale * 100.0).floor() as i32);
            } else if !jump.was_jumping && jumping {
                jump.ticks = 0;
                jump.scale = 0.0;
            } else if jump.was_jumping {
                jump.ticks += 1;
                jump.scale = if jump.ticks < 10 { jump.ticks as f32 * 0.1 } else { 0.8 + 2.0 / (jump.ticks - 9) as f32 * 0.1 };
            }
            if let Some(power) = start_jump {
                self.player_command(encode::COMMAND_START_RIDING_JUMP, power);
            }
        }
        self.jump.was_jumping = jumping;
    }

    pub fn command(&mut self, words: &[&str], line: &str) -> anyhow::Result<()> {
        match words {
            ["wait", ticks] => self.wait_ticks = parse(ticks)?,
            ["wait_entity", rest @ ..] => {
                let (rest, timeout) = read_timeout(rest)?;
                let [name] = rest[..] else { bail!("usage: wait_entity <type> [timeout=N]") };
                let entity_type = entity_type_id(name)?;
                self.start_wait(WaitKind::Entity { entity_type, from_serial: None }, format!("wait_entity {name}"), timeout);
            }
            ["wait_new_entity", rest @ ..] => {
                let (rest, timeout) = read_timeout(rest)?;
                let [name] = rest[..] else { bail!("usage: wait_new_entity <type> [timeout=N]") };
                let entity_type = entity_type_id(name)?;
                let from_serial = Some(self.serial);
                self.start_wait(WaitKind::Entity { entity_type, from_serial }, format!("wait_new_entity {name}"), timeout);
            }
            ["wait_packet", rest @ ..] => {
                let (rest, timeout) = read_timeout(rest)?;
                let [name] = rest[..] else { bail!("usage: wait_packet <name> [timeout=N]") };
                let full = if name.contains(':') { name.to_owned() } else { format!("minecraft:{name}") };
                self.start_wait(WaitKind::Packet(full), format!("wait_packet {name}"), timeout);
            }
            ["wait_chat", rest @ ..] => {
                let (rest, timeout) = read_timeout(rest)?;
                let text = rest.join(" ");
                self.start_wait(WaitKind::Chat(text.clone()), format!("wait_chat {text}"), timeout);
            }
            ["wait_screen", rest @ ..] => {
                let (_, timeout) = read_timeout(rest)?;
                self.start_wait(WaitKind::Screen, "wait_screen".into(), timeout);
            }
            ["wait_mounted", rest @ ..] => {
                let (_, timeout) = read_timeout(rest)?;
                self.start_wait(WaitKind::Mounted, "wait_mounted".into(), timeout);
            }
            ["wait_dismounted", rest @ ..] => {
                let (_, timeout) = read_timeout(rest)?;
                self.start_wait(WaitKind::Dismounted, "wait_dismounted".into(), timeout);
            }
            ["wait_ack", rest @ ..] => {
                let (_, timeout) = read_timeout(rest)?;
                let sequence = self.sequence;
                self.start_wait(WaitKind::Ack(sequence), format!("wait_ack {sequence}"), timeout);
            }
            ["target", entity] => {
                self.last_entity = self.resolve_entity(entity)?;
                println!("probe: target {}", self.last_entity);
            }
            ["entities", rest @ ..] => {
                let filter = rest.first().map(|name| entity_type_id(name)).transpose()?;
                for (id, e) in self.entities.iter().filter(|(_, e)| filter.is_none_or(|t| t == e.entity_type)) {
                    println!("probe: entity {id} {} at {:.3} {:.3} {:.3} yaw {:.1} pitch {:.1}", entity_type_name(e.entity_type), e.x, e.y, e.z, e.y_rot, e.x_rot);
                }
            }
            ["skip_type", entity_type] => self.skip_types.push(parse(entity_type)?),
            ["suggest", id, ..] => {
                let id: i32 = parse(id)?;
                let rest = line.splitn(3, ' ').nth(2).unwrap_or("");
                let text = rest.strip_suffix('$').unwrap_or(rest);
                self.send(play::serverbound::COMMAND_SUGGESTION, |w| {
                    w.varint(id);
                    w.string(text);
                });
            }
            ["respawn"] => self.send(play::serverbound::CLIENT_COMMAND, |w| w.varint(0)),
            ["stats"] => self.send(play::serverbound::CLIENT_COMMAND, |w| w.varint(1)),
            ["change_game_mode", mode] => {
                let mode: i32 = parse(mode)?;
                self.send(play::serverbound::CHANGE_GAME_MODE, |w| w.varint(mode));
            }
            ["input", flags] => {
                let flags: u8 = parse(flags)?;
                self.keys = flags;
                self.sent_keys = flags;
                self.send(play::serverbound::PLAYER_INPUT, |w| encode::player_input(w, flags));
            }
            ["keys", names @ ..] => self.keys = key_mask(names)?,
            ["hold", ticks, names @ ..] => {
                let (ticks, mask): (u32, u8) = (parse(ticks)?, key_mask(names)?);
                self.keys |= mask;
                if ticks > 0 {
                    self.timed_keys.push((ticks, mask));
                }
            }
            ["action", status, x, y, z, face] => {
                let (status, x, y, z, face): (i32, i32, i32, i32, u8) = (parse(status)?, parse(x)?, parse(y)?, parse(z)?, parse(face)?);
                let sequence = self.next_sequence();
                self.send(play::serverbound::PLAYER_ACTION, |w| encode::player_action(w, status, (x, y, z), face, sequence));
            }
            ["move_air", nx, ny, nz, y_rot, x_rot @ ..] => {
                let (nx, ny, nz, y_rot): (f64, f64, f64, f32) = (parse(nx)?, parse(ny)?, parse(nz)?, parse(y_rot)?);
                let x_rot: f32 = x_rot.first().map_or(Ok(0.0), |x_rot| parse(x_rot))?;
                self.pos = Some((nx, ny, nz));
                (self.y_rot, self.x_rot, self.on_ground) = (y_rot, x_rot, 0);
                self.send(play::serverbound::MOVE_PLAYER_POS_ROT, |w| {
                    w.f64(nx);
                    w.f64(ny);
                    w.f64(nz);
                    w.f32(y_rot);
                    w.f32(x_rot);
                    w.u8(0);
                });
            }
            ["move", nx, ny, nz, y_rot] => {
                let (nx, ny, nz, y_rot): (f64, f64, f64, f32) = (parse(nx)?, parse(ny)?, parse(nz)?, parse(y_rot)?);
                self.pos = Some((nx, ny, nz));
                (self.y_rot, self.x_rot, self.on_ground) = (y_rot, 0.0, 1);
                self.send(play::serverbound::MOVE_PLAYER_POS_ROT, |w| {
                    w.f64(nx);
                    w.f64(ny);
                    w.f64(nz);
                    w.f32(y_rot);
                    w.f32(0.0);
                    w.u8(1);
                });
            }
            ["rot", y_rot, x_rot] => {
                let (y_rot, x_rot): (f32, f32) = (parse(y_rot)?, parse(x_rot)?);
                (self.y_rot, self.x_rot) = (y_rot, x_rot);
                let on_ground = self.on_ground != 0;
                self.send(play::serverbound::MOVE_PLAYER_ROT, |w| encode::move_player_rot(w, y_rot, x_rot, on_ground, false));
            }
            ["start_fall_flying"] => self.player_command(6, 0),
            ["command", action, data] => {
                let (action, data): (i32, i32) = (parse(action)?, parse(data)?);
                self.player_command(action, data);
            }
            ["horse_inventory"] => self.player_command(encode::COMMAND_OPEN_INVENTORY, 0),
            ["structure", x, y, z, update_type, mode, name, ox, oy, oz, sx, sy, sz, mirror, rotation, integrity, seed, flags] => {
                let (x, y, z): (i32, i32, i32) = (parse(x)?, parse(y)?, parse(z)?);
                let bytes: Vec<i8> = [ox, oy, oz, sx, sy, sz].iter().map(|v| parse(v)).collect::<Result<_, _>>()?;
                let (update_type, mode, mirror, rotation): (i32, i32, i32, i32) = (parse(update_type)?, parse(mode)?, parse(mirror)?, parse(rotation)?);
                let (integrity, seed, flags): (f32, i64, u8) = (parse(integrity)?, parse(seed)?, parse(flags)?);
                let name = if *name == "-" { "" } else { name };
                self.send(play::serverbound::SET_STRUCTURE_BLOCK, |w| {
                    w.position(x, y, z);
                    w.varint(update_type);
                    w.varint(mode);
                    w.string(name);
                    bytes.iter().for_each(|&b| w.u8(b as u8));
                    w.varint(mirror);
                    w.varint(rotation);
                    w.string("");
                    w.f32(integrity);
                    w.varlong(seed);
                    w.u8(flags);
                });
            }
            ["sign", x, y, z, slot, rest @ ..] => {
                let (x, y, z, slot): (i32, i32, i32, i32) = (parse(x)?, parse(y)?, parse(z)?, parse(slot)?);
                let text = rest.join(" ");
                let mut lines: Vec<&str> = text.split('|').collect();
                lines.resize(4, "");
                self.send(play::serverbound::SIGN_UPDATE, |w| {
                    w.position(x, y, z);
                    for line in &lines[..4] {
                        w.string(line);
                    }
                    w.varint(slot);
                });
            }
            ["button", button] => {
                let button: i32 = parse(button)?;
                let container = self.open_container;
                self.send(play::serverbound::CONTAINER_BUTTON_CLICK, |w| {
                    w.varint(container);
                    w.varint(button);
                });
            }
            ["beacon", primary, secondary] => {
                let (primary, secondary): (i32, i32) = (parse(primary)?, parse(secondary)?);
                self.send(play::serverbound::SET_BEACON, |w| {
                    for effect in [primary, secondary] {
                        w.bool(effect >= 0);
                        if effect >= 0 {
                            w.varint(effect);
                        }
                    }
                });
            }
            ["click", slot, button, mode] => {
                let (slot, button, mode): (i16, u8, i32) = (parse(slot)?, parse(button)?, parse(mode)?);
                self.container_click(slot, button, mode);
            }
            ["quick", slot] => {
                let slot: i16 = parse(slot)?;
                self.container_click(slot, 0, 1);
            }
            ["bundle_select", slot, index] => {
                let (slot, index): (i32, i32) = (parse(slot)?, parse(index)?);
                self.send(play::serverbound::BUNDLE_ITEM_SELECTED, |w| {
                    w.varint(slot);
                    w.varint(index);
                });
            }
            ["place_recipe", recipe, use_max_items] => {
                let (recipe, use_max_items): (i32, bool) = (parse(recipe)?, parse(use_max_items)?);
                let container = self.open_container;
                self.send(play::serverbound::PLACE_RECIPE, |w| {
                    w.varint(container);
                    w.varint(recipe);
                    w.bool(use_max_items);
                });
            }
            ["book_settings", book_type, is_open, is_filtering] => {
                let (book_type, is_open, is_filtering): (i32, bool, bool) = (parse(book_type)?, parse(is_open)?, parse(is_filtering)?);
                self.send(play::serverbound::RECIPE_BOOK_CHANGE_SETTINGS, |w| {
                    w.varint(book_type);
                    w.bool(is_open);
                    w.bool(is_filtering);
                });
            }
            ["rename", rest @ ..] => {
                let name = rest.join(" ");
                self.send(play::serverbound::RENAME_ITEM, |w| w.string(&name));
            }
            ["slot_state", slot_id, new_state] => {
                let (slot_id, new_state): (i32, bool) = (parse(slot_id)?, parse(new_state)?);
                let container = self.open_container;
                self.send(play::serverbound::CONTAINER_SLOT_STATE_CHANGED, |w| {
                    w.varint(slot_id);
                    w.varint(container);
                    w.bool(new_state);
                });
            }
            ["advancement_tab", tab] => {
                self.send(play::serverbound::SEEN_ADVANCEMENTS, |w| {
                    w.varint(0);
                    w.string(tab);
                });
            }
            ["advancement_close"] => self.send(play::serverbound::SEEN_ADVANCEMENTS, |w| w.varint(1)),
            ["seen_recipe", recipe] => {
                let recipe: i32 = parse(recipe)?;
                self.send(play::serverbound::RECIPE_BOOK_SEEN_RECIPE, |w| w.varint(recipe));
            }
            ["close"] => {
                let container = self.open_container;
                self.send(play::serverbound::CONTAINER_CLOSE, |w| w.varint(container));
                self.open_container = 0;
            }
            ["hotbar", slot] => {
                let slot: i16 = parse(slot)?;
                self.send(play::serverbound::SET_CARRIED_ITEM, |w| w.i16(slot));
            }
            ["interact_last", hand, rest @ ..] => {
                let hand: i32 = parse(hand)?;
                let using_secondary_action = rest.first().is_some_and(|flag| *flag == "sneak");
                let entity = self.last_entity;
                self.send(play::serverbound::INTERACT, |w| encode::interact(w, entity, hand, (0.0, 0.2, 0.0), using_secondary_action));
            }
            ["attack_last"] => {
                let entity = self.last_entity;
                self.send(play::serverbound::ATTACK, |w| encode::attack(w, entity));
            }
            ["interact", entity, rest @ ..] => self.interact(entity, rest)?,
            ["mount", entity, rest @ ..] => self.interact(entity, rest)?,
            ["attack", entity] => {
                let entity = self.resolve_entity(entity)?;
                self.send(play::serverbound::ATTACK, |w| encode::attack(w, entity));
                self.send(play::serverbound::PUNCH, |_| {});
            }
            ["punch"] => self.send(play::serverbound::PUNCH, |_| {}),
            ["dismount", rest @ ..] => {
                let (_, timeout) = read_timeout(rest)?;
                if self.vehicle.is_some() {
                    self.keys |= encode::KEY_SHIFT;
                    self.timed_keys.push((timeout.max(1), encode::KEY_SHIFT));
                    self.start_wait(WaitKind::Dismounted, "dismount".into(), timeout);
                }
            }
            ["ride_control", mode] => {
                self.control = match *mode {
                    "auto" => Control::Auto,
                    "on" => Control::On,
                    "off" => Control::Off,
                    _ => bail!("usage: ride_control auto|on|off"),
                };
                if let Some(v) = self.vehicle.as_mut() {
                    v.controlled = match self.control {
                        Control::On => true,
                        Control::Off => false,
                        Control::Auto => v.controlled,
                    };
                    if v.controlled {
                        v.velocity = (0.0, 0.0, 0.0);
                        v.forward = None;
                    }
                }
            }
            ["ride_move", dx, dy, dz] => {
                let velocity = (parse(dx)?, parse(dy)?, parse(dz)?);
                let v = self.vehicle.as_mut().context("not riding anything")?;
                v.velocity = velocity;
                v.forward = None;
            }
            ["ride_forward", speed] => {
                let speed: f64 = parse(speed)?;
                self.vehicle.as_mut().context("not riding anything")?.forward = Some(speed);
            }
            ["ride_to", x, y, z, rest @ ..] => {
                let (x, y, z): (f64, f64, f64) = (parse(x)?, parse(y)?, parse(z)?);
                let rotation = rest.iter().map(|r| parse::<f32>(r)).collect::<Result<Vec<_>, _>>()?;
                let v = self.vehicle.as_mut().context("not riding anything")?;
                (v.x, v.y, v.z) = (x, y, z);
                if let Some(y_rot) = rotation.first() {
                    v.y_rot = *y_rot;
                }
                if let Some(x_rot) = rotation.get(1) {
                    v.x_rot = *x_rot;
                }
            }
            ["ride_rot", y_rot, x_rot] => {
                let (y_rot, x_rot): (f32, f32) = (parse(y_rot)?, parse(x_rot)?);
                let v = self.vehicle.as_mut().context("not riding anything")?;
                (v.y_rot, v.x_rot) = (y_rot, x_rot);
            }
            ["ride_ground", on_ground] => {
                let on_ground: u8 = parse(on_ground)?;
                self.vehicle.as_mut().context("not riding anything")?.on_ground = on_ground != 0;
            }
            ["vehicle", vx, vy, vz, y_rot, rest @ ..] => {
                let (vx, vy, vz, y_rot): (f64, f64, f64, f32) = (parse(vx)?, parse(vy)?, parse(vz)?, parse(y_rot)?);
                let vehicle_on_ground = rest.first().is_some_and(|flag| *flag == "1");
                self.send(play::serverbound::MOVE_VEHICLE, |w| encode::move_vehicle(w, (vx, vy, vz), y_rot, 0.0, vehicle_on_ground));
            }
            ["paddle", left, right] => {
                let (left, right): (u8, u8) = (parse(left)?, parse(right)?);
                self.send(play::serverbound::PADDLE_BOAT, |w| encode::paddle_boat(w, left != 0, right != 0));
            }
            ["use_item", y_rot, x_rot] => {
                let (y_rot, x_rot): (f32, f32) = (parse(y_rot)?, parse(x_rot)?);
                self.use_item(0, y_rot, x_rot);
            }
            ["use_item", hand] => {
                let hand = parse_hand(hand)?;
                let (y_rot, x_rot) = (self.y_rot, self.x_rot);
                self.use_item(hand, y_rot, x_rot);
            }
            ["use_item", hand, y_rot, x_rot] => {
                let (hand, y_rot, x_rot) = (parse_hand(hand)?, parse(y_rot)?, parse(x_rot)?);
                self.use_item(hand, y_rot, x_rot);
            }
            ["use_item"] => {
                let (y_rot, x_rot) = (self.y_rot, self.x_rot);
                self.use_item(0, y_rot, x_rot);
            }
            ["hold_use", hand, ticks] => {
                let (hand, ticks) = (parse_hand(hand)?, parse::<u32>(ticks)?);
                let (y_rot, x_rot) = (self.y_rot, self.x_rot);
                self.use_item(hand, y_rot, x_rot);
                self.activity = Some(Activity::HoldUse { remaining: ticks.max(1) });
            }
            ["release"] => self.release_use_item(),
            ["use", x, y, z, face, rest @ ..] => {
                let (x, y, z): (i32, i32, i32) = (parse(x)?, parse(y)?, parse(z)?);
                let face = parse_face(face)? as i32;
                let (mut hand, mut inside, mut world_border) = (0, false, false);
                let mut cursor = Vec::new();
                for word in rest {
                    match *word {
                        "inside" => inside = true,
                        "border" => world_border = true,
                        "offhand" | "off" => hand = 1,
                        _ => cursor.push(parse::<f32>(word)?),
                    }
                }
                let cursor = if let [cx, cy, cz] = cursor[..] { (cx, cy, cz) } else { (0.5, 0.5, 0.5) };
                let hit = BlockHit { pos: (x, y, z), face, cursor, inside, world_border };
                let sequence = self.next_sequence();
                self.send(play::serverbound::USE_ITEM_ON, |w| encode::use_item_on(w, hand, &hit, sequence));
            }
            ["swap_hands"] => self.send(play::serverbound::PLAYER_ACTION, |w| encode::player_action(w, encode::ACTION_SWAP_ITEM_WITH_OFFHAND, (0, 0, 0), encode::FACE_DOWN, 0)),
            ["drop"] => self.send(play::serverbound::PLAYER_ACTION, |w| encode::player_action(w, encode::ACTION_DROP_ITEM, (0, 0, 0), encode::FACE_DOWN, 0)),
            ["drop", "all"] => self.send(play::serverbound::PLAYER_ACTION, |w| encode::player_action(w, encode::ACTION_DROP_ALL_ITEMS, (0, 0, 0), encode::FACE_DOWN, 0)),
            ["stab"] => self.send(play::serverbound::PLAYER_ACTION, |w| encode::player_action(w, encode::ACTION_STAB, (0, 0, 0), encode::FACE_DOWN, 0)),
            ["break", x, y, z, face, ticks] => {
                let (x, y, z): (i32, i32, i32) = (parse(x)?, parse(y)?, parse(z)?);
                let (face, ticks) = (parse_face(face)?, parse::<u32>(ticks)?);
                let sequence = self.next_sequence();
                self.send(play::serverbound::PLAYER_ACTION, |w| encode::player_action(w, encode::ACTION_START_DESTROY_BLOCK, (x, y, z), face, sequence));
                self.send(play::serverbound::PUNCH, |_| {});
                if ticks > 0 {
                    self.break_target = Some(((x, y, z), face));
                    self.activity = Some(Activity::Break { remaining: ticks });
                }
            }
            ["break_abort", x, y, z] => {
                let (x, y, z): (i32, i32, i32) = (parse(x)?, parse(y)?, parse(z)?);
                self.send(play::serverbound::PLAYER_ACTION, |w| encode::player_action(w, encode::ACTION_ABORT_DESTROY_BLOCK, (x, y, z), encode::FACE_DOWN, 0));
            }
            ["pick_block", x, y, z, rest @ ..] => {
                let (x, y, z): (i32, i32, i32) = (parse(x)?, parse(y)?, parse(z)?);
                let include_data = rest.first().is_some_and(|flag| *flag == "data");
                self.send(play::serverbound::PICK_ITEM_FROM_BLOCK, |w| encode::pick_item_from_block(w, (x, y, z), include_data));
            }
            ["pick_entity", entity, rest @ ..] => {
                let entity = self.resolve_entity(entity)?;
                let include_data = rest.first().is_some_and(|flag| *flag == "data");
                self.send(play::serverbound::PICK_ITEM_FROM_ENTITY, |w| encode::pick_item_from_entity(w, entity, include_data));
            }
            ["select_trade", index] => {
                let index: i32 = parse(index)?;
                self.send(play::serverbound::SELECT_TRADE, |w| encode::select_trade(w, index));
            }
            ["test_instance", action, x, y, z, rest @ ..] => self.test_instance(action, [x, y, z], rest)?,
            ["set_test_block", x, y, z, mode, message @ ..] => {
                let (x, y, z): (i32, i32, i32) = (parse(x)?, parse(y)?, parse(z)?);
                let mode = match *mode {
                    "start" => 0,
                    "log" => 1,
                    "fail" => 2,
                    "accept" => 3,
                    _ => parse(mode)?,
                };
                let message = message.join(" ");
                self.send(play::serverbound::SET_TEST_BLOCK, |w| encode::set_test_block(w, (x, y, z), mode, &message));
            }
            _ => {
                let command = line.strip_prefix('/').unwrap_or(line);
                self.send(play::serverbound::CHAT_COMMAND, |w| w.string(command));
            }
        }
        Ok(())
    }

    fn container_click(&mut self, slot: i16, button: u8, mode: i32) {
        let (container, state_id) = (self.open_container, self.state_id());
        self.send(play::serverbound::CONTAINER_CLICK, |w| encode::container_click(w, container, state_id, slot, button, mode));
    }

    fn use_item(&mut self, hand: i32, y_rot: f32, x_rot: f32) {
        let sequence = self.next_sequence();
        self.send(play::serverbound::USE_ITEM, |w| encode::use_item(w, hand, sequence, y_rot, x_rot));
    }

    fn interact(&mut self, entity: &str, rest: &[&str]) -> anyhow::Result<()> {
        let entity = self.resolve_entity(entity)?;
        let (mut hand, mut sneak, mut at) = (0, self.keys & encode::KEY_SHIFT != 0, (0.0, 0.2, 0.0));
        let mut words = rest.iter();
        while let Some(word) = words.next() {
            match *word {
                "sneak" => sneak = true,
                "at" => {
                    let mut next = || words.next().context("at needs dx dy dz").and_then(|v| parse::<f64>(v));
                    at = (next()?, next()?, next()?);
                }
                _ => hand = parse_hand(word)?,
            }
        }
        self.send(play::serverbound::INTERACT, |w| encode::interact(w, entity, hand, at, sneak));
        Ok(())
    }

    fn test_instance(&mut self, action: &str, pos: [&str; 3], rest: &[&str]) -> anyhow::Result<()> {
        let action = match action {
            "init" => 0,
            "query" => 1,
            "set" => 2,
            "reset" => 3,
            "save" => 4,
            "export" => 5,
            "run" => 6,
            _ => parse(action)?,
        };
        let pos = (parse(pos[0])?, parse(pos[1])?, parse(pos[2])?);
        let mut ignore_entities = false;
        let mut tokens: Vec<&str> = Vec::new();
        for word in rest {
            if *word == "ignore_entities" {
                ignore_entities = true;
            } else {
                tokens.push(word);
            }
        }
        let mut tokens = tokens.into_iter().peekable();
        let mut test = None;
        if let Some(name) = tokens.next_if(|t| t.parse::<i32>().is_err()) {
            if name != "-" {
                test = Some(if name.contains(':') { name.to_owned() } else { format!("minecraft:{name}") });
            }
        }
        let mut numbers = Vec::new();
        for token in tokens {
            numbers.push(match token {
                "none" => 0,
                "clockwise_90" => 1,
                "180" => 2,
                "counterclockwise_90" => 3,
                _ => parse::<i32>(token)?,
            });
        }
        let size = match numbers[..] {
            [] => (0, 0, 0),
            [sx, sy, sz, ..] => (sx, sy, sz),
            _ => bail!("usage: test_instance <action> x y z [test|-] [sx sy sz [rotation]] [ignore_entities]"),
        };
        let rotation = numbers.get(3).copied().unwrap_or(0);
        let data = TestInstanceData { test: test.as_deref(), size, rotation, ignore_entities };
        self.send(play::serverbound::TEST_INSTANCE_BLOCK_ACTION, |w| encode::test_instance_block_action(w, pos, action, &data));
        Ok(())
    }

}

#[cfg(test)]
mod tests {
    use wafer_data::packets::play::{clientbound, serverbound};

    use super::*;

    fn packets(session: &mut Session) -> Vec<(i32, Vec<u8>)> {
        session
            .outbox
            .drain(..)
            .map(|bytes| {
                let mut r = Reader::new(&bytes);
                let id = r.varint().unwrap();
                (id, bytes[bytes.len() - r.remaining()..].to_vec())
            })
            .collect()
    }

    fn run(session: &mut Session, line: &str) {
        let words: Vec<&str> = line.split_whitespace().collect();
        session.command(&words, line).unwrap();
    }

    fn add_entity(session: &mut Session, id: i32, entity_type: &str, pos: (f64, f64, f64)) {
        let mut w = Writer::new();
        w.varint(id);
        w.uuid(id as u128);
        w.varint(entity_type_id(entity_type).unwrap());
        w.f64(pos.0);
        w.f64(pos.1);
        w.f64(pos.2);
        w.lp_vec3(0.0, 0.0, 0.0);
        w.u8(0);
        w.u8(0);
        w.u8(0);
        w.varint(0);
        session.on_packet(clientbound::ADD_ENTITY, w.as_slice()).unwrap();
    }

    fn set_passengers(session: &mut Session, vehicle: i32, passengers: &[i32]) {
        let mut w = Writer::new();
        w.varint(vehicle);
        w.varint(passengers.len() as i32);
        passengers.iter().for_each(|p| w.varint(*p));
        session.on_packet(clientbound::SET_PASSENGERS, w.as_slice()).unwrap();
    }

    fn login(session: &mut Session, player_id: i32) {
        let mut w = Writer::new();
        w.i32(player_id);
        session.on_packet(clientbound::LOGIN, w.as_slice()).unwrap();
    }

    #[test]
    fn use_item_takes_a_new_sequence_each_time_and_uses_current_rotation() {
        let mut s = Session::new();
        run(&mut s, "rot 90 10");
        s.outbox.clear();
        run(&mut s, "use_item");
        run(&mut s, "use_item off");
        let sent = packets(&mut s);
        let mut r = Reader::new(&sent[0].1);
        assert_eq!((sent[0].0, r.varint().unwrap(), r.varint().unwrap(), r.f32().unwrap(), r.f32().unwrap()), (serverbound::USE_ITEM, 0, 1, 90.0, 10.0));
        let mut r = Reader::new(&sent[1].1);
        assert_eq!((r.varint().unwrap(), r.varint().unwrap()), (1, 2));
    }

    #[test]
    fn wait_entity_picks_the_nearest_matching_entity() {
        let mut s = Session::new();
        s.pos = Some((0.0, 0.0, 0.0));
        add_entity(&mut s, 5, "minecraft:pig", (10.0, 0.0, 0.0));
        add_entity(&mut s, 6, "minecraft:pig", (2.0, 0.0, 0.0));
        add_entity(&mut s, 7, "minecraft:cow", (1.0, 0.0, 0.0));
        run(&mut s, "wait_entity pig");
        assert!(!s.script_blocked());
        assert_eq!(s.last_entity, 6);
    }

    #[test]
    fn wait_new_entity_ignores_entities_added_before() {
        let mut s = Session::new();
        add_entity(&mut s, 5, "minecraft:pig", (0.0, 0.0, 0.0));
        run(&mut s, "wait_new_entity pig timeout=3");
        assert!(s.script_blocked());
        add_entity(&mut s, 9, "minecraft:pig", (0.0, 0.0, 0.0));
        assert!(!s.script_blocked());
        assert_eq!(s.last_entity, 9);
    }

    #[test]
    fn wait_times_out_and_aborts_the_script() {
        let mut s = Session::new();
        run(&mut s, "wait_entity cow timeout=2");
        assert!(s.script_blocked());
        assert!(s.script_blocked());
        assert!(s.aborted);
        assert!(!s.script_blocked());
    }

    #[test]
    fn wait_packet_completes_when_the_named_packet_arrives() {
        let mut s = Session::new();
        run(&mut s, "wait_packet open_screen");
        assert!(s.script_blocked());
        let mut w = Writer::new();
        w.varint(3);
        s.on_packet(clientbound::OPEN_SCREEN, w.as_slice()).unwrap();
        assert!(!s.script_blocked());
        assert_eq!(s.open_container, 3);
    }

    #[test]
    fn interact_uses_entity_reference_hand_location_and_shift_key() {
        let mut s = Session::new();
        add_entity(&mut s, 12, "minecraft:armor_stand", (0.0, 0.0, 0.0));
        run(&mut s, "keys sneak");
        run(&mut s, "interact armor_stand off at 0 1 0");
        let sent = packets(&mut s);
        let mut r = Reader::new(&sent[0].1);
        assert_eq!(sent[0].0, serverbound::INTERACT);
        assert_eq!((r.varint().unwrap(), r.varint().unwrap()), (12, 1));
        let (_, y, _) = r.lp_vec3().unwrap();
        assert!((y - 1.0).abs() < 1.0e-4);
        assert!(r.bool().unwrap());
    }

    #[test]
    fn mounting_a_boat_sends_paddles_input_rotation_and_vehicle_movement_in_vanilla_order() {
        let mut s = Session::new();
        login(&mut s, 1);
        s.pos = Some((0.0, 64.0, 0.0));
        add_entity(&mut s, 20, "minecraft:oak_boat", (1.0, 64.0, 1.0));
        set_passengers(&mut s, 20, &[1]);
        assert!(s.riding());
        run(&mut s, "keys forward");
        run(&mut s, "ride_move 0 0 0.25");
        s.tick_riding();
        let ids: Vec<i32> = packets(&mut s).iter().map(|p| p.0).collect();
        assert_eq!(ids, [serverbound::PADDLE_BOAT, serverbound::PLAYER_INPUT, serverbound::MOVE_PLAYER_ROT, serverbound::MOVE_VEHICLE]);
        s.tick_riding();
        let sent = packets(&mut s);
        let ids: Vec<i32> = sent.iter().map(|p| p.0).collect();
        assert_eq!(ids, [serverbound::PADDLE_BOAT, serverbound::MOVE_PLAYER_ROT, serverbound::MOVE_VEHICLE]);
        let mut r = Reader::new(&sent[2].1);
        assert_eq!((r.f64().unwrap(), r.f64().unwrap(), r.f64().unwrap()), (1.0, 64.0, 1.5));
        assert_eq!(&sent[0].1, &[1, 1]);
    }

    #[test]
    fn jump_charge_sends_riding_jump_with_scaled_power_on_release() {
        let mut s = Session::new();
        login(&mut s, 1);
        add_entity(&mut s, 30, "minecraft:horse", (0.0, 64.0, 0.0));
        set_passengers(&mut s, 30, &[1]);
        run(&mut s, "hold 5 jump");
        for _ in 0..8 {
            s.tick_keys();
            s.tick_riding();
        }
        let sent = packets(&mut s);
        let jumps: Vec<&(i32, Vec<u8>)> = sent.iter().filter(|p| p.0 == serverbound::PLAYER_COMMAND).collect();
        assert_eq!(jumps.len(), 1);
        let mut r = Reader::new(&jumps[0].1);
        assert_eq!((r.varint().unwrap(), r.varint().unwrap(), r.varint().unwrap()), (1, encode::COMMAND_START_RIDING_JUMP, 30));
    }

    #[test]
    fn dismount_restores_player_position_to_the_vehicle() {
        let mut s = Session::new();
        login(&mut s, 1);
        s.pos = Some((0.0, 64.0, 0.0));
        add_entity(&mut s, 20, "minecraft:minecart", (4.0, 65.0, 4.0));
        set_passengers(&mut s, 20, &[1]);
        assert!(!s.vehicle.as_ref().unwrap().controlled);
        set_passengers(&mut s, 20, &[]);
        assert!(!s.riding());
        assert_eq!(s.pos, Some((4.0, 65.0, 4.0)));
    }

    #[test]
    fn move_entity_packets_update_tracked_positions() {
        let mut s = Session::new();
        add_entity(&mut s, 8, "minecraft:pig", (1.0, 2.0, 3.0));
        let mut w = Writer::new();
        w.varint(8);
        w.varint(1);
        w.i16(4096);
        w.i16(0);
        w.i16(-2048);
        s.on_packet(clientbound::MOVE_ENTITY_POS, w.as_slice()).unwrap();
        let e = &s.entities[&8];
        assert_eq!((e.x, e.y, e.z, e.on_ground), (2.0, 2.0, 2.5, true));
    }

    #[test]
    fn container_clicks_use_the_state_id_from_the_server() {
        let mut s = Session::new();
        let mut w = Writer::new();
        w.varint(2);
        w.varint(41);
        w.i16(0);
        w.u8(0);
        s.on_packet(clientbound::CONTAINER_SET_SLOT, w.as_slice()).unwrap();
        let mut w = Writer::new();
        w.varint(2);
        s.on_packet(clientbound::OPEN_SCREEN, w.as_slice()).unwrap();
        run(&mut s, "click 3 0 0");
        let sent = packets(&mut s);
        let mut r = Reader::new(&sent[0].1);
        assert_eq!((r.varint().unwrap(), r.varint().unwrap()), (2, 41));
    }

    #[test]
    fn break_sends_start_punches_then_stop() {
        let mut s = Session::new();
        run(&mut s, "break 1 2 3 up 3");
        for _ in 0..3 {
            s.tick_activity();
        }
        let ids: Vec<i32> = packets(&mut s).iter().map(|p| p.0).collect();
        assert_eq!(
            ids,
            [
                serverbound::PLAYER_ACTION,
                serverbound::PUNCH,
                serverbound::PUNCH,
                serverbound::PUNCH,
                serverbound::PLAYER_ACTION,
                serverbound::PUNCH
            ]
        );
        assert!(!s.script_blocked());
    }

    #[test]
    fn hold_use_releases_after_the_requested_ticks() {
        let mut s = Session::new();
        run(&mut s, "hold_use main 2");
        assert!(s.script_blocked());
        s.tick_activity();
        assert!(s.script_blocked());
        s.tick_activity();
        assert!(!s.script_blocked());
        let sent = packets(&mut s);
        assert_eq!(sent[0].0, serverbound::USE_ITEM);
        let mut r = Reader::new(&sent[1].1);
        assert_eq!((sent[1].0, r.varint().unwrap()), (serverbound::PLAYER_ACTION, encode::ACTION_RELEASE_USE_ITEM));
    }

    #[test]
    fn test_instance_parses_test_size_rotation_and_flags() {
        let mut s = Session::new();
        run(&mut s, "test_instance run 10 64 -10 example 3 4 5 180 ignore_entities");
        let sent = packets(&mut s);
        let mut r = Reader::new(&sent[0].1);
        assert_eq!((sent[0].0, r.position().unwrap(), r.varint().unwrap()), (serverbound::TEST_INSTANCE_BLOCK_ACTION, (10, 64, -10), 6));
        assert!(r.bool().unwrap());
        assert_eq!(r.string(100).unwrap(), "minecraft:example");
        assert_eq!((r.varint().unwrap(), r.varint().unwrap(), r.varint().unwrap(), r.varint().unwrap()), (3, 4, 5, 2));
        assert!(r.bool().unwrap());
    }

    #[test]
    fn unknown_lines_are_sent_as_chat_commands() {
        let mut s = Session::new();
        run(&mut s, "/give @s apple");
        let sent = packets(&mut s);
        assert_eq!(sent[0].0, serverbound::CHAT_COMMAND);
        assert_eq!(Reader::new(&sent[0].1).string(100).unwrap(), "give @s apple");
    }
}
