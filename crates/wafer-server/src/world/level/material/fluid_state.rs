use crate::world::item::component::codec::{Codec, Dispatch, Selector, Stream};
use crate::world::item::component::types::Pair;

pub fn codec() -> Pair {
    let cases = vec![
        ("minecraft:empty", vec![]),
        ("minecraft:flowing_water", vec![]),
        ("minecraft:water", vec![]),
        ("minecraft:flowing_lava", vec![]),
        ("minecraft:lava", vec![]),
    ];
    (Codec::Dispatch(Box::new(Dispatch { key: "id", selector: Selector::Registry("minecraft:fluid"), cases })), Stream::Dispatch)
}
