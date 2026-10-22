use wafer_core::Vec3;

use super::data::{DataValue, Pose, ids};
use super::{Entity, EntityDimensions};

pub const HUMANOID_ARM_LEFT: i32 = 0;
pub const HUMANOID_ARM_RIGHT: i32 = 1;
pub const DEFAULT_MAIN_HAND: i32 = HUMANOID_ARM_RIGHT;
pub const DEFAULT_MODEL_CUSTOMIZATION: i8 = 0;
pub const DEFAULT_EYE_HEIGHT: f32 = 1.62;
pub const DEFAULT_VEHICLE_ATTACHMENT: Vec3 = Vec3::new(0.0, 0.6, 0.0);
pub const STANDING_DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(0.6, 1.8).with_eye_height(DEFAULT_EYE_HEIGHT).with_vehicle_attachment(&[DEFAULT_VEHICLE_ATTACHMENT]);

pub fn get_default_dimensions(pose: Pose) -> EntityDimensions {
    match pose {
        Pose::Sleeping => EntityDimensions::fixed(0.2, 0.2).with_eye_height(0.2),
        Pose::FallFlying | Pose::Swimming | Pose::SpinAttack => EntityDimensions::scalable(0.6, 0.6).with_eye_height(0.4),
        Pose::Crouching => EntityDimensions::scalable(0.6, 1.5).with_eye_height(1.27).with_vehicle_attachment(&[DEFAULT_VEHICLE_ATTACHMENT]),
        Pose::Dying => EntityDimensions::fixed(0.2, 0.2).with_eye_height(1.62),
        _ => STANDING_DIMENSIONS,
    }
}

pub fn get_main_arm(entity: &Entity) -> i32 {
    match entity.data.get(ids::PLAYER_MAIN_HAND) {
        DataValue::HumanoidArm(arm) => *arm,
        _ => DEFAULT_MAIN_HAND,
    }
}

pub fn set_main_arm(entity: &mut Entity, main_arm: i32) {
    entity.data.set(ids::PLAYER_MAIN_HAND, DataValue::HumanoidArm(main_arm));
}
