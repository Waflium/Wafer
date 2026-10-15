use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::behavior::declarative::behavior_builder;
use crate::world::entity::ai::behavior::one_shot::OneShot;
use crate::world::entity::ai::memory::memory_module_type;

pub fn create() -> OneShot {
    behavior_builder::create(|i| {
        let play_dead_ticks = i.present(memory_module_type::PLAY_DEAD_TICKS);
        let hurt_by = i.registered(memory_module_type::HURT_BY_ENTITY);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let ticks = play_dead_ticks.get(level, body);
            if ticks <= 0 {
                play_dead_ticks.erase(level, body);
                hurt_by.erase(level, body);
                level.get_brain_mut(body).use_default_activity();
            } else {
                play_dead_ticks.set(level, body, ticks - 1);
            }
            true
        }
    })
}
