use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::brain::Brain;
use crate::world::entity::ai::memory::memory_module_type;

const HIDE_TIMEOUT: i64 = 300;

pub fn create(seconds: i32, close_enough_dist: i32) -> OneShot {
    let stay_hidden_ticks = seconds * 20;
    let mut ticks_hidden = 0;
    behavior_builder::create(|i| {
        let hiding_place = i.present(memory_module_type::HIDING_PLACE);
        let heard_bell_time = i.present(memory_module_type::HEARD_BELL_TIME);
        move |level: &mut ServerLevel, body: EntityId, timestamp: i64| {
            let time_triggered = heard_bell_time.get(level, body);
            let timed_out_trying_to_hide = time_triggered + HIDE_TIMEOUT <= timestamp;
            if ticks_hidden <= stay_hidden_ticks && !timed_out_trying_to_hide {
                let hide_pos = hiding_place.get(level, body).pos;
                if hide_pos.closer_than(level.entity(body).block_position(), close_enough_dist as f64) {
                    ticks_hidden += 1;
                }
            } else {
                heard_bell_time.erase(level, body);
                hiding_place.erase(level, body);
                let (game_time, position) = (level.game_time, level.entity(body).position());
                Brain::update_activity_from_schedule(level, body, game_time, position);
                ticks_hidden = 0;
            }
            true
        }
    })
}
