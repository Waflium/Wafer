use wafer_protocol::nbt::Nbt;

use super::min_max_bounds::Ints;
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::item::component::codec::Wrap;

#[derive(Debug)]
pub struct Entry<P> {
    test: P,
    count: Ints,
}

impl<P> Entry<P> {
    fn test<T>(&self, values: &[T], test: &impl Fn(&P, &T) -> bool) -> bool {
        let count = values.iter().filter(|value| test(&self.test, value)).count();
        self.count.matches(count as i32)
    }
}

#[derive(Debug)]
pub struct CollectionCountsPredicate<P> {
    entries: Vec<Entry<P>>,
}

impl<P> CollectionCountsPredicate<P> {
    pub fn decode(tag: &Nbt, element: fn(&Nbt) -> DataResult<P>) -> DataResult<CollectionCountsPredicate<P>> {
        let entries = codec::wrapped(Wrap::Xmap, || {
            codec::list_of(tag, |t| {
                codec::as_codec(|| {
                    if let Err(error) = codec::map(t) {
                        return DecodeResult::error(error);
                    }
                    let mut record = codec::Record::default();
                    let test = record.required(t, "test", element);
                    let count = record.required(t, "count", Ints::decode_partial);
                    record.build(|| Some(Entry { test: test?, count: count? }))
                })
                .into_result()
            })
        })?;
        Ok(CollectionCountsPredicate { entries })
    }

    pub fn test<T>(&self, values: &[T], test: &impl Fn(&P, &T) -> bool) -> bool {
        self.entries.iter().all(|entry| entry.test(values, test))
    }
}
