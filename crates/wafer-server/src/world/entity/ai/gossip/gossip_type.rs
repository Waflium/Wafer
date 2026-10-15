#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GossipType {
    MajorNegative,
    MinorNegative,
    MinorPositive,
    MajorPositive,
    Trading,
}

impl GossipType {
    pub const VALUES: [GossipType; 5] = [GossipType::MajorNegative, GossipType::MinorNegative, GossipType::MinorPositive, GossipType::MajorPositive, GossipType::Trading];

    pub fn id(self) -> &'static str {
        match self {
            GossipType::MajorNegative => "major_negative",
            GossipType::MinorNegative => "minor_negative",
            GossipType::MinorPositive => "minor_positive",
            GossipType::MajorPositive => "major_positive",
            GossipType::Trading => "trading",
        }
    }

    pub fn by_id(id: &str) -> Option<GossipType> {
        GossipType::VALUES.into_iter().find(|ty| ty.id() == id)
    }

    pub fn weight(self) -> i32 {
        match self {
            GossipType::MajorNegative => -5,
            GossipType::MinorNegative => -1,
            GossipType::MinorPositive => 1,
            GossipType::MajorPositive => 5,
            GossipType::Trading => 1,
        }
    }

    pub fn max(self) -> i32 {
        match self {
            GossipType::MajorNegative => 100,
            GossipType::MinorNegative => 200,
            GossipType::MinorPositive => 25,
            GossipType::MajorPositive => 20,
            GossipType::Trading => 25,
        }
    }

    pub fn decay_per_day(self) -> i32 {
        match self {
            GossipType::MajorNegative => 10,
            GossipType::MinorNegative => 20,
            GossipType::MinorPositive => 1,
            GossipType::MajorPositive => 0,
            GossipType::Trading => 2,
        }
    }

    pub fn decay_per_transfer(self) -> i32 {
        match self {
            GossipType::MajorNegative => 10,
            GossipType::MinorNegative => 20,
            GossipType::MinorPositive => 5,
            GossipType::MajorPositive => 20,
            GossipType::Trading => 20,
        }
    }
}
