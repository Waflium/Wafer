const MULTIPLY_DE_BRUIJN_BIT_POSITION: [i32; 32] = [0, 1, 28, 2, 29, 14, 24, 3, 30, 22, 20, 15, 25, 17, 4, 8, 31, 27, 13, 23, 21, 19, 16, 7, 26, 12, 18, 6, 11, 5, 10, 9];

pub fn make_key(version: i32) -> i32 {
    make_key_with_sub_version(version, 0)
}

pub fn make_key_with_sub_version(version: i32, sub_version: i32) -> i32 {
    version * 10 + sub_version
}

pub fn get_version(key: i32) -> i32 {
    key / 10
}

pub fn get_sub_version(key: i32) -> i32 {
    key % 10
}

pub fn smallest_encompassing_power_of_two(input: i32) -> i32 {
    let mut result = input.wrapping_sub(1);
    result |= result >> 1;
    result |= result >> 2;
    result |= result >> 4;
    result |= result >> 8;
    result |= result >> 16;
    result.wrapping_add(1)
}

fn is_power_of_two(input: i32) -> bool {
    input != 0 && (input & input.wrapping_sub(1)) == 0
}

pub fn ceillog2(input: i32) -> i32 {
    let input = if is_power_of_two(input) { input } else { smallest_encompassing_power_of_two(input) };
    MULTIPLY_DE_BRUIJN_BIT_POSITION[((input as i64).wrapping_mul(125613361) >> 27) as i32 as usize & 31]
}

#[cfg(test)]
mod tests {
    use super::ceillog2;

    #[test]
    fn ceillog2_is_the_ceiling_of_the_binary_logarithm() {
        assert_eq!(ceillog2(0), 0);
        for input in 1..=70000i32 {
            assert_eq!(ceillog2(input), (input as u32).next_power_of_two().trailing_zeros() as i32, "{input}");
        }
    }
}
