use std::cell::{RefCell, RefMut};

use wafer_core::random::{LegacyRandomSource, RandomSource};
use wafer_core::{BlockPos, mth};

use super::structure_processor::StructureProcessor;
use super::structure_template::Palette;
use crate::world::level::block::mirror::Mirror;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::levelgen::structure::BoundingBox;

pub struct StructurePlaceSettings<'p> {
    pub mirror: Mirror,
    pub rotation: Rotation,
    pub rotation_pivot: BlockPos,
    pub bounding_box: Option<BoundingBox>,
    pub apply_waterlogging: bool,
    pub random: PlaceRandom,
    pub processors: Vec<&'p StructureProcessor>,
    pub known_shape: bool,
}

impl Default for StructurePlaceSettings<'_> {
    fn default() -> Self {
        StructurePlaceSettings {
            mirror: Mirror::None,
            rotation: Rotation::None,
            rotation_pivot: BlockPos::new(0, 0, 0),
            bounding_box: None,
            apply_waterlogging: true,
            random: PlaceRandom::Positional,
            processors: Vec::new(),
            known_shape: false,
        }
    }
}

pub enum PlaceRandom {
    Positional,
    Given,
    Own(RefCell<LegacyRandomSource>),
}

pub enum SettingsRandom<'r, R> {
    Given(&'r mut R),
    Own(RefMut<'r, LegacyRandomSource>),
    Positional(LegacyRandomSource),
}

impl<R: RandomSource> RandomSource for SettingsRandom<'_, R> {
    fn set_seed(&mut self, seed: i64) {
        match self {
            SettingsRandom::Given(random) => random.set_seed(seed),
            SettingsRandom::Own(random) => random.set_seed(seed),
            SettingsRandom::Positional(random) => random.set_seed(seed),
        }
    }

    fn next_int(&mut self) -> i32 {
        match self {
            SettingsRandom::Given(random) => random.next_int(),
            SettingsRandom::Own(random) => random.next_int(),
            SettingsRandom::Positional(random) => random.next_int(),
        }
    }

    fn next_int_bounded(&mut self, bound: i32) -> i32 {
        match self {
            SettingsRandom::Given(random) => random.next_int_bounded(bound),
            SettingsRandom::Own(random) => random.next_int_bounded(bound),
            SettingsRandom::Positional(random) => random.next_int_bounded(bound),
        }
    }

    fn next_long(&mut self) -> i64 {
        match self {
            SettingsRandom::Given(random) => random.next_long(),
            SettingsRandom::Own(random) => random.next_long(),
            SettingsRandom::Positional(random) => random.next_long(),
        }
    }

    fn next_boolean(&mut self) -> bool {
        match self {
            SettingsRandom::Given(random) => random.next_boolean(),
            SettingsRandom::Own(random) => random.next_boolean(),
            SettingsRandom::Positional(random) => random.next_boolean(),
        }
    }

    fn next_float(&mut self) -> f32 {
        match self {
            SettingsRandom::Given(random) => random.next_float(),
            SettingsRandom::Own(random) => random.next_float(),
            SettingsRandom::Positional(random) => random.next_float(),
        }
    }

    fn next_double(&mut self) -> f64 {
        match self {
            SettingsRandom::Given(random) => random.next_double(),
            SettingsRandom::Own(random) => random.next_double(),
            SettingsRandom::Positional(random) => random.next_double(),
        }
    }

    fn next_gaussian(&mut self) -> f64 {
        match self {
            SettingsRandom::Given(random) => random.next_gaussian(),
            SettingsRandom::Own(random) => random.next_gaussian(),
            SettingsRandom::Positional(random) => random.next_gaussian(),
        }
    }
}

impl StructurePlaceSettings<'_> {
    pub fn get_random<'r, R: RandomSource>(&'r self, pos: BlockPos, random: &'r mut R) -> SettingsRandom<'r, R> {
        match &self.random {
            PlaceRandom::Given => SettingsRandom::Given(random),
            PlaceRandom::Own(own) => SettingsRandom::Own(own.borrow_mut()),
            PlaceRandom::Positional => SettingsRandom::Positional(LegacyRandomSource::new(mth::get_seed(pos.x, pos.y, pos.z))),
        }
    }

    pub fn get_random_palette<'t, R: RandomSource>(&self, palettes: &'t [Palette], pos: BlockPos, random: &mut R) -> &'t Palette {
        &palettes[self.get_random(pos, random).next_int_bounded(palettes.len() as i32) as usize]
    }
}
