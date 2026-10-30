use wafer_core::Vec3;

use super::abstract_wind_charge::{self, AbstractWindCharge, DIMENSIONS};
use crate::core::particles::particle_options::ParticleOptions;
use crate::core::particles::particle_types;
use crate::server::level::ServerLevel;
use crate::sounds::sound_event::SoundEvent;
use crate::world::entity::data::entity_defaults;
use crate::world::entity::entity_reference::EntityReference;
use crate::world::entity::projectile::hurtingprojectile::abstract_hurting_projectile;
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind};
use crate::world::entity::{EntityId, sound_event};
use crate::world::level::server_explosion::ExplosionInteraction;

const ENTITY_TYPE: &str = "minecraft:wind_charge";
const RADIUS: f32 = 1.2;
const KNOCKBACK_MULTIPLIER: f32 = 1.22;

pub struct WindCharge {
    pub abstract_wind_charge: AbstractWindCharge,
    no_deflect_ticks: i32,
}

impl Default for WindCharge {
    fn default() -> WindCharge {
        WindCharge { abstract_wind_charge: AbstractWindCharge::default(), no_deflect_ticks: 5 }
    }
}

pub fn gust_particle(particle_type: &'static str) -> ParticleOptions {
    particle_types::parse(particle_type, &wafer_protocol::nbt::Nbt::Compound(Vec::new().into())).expect("a gust particle")
}

impl WindCharge {
    pub fn of_type() -> Projectile {
        Projectile::new(ENTITY_TYPE, DIMENSIONS, entity_defaults(), ProjectileKind::WindCharge(WindCharge::default()))
    }

    pub fn from_player(level: &ServerLevel, player: EntityId, x: f64, y: f64, z: f64) -> Projectile {
        let mut projectile = WindCharge::of_type();
        projectile.entity.set_pos(Vec3::new(x, y, z));
        projectile.owner = Some(EntityReference::of(level.entity(player).uuid));
        projectile
    }

    pub fn at(x: f64, y: f64, z: f64, direction: Vec3) -> Projectile {
        abstract_hurting_projectile::at(ENTITY_TYPE, DIMENSIONS, entity_defaults(), ProjectileKind::WindCharge(WindCharge::default()), x, y, z, direction)
    }
}

fn wind_charge(projectile: &mut Projectile) -> &mut WindCharge {
    match &mut projectile.kind {
        ProjectileKind::WindCharge(wind_charge) => wind_charge,
        _ => unreachable!("a wind charge"),
    }
}

impl ServerLevel {
    pub fn wind_charge_tick(&mut self, id: EntityId) {
        self.abstract_wind_charge_tick(id);
        self.then_tick(move |level| level.wind_charge_tick_end(id));
    }

    fn wind_charge_tick_end(&mut self, id: EntityId) {
        let wind_charge = wind_charge(self.projectile_mut(id));
        if wind_charge.no_deflect_ticks > 0 {
            wind_charge.no_deflect_ticks -= 1;
        }
    }

    pub fn wind_charge_can_deflect(&self, id: EntityId) -> bool {
        match &self.projectile(id).kind {
            ProjectileKind::WindCharge(wind_charge) => wind_charge.no_deflect_ticks <= 0,
            _ => true,
        }
    }

    pub fn wind_charge_explode(&mut self, id: EntityId, position: Vec3) {
        let indirect_source = self.projectile_get_owner(id).filter(|&owner| self.is_living(owner));
        self.explode_with_particles(
            Some(id),
            indirect_source,
            None,
            abstract_wind_charge::explosion_damage_calculator(Some(KNOCKBACK_MULTIPLIER)),
            position,
            RADIUS,
            false,
            ExplosionInteraction::Trigger,
            &gust_particle("minecraft:gust_emitter_small"),
            &gust_particle("minecraft:gust_emitter_large"),
            &[],
            &SoundEvent::Reference(sound_event("minecraft:entity.wind_charge.wind_burst")),
        );
    }
}
