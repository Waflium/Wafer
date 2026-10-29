use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::{registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::server::level::events::SOUND_BAT_LIFTOFF;
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::attributes::BAT_ATTRIBUTES;
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{Entity, EntityDimensions, EntityId};
use crate::world::level::BlockGetter;
use crate::world::level::storage::tag_value_input;

pub const DATA_ID_FLAGS: usize = 16;
const FLAG_RESTING: i8 = 1;
const BAT_RESTING_TARGETING: TargetingConditions<'static> = TargetingConditions::for_non_combat().range(4.0);
const DEFAULT_FLAGS: i8 = 0;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.5, 0.9).with_eye_height(0.45);

#[derive(Default)]
pub struct Bat {
    target_position: Option<BlockPos>,
}

fn bat_mut(mob: &mut Mob) -> &mut Bat {
    match &mut mob.kind {
        MobKind::Bat(bat) => bat,
        _ => unreachable!("not a bat"),
    }
}

fn signum(value: f64) -> f64 {
    if value == 0.0 || value.is_nan() { value } else { value.signum() }
}

pub fn bat_defaults() -> Vec<DataValue> {
    let mut values = mob_defaults();
    values.push(DataValue::Byte(DEFAULT_FLAGS));
    values
}

impl Bat {
    pub fn create() -> Mob {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:bat"), DIMENSIONS, bat_defaults());
        let mut mob = Mob::new(entity, LivingEntity::new(&BAT_ATTRIBUTES), MobKind::Bat(Bat::default()));
        mob.set_resting(true);
        mob
    }

    pub fn get_sound_volume() -> f32 {
        0.1
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.bat.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.bat.death"
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        let flags = tag_value_input::get_i64(input, "BatFlags").map_or(DEFAULT_FLAGS, |flags| flags as i8);
        mob.entity.data.set(DATA_ID_FLAGS, DataValue::Byte(flags));
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        out.push(("BatFlags".into(), Nbt::Byte(mob.bat_flags())));
    }
}

impl Mob {
    fn bat_flags(&self) -> i8 {
        match self.entity.data.get(DATA_ID_FLAGS) {
            DataValue::Byte(flags) => *flags,
            _ => DEFAULT_FLAGS,
        }
    }

    pub fn is_resting(&self) -> bool {
        self.bat_flags() & FLAG_RESTING != 0
    }

    pub fn set_resting(&mut self, value: bool) {
        let current = self.bat_flags();
        let flags = if value { current | FLAG_RESTING } else { current & !FLAG_RESTING };
        self.entity.data.set(DATA_ID_FLAGS, DataValue::Byte(flags));
    }
}

