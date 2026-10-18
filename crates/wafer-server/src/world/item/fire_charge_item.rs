use wafer_core::random::RandomSource;
use wafer_core::{Direction, Vec3};

use super::ItemStack;
use super::projectile_item::{DispenseConfig, PositionFunction};
use crate::server::level::ServerLevel;
use crate::server::level::events::SOUND_BLAZE_FIREBALL;
use crate::world::entity::projectile::hurtingprojectile::small_fireball::SmallFireball;
use crate::world::entity::projectile::projectile::Projectile;

pub fn as_projectile(level: &mut ServerLevel, position: Vec3, item_stack: &ItemStack, direction: Direction) -> Projectile {
    let random = &mut level.random;
    let dir_x = random.triangle(direction.step_x() as f64, 0.11485000000000001);
    let dir_y = random.triangle(direction.step_y() as f64, 0.11485000000000001);
    let dir_z = random.triangle(direction.step_z() as f64, 0.11485000000000001);
    let dir = Vec3::new(dir_x, dir_y, dir_z);
    let mut fireball = SmallFireball::at(position.x, position.y, position.z, dir.normalize());
    fireball.fireball_set_item(item_stack);
    fireball
}

pub fn create_dispense_config() -> DispenseConfig {
    DispenseConfig { position_function: PositionFunction::DispensePosition(1.0), uncertainty: 6.6666665, power: 1.0, override_dispense_event: Some(SOUND_BLAZE_FIREBALL) }
}
