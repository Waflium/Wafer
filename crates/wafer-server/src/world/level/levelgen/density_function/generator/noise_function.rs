use crate::world::item::component::codec::{Codec, Field, Stream};
use crate::world::item::component::types::{self, def, req};
use crate::world::level::levelgen::density_function::holder_codec;
use crate::world::level::levelgen::synth;

pub fn fields() -> Vec<Field> {
    vec![
        req("noise", synth::noise_holder_codec()),
        req("xz_scale", (Codec::Double, Stream::Double)),
        req("y_scale", (Codec::Double, Stream::Double)),
        def("shift_x", holder_codec(), types::f(0.0)),
        def("shift_y", holder_codec(), types::f(0.0)),
        def("shift_z", holder_codec(), types::f(0.0)),
    ]
}
