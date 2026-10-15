use wafer_protocol::nbt::Nbt;

use super::expirable_value::ExpirableValue;
use super::memory_module_type::{Erased, MemoryModuleType};

#[derive(Clone, Debug, Default)]
pub struct MemoryMap {
    memories: Vec<(MemoryModuleType<Erased>, ExpirableValue)>,
}

impl MemoryMap {
    pub const EMPTY: MemoryMap = MemoryMap { memories: Vec::new() };

    pub fn get(&self, ty: MemoryModuleType<Erased>) -> Option<&ExpirableValue> {
        self.memories.iter().find(|(t, _)| *t == ty).map(|(_, value)| value)
    }

    pub fn iter(&self) -> impl Iterator<Item = &(MemoryModuleType<Erased>, ExpirableValue)> {
        self.memories.iter()
    }

    pub fn to_nbt(&self) -> Nbt {
        Nbt::Compound(
            self.memories
                .iter()
                .filter_map(|(ty, value)| Some((ty.name().to_string(), ty.get_codec()?.encode(value)?)))
                .collect(),
        )
    }

    pub fn from_nbt(nbt: &Nbt) -> Option<MemoryMap> {
        let Nbt::Compound(entries) = nbt else { return None };
        let mut memories = Vec::with_capacity(entries.len());
        for (key, value) in entries {
            let name = if key.contains(':') { key.clone() } else { format!("minecraft:{key}") };
            let ty = MemoryModuleType::by_name(&name)?;
            memories.push((ty, ty.get_codec()?.decode(value)?));
        }
        Some(MemoryMap { memories })
    }
}

pub struct Builder {
    memories: Vec<(MemoryModuleType<Erased>, ExpirableValue)>,
}

impl Builder {
    pub fn new() -> Builder {
        Builder { memories: Vec::new() }
    }

    pub fn add(&mut self, ty: MemoryModuleType<Erased>, value: ExpirableValue) -> &mut Builder {
        self.memories.push((ty, value));
        self
    }

    pub fn build(self) -> MemoryMap {
        MemoryMap { memories: self.memories }
    }
}
