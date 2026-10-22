#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AngerLevel {
    Calm,
    Agitated,
    Angry,
}

const SORTED_LEVELS: [AngerLevel; 3] = [AngerLevel::Angry, AngerLevel::Agitated, AngerLevel::Calm];

impl AngerLevel {
    pub fn get_minimum_anger(self) -> i32 {
        match self {
            AngerLevel::Calm => 0,
            AngerLevel::Agitated => 40,
            AngerLevel::Angry => 80,
        }
    }

    pub fn get_ambient_sound(self) -> &'static str {
        match self {
            AngerLevel::Calm => "minecraft:entity.warden.ambient",
            AngerLevel::Agitated => "minecraft:entity.warden.agitated",
            AngerLevel::Angry => "minecraft:entity.warden.angry",
        }
    }

    pub fn get_listening_sound(self) -> &'static str {
        match self {
            AngerLevel::Calm => "minecraft:entity.warden.listening",
            AngerLevel::Agitated | AngerLevel::Angry => "minecraft:entity.warden.listening_angry",
        }
    }

    pub fn by_anger(anger: i32) -> AngerLevel {
        SORTED_LEVELS.into_iter().find(|level| anger >= level.get_minimum_anger()).unwrap_or(AngerLevel::Calm)
    }

    pub fn is_angry(self) -> bool {
        self == AngerLevel::Angry
    }
}
