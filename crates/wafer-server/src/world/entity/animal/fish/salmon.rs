use wafer_protocol::nbt::Nbt;

use super::abstract_fish::{AbstractFish, abstract_fish_defaults};
use super::abstract_schooling_fish::AbstractSchoolingFish;
use crate::server::level::ServerLevel;
use crate::util::random::WeightedList;
use crate::world::entity::bucketable;
use crate::world::entity::data::DataValue;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{EntityDimensions, EntityId};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::tag_value_input;

pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.7, 0.4).with_eye_height(0.26);
pub const TAG_TYPE: &str = "type";
pub const DATA_TYPE: usize = 17;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Variant {
    Small,
    Medium,
    Large,
}

impl Variant {
    pub const DEFAULT: Variant = Variant::Medium;
    pub const VALUES: [Variant; 3] = [Variant::Small, Variant::Medium, Variant::Large];

    pub fn get_serialized_name(self) -> &'static str {
        match self {
            Variant::Small => "small",
            Variant::Medium => "medium",
            Variant::Large => "large",
        }
    }

    pub fn id(self) -> i32 {
        self as i32
    }

    pub fn bounding_box_scale(self) -> f32 {
        match self {
            Variant::Small => 0.5,
            Variant::Medium => 1.0,
            Variant::Large => 1.5,
        }
    }

    pub fn by_id(id: i32) -> Variant {
        Variant::VALUES[id.clamp(0, Variant::VALUES.len() as i32 - 1) as usize]
    }

    pub fn by_name(name: &str) -> Option<Variant> {
        Variant::VALUES.into_iter().find(|variant| variant.get_serialized_name() == name)
    }
}

#[derive(Default)]
pub struct Salmon {
    pub abstract_schooling_fish: AbstractSchoolingFish,
}

pub fn salmon_defaults() -> Vec<DataValue> {
    let mut values = abstract_fish_defaults();
    values.push(DataValue::Int(Variant::DEFAULT.id()));
    values
}

impl Salmon {
    pub fn create() -> Mob {
        let mut mob = AbstractFish::create_with("minecraft:salmon", DIMENSIONS, salmon_defaults(), MobKind::Salmon(Salmon::default()));
        mob.refresh_dimensions();
        mob
    }

    pub fn get_max_school_size() -> i32 {
        5
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.salmon.ambient"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.salmon.death"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.salmon.hurt"
    }

    pub fn get_flop_sound() -> &'static str {
        "minecraft:entity.salmon.flop"
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        out.push((TAG_TYPE.into(), Nbt::String(mob.get_salmon_variant().get_serialized_name().into())));
    }

    pub fn read_additional_save_data(mob: &mut Mob, nbt: &Nbt) {
        let variant = tag_value_input::read_or_error(nbt, TAG_TYPE, |tag| crate::util::codec::enum_by_name(tag, Variant::by_name)).unwrap_or(Variant::DEFAULT);
        mob.set_salmon_variant(variant);
    }
}

impl Mob {
    pub fn get_salmon_variant(&self) -> Variant {
        match self.entity.data.get(DATA_TYPE) {
            DataValue::Int(id) => Variant::by_id(*id),
            _ => Variant::DEFAULT,
        }
    }

    pub fn set_salmon_variant(&mut self, variant: Variant) {
        let changed = *self.entity.data.get(DATA_TYPE) != DataValue::Int(variant.id());
        self.entity.data.set(DATA_TYPE, DataValue::Int(variant.id()));
        if changed {
            self.refresh_dimensions();
        }
    }

    pub fn get_salmon_scale(&self) -> f32 {
        self.get_salmon_variant().bounding_box_scale()
    }

    pub fn salmon_save_to_bucket_tag(&self, bucket: &mut ItemStack) {
        bucketable::save_default_data_to_bucket_tag(self, bucket);
        bucket.set(dc::SALMON_SIZE, Some(Nbt::String(self.get_salmon_variant().get_serialized_name().into())));
    }
}

impl ServerLevel {
    pub fn salmon_finalize_spawn(&mut self, id: EntityId) {
        let list = WeightedList::new(vec![(Variant::Small, 30), (Variant::Medium, 50), (Variant::Large, 15)]);
        let mob = self.mob_mut(id).expect("a salmon");
        if let Some(&variant) = list.get_random(&mut mob.entity.random) {
            mob.set_salmon_variant(variant);
        }
    }
}
