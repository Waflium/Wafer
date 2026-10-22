use wafer_core::{Direction, Vec3};
use wafer_data::Item;

use crate::core::dispenser::block_source::BlockSource;
use crate::server::level::ServerLevel;
use crate::server::level::events::SOUND_FIREWORK_SHOOT;
use crate::world::entity::projectile::projectile::Projectile;
use crate::world::item::ItemStack;
use crate::world::item::{arrow_item, experience_bottle_item, fire_charge_item, firework_rocket_item, throwable_potion_item, wind_charge_item};
use crate::world::entity::projectile::throwableitemprojectile::snowball::Snowball;
use crate::world::entity::projectile::throwableitemprojectile::thrown_egg::ThrownEgg;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PositionFunction {
    Default,
    EntityJustOutsideOfBlockPos,
    DispensePosition(f64),
}

impl PositionFunction {
    pub fn get_dispense_position(self, source: &BlockSource, direction: Direction) -> Vec3 {
        match self {
            PositionFunction::Default => {
                let facing = source.facing();
                source.center().add(0.7 * facing.step_x() as f64, 0.7 * facing.step_y() as f64 + 0.1, 0.7 * facing.step_z() as f64)
            }
            PositionFunction::EntityJustOutsideOfBlockPos => firework_rocket_item::get_entity_just_outside_of_block_pos(source, direction),
            PositionFunction::DispensePosition(scale) => {
                let facing = source.facing();
                source.center().add(scale * facing.step_x() as f64, scale * facing.step_y() as f64, scale * facing.step_z() as f64)
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DispenseConfig {
    pub position_function: PositionFunction,
    pub uncertainty: f32,
    pub power: f32,
    pub override_dispense_event: Option<i32>,
}

impl DispenseConfig {
    pub const DEFAULT: DispenseConfig = DispenseConfig { position_function: PositionFunction::Default, uncertainty: 6.0, power: 1.1, override_dispense_event: None };
}

pub fn as_projectile(level: &mut ServerLevel, position: Vec3, item_stack: &ItemStack, direction: Direction) -> Projectile {
    let item = item_stack.get_item();
    let classes = item.info().classes;
    if classes.contains(&"SnowballItem") {
        Snowball::at(position.x, position.y, position.z, item_stack)
    } else if classes.contains(&"EggItem") {
        ThrownEgg::at(position.x, position.y, position.z, item_stack)
    } else if classes.contains(&"ExperienceBottleItem") {
        experience_bottle_item::as_projectile(position, item_stack)
    } else if classes.contains(&"ThrowablePotionItem") {
        throwable_potion_item::as_projectile(item, position, item_stack)
    } else if classes.contains(&"FireworkRocketItem") {
        firework_rocket_item::as_projectile(position, item_stack, direction)
    } else if classes.contains(&"FireChargeItem") {
        fire_charge_item::as_projectile(level, position, item_stack, direction)
    } else if classes.contains(&"WindChargeItem") {
        wind_charge_item::as_projectile(level, position, direction)
    } else if classes.contains(&"TridentItem") {
        let mut trident = crate::world::entity::projectile::arrow::thrown_trident::ThrownTrident::at(level, position.x, position.y, position.z, &item_stack.copy_with_count(1));
        trident.abstract_arrow_mut().expect("an arrow").pickup = crate::world::entity::projectile::arrow::abstract_arrow::Pickup::Allowed;
        trident
    } else {
        arrow_item::as_projectile(level, item, position, item_stack)
    }
}

pub fn is_projectile_item(item: Item) -> bool {
    let classes = item.info().classes;
    ["ArrowItem", "SnowballItem", "EggItem", "ExperienceBottleItem", "ThrowablePotionItem", "FireworkRocketItem", "FireChargeItem", "WindChargeItem", "TridentItem"]
        .iter()
        .any(|class| classes.contains(class))
}

pub fn create_dispense_config(item: Item) -> DispenseConfig {
    let classes = item.info().classes;
    if classes.contains(&"ExperienceBottleItem") || classes.contains(&"ThrowablePotionItem") {
        DispenseConfig { uncertainty: DispenseConfig::DEFAULT.uncertainty * 0.5, power: DispenseConfig::DEFAULT.power * 1.25, ..DispenseConfig::DEFAULT }
    } else if classes.contains(&"FireworkRocketItem") {
        DispenseConfig { position_function: PositionFunction::EntityJustOutsideOfBlockPos, uncertainty: 1.0, power: 0.5, override_dispense_event: Some(SOUND_FIREWORK_SHOOT) }
    } else if classes.contains(&"FireChargeItem") {
        fire_charge_item::create_dispense_config()
    } else if classes.contains(&"WindChargeItem") {
        wind_charge_item::create_dispense_config()
    } else {
        DispenseConfig::DEFAULT
    }
}
