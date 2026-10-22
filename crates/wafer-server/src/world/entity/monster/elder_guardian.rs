use wafer_core::Vec3;
use wafer_data::mob_effects;
use wafer_data::packets::play::clientbound;

use super::guardian::Guardian;
use crate::server::level::ServerLevel;
use crate::world::effect::{MobEffectInstance, mob_effect_util};
use crate::world::entity::ai::MobAi;
use crate::world::entity::attributes::ELDER_GUARDIAN_ATTRIBUTES;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{EntityDimensions, EntityId};

const EFFECT_INTERVAL: i32 = 1200;
const EFFECT_RADIUS: f64 = 50.0;
const EFFECT_DURATION: i32 = 6000;
const EFFECT_AMPLIFIER: i32 = 2;
const EFFECT_DISPLAY_LIMIT: i32 = 1200;
const GUARDIAN_ELDER_EFFECT: u8 = 10;
pub const DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(1.9975, 1.9975).with_eye_height(0.99875).with_passenger_attachments(&[Vec3::new(0.0, 2.350625f32 as f64, 0.0)]);

pub struct ElderGuardian;

impl ElderGuardian {
    pub fn create() -> Mob {
        let mut mob = Guardian::create_kind("minecraft:elder_guardian", DIMENSIONS, &ELDER_GUARDIAN_ATTRIBUTES, MobKind::ElderGuardian(Guardian::default()));
        mob.persistence_required = true;
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        Guardian::register_goals(ai, 400);
    }
}

impl ServerLevel {
    pub fn elder_guardian_custom_server_ai_step(&mut self, id: EntityId) {
        let e = self.entity(id);
        if (e.tick_count + e.id as i32) % EFFECT_INTERVAL == 0 {
            let mining_fatigue = MobEffectInstance::new(mob_effects::MINING_FATIGUE, EFFECT_DURATION, EFFECT_AMPLIFIER);
            let position = e.position();
            let affected_players = mob_effect_util::add_effect_to_players_around(self, Some(id), position, EFFECT_RADIUS, &mining_fatigue, EFFECT_DISPLAY_LIMIT);
            let value = if self.entity(id).is_silent() { 0.0 } else { 1.0 };
            for player in affected_players {
                self.player(player).connection.send(clientbound::GAME_EVENT, |w| {
                    w.u8(GUARDIAN_ELDER_EFFECT);
                    w.f32(value);
                });
            }
        }
        let mob = self.mob_mut(id).expect("an elder guardian");
        if !mob.has_home() {
            let pos = mob.entity.block_position();
            mob.set_home_to(pos, 16);
        }
    }
}
