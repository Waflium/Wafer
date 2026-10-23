use super::spawn_condition::SpawnCondition;
use crate::world::item::component::codec::{Codec, Stream};
use crate::world::item::component::types::{Pair, opt, record, req};

pub struct Selector {
    pub condition: Option<SpawnCondition>,
    pub priority: i32,
}

pub fn select<T: Copy>(entries: &[(T, Selector)], mut test: impl FnMut(&SpawnCondition) -> bool) -> Vec<T> {
    let mut unpacked_entries: Vec<&(T, Selector)> = entries.iter().collect();
    unpacked_entries.sort_by_key(|(_, selector)| std::cmp::Reverse(selector.priority));
    let mut highest_matched_priority = i32::MIN;
    unpacked_entries.retain(|(_, selector)| {
        if selector.priority < highest_matched_priority {
            false
        } else if selector.condition.as_ref().is_none_or(&mut test) {
            highest_matched_priority = selector.priority;
            true
        } else {
            false
        }
    });
    unpacked_entries.into_iter().map(|(entry, _)| *entry).collect()
}

pub fn selector_codec(condition: Pair) -> Pair {
    record(vec![opt("condition", condition), req("priority", (Codec::int(), Stream::Int))])
}
