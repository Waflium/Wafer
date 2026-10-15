#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Markings {
    None,
    White,
    WhiteField,
    WhiteDots,
    BlackDots,
}

impl Markings {
    pub const VALUES: [Markings; 5] = [Markings::None, Markings::White, Markings::WhiteField, Markings::WhiteDots, Markings::BlackDots];

    pub fn get_id(self) -> i32 {
        self as i32
    }

    pub fn by_id(id: i32) -> Markings {
        Markings::VALUES[id.rem_euclid(Markings::VALUES.len() as i32) as usize]
    }
}