impl ServerLevel {
    pub fn bat_get_ambient_sound(&mut self, id: EntityId) -> Option<&'static str> {
        let mob = self.mob_mut(id).expect("a bat");
        if mob.is_resting() && mob.entity.random.next_int_bounded(4) != 0 { None } else { Some("minecraft:entity.bat.ambient") }
    }

    pub fn bat_voice_pitch(&mut self, id: EntityId) -> f32 {
        self.living_entity_voice_pitch(id) * 0.95
    }

    pub fn bat_is_pushable(&self, _id: EntityId) -> bool {
        false
    }

    pub fn bat_push_entities(&mut self, _id: EntityId) {}

    pub fn bat_check_fall_damage(&mut self, _id: EntityId) {}

    pub fn bat_is_ignoring_block_triggers(&self, _id: EntityId) -> bool {
        true
    }

    pub fn bat_tick(&mut self, id: EntityId) {
        self.mob_tick(id);
        self.then_tick(move |level| level.bat_tick_end(id));
    }

    fn bat_tick_end(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a bat");
        let is_resting = mob.is_resting();
        let e = &mut mob.entity;
        if is_resting {
            e.set_delta_movement(Vec3::ZERO);
            let y = mth::floor(e.y()) as f64 + 1.0 - e.dimensions.height as f64;
            e.set_pos_raw(Vec3::new(e.x(), y, e.z()));
        } else {
            e.set_delta_movement(e.delta_movement.multiply(1.0, 0.6, 1.0));
        }
    }

    pub fn bat_custom_server_ai_step(&mut self, id: EntityId) {
        let pos = self.entity(id).block_position();
        let above = pos.above();
        if self.mob(id).expect("a bat").is_resting() {
            let is_silent = self.entity(id).is_silent();
            let above_state = self.block_state(above);
            if self.is_redstone_conductor_at(above, above_state) {
                let e = self.entity_mut(id);
                if e.random.next_int_bounded(200) == 0 {
                    e.y_head_rot = e.random.next_int_bounded(360) as f32;
                }
                let (x, y, z) = (e.x(), e.y(), e.z());
                if self.get_nearest_player(&BAT_RESTING_TARGETING, Some(id), x, y, z).is_some() {
                    self.mob_mut(id).expect("a bat").set_resting(false);
                    if !is_silent {
                        self.level_event(None, SOUND_BAT_LIFTOFF, pos, 0);
                    }
                }
            } else {
                self.mob_mut(id).expect("a bat").set_resting(false);
                if !is_silent {
                    self.level_event(None, SOUND_BAT_LIFTOFF, pos, 0);
                }
            }
        } else {
            let target_position = bat_mut(self.mob_mut(id).expect("a bat")).target_position;
            let min_y = self.get_min_y();
            let target_position = target_position.filter(|&target| self.block_state(target).is_air() && target.y > min_y);
            let mob = self.mob_mut(id).expect("a bat");
            let e = &mut mob.entity;
            let target_position = match target_position {
                Some(target) if e.random.next_int_bounded(30) != 0 && !target.closer_to_center_than(e.position(), 2.0) => target,
                _ => {
                    let x = e.x() + e.random.next_int_bounded(7) as f64 - e.random.next_int_bounded(7) as f64;
                    let y = e.y() + e.random.next_int_bounded(6) as f64 - 2.0;
                    let z = e.z() + e.random.next_int_bounded(7) as f64 - e.random.next_int_bounded(7) as f64;
                    BlockPos::containing(x, y, z)
                }
            };
            bat_mut(mob).target_position = Some(target_position);
            let e = &mut mob.entity;
            let dx = target_position.x as f64 + 0.5 - e.x();
            let dy = target_position.y as f64 + 0.1 - e.y();
            let dz = target_position.z as f64 + 0.5 - e.z();
            let movement = e.delta_movement;
            let new_movement = movement.add(
                (signum(dx) * 0.5 - movement.x) * 0.1f32 as f64,
                (signum(dy) * 0.7f32 as f64 - movement.y) * 0.1f32 as f64,
                (signum(dz) * 0.5 - movement.z) * 0.1f32 as f64,
            );
            e.set_delta_movement(new_movement);
            let y_rot_d = (mth::atan2(new_movement.z, new_movement.x) * mth::RAD_TO_DEG as f64) as f32 - 90.0;
            let rot_diff = mth::wrap_degrees(y_rot_d - e.y_rot);
            mob.living.zza = 0.5;
            let y_rot = mob.entity.y_rot + rot_diff;
            mob.entity.set_y_rot(y_rot);
            if mob.entity.random.next_int_bounded(100) == 0 && {
                let above_state = self.block_state(above);
                self.is_redstone_conductor_at(above, above_state)
            } {
                self.mob_mut(id).expect("a bat").set_resting(true);
            }
        }
    }

    pub fn bat_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        if self.is_invulnerable_to(id, &source) {
            return false;
        }
        let mob = self.mob_mut(id).expect("a bat");
        if mob.is_resting() {
            mob.set_resting(false);
        }
        self.living_entity_hurt_server(id, source, damage)
    }
}
