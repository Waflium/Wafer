use std::sync::LazyLock;

use crate::world::item::component::codec::{Codec, Field, Stream, Wrap};
use crate::world::item::component::types::{self, Pair, record, req};
use crate::world::level::levelgen::density_function::holder_codec;

fn spline_codec_lazy() -> &'static Pair {
    static CODEC: LazyLock<Pair> = LazyLock::new(spline_codec);
    &CODEC
}

fn point_codec() -> Pair {
    record(vec![
        req("location", types::float(Codec::float())),
        req("value", recursive_spline()),
        req("derivative", types::float(Codec::float())),
    ])
}

fn multipoint_codec() -> Pair {
    record(vec![req("coordinate", holder_codec()), req("points", types::non_empty_list(point_codec()))])
}

fn spline_codec() -> Pair {
    (Codec::Either(Box::new(Codec::float()), Box::new(multipoint_codec().0)), Stream::Nbt)
}

pub fn fields() -> Vec<Field> {
    vec![req("spline", recursive_spline())]
}

fn recursive_spline() -> Pair {
    (Codec::Framed(Box::new(Codec::Lazy(spline_codec_lazy)), Wrap::Recursive), Stream::Lazy)
}
