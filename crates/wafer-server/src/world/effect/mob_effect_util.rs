use wafer_data::mob_effects;

use wafer_core::Vec3;

use super::MobEffectInstance;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::living_entity::LivingEntity;

pub fn has_dig_speed(mob: &LivingEntity) -> bool {
    mob.has_effect(mob_effects::HASTE) || mob.has_effect(mob_effects::CONDUIT_POWER)
}

pub fn get_dig_speed_amplification(mob: &LivingEntity) -> i32 {
    let a = mob.effect(mob_effects::HASTE).map_or(0, |effect| effect.amplifier);
    let b = mob.effect(mob_effects::CONDUIT_POWER).map_or(0, |effect| effect.amplifier);
    a.max(b)
}

pub fn add_effect_to_players_around(level: &mut ServerLevel, source: Option<EntityId>, position: Vec3, radius: f64, effect_instance: &MobEffectInstance, display_effect_limit: i32) -> Vec<EntityId> {
    let effect = effect_instance.effect;
    let players: Vec<EntityId> = level
        .players
        .keys()
        .copied()
        .filter(|&input| {
            let living = &level.player(input).living;
            level.player(input).game_mode.game_type.is_survival()
                && source.is_none_or(|source| !level.is_allied_to(source, input))
                && position.closer_than(level.entity(input).position(), radius)
                && living.effect(effect).is_none_or(|current| current.amplifier < effect_instance.amplifier || current.ends_within(display_effect_limit - 1))
        })
        .collect();
    for &player in &players {
        level.add_effect(player, effect_instance.clone(), source);
    }
    players
}
