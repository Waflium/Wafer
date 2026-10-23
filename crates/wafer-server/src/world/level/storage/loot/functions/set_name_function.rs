use wafer_protocol::nbt::Nbt;

use crate::commands::arguments::selector::entity_world::LevelWorld;
use crate::commands::source::{Anchor, CommandResultCallback, CommandSource, CommandSourceStack, Names};
use crate::network::chat::{Component, component_serialization, component_utils};
use crate::util::codec::{self, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc, DataComponentType};
use crate::world::level::storage::loot::loot_context::{EntityTarget, LootContext};
use crate::world::level::storage::loot::parameters::loot_context_params::LootContextParam;

#[derive(Clone, Copy)]
enum Target {
    CustomName,
    ItemName,
}

impl Target {
    fn component(self) -> DataComponentType {
        match self {
            Target::CustomName => dc::CUSTOM_NAME,
            Target::ItemName => dc::ITEM_NAME,
        }
    }
}

pub fn decode_component(tag: &Nbt) -> DecodeResult<Nbt> {
    match dc::CUSTOM_NAME.decode(tag) {
        Ok(value) => DecodeResult::success(value),
        Err(error) => DecodeResult { value: component_serialization::decode_or_partial(tag).map(|component| component.to_nbt()), error: Some(error) },
    }
}

pub struct SetNameFunction {
    name: Option<Nbt>,
    resolution_context: Option<EntityTarget>,
    target: Target,
}

impl SetNameFunction {
    pub fn decode(tag: &Nbt) -> DecodeResult<SetNameFunction> {
        let mut record = codec::Record::default();
        let name = record.optional(tag, "name", decode_component);
        let resolution_context = record.optional(tag, "entity", EntityTarget::decode);
        let target = record.with_default(tag, "target", Target::CustomName, |t| codec::string_enum(t, &[("custom_name", Target::CustomName), ("item_name", Target::ItemName)]));
        record.build(|| Some(SetNameFunction { name, resolution_context, target }))
    }

    pub fn create_resolver<'a>(context: &'a LootContext, entity_target: Option<EntityTarget>) -> impl Fn(&Nbt) -> Nbt + 'a {
        let level = &*context.params.level;
        let source = entity_target.and_then(|target| target.get(&context.params)).and_then(|id| level.entity_ref(id)).map(|entity| CommandSourceStack {
            source: CommandSource::Null,
            position: entity.position(),
            rotation: (entity.x_rot, entity.y_rot),
            level: level.index,
            permission: 2,
            names: Names::ForEntity,
            entity: Some(entity.id),
            silent: false,
            anchor: Anchor::Feet,
            callback: CommandResultCallback::default(),
            signing_context: None,
        });
        move |line| {
            let Some(source) = &source else { return line.clone() };
            let component = Component::from_nbt(line);
            if !component_utils::needs_resolution(&component) {
                return line.clone();
            }
            match component_utils::resolve(&LevelWorld(level), source, &component) {
                Ok(resolved) => resolved.to_nbt(),
                Err(_) => {
                    warn!("Failed to resolve text component");
                    line.clone()
                }
            }
        }
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        if let Some(name) = &self.name {
            let resolved = SetNameFunction::create_resolver(context, self.resolution_context)(name);
            item_stack.set(self.target.component(), Some(resolved));
        }
        item_stack
    }
}

impl SetNameFunction {
    pub fn get_referenced_context_params(&self) -> Vec<LootContextParam> {
        self.resolution_context.map(|target| vec![target.context_param()]).unwrap_or_default()
    }
}
