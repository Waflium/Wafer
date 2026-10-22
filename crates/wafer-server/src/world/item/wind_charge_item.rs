use wafer_core::random::RandomSource;
use wafer_core::{Direction, Vec3};

use super::ItemStack;
use super::projectile_item::{DispenseConfig, PositionFunction};
use crate::server::level::events::SOUND_WIND_CHARGE_SHOOT;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::projectile::hurtingprojectile::windcharge::wind_charge::WindCharge;
use crate::world::entity::projectile::projectile::Projectile;
use crate::world::entity::{EntityId, sound_event};
use crate::world::{InteractionHand, InteractionResult};

pub const PROJECTILE_SHOOT_POWER: f32 = 1.5;

pub fn use_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand) -> (InteractionResult, Option<ItemStack>) {
    let Some(mut item_stack) = level.player(player).item_in_hand(hand) else { return (InteractionResult::Pass, None) };
    let e = level.entity(player);
    let (x, eye_y, z) = (e.x(), e.eye_y(), e.z());
    let projectile = WindCharge::from_player(level, player, x, eye_y, z);
    level.spawn_projectile_from_rotation(projectile, &item_stack, player, 0.0, PROJECTILE_SHOOT_POWER, 1.0);
    let e = level.entity(player);
    let (x, y, z) = (e.x(), e.y(), e.z());
    let pitch = 0.4 / (level.random.next_float() * 0.4 + 0.8);
    level.play_sound_at(None, x, y, z, sound_event("minecraft:entity.wind_charge.throw"), SoundSource::Neutral, 0.5, pitch);
    level.award_stat(player, crate::stats::stats::ITEM_USED.get(item_stack.get_item().0), 1);
    item_stack.consume(1, level, Some(player));
    (InteractionResult::SUCCESS, Some(item_stack))
}

pub fn as_projectile(level: &mut ServerLevel, position: Vec3, direction: Direction) -> Projectile {
    let random = &mut level.random;
    let dir_x = random.triangle(direction.step_x() as f64, 0.11485000000000001);
    let dir_y = random.triangle(direction.step_y() as f64, 0.11485000000000001);
    let dir_z = random.triangle(direction.step_z() as f64, 0.11485000000000001);
    let dir = Vec3::new(dir_x, dir_y, dir_z);
    let mut wind_charge = WindCharge::at(position.x, position.y, position.z, dir);
    wind_charge.entity.set_delta_movement(dir);
    wind_charge
}

pub fn create_dispense_config() -> DispenseConfig {
    DispenseConfig { position_function: PositionFunction::DispensePosition(1.0), uncertainty: 6.6666665, power: 1.0, override_dispense_event: Some(SOUND_WIND_CHARGE_SHOOT) }
}
