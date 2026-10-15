use wafer_protocol::Writer;
use wafer_protocol::nbt::Nbt;

use super::merchant_offer::MerchantOffer;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;

#[derive(Clone, Debug, Default)]
pub struct MerchantOffers(pub Vec<MerchantOffer>);

impl MerchantOffers {
    pub fn decode(tag: &Nbt) -> DataResult<MerchantOffers> {
        codec::map(tag)?;
        let recipes = codec::decode_or(tag, "Recipes", Vec::new(), |t| codec::list_of(t, MerchantOffer::decode))?;
        Ok(MerchantOffers(recipes))
    }

    pub fn to_nbt(&self) -> Nbt {
        if self.0.is_empty() {
            return Nbt::Compound(Vec::new());
        }
        Nbt::Compound(vec![("Recipes".into(), Nbt::List(self.0.iter().map(MerchantOffer::to_nbt).collect()))])
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn get_recipe_for(&self, buy_a: &ItemStack, buy_b: &ItemStack, selection_hint: i32) -> Option<usize> {
        if selection_hint > 0 && (selection_hint as usize) < self.0.len() {
            let offer = &self.0[selection_hint as usize];
            return offer.satisfied_by(buy_a, buy_b).then_some(selection_hint as usize);
        }
        self.0.iter().position(|offer| offer.satisfied_by(buy_a, buy_b))
    }

    pub fn write(&self, w: &mut Writer) {
        w.varint(self.0.len() as i32);
        for offer in &self.0 {
            offer.write(w);
        }
    }
}
