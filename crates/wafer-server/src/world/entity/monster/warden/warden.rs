use wafer_core::Vec3;
use wafer_data::mob_effects;

use crate::server::level::ServerLevel;
use crate::world::effect::{MobEffectInstance, mob_effect_util};
use crate::world::entity::EntityId;

pub fn apply_darkness_around(level: &mut ServerLevel, position: Vec3, source: Option<EntityId>, darkness_radius: i32) {
    let darkness = MobEffectInstance::with(mob_effects::DARKNESS, 260, 0, false, false, false);
    mob_effect_util::add_effect_to_players_around(level, source, position, darkness_radius as f64, &darkness, 200);
}
