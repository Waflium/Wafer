use wafer_protocol::nbt::Nbt;

use super::abstract_skeleton::AbstractSkeleton;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::attributes::SKELETON_ATTRIBUTES;
use crate::world::entity::conversion_tracker::{Conversion, ConversionTracker};
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::mob::{Mob, MobKind};

pub const DATA_STRAY_CONVERSION_ID: usize = 16;

pub struct Skeleton {
    pub abstract_skeleton: AbstractSkeleton,
    pub freezing_tracker: ConversionTracker,
}

impl Default for Skeleton {
    fn default() -> Skeleton {
        Skeleton {
            abstract_skeleton: AbstractSkeleton::default(),
            freezing_tracker: ConversionTracker::new(DATA_STRAY_CONVERSION_ID, "FreezingTime", 140, "StrayConversionTime", 300),
        }
    }
}

fn freezing_tracker(kind: &mut MobKind) -> &mut ConversionTracker {
    match kind {
        MobKind::Skeleton(skeleton) => &mut skeleton.freezing_tracker,
        _ => unreachable!("not a skeleton"),
    }
}

fn skeleton(mob: &Mob) -> &Skeleton {
    match &mob.kind {
        MobKind::Skeleton(skeleton) => skeleton,
        _ => unreachable!("not a skeleton"),
    }
}

impl Skeleton {
    pub fn create() -> Mob {
        let mut data = mob_defaults();
        data.push(DataValue::Boolean(false));
        AbstractSkeleton::create_with("minecraft:skeleton", &SKELETON_ATTRIBUTES, data, MobKind::Skeleton(Skeleton::default()))
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        let is_afflicted = mob.entity.is_in_powder_snow;
        skeleton(mob).freezing_tracker.add_additional_save_data(&mob.entity, is_afflicted, out);
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        let Mob { entity, kind, .. } = mob;
        freezing_tracker(kind).read_additional_save_data(entity, input);
    }

    pub fn start_freeze_conversion(mob: &mut Mob, time: i32) {
        let Mob { entity, kind, .. } = mob;
        freezing_tracker(kind).start_conversion(entity, time);
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.skeleton.ambient"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.skeleton.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.skeleton.death"
    }

    pub fn get_step_sound() -> &'static str {
        "minecraft:entity.skeleton.step"
    }
}

impl ServerLevel {
    pub fn skeleton_tick(&mut self, id: EntityId) {
        let conversion = Conversion { converts_to: "minecraft:stray", conversion_sound: 1048, is_afflicted: self.entity(id).is_in_powder_snow, post_conversion_step: None };
        self.conversion_tracker_tick(id, freezing_tracker, conversion);
        self.mob_tick(id);
    }

    pub fn skeleton_can_freeze(&self) -> bool {
        false
    }
}
