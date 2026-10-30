use wafer_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::util::codec::{self, DecodeResult};

pub struct Criterion {
    pub trigger: CriterionTrigger,
    pub trigger_instance: CriterionTriggerInstance,
}

impl Criterion {
    pub fn decode(tag: &Nbt) -> DecodeResult<Criterion> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let Some(type_name) = tag.get("trigger") else {
            return DecodeResult::error(format!("Missing \"trigger\" in: MapLike[{}]", crate::nbt::to_string(tag)));
        };
        let name = match codec::identifier(type_name) {
            Ok(name) => name,
            Err(error) => return DecodeResult::error(error),
        };
        let Some(trigger) = CriterionTrigger::by_name(&name) else {
            return DecodeResult::error(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:trigger_type]: {name}"));
        };
        let empty = Nbt::Compound(Vec::new().into());
        let value = tag.get("conditions").unwrap_or(&empty);
        trigger.decode_instance(value).map(|trigger_instance| Criterion { trigger, trigger_instance })
    }
}
