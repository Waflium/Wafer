use super::valueproviders::IntProvider;

pub const fn range_of_seconds(min_inclusive: i32, max_inclusive: i32) -> IntProvider {
    IntProvider::Uniform { min_inclusive: min_inclusive * 20, max_inclusive: max_inclusive * 20 }
}
