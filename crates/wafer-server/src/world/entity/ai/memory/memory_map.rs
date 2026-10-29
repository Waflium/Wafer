use wafer_protocol::nbt::Nbt;

use super::expirable_value::ExpirableValue;
use super::memory_module_type::{Erased, MemoryModuleType};
use crate::util::codec::{self, DecodeResult};

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

    pub fn decode(nbt: &Nbt) -> DecodeResult<MemoryMap> {
        codec::dispatched_map_partial(
            nbt,
            |key| {
                let name = codec::identifier(&Nbt::String(key.to_string()))?;
                let ty = MemoryModuleType::by_name(&name).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:memory_module_type]: {name}"))?;
                if ty.can_serialize() { Ok(ty) } else { Err(format!("Memory module {name} cannot be encoded")) }
            },
            |ty, value| ty.get_codec().expect("a serializable memory").decode(value),
        )
        .map(|memories| MemoryMap { memories })
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
