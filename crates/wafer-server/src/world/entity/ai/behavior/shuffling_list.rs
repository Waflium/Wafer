use wafer_core::random::{LegacyRandomSource, RandomSource};

pub struct WeightedEntry<T> {
    data: T,
    weight: i32,
    rand_weight: f64,
}

impl<T> WeightedEntry<T> {
    fn set_random(&mut self, random: f32) {
        self.rand_weight = -(random as f64).powf((1.0f32 / self.weight as f32) as f64);
    }
}

pub struct ShufflingList<U> {
    entries: Vec<WeightedEntry<U>>,
    random: LegacyRandomSource,
}

impl<U> Default for ShufflingList<U> {
    fn default() -> ShufflingList<U> {
        ShufflingList { entries: Vec::new(), random: LegacyRandomSource::create() }
    }
}

impl<U> ShufflingList<U> {
    pub fn add(&mut self, data: U, weight: i32) -> &mut ShufflingList<U> {
        self.entries.push(WeightedEntry { data, weight, rand_weight: 0.0 });
        self
    }

    pub fn shuffle(&mut self) -> &mut ShufflingList<U> {
        for entry in &mut self.entries {
            let random = self.random.next_float();
            entry.set_random(random);
        }
        self.entries.sort_by(|a, b| a.rand_weight.total_cmp(&b.rand_weight));
        self
    }

    pub fn iter(&self) -> impl Iterator<Item = &U> {
        self.entries.iter().map(|entry| &entry.data)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut U> {
        self.entries.iter_mut().map(|entry| &mut entry.data)
    }

    #[cfg(test)]
    pub fn set_seed(&mut self, seed: i64) {
        self.random = LegacyRandomSource::new(seed);
    }
}
