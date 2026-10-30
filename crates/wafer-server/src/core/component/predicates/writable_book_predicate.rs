use wafer_protocol::nbt::Nbt;

use crate::advancements::predicates::collection_predicate;
use crate::world::item::component::types::{self, Pair, opt, record};

pub fn codec() -> Pair {
    record(vec![opt("pages", collection_predicate::codec(types::string))])
}

use crate::advancements::predicates::collection_predicate::CollectionPredicate;
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::item::component::codec::Wrap;

#[derive(Debug)]
pub struct PagePredicate {
    contents: String,
}

impl PagePredicate {
    fn decode(tag: &Nbt) -> DataResult<PagePredicate> {
        codec::wrapped(Wrap::Xmap, || codec::string(tag).map(str::to_string)).map(|contents| PagePredicate { contents })
    }

    fn test(&self, value: &Nbt) -> bool {
        let raw = match value {
            Nbt::String(raw) => Some(raw.as_str()),
            other => other.get("raw").and_then(Nbt::as_str),
        };
        raw == Some(self.contents.as_str())
    }
}

#[derive(Debug)]
pub struct WritableBookPredicate {
    pages: Option<CollectionPredicate<PagePredicate>>,
}

impl WritableBookPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<WritableBookPredicate> {
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = codec::Record::default();
            let pages = record.optional(tag, "pages", |t| CollectionPredicate::decode(t, PagePredicate::decode));
            record.build(|| Some(WritableBookPredicate { pages }))
        })
        .into_result()
    }

    pub fn matches(&self, value: &Nbt) -> bool {
        let Some(pages) = &self.pages else { return true };
        pages.test(value.get("pages").map_or(&[][..], Nbt::as_list), |p, page| p.test(page))
    }
}
