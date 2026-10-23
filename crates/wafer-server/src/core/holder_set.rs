use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
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
        if let Nbt::String(s) = tag
            && let Some(tag_name) = s.strip_prefix('#')
        {
            let key = crate::resources::identifier::read(tag_name)?;
            if !component_codec::tag_exists(registry, &key)
                && let Some(error) = crate::core::data_registries::missing_tag(registry, &key)
            {
                return Err(error);
            }
            return Ok(HolderSet::named(registry, key));
        }
        let ids = codec::compact_list_of(tag, |element| {
            let key = codec::identifier(element)?;
            component_codec::registry_id(registry, &key)
        })?;
        Ok(HolderSet::Direct(ids))
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
