use crate::world::item::ItemStack;

pub struct Crackiness {
    fraction_low: f32,
    fraction_medium: f32,
    fraction_high: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    None,
    Low,
    Medium,
    High,
}

pub const WOLF_ARMOR: Crackiness = Crackiness { fraction_low: 0.95, fraction_medium: 0.69, fraction_high: 0.32 };

impl Crackiness {
    pub fn by_fraction(&self, fraction: f32) -> Level {
        if fraction < self.fraction_high {
            Level::High
        } else if fraction < self.fraction_medium {
            Level::Medium
        } else if fraction < self.fraction_low {
            Level::Low
        } else {
            Level::None
        }
    }

    pub fn by_item(&self, item: &ItemStack) -> Level {
        if !item.is_damageable_item() { Level::None } else { self.by_damage(item.damage_value(), item.max_damage()) }
    }

    pub fn by_damage(&self, damage: i32, max_damage: i32) -> Level {
        self.by_fraction((max_damage - damage) as f32 / max_damage as f32)
    }
}
