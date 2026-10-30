use wafer_protocol::nbt::Nbt;

use super::list_operation::StandAlone;
use crate::core::holder::Holder;
use crate::util::codec::{self, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::item::component::codec::Wrap;
use crate::world::item::component::types;
use crate::world::item::component::{self as dc, codec as component_codec};
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::providers::number::floats::ContextFloatProvider;
use crate::world::level::storage::loot::providers::number::ints::{self, ContextIntProvider};
use crate::world::level::storage::loot::validatable;
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub struct SetCustomModelDataFunction {
    floats: Option<StandAlone<Holder<ContextFloatProvider>>>,
    flags: Option<StandAlone<bool>>,
    strings: Option<StandAlone<String>>,
    colors: Option<StandAlone<Holder<ContextIntProvider>>>,
}

fn color_provider(tag: &Nbt) -> DecodeResult<Holder<ContextIntProvider>> {
    codec::wrapped(Wrap::Xmap, || {
        codec::either_result(
            codec::either_first(|| ContextIntProvider::decode_holder_partial(tag)),
            || codec::either_second(|| types::rgb_color().decode_partial(tag).map(|color| ints::exactly(component_codec::as_i32(&color)))),
        )
    })
    .map(|either| match either {
        Ok(provider) | Err(provider) => provider,
    })
}

impl SetCustomModelDataFunction {
    pub fn decode(tag: &Nbt) -> DecodeResult<SetCustomModelDataFunction> {
        let mut record = codec::Record::default();
        let floats = record.optional(tag, "floats", |t| StandAlone::decode(t, ContextFloatProvider::decode_holder_partial, i32::MAX));
        let flags = record.optional(tag, "flags", |t| StandAlone::decode(t, codec::bool_value, i32::MAX));
        let strings = record.optional(tag, "strings", |t| StandAlone::decode(t, |s| Ok::<String, String>(codec::string(s)?.to_string()), i32::MAX));
        let colors = record.optional(tag, "colors", |t| StandAlone::decode(t, color_provider, i32::MAX));
        record.build(|| Some(SetCustomModelDataFunction { floats, flags, strings, colors }))
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        let component = item_stack.get(dc::CUSTOM_MODEL_DATA).cloned().unwrap_or(Nbt::Compound(Vec::new().into()));
        let list = |key: &str| component.get(key).map_or(Vec::new(), |v| v.as_list().to_vec());
        let floats = match &self.floats {
            Some(operation) => {
                let replacement: Vec<Nbt> = operation.value.iter().map(|provider| Nbt::Float(provider.value().get_float(context))).collect();
                operation.operation.apply(&list("floats"), &replacement, i32::MAX as usize)
            }
            None => list("floats"),
        };
        let flags = match &self.flags {
            Some(operation) => {
                let replacement: Vec<Nbt> = operation.value.iter().map(|&flag| Nbt::Byte(flag as i8)).collect();
                operation.operation.apply(&list("flags"), &replacement, i32::MAX as usize)
            }
            None => list("flags"),
        };
        let strings = match &self.strings {
            Some(operation) => {
                let replacement: Vec<Nbt> = operation.value.iter().map(|s| Nbt::String(s.clone())).collect();
                operation.operation.apply(&list("strings"), &replacement, i32::MAX as usize)
            }
            None => list("strings"),
        };
        let colors = match &self.colors {
            Some(operation) => {
                let replacement: Vec<Nbt> = operation.value.iter().map(|provider| Nbt::Int(provider.value().get_int(context))).collect();
                let current: Vec<Nbt> = list("colors").iter().map(|c| Nbt::Int(component_codec::as_i32(c))).collect();
                operation.operation.apply(&current, &replacement, i32::MAX as usize)
            }
            None => list("colors"),
        };
        let value = Nbt::Compound(vec![
            ("floats".into(), Nbt::List(floats)),
            ("flags".into(), Nbt::List(flags)),
            ("strings".into(), Nbt::List(strings)),
            ("colors".into(), Nbt::List(colors)),
        ].into());
        let value = dc::CUSTOM_MODEL_DATA.decode(&value).unwrap_or(value);
        item_stack.set(dc::CUSTOM_MODEL_DATA, Some(value));
        item_stack
    }
}

impl SetCustomModelDataFunction {
    pub fn validate(&self, context: &ValidationContext) {
        if let Some(floats) = &self.floats {
            validatable::validate_holder_list(context, "floats", &floats.value);
        }
        if let Some(colors) = &self.colors {
            validatable::validate_holder_list(context, "colors", &colors.value);
        }
    }
}
