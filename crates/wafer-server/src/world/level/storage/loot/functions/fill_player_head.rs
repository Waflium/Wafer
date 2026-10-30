use wafer_data::items;
use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::loot::loot_context::{EntityTarget, LootContext};
use crate::world::level::storage::loot::parameters::loot_context_params::LootContextParam;

pub struct FillPlayerHead {
    entity_target: EntityTarget,
}

impl FillPlayerHead {
    pub fn decode(tag: &Nbt) -> DecodeResult<FillPlayerHead> {
        let mut record = codec::Record::default();
        let entity_target = record.required(tag, "entity", EntityTarget::decode);
        record.build(|| Some(FillPlayerHead { entity_target: entity_target? }))
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        if !item_stack.is(items::PLAYER_HEAD) {
            return item_stack;
        }
        let Some(player) = self.entity_target.get(&context.params).and_then(|id| context.params.level.find_player(id)) else { return item_stack };
        let profile = &player.profile;
        let uuid = profile.uuid;
        let id = Nbt::IntArray(vec![(uuid >> 96) as i32, (uuid >> 64) as i32, (uuid >> 32) as i32, uuid as i32]);
        let properties = profile
            .properties
            .iter()
            .map(|property| {
                let mut fields = vec![("name".to_string(), Nbt::String(property.name.clone())), ("value".to_string(), Nbt::String(property.value.clone()))];
                if let Some(signature) = &property.signature {
                    fields.push(("signature".into(), Nbt::String(signature.clone())));
                }
                Nbt::Compound(fields.into())
            })
            .collect();
        let value = Nbt::Compound(vec![("name".into(), Nbt::String(profile.name.clone())), ("id".into(), id), ("properties".into(), Nbt::List(properties))].into());
        let value = dc::PROFILE.decode(&value).unwrap_or(value);
        item_stack.set(dc::PROFILE, Some(value));
        item_stack
    }
}

impl FillPlayerHead {
    pub fn get_referenced_context_params(&self) -> Vec<LootContextParam> {
        vec![self.entity_target.context_param()]
    }
}
