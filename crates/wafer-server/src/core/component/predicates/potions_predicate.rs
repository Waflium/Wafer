use wafer_protocol::nbt::Nbt;

use crate::advancements::predicates::collection_predicate;
use crate::advancements::predicates::mob_effects_predicate;
use crate::world::item::component::types::{self, Pair, opt, record};

pub fn codec() -> Pair {
    record(vec![opt("potions", types::holder_set("minecraft:potion")), opt("effects", collection_predicate::codec(mob_effects_predicate::codec))])
}

use crate::advancements::predicates::collection_predicate::CollectionPredicate;
use crate::advancements::predicates::mob_effects_predicate::MobEffectsPredicate;
use crate::core::holder_set::HolderSet;
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::item::alchemy::potion_contents::PotionContents;
use crate::world::item::component::codec::registry_id;

#[derive(Debug)]
pub struct PotionsPredicate {
    potions: Option<HolderSet>,
    effects: Option<CollectionPredicate<MobEffectsPredicate>>,
}

impl PotionsPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<PotionsPredicate> {
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = codec::Record::default();
            let potions = record.optional(tag, "potions", |t| HolderSet::decode_partial("minecraft:potion", t));
            let effects = record.optional(tag, "effects", |t| CollectionPredicate::decode(t, MobEffectsPredicate::decode));
            record.build(|| Some(PotionsPredicate { potions, effects }))
        })
        .into_result()
    }

    pub fn matches(&self, value: &Nbt) -> bool {
        let potion_contents = PotionContents::new(value);
        if let Some(potions) = &self.potions {
            let potion = potion_contents.potion_id().and_then(|id| registry_id("minecraft:potion", id).ok());
            if !potion.is_some_and(|id| potions.contains(id)) {
                return false;
            }
        }
        self.effects.as_ref().is_none_or(|effects| effects.test(&potion_contents.get_all_effects(), |p, effect| p.test(effect)))
    }
}
