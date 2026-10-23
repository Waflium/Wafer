use wafer_protocol::nbt::Nbt;

use super::squid::Squid;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::EntityId;
use crate::world::level::block::entity::int_value;
use crate::world::level::storage::tag_value_input;

pub const DATA_DARK_TICKS_REMAINING: usize = 18;
const DEFAULT_DARK_TICKS_REMAINING: i32 = 0;

#[derive(Default)]
pub struct GlowSquid {
    pub squid: Squid,
}

pub fn glow_squid_defaults() -> Vec<DataValue> {
    let mut values = ageable_mob_defaults();
    values.push(DataValue::Int(DEFAULT_DARK_TICKS_REMAINING));
    values
}

impl GlowSquid {
    pub fn create() -> Mob {
        Squid::create_with("minecraft:glow_squid", glow_squid_defaults(), MobKind::GlowSquid(GlowSquid::default()))
    }

    pub fn get_ink_particle() -> &'static str {
        "minecraft:glow_squid_ink"
    }

    pub fn get_squirt_sound() -> &'static str {
        "minecraft:entity.glow_squid.squirt"
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.glow_squid.ambient"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.glow_squid.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.glow_squid.death"
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        out.push(("DarkTicksRemaining".into(), Nbt::Int(mob.get_dark_ticks_remaining())));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        mob.set_dark_ticks(tag_value_input::get_numeric(input, "DarkTicksRemaining").and_then(int_value).unwrap_or(DEFAULT_DARK_TICKS_REMAINING));
    }
}

impl Mob {
    fn set_dark_ticks(&mut self, ticks: i32) {
        self.entity.data.set(DATA_DARK_TICKS_REMAINING, DataValue::Int(ticks));
    }

    pub fn get_dark_ticks_remaining(&self) -> i32 {
        match self.entity.data.get(DATA_DARK_TICKS_REMAINING) {
            DataValue::Int(ticks) => *ticks,
            _ => DEFAULT_DARK_TICKS_REMAINING,
        }
    }
}

impl ServerLevel {
    pub fn glow_squid_get_breed_offspring(&mut self, _id: EntityId, _partner: EntityId) -> Option<Box<Mob>> {
        Some(Box::new(GlowSquid::create()))
    }

    pub fn glow_squid_ai_step(&mut self, id: EntityId) {
        self.squid_ai_step(id);
        let mob = self.mob_mut(id).expect("a glow squid");
        let dark_ticks = mob.get_dark_ticks_remaining();
        if dark_ticks > 0 {
            mob.set_dark_ticks(dark_ticks - 1);
        }
        let e = &mut mob.entity;
        e.random_x(0.6);
        e.random_y();
        e.random_z(0.6);
    }

    pub fn glow_squid_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        let hurt = self.squid_hurt_server(id, source, damage);
        if hurt {
            self.mob_mut(id).expect("a glow squid").set_dark_ticks(100);
        }
        hurt
    }
}
