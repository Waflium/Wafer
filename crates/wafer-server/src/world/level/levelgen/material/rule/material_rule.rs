use std::sync::LazyLock;

use super::{block_rule, condition_rule, ore_vein_rule, sequence_rule};
use crate::world::item::component::codec::{Codec, Dispatch, Selector, Stream, Wrap};
use crate::world::item::component::types::Pair;

fn direct_codec_lazy() -> &'static Pair {
    static CODEC: LazyLock<Pair> = LazyLock::new(direct_codec);
    &CODEC
}

pub fn direct_codec() -> Pair {
    let cases = vec![
        ("minecraft:block", block_rule::fields()),
        ("minecraft:bandlands", Vec::new()),
        ("minecraft:sequence", sequence_rule::fields()),
        ("minecraft:condition", condition_rule::fields()),
        ("minecraft:ore_vein", ore_vein_rule::fields()),
    ];
    (Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:worldgen/material_rule_type"), cases })), Stream::Nbt)
}

pub fn holder_codec() -> Pair {
    (Codec::Framed(Box::new(Codec::Holder("minecraft:worldgen/material_rule", Some(Box::new(Codec::Lazy(direct_codec_lazy))))), Wrap::Xmap), Stream::Nbt)
}
