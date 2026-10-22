use super::piglin_ai;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::behavior::declarative::behavior_builder;
use crate::world::entity::ai::behavior::one_shot::OneShot;
use crate::world::entity::ai::memory::memory_module_type;

pub fn create() -> OneShot {
    behavior_builder::create(|i| {
        let huntable = i.present(memory_module_type::NEAREST_VISIBLE_HUNTABLE_HOGLIN);
        let _angry_at = i.absent(memory_module_type::ANGRY_AT);
        let _hunted_recently = i.absent(memory_module_type::HUNTED_RECENTLY);
        let nearest_piglins = i.registered(memory_module_type::NEAREST_VISIBLE_ADULT_PIGLINS);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let nearest = nearest_piglins.try_get(level, body);
            if level.is_baby(body) || nearest.as_ref().is_some_and(|piglins| piglins.iter().any(|&piglin| has_hunted_recently(level, piglin))) {
                return false;
            }
            let target = huntable.get(level, body);
            piglin_ai::set_anger_target(level, body, target);
            piglin_ai::dont_kill_any_more_hoglins_for_a_while(level, body);
            piglin_ai::broadcast_anger_target(level, body, target);
            if let Some(piglins) = nearest {
                for piglin in piglins {
                    piglin_ai::dont_kill_any_more_hoglins_for_a_while(level, piglin);
                }
            }
            true
        }
    })
}

fn has_hunted_recently(level: &ServerLevel, other_piglin: EntityId) -> bool {
    level.get_brain(other_piglin).has_memory_value(memory_module_type::HUNTED_RECENTLY)
}
