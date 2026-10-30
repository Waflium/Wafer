use wafer_protocol::nbt::Nbt;

use super::min_max_bounds::ints_codec;
use crate::world::item::component::types::{self, Pair, opt, record, req};

pub fn codec(element: fn() -> Pair) -> Pair {
    record(vec![
        opt("contains", types::list(element())),
        opt("count", types::list(record(vec![req("test", element()), req("count", ints_codec())]))),
        opt("size", ints_codec()),
    ])
}

use super::collection_contents_predicate::CollectionContentsPredicate;
use super::collection_counts_predicate::CollectionCountsPredicate;
use super::min_max_bounds::Ints;
use crate::util::codec::{self, DataResult, DecodeResult};

#[derive(Debug)]
pub struct CollectionPredicate<P> {
    contains: Option<CollectionContentsPredicate<P>>,
    counts: Option<CollectionCountsPredicate<P>>,
    size: Option<Ints>,
}

impl<P> CollectionPredicate<P> {
    pub fn decode(tag: &Nbt, element: fn(&Nbt) -> DataResult<P>) -> DataResult<CollectionPredicate<P>> {
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = codec::Record::default();
            let contains = record.optional(tag, "contains", |t| CollectionContentsPredicate::decode(t, element));
            let counts = record.optional(tag, "count", |t| CollectionCountsPredicate::decode(t, element));
            let size = record.optional(tag, "size", Ints::decode_partial);
            record.build(|| Some(CollectionPredicate { contains, counts, size }))
        })
        .into_result()
    }

    pub fn test<T>(&self, values: &[T], test: impl Fn(&P, &T) -> bool) -> bool {
        if self.contains.as_ref().is_some_and(|contains| !contains.test(values, &test)) {
            return false;
        }
        if self.counts.as_ref().is_some_and(|counts| !counts.test(values, &test)) {
            return false;
        }
        self.size.is_none_or(|size| size.matches(values.len() as i32))
    }
}
