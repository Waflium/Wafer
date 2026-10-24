use std::fmt;

use wafer_protocol::nbt::Nbt;

use crate::resources::identifier;
use crate::util::codec::{self, DecodeResult, Record};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TagEntry {
    pub id: String,
    pub tag: bool,
    pub required: bool,
}

pub trait Lookup<T> {
    fn element(&self, key: &str, required: bool) -> Option<T>;

    fn tag(&self, key: &str) -> Option<Vec<T>>;
}

impl TagEntry {
    pub fn decode_partial(tag: &Nbt) -> DecodeResult<TagEntry> {
        let first = DecodeResult::from(codec::string(tag).and_then(tag_or_element_id)).map(|(id, tag)| TagEntry { id, tag, required: true });
        codec::either_result(first, || TagEntry::decode_full(tag)).map(|either| match either {
            Ok(entry) | Err(entry) => entry,
        })
    }

    fn decode_full(tag: &Nbt) -> DecodeResult<TagEntry> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let id = record.required(tag, "id", |id| codec::string(id).and_then(tag_or_element_id));
        let required = record.with_default(tag, "required", true, codec::bool_value);
        record.build(|| id.map(|(id, tag)| TagEntry { id, tag, required }))
    }

    pub fn build<T>(&self, lookup: &dyn Lookup<T>, output: &mut dyn FnMut(T)) -> bool {
        if self.tag {
            let Some(result) = lookup.tag(&self.id) else { return !self.required };
            result.into_iter().for_each(output);
        } else {
            let Some(result) = lookup.element(&self.id, self.required) else { return !self.required };
            output(result);
        }
        true
    }

    pub fn visit_required_dependencies(&self, output: &mut dyn FnMut(&str)) {
        if self.tag && self.required {
            output(&self.id);
        }
    }

    pub fn visit_optional_dependencies(&self, output: &mut dyn FnMut(&str)) {
        if self.tag && !self.required {
            output(&self.id);
        }
    }
}

fn tag_or_element_id(name: &str) -> Result<(String, bool), String> {
    match name.strip_prefix('#') {
        Some(tag) => identifier::read(tag).map(|id| (id, true)),
        None => identifier::read(name).map(|id| (id, false)),
    }
}

impl fmt::Display for TagEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.tag {
            f.write_str("#")?;
        }
        f.write_str(&self.id)?;
        if !self.required {
            f.write_str("?")?;
        }
        Ok(())
    }
}
