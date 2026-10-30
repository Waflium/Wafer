use wafer_core::mth;
use wafer_protocol::Writer;
use wafer_protocol::nbt::Nbt;

use super::item_cost::ItemCost;
use crate::util::codec::{self, DecodeResult, Record};
use crate::world::item::{ItemStack, write_slot};

#[derive(Clone, Debug)]
pub struct MerchantOffer {
    base_cost_a: ItemCost,
    cost_b: Option<ItemCost>,
    result: ItemStack,
    uses: i32,
    max_uses: i32,
    reward_exp: bool,
    special_price_diff: i32,
    demand: i32,
    price_multiplier: f32,
    xp: i32,
}

impl MerchantOffer {
    pub fn new(base_cost_a: ItemCost, cost_b: Option<ItemCost>, result: ItemStack, max_uses: i32, xp: i32, price_multiplier: f32) -> MerchantOffer {
        MerchantOffer::with_uses(base_cost_a, cost_b, result, 0, max_uses, xp, price_multiplier, 0)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_uses(base_cost_a: ItemCost, cost_b: Option<ItemCost>, result: ItemStack, uses: i32, max_uses: i32, xp: i32, price_multiplier: f32, demand: i32) -> MerchantOffer {
        MerchantOffer { base_cost_a, cost_b, result, uses, max_uses, reward_exp: true, special_price_diff: 0, demand, price_multiplier, xp }
    }

    pub fn decode(tag: &Nbt) -> DecodeResult<MerchantOffer> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let lenient_int = |key: &str, default: i32| tag.get(key).and_then(|t| codec::int(t).ok()).unwrap_or(default);
        let mut record = Record::default();
        let base_cost_a = record.required(tag, "buy", |t| DecodeResult::from(ItemCost::decode(t)));
        let result = record.required(tag, "sell", |t| DecodeResult::from(ItemStack::from_nbt(t)));
        record.build(|| {
            Some(MerchantOffer {
                base_cost_a: base_cost_a?,
                cost_b: tag.get("buyB").and_then(|t| ItemCost::decode(t).ok()),
                result: result?,
                uses: lenient_int("uses", 0),
                max_uses: lenient_int("maxUses", 4),
                reward_exp: tag.get("rewardExp").and_then(|t| codec::bool_value(t).ok()).unwrap_or(true),
                special_price_diff: lenient_int("specialPrice", 0),
                demand: lenient_int("demand", 0),
                price_multiplier: tag.get("priceMultiplier").and_then(|t| codec::float(t).ok()).unwrap_or(0.0),
                xp: lenient_int("xp", 1),
            })
        })
    }

    pub fn to_nbt(&self) -> Nbt {
        let mut fields = vec![("buy".to_string(), self.base_cost_a.to_nbt())];
        if let Some(cost_b) = &self.cost_b {
            fields.push(("buyB".into(), cost_b.to_nbt()));
        }
        fields.push(("sell".into(), self.result.to_nbt()));
        if self.uses != 0 {
            fields.push(("uses".into(), Nbt::Int(self.uses)));
        }
        if self.max_uses != 4 {
            fields.push(("maxUses".into(), Nbt::Int(self.max_uses)));
        }
        if !self.reward_exp {
            fields.push(("rewardExp".into(), Nbt::Byte(0)));
        }
        if self.special_price_diff != 0 {
            fields.push(("specialPrice".into(), Nbt::Int(self.special_price_diff)));
        }
        if self.demand != 0 {
            fields.push(("demand".into(), Nbt::Int(self.demand)));
        }
        if self.price_multiplier != 0.0 {
            fields.push(("priceMultiplier".into(), Nbt::Float(self.price_multiplier)));
        }
        if self.xp != 1 {
            fields.push(("xp".into(), Nbt::Int(self.xp)));
        }
        Nbt::Compound(fields.into())
    }

    pub fn get_base_cost_a(&self) -> &ItemStack {
        &self.base_cost_a.item_stack
    }

