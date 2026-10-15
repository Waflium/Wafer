use serde_json::Value;

use super::spawn_context::SpawnContext;
use crate::advancements::predicates::min_max_bounds::Doubles;
use crate::world::attribute::environment_attributes;
use crate::world::level::moon_phase::MOON_BRIGHTNESS_PER_PHASE;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

pub struct MoonBrightnessCheck {
    range: Doubles,
}

impl MoonBrightnessCheck {
    pub fn from_json(condition: &Value) -> MoonBrightnessCheck {
        let range = crate::nbt::from_json(&condition["range"]).and_then(|range| Doubles::decode(&range).ok()).unwrap_or(Doubles::ANY);
        MoonBrightnessCheck { range }
    }

    pub fn test<L: ServerLevelAccessor + ?Sized>(&self, context: &mut SpawnContext<L>) -> bool {
        let moon_phase = context.level.get_environment_attribute(environment_attributes::MOON_PHASE, context.pos);
        let moon_brightness = MOON_BRIGHTNESS_PER_PHASE[moon_phase.index() as usize];
        self.range.matches(moon_brightness as f64)
    }
}
