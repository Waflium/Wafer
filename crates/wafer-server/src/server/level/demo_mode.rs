use wafer_data::packets::play::clientbound;

use super::ServerLevel;
use crate::network::chat::Component;
use crate::world::entity::EntityId;

pub const DEMO_DAYS: i32 = 5;
pub const TOTAL_PLAY_TICKS: i64 = 120500;
const GAME_EVENT_DEMO_EVENT: u8 = 5;

#[derive(Default)]
pub struct DemoMode {
    displayed_intro: bool,
    pub demo_has_ended: bool,
    demo_ended_reminder: i32,
    game_mode_ticks: i32,
}

impl ServerLevel {
    fn send_demo_event(&self, player: EntityId, param: f32) {
        self.player(player).connection.send(clientbound::GAME_EVENT, |w| {
            w.u8(GAME_EVENT_DEMO_EVENT);
            w.f32(param);
        });
    }

    pub fn demo_mode_tick(&mut self, player: EntityId) {
        let time = self.game_time;
        let day = time / 24000 + 1;
        let Some(demo) = self.player_mut(player).game_mode.demo.as_mut() else { return };
        demo.game_mode_ticks += 1;
        let show_intro = !demo.displayed_intro && demo.game_mode_ticks > 20;
        if show_intro {
            demo.displayed_intro = true;
        }
        demo.demo_has_ended = time > TOTAL_PLAY_TICKS;
        if demo.demo_has_ended {
            demo.demo_ended_reminder += 1;
        }
        if show_intro {
            self.send_demo_event(player, 0.0);
        }
        if time % 24000 == 500 {
            if day <= 6 {
                if day == 6 {
                    self.send_demo_event(player, 104.0);
                } else {
                    self.player(player).send_system_message(&Component::translatable(&format!("demo.day.{day}")), false);
                }
            }
        } else if day == 1 {
            match time {
                100 => self.send_demo_event(player, 101.0),
                175 => self.send_demo_event(player, 102.0),
                250 => self.send_demo_event(player, 103.0),
                _ => {}
            }
        } else if day == DEMO_DAYS as i64 && time % 24000 == 22000 {
            self.player(player).send_system_message(&Component::translatable("demo.day.warning"), false);
        }
    }

    pub fn demo_has_ended(&self, player: EntityId) -> bool {
        self.player(player).game_mode.demo.as_ref().is_some_and(|demo| demo.demo_has_ended)
    }

    pub fn output_demo_reminder(&mut self, player: EntityId) {
        let Some(demo) = self.player_mut(player).game_mode.demo.as_mut() else { return };
        if demo.demo_ended_reminder > 100 {
            demo.demo_ended_reminder = 0;
            self.player(player).send_system_message(&Component::translatable("demo.reminder"), false);
        }
    }
}
