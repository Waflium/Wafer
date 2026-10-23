use wafer_core::BlockPos;
use wafer_core::random::RandomSource;
use wafer_data::{blocks, fluid_tags};
use wafer_protocol::nbt::Nbt;

use super::abstract_fish::{AbstractFish, abstract_fish_defaults};
use super::abstract_schooling_fish::{AbstractSchoolingFish, SchoolSpawnGroupData};
use super::water_animal;
use crate::server::level::ServerLevel;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::data::DataValue;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{EntityDimensions, EntityId};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::dye_color::DyeColor;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::storage::tag_value_input;

pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.5, 0.4).with_eye_height(0.26);
pub const DATA_ID_TYPE_VARIANT: usize = 17;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Base {
    Small,
    Large,
}

impl Base {
    fn id(self) -> i32 {
        self as i32
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pattern {
    Kob,
    Sunstreak,
    Snooper,
    Dasher,
    Brinely,
    Spotty,
    Flopper,
    Stripey,
    Glitter,
    Blockfish,
    Betty,
    Clayfish,
}

impl Pattern {
    pub const VALUES: [Pattern; 12] = [
        Pattern::Kob,
        Pattern::Sunstreak,
        Pattern::Snooper,
        Pattern::Dasher,
        Pattern::Brinely,
        Pattern::Spotty,
        Pattern::Flopper,
        Pattern::Stripey,
        Pattern::Glitter,
        Pattern::Blockfish,
        Pattern::Betty,
        Pattern::Clayfish,
    ];

    fn definition(self) -> (&'static str, Base, i32) {
        match self {
            Pattern::Kob => ("kob", Base::Small, 0),
            Pattern::Sunstreak => ("sunstreak", Base::Small, 1),
            Pattern::Snooper => ("snooper", Base::Small, 2),
            Pattern::Dasher => ("dasher", Base::Small, 3),
            Pattern::Brinely => ("brinely", Base::Small, 4),
            Pattern::Spotty => ("spotty", Base::Small, 5),
            Pattern::Flopper => ("flopper", Base::Large, 0),
            Pattern::Stripey => ("stripey", Base::Large, 1),
            Pattern::Glitter => ("glitter", Base::Large, 2),
            Pattern::Blockfish => ("blockfish", Base::Large, 3),
            Pattern::Betty => ("betty", Base::Large, 4),
            Pattern::Clayfish => ("clayfish", Base::Large, 5),
        }
    }

    pub fn by_id(packed_id: i32) -> Pattern {
        Pattern::VALUES.into_iter().find(|pattern| pattern.get_packed_id() == packed_id).unwrap_or(Pattern::Kob)
    }

    pub fn by_name(name: &str) -> Option<Pattern> {
        Pattern::VALUES.into_iter().find(|pattern| pattern.get_serialized_name() == name)
    }

    pub fn base(self) -> Base {
        self.definition().1
    }

    pub fn get_packed_id(self) -> i32 {
        let (_, base, index) = self.definition();
        base.id() | index << 8
    }

    pub fn get_serialized_name(self) -> &'static str {
        self.definition().0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Variant {
    pub pattern: Pattern,
    pub base_color: DyeColor,
    pub pattern_color: DyeColor,
}

impl Variant {
    pub const fn new(pattern: Pattern, base_color: DyeColor, pattern_color: DyeColor) -> Variant {
        Variant { pattern, base_color, pattern_color }
    }

    pub fn from_packed_id(packed_id: i32) -> Variant {
        Variant::new(get_pattern(packed_id), get_base_color(packed_id), get_pattern_color(packed_id))
    }

    pub fn get_packed_id(self) -> i32 {
        pack_variant(self.pattern, self.base_color, self.pattern_color)
    }
}

const fn color(id: usize) -> DyeColor {
    DyeColor::VALUES[id]
}

const WHITE: DyeColor = color(0);
const ORANGE: DyeColor = color(1);
const LIGHT_BLUE: DyeColor = color(3);
const YELLOW: DyeColor = color(4);
const LIME: DyeColor = color(5);
const PINK: DyeColor = color(6);
const GRAY: DyeColor = color(7);
const CYAN: DyeColor = color(9);
const PURPLE: DyeColor = color(10);
const BLUE: DyeColor = color(11);
const RED: DyeColor = color(14);

pub const DEFAULT_VARIANT: Variant = Variant::new(Pattern::Kob, WHITE, WHITE);

pub const COMMON_VARIANTS: [Variant; 22] = [
    Variant::new(Pattern::Stripey, ORANGE, GRAY),
    Variant::new(Pattern::Flopper, GRAY, GRAY),
    Variant::new(Pattern::Flopper, GRAY, BLUE),
    Variant::new(Pattern::Clayfish, WHITE, GRAY),
    Variant::new(Pattern::Sunstreak, BLUE, GRAY),
    Variant::new(Pattern::Kob, ORANGE, WHITE),
    Variant::new(Pattern::Spotty, PINK, LIGHT_BLUE),
    Variant::new(Pattern::Blockfish, PURPLE, YELLOW),
    Variant::new(Pattern::Clayfish, WHITE, RED),
    Variant::new(Pattern::Spotty, WHITE, YELLOW),
    Variant::new(Pattern::Glitter, WHITE, GRAY),
    Variant::new(Pattern::Clayfish, WHITE, ORANGE),
    Variant::new(Pattern::Dasher, CYAN, PINK),
    Variant::new(Pattern::Brinely, LIME, LIGHT_BLUE),
    Variant::new(Pattern::Betty, RED, WHITE),
    Variant::new(Pattern::Snooper, GRAY, RED),
    Variant::new(Pattern::Blockfish, RED, WHITE),
    Variant::new(Pattern::Flopper, WHITE, YELLOW),
    Variant::new(Pattern::Kob, RED, WHITE),
    Variant::new(Pattern::Sunstreak, GRAY, WHITE),
    Variant::new(Pattern::Dasher, CYAN, YELLOW),
    Variant::new(Pattern::Flopper, YELLOW, YELLOW),
];

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TropicalFishGroupData {
    pub school: SchoolSpawnGroupData,
    pub variant: Variant,
}

pub struct TropicalFish {
    pub abstract_schooling_fish: AbstractSchoolingFish,
    pub is_school: bool,
}

impl Default for TropicalFish {
    fn default() -> TropicalFish {
        TropicalFish { abstract_schooling_fish: AbstractSchoolingFish::default(), is_school: true }
    }
}

fn pack_variant(pattern: Pattern, base_color: DyeColor, pattern_color: DyeColor) -> i32 {
    pattern.get_packed_id() & 65535 | (base_color.id & 0xFF) << 16 | (pattern_color.id & 0xFF) << 24
}

pub fn get_base_color(packed_variant: i32) -> DyeColor {
    DyeColor::by_id(packed_variant >> 16 & 0xFF)
}

pub fn get_pattern_color(packed_variant: i32) -> DyeColor {
    DyeColor::by_id(packed_variant >> 24 & 0xFF)
}

pub fn get_pattern(packed_variant: i32) -> Pattern {
    Pattern::by_id(packed_variant & 65535)
}

pub fn tropical_fish_defaults() -> Vec<DataValue> {
    let mut values = abstract_fish_defaults();
    values.push(DataValue::Int(DEFAULT_VARIANT.get_packed_id()));
    values
}

impl TropicalFish {
    pub fn create() -> Mob {
        AbstractFish::create_with("minecraft:tropical_fish", DIMENSIONS, tropical_fish_defaults(), MobKind::TropicalFish(TropicalFish::default()))
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.tropical_fish.ambient"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.tropical_fish.death"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.tropical_fish.hurt"
    }

    pub fn get_flop_sound() -> &'static str {
        "minecraft:entity.tropical_fish.flop"
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        out.push(("Variant".into(), Nbt::Int(Variant::from_packed_id(mob.get_packed_variant()).get_packed_id())));
    }

    pub fn read_additional_save_data(mob: &mut Mob, nbt: &Nbt) {
        let variant = tag_value_input::get_i64(nbt, "Variant").map_or(DEFAULT_VARIANT, |id| Variant::from_packed_id(id as i32));
        mob.set_packed_variant(variant.get_packed_id());
    }

    pub fn is_max_group_size_reached(tropical_fish: &TropicalFish) -> bool {
        !tropical_fish.is_school
    }
}

impl Mob {
    pub fn get_packed_variant(&self) -> i32 {
        match self.entity.data.get(DATA_ID_TYPE_VARIANT) {
            DataValue::Int(packed) => *packed,
            _ => DEFAULT_VARIANT.get_packed_id(),
        }
    }

    pub fn set_packed_variant(&mut self, packed: i32) {
        self.entity.data.set(DATA_ID_TYPE_VARIANT, DataValue::Int(packed));
    }

    pub fn get_tropical_fish_base_color(&self) -> DyeColor {
        get_base_color(self.get_packed_variant())
    }

    pub fn get_tropical_fish_pattern_color(&self) -> DyeColor {
        get_pattern_color(self.get_packed_variant())
    }

    pub fn get_tropical_fish_pattern(&self) -> Pattern {
        get_pattern(self.get_packed_variant())
    }

    pub fn set_tropical_fish_pattern(&mut self, pattern: Pattern) {
        let base = self.get_packed_variant();
        self.set_packed_variant(pack_variant(pattern, get_base_color(base), get_pattern_color(base)));
    }

    pub fn set_tropical_fish_base_color(&mut self, base_color: DyeColor) {
        let base = self.get_packed_variant();
        self.set_packed_variant(pack_variant(get_pattern(base), base_color, get_pattern_color(base)));
    }

    pub fn set_tropical_fish_pattern_color(&mut self, pattern_color: DyeColor) {
        let base = self.get_packed_variant();
        self.set_packed_variant(pack_variant(get_pattern(base), get_base_color(base), pattern_color));
    }

    pub fn tropical_fish_save_to_bucket_tag(&self, bucket: &mut ItemStack) {
        self.abstract_fish_save_to_bucket_tag(bucket);
        bucket.set(dc::TROPICAL_FISH_PATTERN, Some(Nbt::String(self.get_tropical_fish_pattern().get_serialized_name().into())));
        bucket.set(dc::TROPICAL_FISH_BASE_COLOR, Some(Nbt::String(self.get_tropical_fish_base_color().name.into())));
        bucket.set(dc::TROPICAL_FISH_PATTERN_COLOR, Some(Nbt::String(self.get_tropical_fish_pattern_color().name.into())));
    }
}

impl ServerLevel {
    pub fn tropical_fish_finalize_spawn(
        &mut self,
        id: EntityId,
        difficulty: &DifficultyInstance,
        spawn_reason: EntitySpawnReason,
        group_data: Option<SpawnGroupData>,
    ) -> Option<SpawnGroupData> {
        let mut group_data = self.abstract_schooling_fish_finalize_spawn(id, difficulty, spawn_reason, group_data);
        let variant = if let Some(SpawnGroupData::TropicalFish(tropical_fish_group_data)) = group_data {
            tropical_fish_group_data.variant
        } else if (self.random.next_float() as f64) < 0.9 {
            let variant = COMMON_VARIANTS[self.random.next_int_bounded(COMMON_VARIANTS.len() as i32) as usize];
            group_data = Some(SpawnGroupData::TropicalFish(TropicalFishGroupData { school: SchoolSpawnGroupData { leader: id }, variant }));
            variant
        } else {
            if let Some(MobKind::TropicalFish(tropical_fish)) = self.mob_mut(id).map(|mob| &mut mob.kind) {
                tropical_fish.is_school = false;
            }
            let pattern = Pattern::VALUES[self.random.next_int_bounded(Pattern::VALUES.len() as i32) as usize];
            let base_color = DyeColor::VALUES[self.random.next_int_bounded(DyeColor::VALUES.len() as i32) as usize];
            let pattern_color = DyeColor::VALUES[self.random.next_int_bounded(DyeColor::VALUES.len() as i32) as usize];
            Variant::new(pattern, base_color, pattern_color)
        };
        self.mob_mut(id).expect("a tropical fish").set_packed_variant(variant.get_packed_id());
        group_data
    }
}

pub fn check_tropical_fish_spawn_rules(level: &mut (impl ServerLevelAccessor + ?Sized), pos: BlockPos) -> bool {
    level.fluid_state(pos.below()).is(&fluid_tags::WATER)
        && level.block_state(pos.above()).is_block(blocks::WATER)
        && (level.get_biome(pos).is("minecraft:allows_tropical_fish_spawns_at_any_height") || water_animal::check_surface_water_animal_spawn_rules(level, pos))
}
