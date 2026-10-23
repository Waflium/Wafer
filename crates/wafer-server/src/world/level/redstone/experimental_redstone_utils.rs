use wafer_core::Direction;

use super::orientation::{Orientation, SideBias};
use crate::server::level::ServerLevel;
use crate::world::flag::FeatureFlagSet;

pub fn initial_orientation(level: &mut ServerLevel, front: Option<Direction>, up: Option<Direction>) -> Option<Orientation> {
    if !level.enabled_features.contains(FeatureFlagSet::REDSTONE_EXPERIMENTS) {
        return None;
    }
    let mut orientation = Orientation::random(&mut level.random).with_side_bias(SideBias::Left);
    if let Some(up) = up {
        orientation = orientation.with_up(up);
    }
    if let Some(front) = front {
        orientation = orientation.with_front(front);
    }
    Some(orientation)
}

pub fn with_front(orientation: Option<Orientation>, front: Direction) -> Option<Orientation> {
    orientation.map(|orientation| orientation.with_front(front))
}
