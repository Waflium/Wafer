use wafer_protocol::nbt::Nbt;

use crate::util::datafix::data_fix_types::{DataFixTypes, current_version};

pub fn upgrade_chunk_tag(mut chunk_tag: Nbt, default_version: i32, data_fix_context_tag: Option<&Nbt>, target_version: i32, data_fix_type: DataFixTypes) -> Nbt {
    let version = crate::nbt::get_data_version(&chunk_tag, default_version);
    if version >= target_version {
        return chunk_tag;
    }
    inject_datafixing_context(&mut chunk_tag, data_fix_context_tag);
    let mut chunk_tag = data_fix_type.update_tag(chunk_tag, version, target_version);
    remove_datafixing_context(&mut chunk_tag);
    crate::nbt::add_data_version(&mut chunk_tag, target_version);
    chunk_tag
}

pub fn upgrade_chunk_tag_to_current_version(chunk_tag: &Nbt, default_version: i32, data_fix_type: DataFixTypes) -> std::borrow::Cow<'_, Nbt> {
    if crate::nbt::get_data_version(chunk_tag, default_version) >= current_version() {
        return std::borrow::Cow::Borrowed(chunk_tag);
    }
    std::borrow::Cow::Owned(upgrade_chunk_tag(chunk_tag.clone(), default_version, None, current_version(), data_fix_type))
}

pub fn inject_datafixing_context(chunk_tag: &mut Nbt, context_tag: Option<&Nbt>) {
    if let (Nbt::Compound(entries), Some(context_tag)) = (chunk_tag, context_tag) {
        match entries.iter_mut().find(|(key, _)| key == "__context") {
            Some((_, value)) => *value = context_tag.clone(),
            None => entries.push(("__context".to_string(), context_tag.clone())),
        }
    }
}

fn remove_datafixing_context(chunk_tag: &mut Nbt) {
    if let Nbt::Compound(entries) = chunk_tag {
        entries.retain(|(key, _)| key != "__context");
    }
}
