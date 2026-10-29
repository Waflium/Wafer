use wafer_protocol::nbt::Nbt;

use super::conversion_params::ConversionParams;
use super::data::DataValue;
use super::mob::{Mob, MobKind};
use super::{Entity, EntityId};
use crate::server::level::ServerLevel;
use crate::world::level::block::entity::int_value;
use crate::world::level::storage::tag_value_input;

const NOT_CONVERTING: i32 = -1;
const DEFAULT_AFFLICTION_TIME: i32 = 0;

pub struct ConversionTracker {
    conversion_id: usize,
    affliction_time_tag: &'static str,
    total_affliction_time: i32,
    conversion_time_tag: &'static str,
    total_conversion_time: i32,
    affliction_time: i32,
    conversion_time: i32,
}

pub struct Conversion {
    pub converts_to: &'static str,
    pub conversion_sound: i32,
    pub is_afflicted: bool,
    pub post_conversion_step: Option<fn(&mut ServerLevel, EntityId)>,
}

impl ConversionTracker {
    pub const fn new(conversion_id: usize, affliction_time_tag: &'static str, total_affliction_time: i32, conversion_time_tag: &'static str, total_conversion_time: i32) -> ConversionTracker {
        ConversionTracker { conversion_id, affliction_time_tag, total_affliction_time, conversion_time_tag, total_conversion_time, affliction_time: DEFAULT_AFFLICTION_TIME, conversion_time: 0 }
    }

    pub fn is_converting(&self, entity: &Entity) -> bool {
        matches!(entity.data.get(self.conversion_id), DataValue::Boolean(true))
    }

    fn set_converting(&self, entity: &mut Entity, is_converting: bool) {
        entity.data.set(self.conversion_id, DataValue::Boolean(is_converting));
    }

    pub fn add_additional_save_data(&self, entity: &Entity, is_afflicted: bool, out: &mut Vec<(String, Nbt)>) {
        out.push((self.conversion_time_tag.into(), Nbt::Int(if self.is_converting(entity) { self.conversion_time } else { NOT_CONVERTING })));
        out.push((self.affliction_time_tag.into(), Nbt::Int(if is_afflicted { self.affliction_time } else { NOT_CONVERTING })));
    }

    pub fn read_additional_save_data(&mut self, entity: &mut Entity, input: &Nbt) {
        self.affliction_time = tag_value_input::get_numeric(input, self.affliction_time_tag).and_then(int_value).unwrap_or(DEFAULT_AFFLICTION_TIME);
        let conversion_time = tag_value_input::get_numeric(input, self.conversion_time_tag).and_then(int_value).unwrap_or(NOT_CONVERTING);
        if conversion_time != NOT_CONVERTING {
            self.start_conversion(entity, conversion_time);
        } else {
            self.set_converting(entity, false);
        }
    }

    pub fn start_conversion(&mut self, entity: &mut Entity, time: i32) {
        self.conversion_time = time;
        self.set_converting(entity, true);
    }

    pub fn set_affliction_time(&mut self, affliction_time: i32) {
        self.affliction_time = affliction_time;
    }

    pub fn set_conversion_time(&mut self, conversion_time: i32) {
        self.conversion_time = conversion_time;
    }
}

impl ServerLevel {
    pub fn conversion_tracker_tick(&mut self, mob: EntityId, tracker: fn(&mut MobKind) -> &mut ConversionTracker, conversion: Conversion) {
        if !self.is_alive(mob) || self.mob(mob).expect("a mob").is_no_ai() {
            return;
        }
        let Mob { entity, kind, .. } = self.mob_mut(mob).expect("a mob");
        let t = tracker(kind);
        if conversion.is_afflicted {
            if t.is_converting(entity) {
                t.conversion_time -= 1;
                if t.conversion_time < 0 {
                    self.do_conversion(mob, conversion);
                }
            } else {
                t.affliction_time += 1;
                if t.affliction_time >= t.total_affliction_time {
                    let total_conversion_time = t.total_conversion_time;
                    t.start_conversion(entity, total_conversion_time);
                }
            }
        } else {
            t.affliction_time = -1;
            t.set_converting(entity, false);
        }
    }

    fn do_conversion(&mut self, mob: EntityId, conversion: Conversion) {
        let params = ConversionParams::single(self, mob, true, true);
        self.convert_to(mob, conversion.converts_to, params, |level, converted| {
            if let Some(post_conversion_step) = conversion.post_conversion_step {
                post_conversion_step(level, converted);
            }
            if !level.entity(converted).is_silent() {
                let pos = level.entity(converted).block_position();
                level.level_event(None, conversion.conversion_sound, pos, 0);
            }
        });
    }
}
