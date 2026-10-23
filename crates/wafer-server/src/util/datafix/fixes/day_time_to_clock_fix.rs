use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::LEVEL;

pub struct DayTimeToClockFix {
    base: DataFixBase,
}

impl DayTimeToClockFix {
    pub fn new(output_schema: &Arc<Schema>) -> DayTimeToClockFix {
        DayTimeToClockFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for DayTimeToClockFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("DayTimeToClockFix", self.get_input_schema().get_type(LEVEL), |typed| {
            typed.update_remainder(|input| {
                let game_time = input.get("Time").as_long(0);
                let day_time = input.get("DayTime").as_long(game_time);
                let input = input.remove("DayTime");
                let overworld_clock = create_clock(&input, day_time);
                let clocks = input.empty_map().set("minecraft:overworld", overworld_clock);
                input.set("world_clocks", clocks)
            })
        })
    }
}

fn create_clock(input: &Dynamic, total_ticks: i64) -> Dynamic {
    input.empty_map().set("total_ticks", input.create_long(total_ticks)).set("paused", input.create_boolean(false))
}
