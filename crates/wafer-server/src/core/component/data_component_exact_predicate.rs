use wafer_protocol::nbt::Nbt;

use super::data_component_getter::DataComponentGetter;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::DataComponentType;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DataComponentExactPredicate {
    expected_components: Vec<(DataComponentType, Nbt)>,
}

impl DataComponentExactPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<DataComponentExactPredicate> {
        let expected_components = codec::dispatched_map(
            tag,
            |id| {
                let id = crate::resources::identifier::read(id)?;
                DataComponentType::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:data_component_type]: {id}"))
            },
            |ty, value| ty.decode(value),
        )?;
        Ok(DataComponentExactPredicate { expected_components })
    }

    pub fn is_empty(&self) -> bool {
        self.expected_components.is_empty()
    }

    pub fn to_nbt(&self) -> Nbt {
        Nbt::Compound(self.expected_components.iter().filter(|(ty, _)| !ty.is_transient()).map(|(ty, value)| (ty.name().to_string(), value.clone())).collect())
    }

    pub fn as_patch(&self) -> crate::world::item::component::DataComponentPatch {
        let mut patch = crate::world::item::component::DataComponentPatch::default();
        for (ty, value) in &self.expected_components {
            patch.put(*ty, Some(value.clone()));
        }
        patch
    }

    pub fn write(&self, w: &mut wafer_protocol::Writer) -> Result<(), String> {
        w.varint(self.expected_components.len() as i32);
        for (ty, value) in &self.expected_components {
            w.varint(ty.0 as i32);
            ty.write(value, w)?;
        }
        Ok(())
    }

    pub fn test(&self, actual_components: &(impl DataComponentGetter + ?Sized)) -> bool {
        self.expected_components.iter().all(|(ty, expected)| actual_components.get(*ty).is_some_and(|actual| *actual == *expected))
    }
}
