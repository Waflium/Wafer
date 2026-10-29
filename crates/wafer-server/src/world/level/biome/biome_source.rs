use super::MultiNoiseBiomeSource;
use super::the_end_biome_source::TheEndBiomeSource;
use crate::world::level::chunk::Biome;

pub struct ArithmeticException;

pub struct CheckerboardColumnBiomeSource {
    pub allowed_biomes: Vec<Biome>,
    bit_shift: i32,
    pub size: i32,
}

impl CheckerboardColumnBiomeSource {
    pub fn new(allowed_biomes: Vec<Biome>, size: i32) -> CheckerboardColumnBiomeSource {
        CheckerboardColumnBiomeSource { allowed_biomes, bit_shift: size + 2, size }
    }

    pub fn get_noise_biome(&self, quart_x: i32, quart_z: i32) -> Biome {
        if self.allowed_biomes.is_empty() {
            std::panic::resume_unwind(Box::new(ArithmeticException));
        }
        self.allowed_biomes[((quart_x >> self.bit_shift) + (quart_z >> self.bit_shift)).rem_euclid(self.allowed_biomes.len() as i32) as usize]
    }
}

pub enum BiomeSource {
    MultiNoise(MultiNoiseBiomeSource),
    TheEnd(TheEndBiomeSource),
    Fixed(Biome),
    Checkerboard(CheckerboardColumnBiomeSource),
}

impl BiomeSource {
    pub fn possible_biomes(&self) -> Vec<Biome> {
        match self {
            BiomeSource::MultiNoise(source) => source.possible_biomes(),
            BiomeSource::TheEnd(source) => source.collect_possible_biomes(),
            BiomeSource::Fixed(biome) => vec![*biome],
            BiomeSource::Checkerboard(source) => {
                let mut biomes = Vec::new();
                for &biome in &source.allowed_biomes {
                    if !biomes.contains(&biome) {
                        biomes.push(biome);
                    }
                }
                biomes
            }
        }
    }

    pub fn fixed_biome(&self) -> Option<Biome> {
        match self {
            BiomeSource::Fixed(biome) => Some(*biome),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ArithmeticException, CheckerboardColumnBiomeSource};
    use crate::world::level::chunk::Biome;

    #[test]
    fn an_empty_checkerboard_fails_with_an_arithmetic_exception_when_asked_for_a_biome() {
        let payload = std::panic::catch_unwind(|| CheckerboardColumnBiomeSource::new(Vec::new(), 2).get_noise_biome(0, 0)).unwrap_err();
        assert!(payload.is::<ArithmeticException>());
        assert_eq!(CheckerboardColumnBiomeSource::new(vec![Biome(3)], 2).get_noise_biome(5, -9), Biome(3));
    }
}
