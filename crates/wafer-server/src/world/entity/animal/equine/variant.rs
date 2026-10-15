#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Variant {
    White,
    Creamy,
    Chestnut,
    Brown,
    Black,
    Gray,
    DarkBrown,
}

impl Variant {
    pub const VALUES: [Variant; 7] = [Variant::White, Variant::Creamy, Variant::Chestnut, Variant::Brown, Variant::Black, Variant::Gray, Variant::DarkBrown];

    pub fn get_id(self) -> i32 {
        self as i32
    }

    pub fn by_id(id: i32) -> Variant {
        Variant::VALUES[id.rem_euclid(Variant::VALUES.len() as i32) as usize]
    }

    pub fn get_serialized_name(self) -> &'static str {
        match self {
            Variant::White => "white",
            Variant::Creamy => "creamy",
            Variant::Chestnut => "chestnut",
            Variant::Brown => "brown",
            Variant::Black => "black",
            Variant::Gray => "gray",
            Variant::DarkBrown => "dark_brown",
        }
    }

    pub fn by_name(name: &str) -> Option<Variant> {
        Variant::VALUES.into_iter().find(|variant| variant.get_serialized_name() == name)
    }
}
