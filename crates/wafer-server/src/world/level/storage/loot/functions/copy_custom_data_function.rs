use wafer_protocol::nbt::Nbt;

use crate::commands::arguments::nbt_path_argument::NbtPath;
use crate::util::codec::{self, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::providers::nbt::NbtProvider;
use crate::world::level::storage::loot::validatable;
use crate::world::level::storage::loot::validation_context::ValidationContext;

#[derive(Clone, Copy)]
enum MergeStrategy {
    Replace,
    Append,
    Merge,
}

impl MergeStrategy {
    fn merge(self, target: &mut Nbt, path: &NbtPath, sources: &[Nbt]) {
        match self {
            MergeStrategy::Replace => {
                let _ = path.set(target, sources.last().expect("non-empty sources"));
            }
            MergeStrategy::Append => {
                let _ = path.get_or_create_each(target, &|| Nbt::List(Vec::new()), |tag| {
                    if let Nbt::List(list) = tag {
                        list.extend(sources.iter().cloned());
                    }
                    Ok(0)
                });
            }
            MergeStrategy::Merge => {
                let _ = path.get_or_create_each(target, &|| Nbt::Compound(Vec::new().into()), |tag| {
                    if let Nbt::Compound(fields) = tag {
                        for source in sources {
                            if let Nbt::Compound(source_fields) = source {
                                crate::nbt::merge(fields, source_fields);
                            }
                        }
                    }
                    Ok(0)
                });
            }
        }
    }
}

struct CopyOperation {
    source_path: NbtPath,
    target_path: NbtPath,
    op: MergeStrategy,
}

impl CopyOperation {
    fn decode(tag: &Nbt) -> DecodeResult<CopyOperation> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = codec::Record::default();
        let source_path = record.required(tag, "source", NbtPath::decode);
        let target_path = record.required(tag, "target", NbtPath::decode);
        let op = record.required(tag, "op", |t| codec::string_enum(t, &[("replace", MergeStrategy::Replace), ("append", MergeStrategy::Append), ("merge", MergeStrategy::Merge)]));
        record.build(|| Some(CopyOperation { source_path: source_path?, target_path: target_path?, op: op? }))
    }
}

pub struct CopyCustomDataFunction {
    source: NbtProvider,
    operations: Vec<CopyOperation>,
}

impl CopyCustomDataFunction {
    pub fn decode(tag: &Nbt) -> DecodeResult<CopyCustomDataFunction> {
        let mut record = codec::Record::default();
        let source = record.required(tag, "source", NbtProvider::decode);
        let operations = record.required(tag, "ops", |t| codec::list_partial(t, CopyOperation::decode));
        record.build(|| Some(CopyCustomDataFunction { source: source?, operations: operations? }))
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        let Some(source_tag) = self.source.get(context) else { return item_stack };
        let mut result: Option<Nbt> = None;
        for op in &self.operations {
            if let Ok(source_tags) = op.source_path.get(&source_tag)
                && !source_tags.is_empty()
            {
                let target = result.get_or_insert_with(|| item_stack.get(dc::CUSTOM_DATA).cloned().unwrap_or(Nbt::Compound(Vec::new().into())));
                op.op.merge(target, &op.target_path, &source_tags);
            }
        }
        if let Some(result_tag) = result {
            let empty = matches!(&result_tag, Nbt::Compound(fields) if fields.is_empty());
            item_stack.set(dc::CUSTOM_DATA, if empty { None } else { Some(result_tag) });
        }
        item_stack
    }
}

impl CopyCustomDataFunction {
    pub fn validate(&self, context: &ValidationContext) {
        validatable::validate(context, "source", &self.source);
    }
}
