use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DecodeResult};
use crate::util::dfu_frames;
use crate::world::item::component::codec::Wrap;

#[derive(Clone, Copy, Debug)]
pub enum ListOperation {
    ReplaceAll,
    ReplaceSection { offset: i32, size: Option<i32> },
    Insert { offset: i32 },
    Append,
}

impl ListOperation {
    pub fn decode(tag: &Nbt, max_size: i32) -> DecodeResult<ListOperation> {
        codec::wrapped(Wrap::MapFlatXmap, || {
            let mode = match codec::decode_field(tag, "mode", |t| codec::string_enum(t, &[("replace_all", 0), ("replace_section", 1), ("insert", 2), ("append", 3)])) {
                Ok(mode) => mode,
                Err(error) => return DecodeResult::error(error),
            };
            let mut record = codec::Record::default();
            let operation = codec::with_frames(dfu_frames::key_dispatch, || match mode {
                0 => DecodeResult::success(ListOperation::ReplaceAll),
                1 => {
                    let offset = record.with_default(tag, "offset", 0, codec::non_negative_int);
                    let size = record.optional(tag, "size", codec::non_negative_int);
                    record.build(|| Some(ListOperation::ReplaceSection { offset, size }))
                }
                2 => {
                    let offset = record.with_default(tag, "offset", 0, codec::non_negative_int);
                    record.build(|| Some(ListOperation::Insert { offset }))
                }
                _ => DecodeResult::success(ListOperation::Append),
            });
            operation.validate(|operation| match operation {
                ListOperation::ReplaceSection { size: Some(size), .. } if *size > max_size => Err(format!("Size value too large: {size}, max size is {max_size}")),
                _ => Ok(()),
            })
        })
    }

    pub fn apply<T: Clone>(self, original: &[T], replacement: &[T], max_size: usize) -> Vec<T> {
        match self {
            ListOperation::ReplaceAll => replacement.to_vec(),
            ListOperation::Append => {
                if original.len() + replacement.len() > max_size {
                    error!("Contents overflow in section append");
                    return original.to_vec();
                }
                original.iter().chain(replacement).cloned().collect()
            }
            ListOperation::Insert { offset } => {
                let offset = offset as usize;
                if offset > original.len() {
                    error!("Cannot insert when offset is out of bounds");
                    return original.to_vec();
                }
                if original.len() + replacement.len() > max_size {
                    error!("Contents overflow in section insertion");
                    return original.to_vec();
                }
                original[..offset].iter().chain(replacement).chain(&original[offset..]).cloned().collect()
            }
            ListOperation::ReplaceSection { offset, size } => {
                let offset = offset as usize;
                if offset > original.len() {
                    error!("Cannot replace when offset is out of bounds");
                    return original.to_vec();
                }
                let mut new_list: Vec<T> = original[..offset].to_vec();
                new_list.extend_from_slice(replacement);
                let resume_index = offset + size.map_or(replacement.len(), |s| s as usize);
                if resume_index < original.len() {
                    new_list.extend_from_slice(&original[resume_index..]);
                }
                if new_list.len() > max_size {
                    error!("Contents overflow in section replacement");
                    return original.to_vec();
                }
                new_list
            }
        }
    }
}

pub struct StandAlone<T> {
    pub value: Vec<T>,
    pub operation: ListOperation,
}

impl<T> StandAlone<T> {
    pub fn decode<R: Into<DecodeResult<T>>>(tag: &Nbt, value: impl Fn(&Nbt) -> R, max_size: i32) -> DecodeResult<StandAlone<T>> {
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = codec::Record::default();
            let values = record.required(tag, "values", |t| codec::sized_list_partial(t, max_size as usize, |element| value(element).into()));
            let operation = record.nested(|| ListOperation::decode(tag, max_size));
            record.build(|| Some(StandAlone { value: values?, operation: operation? }))
        })
    }
}

impl<T: Clone> StandAlone<T> {
    pub fn apply(&self, input: &[T]) -> Vec<T> {
        self.operation.apply(input, &self.value, i32::MAX as usize)
    }
}
