use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct CopperGolemWeatherStateFix;

impl CopperGolemWeatherStateFix {
    pub fn new(output_schema: &Arc<Schema>) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, false, "CopperGolemWeatherStateFix", ENTITY, "minecraft:copper_golem", fix)
    }
}

fn fix(entity: Typed) -> Typed {
    entity.update_remainder(|tag| tag.update("weather_state", fix_weather_state))
}

fn fix_weather_state(value: Dynamic) -> Dynamic {
    match value.as_int(0) {
        1 => value.create_string("exposed"),
        2 => value.create_string("weathered"),
        3 => value.create_string("oxidized"),
        _ => value.create_string("unaffected"),
    }
}
