use std::sync::LazyLock;

use wafer_data::DimensionType;

use crate::util::valueproviders::IntProvider;

static MONSTER_SPAWN_LIGHT_TESTS: LazyLock<Vec<IntProvider>> = LazyLock::new(|| {
    wafer_data::dimension_types()
        .iter()
        .map(|dimension| {
            let json = serde_json::from_str(dimension.monster_spawn_light_level).expect("monster_spawn_light_level JSON");
            IntProvider::parse(&json).unwrap_or_else(|e| panic!("monster_spawn_light_level of {}: {e}", dimension.name))
        })
        .collect()
});

pub fn monster_spawn_light_test(dimension: &'static DimensionType) -> &'static IntProvider {
    let index = wafer_data::dimension_types().iter().position(|d| d.name == dimension.name).expect("a registered dimension type");
    &MONSTER_SPAWN_LIGHT_TESTS[index]
}

pub fn monster_spawn_block_light_limit(dimension: &'static DimensionType) -> i32 {
    dimension.monster_spawn_block_light_limit
}

pub const MIN_Y: i32 = -2032;
pub const MAX_Y: i32 = 2031;
