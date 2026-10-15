use super::declarative::behavior_builder;
use super::declarative::trigger::Trigger;
use super::gate_behavior::{OrderPolicy, RunningPolicy};
use super::one_shot::OneShot;
use super::shuffling_list::ShufflingList;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub fn trigger_one_shuffled(weighted_triggers: Vec<(Box<dyn Trigger>, i32)>) -> OneShot {
    trigger_gate(weighted_triggers, OrderPolicy::Shuffled, RunningPolicy::RunOne)
}

pub fn trigger_gate(weighted_behaviors: Vec<(Box<dyn Trigger>, i32)>, order_policy: OrderPolicy, running_policy: RunningPolicy) -> OneShot {
    let mut behaviors: ShufflingList<Box<dyn Trigger>> = ShufflingList::default();
    for (behavior, weight) in weighted_behaviors {
        behaviors.add(behavior, weight);
    }
    behavior_builder::create(|_| {
        move |level: &mut ServerLevel, body: EntityId, timestamp: i64| {
            if order_policy == OrderPolicy::Shuffled {
                behaviors.shuffle();
            }
            for behavior in behaviors.iter_mut() {
                if behavior.trigger(level, body, timestamp) && running_policy == RunningPolicy::RunOne {
                    break;
                }
            }
            true
        }
    })
}
