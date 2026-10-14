use wafer_data::mob_effects;

use crate::world::entity::living_entity::LivingEntity;

pub fn has_dig_speed(mob: &LivingEntity) -> bool {
    mob.has_effect(mob_effects::HASTE) || mob.has_effect(mob_effects::CONDUIT_POWER)
}

pub fn get_dig_speed_amplification(mob: &LivingEntity) -> i32 {
    let a = mob.effect(mob_effects::HASTE).map_or(0, |effect| effect.amplifier);
    let b = mob.effect(mob_effects::CONDUIT_POWER).map_or(0, |effect| effect.amplifier);
    a.max(b)
}
