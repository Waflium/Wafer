use std::sync::LazyLock;

use super::{biome_condition, noise_threshold_condition, not_condition, stone_depth_condition, vertical_gradient_condition, water_condition, y_condition};
use crate::world::item::component::codec::{Codec, Dispatch, Selector, Stream};
use crate::world::item::component::types::Pair;

fn direct_codec_lazy() -> &'static Pair {
    static CODEC: LazyLock<Pair> = LazyLock::new(direct_codec);
    &CODEC
}

pub fn direct_codec() -> Pair {
    let cases = vec![
        ("minecraft:biome", biome_condition::fields()),
        ("minecraft:noise_threshold", noise_threshold_condition::fields()),
        ("minecraft:vertical_gradient", vertical_gradient_condition::fields()),
        ("minecraft:y_above", y_condition::fields()),
        ("minecraft:water", water_condition::fields()),
        ("minecraft:temperature", Vec::new()),
        ("minecraft:steep", Vec::new()),
        ("minecraft:not", not_condition::fields()),
        ("minecraft:hole", Vec::new()),
        ("minecraft:above_preliminary_surface", Vec::new()),
        ("minecraft:stone_depth", stone_depth_condition::fields()),
    ];
    (Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:worldgen/material_condition_type"), cases })), Stream::Nbt)
}

pub fn holder_codec() -> Pair {
    (Codec::Holder("minecraft:worldgen/material_condition", Some(Box::new(Codec::Lazy(direct_codec_lazy)))), Stream::Nbt)
}