    pub fn get_cost_a(&self) -> ItemStack {
        self.base_cost_a.item_stack.copy_with_count(self.get_modified_cost_count(&self.base_cost_a))
    }

    fn get_modified_cost_count(&self, cost: &ItemCost) -> i32 {
        let base_price = cost.count;
        let demand_diff = 0.max(mth::floor_f((base_price * self.demand) as f32 * self.price_multiplier));
        (base_price + demand_diff + self.special_price_diff).clamp(1, cost.item_stack.max_stack_size())
    }

    pub fn get_cost_b(&self) -> ItemStack {
        self.cost_b.as_ref().map_or_else(ItemStack::empty, |cost| cost.item_stack.clone())
    }

    pub fn get_item_cost_a(&self) -> &ItemCost {
        &self.base_cost_a
    }

    pub fn get_item_cost_b(&self) -> Option<&ItemCost> {
        self.cost_b.as_ref()
    }

    pub fn get_result(&self) -> &ItemStack {
        &self.result
    }

    pub fn update_demand(&mut self) {
        self.demand = self.demand + self.uses - (self.max_uses - self.uses);
    }

    pub fn assemble(&self) -> ItemStack {
        self.result.copy()
    }

    pub fn get_uses(&self) -> i32 {
        self.uses
    }

    pub fn reset_uses(&mut self) {
        self.uses = 0;
    }

    pub fn get_max_uses(&self) -> i32 {
        self.max_uses
    }

    pub fn increase_uses(&mut self) {
        self.uses += 1;
    }

    pub fn get_demand(&self) -> i32 {
        self.demand
    }

    pub fn add_to_special_price_diff(&mut self, add: i32) {
        self.special_price_diff += add;
    }

    pub fn reset_special_price_diff(&mut self) {
        self.special_price_diff = 0;
    }

    pub fn get_special_price_diff(&self) -> i32 {
        self.special_price_diff
    }

    pub fn set_special_price_diff(&mut self, value: i32) {
        self.special_price_diff = value;
    }

    pub fn get_price_multiplier(&self) -> f32 {
        self.price_multiplier
    }

    pub fn get_xp(&self) -> i32 {
        self.xp
    }

    pub fn is_out_of_stock(&self) -> bool {
        self.uses >= self.max_uses
    }

    pub fn set_to_out_of_stock(&mut self) {
        self.uses = self.max_uses;
    }

    pub fn needs_restock(&self) -> bool {
        self.uses > 0
    }

    pub fn should_reward_exp(&self) -> bool {
        self.reward_exp
    }

    pub fn satisfied_by(&self, buy_a: &ItemStack, buy_b: &ItemStack) -> bool {
        if !self.base_cost_a.test(buy_a) || buy_a.count < self.get_modified_cost_count(&self.base_cost_a) {
            return false;
        }
        match &self.cost_b {
            None => buy_b.is_empty(),
            Some(cost_b) => cost_b.test(buy_b) && buy_b.count >= cost_b.count,
        }
    }

    pub fn take(&self, buy_a: &mut ItemStack, buy_b: &mut ItemStack) -> bool {
        if !self.satisfied_by(buy_a, buy_b) {
            return false;
        }
        buy_a.shrink(self.get_cost_a().count);
        let cost_b = self.get_cost_b();
        if !cost_b.is_empty() {
            buy_b.shrink(cost_b.count);
        }
        true
    }

    pub fn write(&self, w: &mut Writer) {
        self.base_cost_a.write(w);
        write_slot(w, Some(&self.result));
        ItemCost::write_optional(self.cost_b.as_ref(), w);
        w.bool(self.is_out_of_stock());
        w.i32(self.uses);
        w.i32(self.max_uses);
        w.i32(self.xp);
        w.i32(self.special_price_diff);
        w.f32(self.price_multiplier);
        w.i32(self.demand);
    }
}
