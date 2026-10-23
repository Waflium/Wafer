use wafer_core::mth;

pub struct PackedBitStorage {
    data: Vec<i64>,
    bits: i32,
    mask: i64,
    size: i32,
}

fn inclusive_between(start: i64, end: i64, value: i64) {
    if value < start || value > end {
        panic!("The value {value} is not in the specified inclusive range of {start} to {end}");
    }
}

fn required_length(size: i32, bits: i32) -> i32 {
    mth::positive_ceil_div(size.wrapping_mul(bits), 64).wrapping_mul(64) / 64
}

impl PackedBitStorage {
    pub fn new(bits: i32, size: i32) -> PackedBitStorage {
        PackedBitStorage::with_data(bits, size, vec![0; required_length(size, bits) as usize])
    }

    pub fn with_data(bits: i32, size: i32, data: Vec<i64>) -> PackedBitStorage {
        inclusive_between(1, 32, bits as i64);
        let mask = (1i64 << bits) - 1;
        let required_length = required_length(size, bits);
        if data.len() as i32 != required_length {
            panic!("Invalid length given for storage, got: {} but expected: {}", data.len(), required_length);
        }
        PackedBitStorage { data, bits, mask, size }
    }

    pub fn set(&mut self, index: i32, value: i32) {
        inclusive_between(0, self.size as i64 - 1, index as i64);
        inclusive_between(0, self.mask, value as i64);
        let position = index.wrapping_mul(self.bits);
        let start_data = (position >> 6) as usize;
        let end_data = (index.wrapping_add(1).wrapping_mul(self.bits).wrapping_sub(1) >> 6) as usize;
        let start_bit = position ^ ((start_data as i32) << 6);
        self.data[start_data] = self.data[start_data] & !(self.mask << start_bit) | (value as i64 & self.mask) << start_bit;
        if start_data != end_data {
            let shift_bits = 64 - start_bit;
            let wanted_bits = self.bits - shift_bits;
            self.data[end_data] = ((self.data[end_data] as u64) >> wanted_bits << wanted_bits) as i64 | (value as i64 & self.mask) >> shift_bits;
        }
    }

    pub fn get(&self, index: i32) -> i32 {
        inclusive_between(0, self.size as i64 - 1, index as i64);
        let position = index.wrapping_mul(self.bits);
        let start_data = (position >> 6) as usize;
        let end_data = (index.wrapping_add(1).wrapping_mul(self.bits).wrapping_sub(1) >> 6) as usize;
        let start_bit = position ^ ((start_data as i32) << 6);
        if start_data == end_data {
            return (((self.data[start_data] as u64) >> start_bit) as i64 & self.mask) as i32;
        }
        let shift_bits = 64 - start_bit;
        ((((self.data[start_data] as u64) >> start_bit) as i64 | self.data[end_data] << shift_bits) & self.mask) as i32
    }

    pub fn get_raw(&self) -> &[i64] {
        &self.data
    }

    pub fn get_bits(&self) -> i32 {
        self.bits
    }
}
