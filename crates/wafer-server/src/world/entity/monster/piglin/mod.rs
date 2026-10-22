pub mod abstract_piglin;
#[allow(clippy::module_inception)]
pub mod piglin;
pub mod piglin_ai;
pub mod piglin_brute;
pub mod piglin_brute_ai;
pub mod remember_if_hoglin_was_killed;
pub mod start_admiring_item_if_seen;
pub mod start_hunting_hoglin;
pub mod stop_admiring_if_item_too_far_away;
pub mod stop_admiring_if_tired_of_trying_to_reach_item;
pub mod stop_holding_item_if_no_longer_admiring;
#[cfg(test)]
mod piglin_tests;
