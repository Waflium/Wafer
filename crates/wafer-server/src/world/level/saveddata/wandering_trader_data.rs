use wafer_protocol::nbt::Nbt;

pub const TYPE: &str = "wandering_trader";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WanderingTraderData {
    spawn_delay: i32,
    spawn_chance: i32,
    pub dirty: bool,
}

impl Default for WanderingTraderData {
    fn default() -> WanderingTraderData {
        WanderingTraderData { spawn_delay: 24000, spawn_chance: 25, dirty: false }
    }
}

impl WanderingTraderData {
    pub fn new_dirty() -> WanderingTraderData {
        WanderingTraderData { dirty: true, ..WanderingTraderData::default() }
    }

    pub fn decode(nbt: &Nbt) -> WanderingTraderData {
        let int = |key: &str, default: i32| nbt.get(key).and_then(Nbt::as_i64).map_or(default, |v| v as i32);
        WanderingTraderData { spawn_delay: int("spawn_delay", 24000), spawn_chance: int("spawn_chance", 25), dirty: false }
    }

    pub fn encode(&self) -> Nbt {
        Nbt::Compound(vec![("spawn_delay".into(), Nbt::Int(self.spawn_delay)), ("spawn_chance".into(), Nbt::Int(self.spawn_chance))])
    }

    pub fn spawn_delay(&self) -> i32 {
        self.spawn_delay
    }

    pub fn set_spawn_delay(&mut self, spawn_delay: i32) {
        if self.spawn_delay != spawn_delay {
            self.spawn_delay = spawn_delay;
            self.dirty = true;
        }
    }

    pub fn spawn_chance(&self) -> i32 {
        self.spawn_chance
    }

    pub fn set_spawn_chance(&mut self, spawn_chance: i32) {
        if self.spawn_chance != spawn_chance {
            self.spawn_chance = spawn_chance;
            self.dirty = true;
        }
    }
}
