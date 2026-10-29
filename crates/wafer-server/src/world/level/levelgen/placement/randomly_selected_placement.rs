use super::placement_modifier;
use crate::world::item::component::codec::Field;
use crate::world::item::component::types::{self, req};

pub fn fields() -> Vec<Field> {
    vec![req("placements", types::non_empty_list(placement_modifier::lazy()))]
}
