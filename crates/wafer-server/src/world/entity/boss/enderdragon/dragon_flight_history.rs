pub const LENGTH: usize = 64;
const MASK: i32 = 63;

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Sample {
    pub y: f64,
    pub y_rot: f32,
}

#[derive(Clone)]
pub struct DragonFlightHistory {
    samples: [Sample; LENGTH],
    head: i32,
}

impl Default for DragonFlightHistory {
    fn default() -> DragonFlightHistory {
        DragonFlightHistory { samples: [Sample::default(); LENGTH], head: -1 }
    }
}

impl DragonFlightHistory {
    pub fn record(&mut self, y: f64, y_rot: f32) {
        let sample = Sample { y, y_rot };
        if self.head < 0 {
            self.samples = [sample; LENGTH];
        }
        self.head += 1;
        if self.head == LENGTH as i32 {
            self.head = 0;
        }
        self.samples[self.head as usize] = sample;
    }

    pub fn get(&self, delay: i32) -> Sample {
        self.samples[((self.head - delay) & MASK) as usize]
    }
}
