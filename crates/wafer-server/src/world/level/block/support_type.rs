use std::sync::LazyLock;

use wafer_core::shapes::{self, BooleanOp, VoxelShape};
use wafer_core::{AABB, Direction};
use wafer_data::SupportType;

static CENTER_SUPPORT_SHAPE: LazyLock<VoxelShape> = LazyLock::new(|| VoxelShape::from_boxes(&[AABB::new(7.0 / 16.0, 0.0, 7.0 / 16.0, 9.0 / 16.0, 10.0 / 16.0, 9.0 / 16.0)]));
static RIGID_SUPPORT_SHAPE: LazyLock<VoxelShape> = LazyLock::new(|| {
    shapes::join_unoptimized(&VoxelShape::block(), &VoxelShape::from_boxes(&[AABB::new(2.0 / 16.0, 0.0, 2.0 / 16.0, 14.0 / 16.0, 1.0, 14.0 / 16.0)]), BooleanOp::OnlyFirst)
});

pub fn is_supporting(support_shape: &VoxelShape, direction: Direction, support: SupportType) -> bool {
    match support {
        SupportType::Full => shapes::is_face_full(support_shape, direction),
        SupportType::Center => !shapes::join_is_not_empty(&support_shape.face_shape(direction), &CENTER_SUPPORT_SHAPE, BooleanOp::OnlySecond),
        SupportType::Rigid => !shapes::join_is_not_empty(&support_shape.face_shape(direction), &RIGID_SUPPORT_SHAPE, BooleanOp::OnlySecond),
    }
}
