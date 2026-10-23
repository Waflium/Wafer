use wafer_protocol::nbt::Nbt;

use super::min_max_bounds::{Doubles, doubles_codec};
use crate::util::codec::{self, DataResult};
use crate::world::item::component::types::{self, Pair, def, record};

pub fn codec() -> Pair {
    record(vec![
        def("x", doubles_codec(), types::empty_map()),
        def("y", doubles_codec(), types::empty_map()),
        def("z", doubles_codec(), types::empty_map()),
        def("horizontal", doubles_codec(), types::empty_map()),
        def("absolute", doubles_codec(), types::empty_map()),
    ])
}

#[derive(Debug)]
pub struct DistancePredicate {
    x: Doubles,
    y: Doubles,
    z: Doubles,
    horizontal: Doubles,
    absolute: Doubles,
}

impl DistancePredicate {
    pub fn decode(tag: &Nbt) -> DataResult<DistancePredicate> {
        codec::map(tag)?;
        let mut record = codec::Record::default();
        let mut bound = |key| record.field(codec::decode_or(tag, key, Doubles::ANY, Doubles::decode));
        let (x, y, z, horizontal, absolute) = (bound("x"), bound("y"), bound("z"), bound("horizontal"), bound("absolute"));
        record.finish()?;
        Ok(DistancePredicate { x: x.unwrap(), y: y.unwrap(), z: z.unwrap(), horizontal: horizontal.unwrap(), absolute: absolute.unwrap() })
    }

    pub fn matches(&self, x0: f64, y0: f64, z0: f64, x1: f64, y1: f64, z1: f64) -> bool {
        let xd = (x0 - x1) as f32;
        let yd = (y0 - y1) as f32;
        let zd = (z0 - z1) as f32;
        if !self.x.matches(xd.abs() as f64) || !self.y.matches(yd.abs() as f64) || !self.z.matches(zd.abs() as f64) {
            return false;
        }
        self.horizontal.matches_sqr((xd * xd + zd * zd) as f64) && self.absolute.matches_sqr((xd * xd + yd * yd + zd * zd) as f64)
    }
}
