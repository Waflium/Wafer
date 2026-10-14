#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayerModelPart {
    Hat,
}

impl PlayerModelPart {
    fn bit(self) -> i32 {
        match self {
            PlayerModelPart::Hat => 6,
        }
    }

    pub fn get_mask(self) -> i32 {
        1 << self.bit()
    }
}
