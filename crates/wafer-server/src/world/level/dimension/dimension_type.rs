use std::sync::LazyLock;

use wafer_data::{DIMENSION_TYPES, DimensionType};

use crate::util::valueproviders::IntProvider;

static MONSTER_SPAWN_LIGHT_TESTS: LazyLock<Vec<IntProvider>> = LazyLock::new(|| {
    DIMENSION_TYPES
        .iter()
        .map(|dimension| {
            let json = serde_json::from_str(dimension.monster_spawn_light_level).expect("monster_spawn_light_level JSON");
            IntProvider::parse(&json).unwrap_or_else(|e| panic!("monster_spawn_light_level of {}: {e}", dimension.name))
        })
        .collect()
});

pub fn monster_spawn_light_test(dimension: &'static DimensionType) -> &'static IntProvider {
    let index = DIMENSION_TYPES.iter().position(|d| std::ptr::eq(d, dimension)).expect("a registered dimension type");
    &MONSTER_SPAWN_LIGHT_TESTS[index]
}

pub fn monster_spawn_block_light_limit(dimension: &'static DimensionType) -> i32 {
    dimension.monster_spawn_block_light_limit
}
