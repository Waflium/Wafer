use wafer_core::mth;
use wafer_core::random::RandomSource;

use super::Entity;
use super::data::DataValue;

const MIN_BOOST_TIME: i32 = 140;
const MAX_BOOST_TIME: i32 = 700;

#[derive(Default)]
pub struct ItemBasedSteering {
    boosting: bool,
    boost_time: i32,
}

impl ItemBasedSteering {
    pub fn boost(&mut self, entity: &mut Entity, boost_time_accessor: usize) -> bool {
        if self.boosting {
            return false;
        }
        self.boosting = true;
        self.boost_time = 0;
        let boost_time_total = entity.random.next_int_bounded(MAX_BOOST_TIME + MIN_BOOST_TIME + 1) + MIN_BOOST_TIME;
        entity.data.set(boost_time_accessor, DataValue::Int(boost_time_total));
        true
    }

    pub fn tick_boost(&mut self, entity: &Entity, boost_time_accessor: usize) {
        if self.boosting {
            let boost_time = self.boost_time;
            self.boost_time += 1;
            if boost_time > boost_time_total(entity, boost_time_accessor) {
                self.boosting = false;
            }
        }
    }

    pub fn boost_factor(&self, entity: &Entity, boost_time_accessor: usize) -> f32 {
        if self.boosting {
            1.0 + 1.15 * mth::sin((self.boost_time as f32 / boost_time_total(entity, boost_time_accessor) as f32 * std::f32::consts::PI) as f64)
        } else {
            1.0
        }
    }
}

fn boost_time_total(entity: &Entity, boost_time_accessor: usize) -> i32 {
    match entity.data.get(boost_time_accessor) {
        DataValue::Int(boost_time) => *boost_time,
        _ => 0,
    }
}
