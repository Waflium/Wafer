use wafer_protocol::nbt::Nbt;

use crate::advancements::predicates::min_max_bounds::Doubles;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct MovementPredicate {
    x: Doubles,
    y: Doubles,
    z: Doubles,
    speed: Doubles,
    horizontal_speed: Doubles,
    vertical_speed: Doubles,
    fall_distance: Doubles,
}

impl MovementPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<MovementPredicate> {
        codec::map(tag)?;
        let mut record = codec::Record::default();
        let mut bound = |key| record.field(codec::decode_or(tag, key, Doubles::ANY, Doubles::decode));
        let (x, y, z) = (bound("x"), bound("y"), bound("z"));
        let (speed, horizontal_speed, vertical_speed, fall_distance) = (bound("speed"), bound("horizontal_speed"), bound("vertical_speed"), bound("fall_distance"));
        record.finish()?;
        Ok(MovementPredicate {
            x: x.unwrap(),
            y: y.unwrap(),
            z: z.unwrap(),
            speed: speed.unwrap(),
            horizontal_speed: horizontal_speed.unwrap(),
            vertical_speed: vertical_speed.unwrap(),
            fall_distance: fall_distance.unwrap(),
        })
    }

    pub fn matches(&self, x: f64, y: f64, z: f64, fall_distance: f64) -> bool {
        if !(self.x.matches(x) && self.y.matches(y) && self.z.matches(z)) {
            return false;
        }
        if !self.speed.matches_sqr(x * x + y * y + z * z) {
            return false;
        }
        if !self.horizontal_speed.matches_sqr(x * x + z * z) {
            return false;
        }
        self.vertical_speed.matches(y.abs()) && self.fall_distance.matches(fall_distance)
    }

    pub fn matches_entity(&self, level: &ServerLevel, entity: EntityId) -> bool {
        let velocity = level.known_movement(entity).scale(20.0);
        self.matches(velocity.x, velocity.y, velocity.z, level.entity(entity).fall_distance)
    }
}
