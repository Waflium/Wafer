use wafer_protocol::nbt::Nbt;

use crate::core::component::data_component_exact_predicate::DataComponentExactPredicate;
use crate::core::component::data_component_getter::DataComponentGetter;
use crate::core::component::predicates::data_component_predicate::DataComponentPredicate;
use crate::util::codec::{self, DataResult, DecodeResult};

#[derive(Debug, Default)]
pub struct DataComponentMatchers {
    pub exact: DataComponentExactPredicate,
    pub partial: Vec<DataComponentPredicate>,
}

impl DataComponentMatchers {
    pub fn decode(tag: &Nbt) -> DataResult<DataComponentMatchers> {
        DataComponentMatchers::decode_partial(tag).into_result()
    }

    pub fn decode_partial(tag: &Nbt) -> DecodeResult<DataComponentMatchers> {
        let mut record = codec::Record::default();
        let exact = record.with_default(tag, "components", DataComponentExactPredicate::default(), DataComponentExactPredicate::decode_partial);
        let partial = record.with_default(tag, "predicates", Vec::new(), DataComponentPredicate::decode_map_partial);
        record.build(|| Some(DataComponentMatchers { exact, partial }))
    }

    pub fn test(&self, values: &(impl DataComponentGetter + ?Sized)) -> bool {
        self.exact.test(values) && self.partial.iter().all(|predicate| predicate.matches(values))
    }

    pub fn is_empty(&self) -> bool {
        self.exact.is_empty() && self.partial.is_empty()
    }
}
