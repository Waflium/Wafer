use wafer_core::Vec3;

use super::abstract_wind_charge::{self, AbstractWindCharge, DIMENSIONS};
use super::wind_charge::gust_particle;
use crate::server::level::ServerLevel;
use crate::sounds::sound_event::SoundEvent;
use crate::world::entity::data::entity_defaults;
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind};
use crate::world::entity::{EntityId, sound_event};
use crate::world::level::server_explosion::ExplosionInteraction;

const ENTITY_TYPE: &str = "minecraft:breeze_wind_charge";
const RADIUS: f32 = 3.0;

#[derive(Default)]
pub struct BreezeWindCharge {
    pub abstract_wind_charge: AbstractWindCharge,
}

impl BreezeWindCharge {
    pub fn of_type() -> Projectile {
        Projectile::new(ENTITY_TYPE, DIMENSIONS, entity_defaults(), ProjectileKind::BreezeWindCharge(BreezeWindCharge::default()))
    }

    pub fn from_breeze(level: &ServerLevel, breeze: EntityId) -> Projectile {
        let mut projectile = BreezeWindCharge::of_type();
        let e = level.entity(breeze);
        projectile.entity.set_pos(Vec3::new(e.x(), level.breeze_get_firing_y_position(breeze), e.z()));
        projectile.owner = Some(crate::world::entity::entity_reference::EntityReference::of(e.uuid));
        projectile
    }
}

impl ServerLevel {
    pub fn breeze_wind_charge_explode(&mut self, id: EntityId, position: Vec3) {
        let indirect_source = self.projectile_get_owner(id).filter(|&owner| self.is_living(owner));
        self.explode_with_particles(
            Some(id),
            indirect_source,
            None,
            abstract_wind_charge::explosion_damage_calculator(None),
            position,
            RADIUS,
            false,
            ExplosionInteraction::Trigger,
            &gust_particle("minecraft:gust_emitter_small"),
            &gust_particle("minecraft:gust_emitter_large"),
            &[],
            &SoundEvent::Reference(sound_event("minecraft:entity.breeze.wind_burst")),
        );
    }
}
