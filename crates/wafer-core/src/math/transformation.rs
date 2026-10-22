use super::matrix_util;
use crate::joml::{Matrix4f, Quaternionf, Vector3f};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Transformation {
    pub translation: Vector3f,
    pub left_rotation: Quaternionf,
    pub scale: Vector3f,
    pub right_rotation: Quaternionf,
}

impl Transformation {
    pub const IDENTITY: Transformation =
        Transformation { translation: Vector3f::new(0.0, 0.0, 0.0), left_rotation: Quaternionf::IDENTITY, scale: Vector3f::new(1.0, 1.0, 1.0), right_rotation: Quaternionf::IDENTITY };

    pub fn new(translation: Vector3f, left_rotation: Quaternionf, scale: Vector3f, right_rotation: Quaternionf) -> Transformation {
        Transformation { translation, left_rotation, scale, right_rotation }
    }

    pub fn from_matrix(matrix: &Matrix4f) -> Transformation {
        let (translation, left_rotation, scale, right_rotation) = matrix_util::svd_decompose(matrix);
        Transformation { translation, left_rotation, scale, right_rotation }
    }
}
