use super::Vector3f;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Matrix4f {
    pub m: [[f32; 4]; 4],
}

impl Default for Matrix4f {
    fn default() -> Matrix4f {
        Matrix4f { m: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]] }
    }
}

impl Matrix4f {
    pub fn set_row_column(&mut self, row: usize, column: usize, value: f32) {
        self.m[column][row] = value;
    }

    pub fn get_row_column(&self, row: usize, column: usize) -> f32 {
        self.m[column][row]
    }

    pub fn m33(&self) -> f32 {
        self.m[3][3]
    }

    pub fn get_translation(&self) -> Vector3f {
        Vector3f::new(self.m[3][0], self.m[3][1], self.m[3][2])
    }
}
