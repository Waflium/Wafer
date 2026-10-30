use wafer_protocol::nbt::Nbt;

use crate::commands::arguments::nbt_path_argument::compare_nbt;
use crate::core::component::data_component_getter::DataComponentGetter;
use crate::util::codec::DataResult;
use crate::world::item::component as dc;

#[derive(Clone, Debug)]
pub struct NbtPredicate {
    pub tag: Nbt,
}

impl NbtPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<NbtPredicate> {
        let flattened_error = match tag {
            Nbt::String(snbt) => match crate::nbt::parse_fully(snbt) {
                Ok(tag @ Nbt::Compound(_)) => return Ok(NbtPredicate { tag }),
                Ok(other) => format!("Expected compound tag, got {}", crate::nbt::to_string(&other)),
                Err(e) => e.get_message(),
            },
            Nbt::Compound(_) => return Ok(NbtPredicate { tag: tag.clone() }),
            _ => "Not a string".to_string(),
        };
        Err(format!("Failed to parse either. First: {flattened_error}; Second: Not a compound tag: {}", crate::nbt::to_string(tag)))
    }

    pub fn matches_components(&self, components: &(impl DataComponentGetter + ?Sized)) -> bool {
        match components.get(dc::CUSTOM_DATA) {
            Some(data) => compare_nbt(&self.tag, Some(&data)),
            None => compare_nbt(&self.tag, Some(&Nbt::Compound(Vec::new().into()))),
        }
    }

    pub fn matches(&self, tag: Option<&Nbt>) -> bool {
        tag.is_some_and(|tag| compare_nbt(&self.tag, Some(tag)))
    }
}
