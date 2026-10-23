use wafer_data::{damage_types, items};
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::entity::data::{DataValue, entity_defaults};
use crate::world::entity::projectile::projectile::Projectile;
use crate::world::item::ItemStack;
use crate::world::level::storage::tag_value_input;

pub const DATA_ITEM_STACK: usize = 8;

pub fn fireball_defaults() -> Vec<DataValue> {
    let mut data = entity_defaults();
    data.push(DataValue::ItemStack(Some(get_default_item())));
    data
}

fn get_default_item() -> ItemStack {
    ItemStack::new(items::FIRE_CHARGE, 1)
}

impl Projectile {
    pub fn fireball_set_item(&mut self, source: &ItemStack) {
        let item = if source.is_empty() { get_default_item() } else { source.copy_with_count(1) };
        self.entity.data.set(DATA_ITEM_STACK, DataValue::ItemStack(Some(item)));
    }

    pub fn fireball_get_item(&self) -> ItemStack {
        match self.entity.data.get(DATA_ITEM_STACK) {
            DataValue::ItemStack(Some(item)) => item.clone(),
            _ => ItemStack::empty(),
        }
    }

    pub fn fireball_add_additional_save_data(&self, output: &mut Vec<(String, Nbt)>) {
        self.abstract_hurting_projectile_add_additional_save_data(output);
        output.push(("Item".into(), self.fireball_get_item().to_nbt()));
    }

    pub fn fireball_read_additional_save_data(&mut self, input: &Nbt) {
        self.abstract_hurting_projectile_read_additional_save_data(input);
        let item = tag_value_input::read(input, "Item", ItemStack::from_nbt_partial).unwrap_or_else(get_default_item);
        self.fireball_set_item(&item);
    }
}

impl ServerLevel {
    pub fn fireball(&self, fireball: EntityId, owner: Option<EntityId>) -> DamageSource {
        match owner {
            Some(owner) => DamageSource::source(damage_types::FIREBALL, Some(fireball), Some(owner)),
            None => DamageSource::entity(damage_types::UNATTRIBUTED_FIREBALL, fireball),
        }
    }
}
