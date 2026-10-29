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

    pub fn decode(nbt: &Nbt) -> crate::util::codec::DecodeResult<WanderingTraderData> {
        if let Err(error) = crate::util::codec::map(nbt) {
            return crate::util::codec::DecodeResult::error(error);
        }
        let mut record = crate::util::codec::Record::default();
        let spawn_delay = record.with_default(nbt, "spawn_delay", 24000, crate::util::codec::int);
        let spawn_chance = record.with_default(nbt, "spawn_chance", 25, crate::util::codec::int);
        record.build(|| Some(WanderingTraderData { spawn_delay, spawn_chance, dirty: false }))
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
