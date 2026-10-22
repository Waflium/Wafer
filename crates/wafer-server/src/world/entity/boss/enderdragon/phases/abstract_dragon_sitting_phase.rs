use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;

impl ServerLevel {
    pub fn abstract_dragon_sitting_phase_on_hurt(&mut self, source: &DamageSource, damage: f32) -> f32 {
        let Some(direct) = source.direct_entity else { return damage };
        let is_arrow = self.as_projectile(direct).is_some_and(|projectile| projectile.abstract_arrow().is_some());
        let is_wind_charge = self.as_projectile(direct).is_some_and(|projectile| matches!(projectile.kind, crate::world::entity::projectile::projectile::ProjectileKind::WindCharge(_)));
        if !is_arrow && !is_wind_charge {
            return damage;
        }
        self.ignite_for_seconds(direct, 1.0);
        0.0
    }
}
