use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::item::component::codec as component_codec;

#[derive(Clone, Debug, PartialEq)]
pub enum HolderSet {
    Direct(Vec<i32>),
    Named(String, &'static wafer_data::TagContents),
}

impl HolderSet {
    pub fn named(registry: &'static str, key: String) -> HolderSet {
        let tag = wafer_data::tag_contents(registry, &key);
        HolderSet::Named(key, tag)
    }

    pub fn decode(registry: &'static str, tag: &Nbt) -> DataResult<HolderSet> {
        HolderSet::decode_partial(registry, tag).into_result()
    }

    pub fn decode_partial(registry: &'static str, tag: &Nbt) -> DecodeResult<HolderSet> {
        if let Nbt::String(s) = tag
            && let Some(tag_name) = s.strip_prefix('#')
        {
            let key = match crate::resources::identifier::read(tag_name) {
                Ok(key) => key,
                Err(error) => return DecodeResult::error(error),
            };
            if !component_codec::tag_exists(registry, &key)
                && let Some(error) = crate::core::data_registries::missing_tag(registry, &key)
            {
                return DecodeResult::error(error);
            }
            return DecodeResult::success(HolderSet::named(registry, key));
        }
        codec::holder_set_partial(tag, registry, |element| {
            let key = match codec::identifier(element) {
                Ok(key) => key,
                Err(error) => return DecodeResult::error(error),
            };
            if component_codec::registry(registry).contains(&key.as_str()) {
                crate::core::data_registries::known_reference(registry, &key);
                return DecodeResult::from(component_codec::registry_id(registry, &key));
            }
            if !codec::is_nbt_ops() && crate::core::data_registries::is_loading() && crate::core::data_registries::get(registry).is_some() {
                crate::core::data_registries::unknown_reference(registry, &key);
                return DecodeResult::success(-1);
            }
            DecodeResult::error(format!("Failed to get element {key} from registry {registry}"))
        })
        .map(HolderSet::Direct)
    }

    pub fn contents(&self) -> &[i32] {
        match self {
            HolderSet::Direct(ids) => ids,
            HolderSet::Named(_, tag) => tag.get(),
        }
    }

    pub fn contains(&self, id: i32) -> bool {
        self.contents().contains(&id)
    }
}
